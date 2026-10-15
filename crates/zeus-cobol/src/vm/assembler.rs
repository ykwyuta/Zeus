//! Assembler module responsibility counts and active executions.
use super::Machine;
use crate::hlasm::{self, DynamicStorage, Registers};
use std::collections::{HashMap, HashSet};

pub(super) struct Invocation<'a> {
    pub parameter_mode: Option<u8>,
    pub allow_64: bool,
    pub floating: Option<&'a mut hlasm::FloatingState>,
}

#[derive(Default)]
pub(super) struct Residency {
    pub pinned: HashSet<usize>,
    pub active: HashMap<usize, usize>,
    loads: HashMap<usize, usize>,
    pub posts: std::collections::VecDeque<(usize, u32)>,
    pub messages: std::collections::VecDeque<hlasm::OperatorMessage>,
    pub message_id: u32,
    dispatch: HashMap<String, (usize, usize)>,
}

impl Machine {
    pub(super) fn hlasm_residency_condition_name(&self, address: usize) -> Option<String> {
        self.hlasm_residency
            .dispatch
            .iter()
            .find(|(name, (at, _))| *at == address && super::le::parameter_count(name).is_none())
            .map(|(name, _)| name.clone())
    }

    pub(super) fn dispatch_assembler_target(
        &mut self,
        request: hlasm::AssemblerCall<'_>,
    ) -> Result<i32, String> {
        let (name, args) = (request.name, request.args);
        if let Some(result) = self.call_hlasm_mode(
            name,
            args,
            request.terminated,
            false,
            Invocation {
                parameter_mode: Some(request.parameter_mode),
                allow_64: request.allow_64,
                floating: request.floating,
            },
        ) {
            return result;
        }

        if super::le::parameter_count(name).is_some() {
            if request.parameter_mode == 64 {
                return Err(format!(
                    "{name}: LE callable services require a four-byte AMODE 31 parameter list"
                ));
            }
            if args.iter().flatten().any(|at| *at > 0x7fff_ffff) {
                return Err(format!("{name}: LE parameter address must be below 2 GB"));
            }
            self.le_service(name, args).map_err(|error| error.0)?;
            return Ok(0);
        }
        let refs: Vec<_> = args
            .iter()
            .map(|addr| crate::interlang::XRef {
                addr: *addr,
                ..Default::default()
            })
            .collect();
        if let Some(result) = self.call_native(name, &refs) {
            return result;
        }
        match self.call_external(name, &refs)? {
            Some(end) if end.stop => {
                self.return_code = end.rc;
                Err(super::STOP_RUN.into())
            }
            Some(end) => Ok(end.rc),
            None => Err(format!("module entry {name} not found")),
        }
    }
    /// Queue an asynchronous producer's completion for cooperative dispatch
    /// by WAIT. The producer must keep its ECB alive until completion.
    pub fn schedule_hlasm_post(&mut self, ecb: usize, completion: u32) -> Result<(), String> {
        if !ecb.is_multiple_of(4)
            || ecb.checked_add(4).is_none_or(|end| end > self.mem.len())
            || completion > 0x3fff_ffff
        {
            return Err("invalid scheduled ECB or completion code".into());
        }
        self.hlasm_residency.posts.push_back((ecb, completion));
        Ok(())
    }

    /// Most recent operator messages (up to 1024); the output sink receives
    /// every message, including those evicted from this in-memory history.
    pub fn operator_messages(&self) -> &std::collections::VecDeque<hlasm::OperatorMessage> {
        &self.hlasm_residency.messages
    }
    pub(super) fn with_hlasm_memory<T>(
        &mut self,
        memory: &mut Vec<u8>,
        storage: &mut DynamicStorage,
        action: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        std::mem::swap(&mut self.mem, memory);
        std::mem::swap(&mut self.hlasm_storage, storage);
        let result = action(self);
        std::mem::swap(&mut self.mem, memory);
        std::mem::swap(&mut self.hlasm_storage, storage);
        result
    }

    pub(super) fn discard_hlasm_if_unused(&mut self, base: usize) -> Result<(), String> {
        if self.hlasm_residency.pinned.contains(&base)
            || self.hlasm_residency.active.get(&base).copied().unwrap_or(0) != 0
            || self.hlasm_residency.loads.get(&base).copied().unwrap_or(0) != 0
        {
            return Ok(());
        }
        let module = self
            .hlasm
            .values()
            .find(|(_, address, _)| *address == base)
            .map(|(module, _, _)| module.clone());
        if let Some(module) = module {
            self.close_assembler_range(base, base + module.storage.len())?;
            self.hlasm.retain(|_, (_, address, _)| *address != base);
            self.hlasm_storage
                .release_module(&mut self.mem, &module, base);
        }
        Ok(())
    }

    pub(super) fn leave_hlasm(&mut self, base: usize) -> Result<(), String> {
        if let Some(count) = self.hlasm_residency.active.get_mut(&base) {
            *count -= 1;
            if *count == 0 {
                self.hlasm_residency.active.remove(&base);
            }
        }
        self.discard_hlasm_if_unused(base)
    }

    pub(super) fn load_assembler_service(&mut self, name: &str) -> Result<(u32, u32), String> {
        if !self.load_hlasm(name)? {
            let name = name.to_ascii_uppercase();
            if let Some((address, count)) = self.hlasm_residency.dispatch.get_mut(&name) {
                *count = count
                    .checked_add(1)
                    .ok_or("LOAD responsibility count overflow")?;
                return Ok((*address as u32 | 0x8000_0000, 1));
            }
            if super::le::parameter_count(&name).is_none()
                && self.find_program(&name).is_none()
                && !self.natives.contains_key(&name)
                && !self.natives_mach.contains_key(&name)
                && !self.natives_ref.contains_key(&name)
                && !self
                    .linker
                    .as_ref()
                    .is_some_and(|linker| linker.load(&name))
            {
                return Err(format!("LOAD entry {name} not found"));
            }
            // A source-runtime entry has an eight-byte virtual dispatch slot,
            // not a host-code pointer or fabricated IBM machine instruction.
            let address = self.hlasm_storage.frame(&mut self.mem, 8)?;
            self.hlasm_residency.dispatch.insert(name, (address, 1));
            return Ok((address as u32 | 0x8000_0000, 1));
        }
        let (module, base, entry) = &self.hlasm[&name.to_ascii_uppercase()];
        let count = self.hlasm_residency.loads.entry(*base).or_default();
        *count = count
            .checked_add(1)
            .ok_or("LOAD responsibility count overflow")?;
        Ok((
            hlasm::abi::tagged_entry(*base as u64 + u64::from(*entry), module.entry_mode(*entry))?
                as u32,
            module.storage.len().div_ceil(8) as u32,
        ))
    }

    pub(super) fn delete_assembler_service(&mut self, name: &str) -> Result<i32, String> {
        let name = name.to_ascii_uppercase();
        if let Some((address, count)) = self.hlasm_residency.dispatch.get_mut(&name) {
            *count -= 1;
            if *count == 0 {
                let address = *address;
                self.hlasm_residency.dispatch.remove(&name);
                self.hlasm_storage.release_frame(&mut self.mem, address);
            }
            return Ok(0);
        }
        let Some((_, base, _)) = self.hlasm.get(&name) else {
            return Ok(4);
        };
        let base = *base;
        let Some(count) = self.hlasm_residency.loads.get_mut(&base) else {
            return Ok(4);
        };
        *count -= 1;
        if *count == 0 {
            self.hlasm_residency.loads.remove(&base);
        }
        self.discard_hlasm_if_unused(base)?;
        Ok(0)
    }

    pub(super) fn branch_assembler_service(
        &mut self,
        target: usize,
        registers: &mut Registers,
    ) -> Result<usize, String> {
        if let Some(name) = self
            .hlasm_residency
            .dispatch
            .iter()
            .find(|(_, (address, _))| *address == target)
            .map(|(name, _)| name.clone())
        {
            let count = self
                .by_name
                .get(&name)
                .map(|index| self.instances[*index].code.using.len())
                .or_else(|| {
                    self.linker
                        .as_ref()
                        .and_then(|linker| linker.parameter_count(&name))
                });
            let list =
                hlasm::abi::register(&registers.low, &registers.high, 1, registers.address_mode)?;
            let known = registers
                .parameter_list
                .filter(|(address, _, _)| *address == list as u64);
            let count = count
                .or_else(|| super::le::parameter_count(&name))
                .or(known.map(|(_, n, _)| n));
            let mode = known.map_or(registers.address_mode, |(_, _, mode)| mode);
            let args = self.assembler_parameters(list, count, mode)?;
            let resume = registers.return_address()?;
            registers.low[15] = self.dispatch_assembler_target(hlasm::AssemblerCall {
                name: &name,
                args: &args,
                terminated: mode != 64,
                parameter_mode: mode,
                allow_64: true,
                floating: Some(&mut registers.floating),
            })? as u32;
            if let Some(mode) = registers.return_mode {
                registers.address_mode = mode;
            }
            return Ok(resume);
        }
        let (module, base, entry) = self
            .hlasm
            .values()
            .find(|(_, base, entry)| base + *entry as usize == target)
            .cloned()
            .ok_or_else(|| format!("HLASM branch does not address a loaded entry: {target:#x}"))?;
        if self.hlasm_call_depth >= 128 {
            return Err("HLASM nested call limit exceeded".into());
        }
        let resume = registers.return_address()?;
        *self.hlasm_residency.active.entry(base).or_default() += 1;
        let mut memory = std::mem::take(&mut self.mem);
        let mut storage = std::mem::take(&mut self.hlasm_storage);
        self.hlasm_call_depth += 1;
        let frame = self.enter_language_frame();
        let result = hlasm::execute_entry_with_host(
            &module,
            &mut memory,
            base,
            &[],
            entry,
            &mut storage,
            self,
            true,
            None,
            Some(registers),
            None,
        );
        self.leave_language_frame(frame);
        self.hlasm_call_depth -= 1;
        self.mem = memory;
        self.hlasm_storage = storage;
        let cleanup = self.leave_hlasm(base);
        result.and(cleanup).map(|_| resume)
    }

    fn assembler_parameters(
        &self,
        list: usize,
        count: Option<usize>,
        mode: u8,
    ) -> Result<Vec<Option<usize>>, String> {
        if mode == 64 && count.is_none() {
            return Err("AMODE 64 parameter lists require a known parameter count".into());
        }
        let width = hlasm::abi::parameter_width(mode);
        let mut args = vec![];
        for index in 0..count.unwrap_or(4096) {
            if index >= 4096 {
                return Err("assembler parameter list exceeds 4096 entries".into());
            }
            let at = list
                .checked_add(index * width)
                .ok_or("parameter list overflow")?;
            let end = at.checked_add(width).ok_or("parameter list overflow")?;
            let bytes = self
                .mem
                .get(at..end)
                .ok_or("parameter list outside run-unit storage")?;
            let value = if width == 8 {
                u64::from_be_bytes(bytes.try_into().unwrap())
            } else {
                u64::from(u32::from_be_bytes(bytes.try_into().unwrap()))
            };
            let address = if width == 8 {
                value
            } else {
                value & 0x7fff_ffff
            };
            let addr = usize::try_from(address)
                .map_err(|_| "parameter address exceeds host offset range")?;
            args.push((addr != 0).then_some(addr));
            if width == 4 && value & 0x8000_0000 != 0 || count == Some(index + 1) {
                return Ok(args);
            }
        }
        if count == Some(0) {
            Ok(args)
        } else {
            Err("assembler parameter list has no terminator within 4096 entries".into())
        }
    }
}

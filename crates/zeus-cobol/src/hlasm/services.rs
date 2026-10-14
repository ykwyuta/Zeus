//! Calls through the owning run unit, without lending out host pointers.
use super::{DynamicStorage, ModuleName, ModuleService, StorageOperand};

#[derive(Clone, Default)]
pub(crate) struct Registers {
    pub low: [u32; 16],
    pub high: [u32; 16],
    pub cc: u8,
}

pub(crate) trait Host {
    fn data(
        &mut self,
        _request: super::DataRequest,
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
    ) -> Result<super::DataReply, String> {
        Err("data management requires an owning Machine run unit".into())
    }
    fn parameter_count(
        &mut self,
        _name: &str,
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
    ) -> Result<Option<usize>, String> {
        Ok(None)
    }
    fn message(&mut self, _message: super::OperatorMessage) -> Result<u32, String> {
        Err("WTO requires an owning Machine run unit".into())
    }
    fn wait(
        &mut self,
        events: &[usize],
        needed: usize,
        memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
    ) -> Result<(), String> {
        if super::system_services::completed(events, memory)? >= needed {
            Ok(())
        } else {
            Err("WAIT deadlock: no pending event producer in this serialized run unit".into())
        }
    }
    fn load(
        &mut self,
        _name: &str,
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
    ) -> Result<(u32, u32), String> {
        Err("LOAD requires an owning Machine run unit".into())
    }
    fn delete(
        &mut self,
        _name: &str,
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
    ) -> Result<i32, String> {
        Err("DELETE requires an owning Machine run unit".into())
    }
    fn branch(
        &mut self,
        _target: usize,
        _registers: &mut Registers,
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
    ) -> Result<usize, String> {
        Err("HLASM branch does not address an instruction or loaded entry".into())
    }
    fn call(
        &mut self,
        name: &str,
        args: &[Option<usize>],
        memory: &mut Vec<u8>,
        storage: &mut DynamicStorage,
        terminated: bool,
    ) -> Result<i32, String>;
}

pub(super) struct NoHost;
impl Host for NoHost {
    fn call(
        &mut self,
        _name: &str,
        _args: &[Option<usize>],
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
        _terminated: bool,
    ) -> Result<i32, String> {
        Err("module services require an owning Machine run unit".into())
    }
}

pub(super) fn pointer(operand: &StorageOperand, registers: &[u32; 16]) -> Result<usize, String> {
    match operand {
        StorageOperand::Register(r) => registers
            .get(*r as usize)
            .map(|v| (v & 0x7fff_ffff) as usize)
            .ok_or_else(|| "invalid service register".into()),
        StorageOperand::Address(a) => super::execute::address(a, registers),
        _ => Err("invalid module address operand".into()),
    }
}

pub(super) fn arguments(
    request: &ModuleService,
    registers: &[u32; 16],
    memory: &[u8],
    empty_list: Option<usize>,
    count: Option<usize>,
) -> Result<(String, Vec<Option<usize>>), String> {
    let name = module_name(request, registers, memory)?;
    let args = if let Some(parameters) = &request.parameters {
        parameters
            .iter()
            .map(|p| {
                let at = pointer(p, registers)?;
                if at >= memory.len() {
                    return Err("module parameter outside run-unit storage".into());
                }
                Ok(Some(at))
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        let list = (registers[1] & 0x7fff_ffff) as usize;
        if empty_list == Some(list) {
            return Ok((name, vec![]));
        }
        let mut args = vec![];
        if count == Some(0) {
            return Ok((name, args));
        }
        loop {
            if args.len() >= 4096 {
                return Err("module parameter list has no terminator within 4096 entries".into());
            }
            let at = list
                .checked_add(args.len() * 4)
                .ok_or("parameter list overflow")?;
            let value = super::execute::word(memory, at)?;
            let addr = (value & 0x7fff_ffff) as usize;
            if addr >= memory.len() {
                return Err("module parameter outside run-unit storage".into());
            }
            args.push((addr != 0).then_some(addr));
            if value & 0x8000_0000 != 0 || count == Some(args.len()) {
                break;
            }
        }
        args
    };
    Ok((name, args))
}

pub(super) fn module_name(
    request: &ModuleService,
    registers: &[u32; 16],
    memory: &[u8],
) -> Result<String, String> {
    let name = match &request.name {
        ModuleName::Literal(name) => name.clone(),
        ModuleName::Address(operand) => {
            let at = pointer(operand, registers)?;
            let bytes = memory
                .get(at..at.checked_add(8).ok_or("module name overflow")?)
                .ok_or("module name outside run-unit storage")?;
            crate::ebcdic::decode(bytes).trim_end().to_ascii_uppercase()
        }
    };
    if name.is_empty() || name.len() > 8 || !super::assemble::symbol_name(&name) {
        return Err("module entry name must be a symbol of 1..8 characters".into());
    }
    Ok(name)
}

//! Calls through the owning run unit, without lending out host pointers.
use super::{DynamicStorage, ModuleName, ModuleService, StorageOperand};

#[derive(Clone)]
pub(crate) struct Registers {
    pub low: [u32; 16],
    pub high: [u32; 16],
    pub cc: u8,
    pub floating: super::execute::floating::State,
    pub address_mode: u8,
    pub return_mode: Option<u8>,
    pub parameter_list: Option<(u64, usize, u8)>,
}
impl Default for Registers {
    fn default() -> Self {
        Self {
            low: [0; 16],
            high: [0; 16],
            cc: 0,
            floating: Default::default(),
            address_mode: 31,
            return_mode: None,
            parameter_list: None,
        }
    }
}
impl Registers {
    pub(crate) fn return_address(&self) -> Result<usize, String> {
        let mode = self.return_mode.unwrap_or(self.address_mode);
        let address = super::abi::register(&self.low, &self.high, 14, mode)?;
        Ok(if mode == 64 { address & !1 } else { address })
    }
}

pub(crate) struct Call<'a> {
    pub name: &'a str,
    pub args: &'a [Option<usize>],
    pub terminated: bool,
    pub parameter_mode: u8,
    pub allow_64: bool,
    pub floating: Option<&'a mut super::execute::floating::State>,
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
        request: Call<'_>,
        memory: &mut Vec<u8>,
        storage: &mut DynamicStorage,
    ) -> Result<i32, String>;
}

pub(super) struct NoHost;
impl Host for NoHost {
    fn call(
        &mut self,
        _request: Call<'_>,
        _memory: &mut Vec<u8>,
        _storage: &mut DynamicStorage,
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
    list: usize,
    list_mode: u8,
) -> Result<(String, Vec<Option<usize>>), String> {
    let name = module_name(request, registers, memory)?;
    let args = if let Some(parameters) = &request.parameters {
        parameters
            .iter()
            .map(|p| {
                let at = pointer(p, registers)?;
                if at >= memory.len() || list_mode != 64 && at > 0x7fff_ffff {
                    return Err("module parameter outside run-unit storage".into());
                }
                Ok(Some(at))
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        if empty_list == Some(list) {
            return Ok((name, vec![]));
        }
        let mut args = vec![];
        if list_mode == 64 && count.is_none() {
            return Err("AMODE 64 parameter lists require a known parameter count".into());
        }
        let width = super::abi::parameter_width(list_mode);
        if count == Some(0) {
            return Ok((name, args));
        }
        loop {
            if args.len() >= 4096 {
                return Err("module parameter list has no terminator within 4096 entries".into());
            }
            let at = list
                .checked_add(args.len() * width)
                .ok_or("parameter list overflow")?;
            let value = if width == 8 {
                let bytes = memory
                    .get(at..at.checked_add(8).ok_or("parameter list overflow")?)
                    .ok_or("parameter list outside run-unit storage")?;
                u64::from_be_bytes(bytes.try_into().unwrap())
            } else {
                u64::from(super::execute::word(memory, at)?)
            };
            let address = if width == 8 {
                value
            } else {
                value & 0x7fff_ffff
            };
            let addr = usize::try_from(address)
                .map_err(|_| "parameter address exceeds host offset range")?;
            if addr >= memory.len() {
                return Err("module parameter outside run-unit storage".into());
            }
            args.push((addr != 0).then_some(addr));
            if width == 4 && value & 0x8000_0000 != 0 || count == Some(args.len()) {
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

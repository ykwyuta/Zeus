mod atomic;
mod bits;
mod conditional;
mod edit;
pub(super) mod floating;
mod integer;
mod long;
mod masked;
mod memory;
mod registers;
mod wide;
use super::{Address, Module};
use std::collections::HashMap;

fn range(mem: &[u8], addr: usize, len: usize) -> Result<std::ops::Range<usize>, String> {
    let end = addr
        .checked_add(len)
        .filter(|&end| end <= mem.len())
        .ok_or("HLASM address outside run-unit storage")?;
    Ok(addr..end)
}

pub(super) fn word(mem: &[u8], addr: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        mem[range(mem, addr, 4)?]
            .try_into()
            .map_err(|_| "invalid fullword")?,
    ))
}

fn put_word(mem: &mut [u8], addr: usize, value: u32) -> Result<(), String> {
    let r = range(mem, addr, 4)?;
    mem[r].copy_from_slice(&value.to_be_bytes());
    Ok(())
}

pub(super) fn address(a: &Address, registers: &[u32; 16]) -> Result<usize, String> {
    let register = |r: u8| -> Result<u32, String> {
        if r == 0 {
            Ok(0)
        } else {
            registers
                .get(r as usize)
                .copied()
                .ok_or_else(|| "invalid address register".into())
        }
    };
    // A parameter-list terminator occupies the high bit of the last address.
    let base = register(a.base)? & 0x7fff_ffff;
    let index = register(a.index)? & 0x7fff_ffff;
    usize::try_from(i64::from(base) + i64::from(index) + a.displacement)
        .map_err(|_| "negative HLASM address".into())
}

fn branch_address(a: &Address, registers: &[u32; 16], mode: u8) -> Result<usize, String> {
    let origin = address(
        &Address {
            displacement: 0,
            base: a.base,
            index: a.index,
        },
        registers,
    )?;
    Ok(((origin as i64).wrapping_add(a.displacement) as u64 & super::abi::mask(mode)?) as usize)
}

fn relative_address(ins: &super::Instruction, base: usize, mode: u8) -> Result<usize, String> {
    let spec = super::operation::relative(&ins.op).ok_or("invalid relative instruction")?;
    let relative = ins.address.as_ref().ok_or("missing relative target")?;
    if relative.base != 0 || relative.index != 0 {
        return Err("relative target cannot use base/index registers".into());
    }
    super::operation::check_relative(relative.displacement, spec.bits)?;
    let target = if let Some(expression) = &ins.relative_expression {
        expression.target_at(base)? as u64
    } else if ins.relative_external.is_some() {
        0
    } else {
        (base as i64 + i64::from(ins.offset) + relative.displacement) as u64
    };
    usize::try_from(target & super::abi::mask(mode)?)
        .map_err(|_| "relative address exceeds host offset range".into())
}

/// Run with real addresses (offsets in the shared run-unit Vec). Static
/// storage at `base` persists; only the argument list and caller save area
/// are allocated per call and reclaimed, including on errors.
pub fn execute(
    module: &Module,
    mem: &mut Vec<u8>,
    base: usize,
    args: &[Option<usize>],
) -> Result<i32, String> {
    execute_with_storage(
        module,
        mem,
        base,
        args,
        &mut super::DynamicStorage::default(),
    )
}

pub fn execute_with_storage(
    module: &Module,
    mem: &mut Vec<u8>,
    base: usize,
    args: &[Option<usize>],
    storage: &mut super::DynamicStorage,
) -> Result<i32, String> {
    execute_entry(module, mem, base, args, module.entry, storage)
}

pub(crate) fn execute_entry(
    module: &Module,
    mem: &mut Vec<u8>,
    base: usize,
    args: &[Option<usize>],
    entry: u32,
    storage: &mut super::DynamicStorage,
) -> Result<i32, String> {
    execute_entry_with_host(
        module,
        mem,
        base,
        args,
        entry,
        storage,
        &mut super::services::NoHost,
        true,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_entry_with_host(
    module: &Module,
    mem: &mut Vec<u8>,
    base: usize,
    args: &[Option<usize>],
    entry: u32,
    storage: &mut super::DynamicStorage,
    host: &mut dyn super::Host,
    terminated: bool,
    parameter_mode: Option<u8>,
    initial: Option<&mut super::Registers>,
    mut call_state: Option<&mut floating::State>,
) -> Result<i32, String> {
    module.validate_origin(base)?;
    module.validate_relative_references(base)?;
    storage.verify_protection(mem)?;
    storage.protect(module, mem, base)?;
    if module.storage.len() > 16 << 20
        || base
            .checked_add(module.storage.len())
            .is_none_or(|end| end > mem.len())
    {
        return Err("invalid HLASM module storage".into());
    }
    let mut amode = initial
        .as_ref()
        .map_or_else(|| module.entry_mode(entry), |r| r.address_mode);
    super::abi::mask(amode)?;
    let parameter_mode = parameter_mode.unwrap_or(amode);
    super::abi::mask(parameter_mode)?;
    // Validate the saved caller mode before acquiring an invocation frame.
    // Every fallible step after frame acquisition must use the cleanup path.
    let return_target = if let Some(initial) = initial.as_ref() {
        initial.return_address()? as u64
    } else {
        super::abi::return_sentinel(amode)
    };
    let save_length = super::abi::save_length(amode);
    let parameter_width = super::abi::parameter_width(parameter_mode);
    let mark = mem.len();
    let frame_length = args
        .len()
        .max(1)
        .checked_mul(parameter_width)
        .and_then(|n| n.checked_add(save_length))
        .ok_or("HLASM invocation frame length overflow")?;
    let frame = storage.invocation(mem, frame_length, amode)?;
    let mut registers = initial.as_ref().map_or([0; 16], |r| r.low);
    let mut high = initial.as_ref().map_or([0; 16], |r| r.high);
    let mut cc = initial.as_ref().map_or(0, |r| r.cc);
    let mut floating = initial.as_ref().map_or_else(
        || call_state.as_deref().cloned().unwrap_or_default(),
        |r| r.floating.clone(),
    );
    let original_parameters = initial.as_ref().and_then(|r| r.parameter_list).or_else(|| {
        initial
            .is_none()
            .then_some(((frame + save_length) as u64, args.len(), parameter_mode))
    });
    if initial.is_none() {
        wide::write(&mut registers, &mut high, 1, (frame + save_length) as u64);
        wide::write(&mut registers, &mut high, 13, frame as u64);
        wide::write(
            &mut registers,
            &mut high,
            14,
            if amode == 31 {
                super::abi::return_sentinel(amode) | 0x80000000
            } else {
                super::abi::return_sentinel(amode)
            },
        );
        wide::write(
            &mut registers,
            &mut high,
            15,
            base as u64 + u64::from(entry),
        );
    }
    let result = (|| {
        let list = frame + save_length;
        for (i, arg) in args.iter().enumerate() {
            if arg.is_some_and(|address| {
                address >= mark
                    || parameter_mode != 64
                        && address as u64 > super::abi::mask(parameter_mode).unwrap_or(0)
            }) {
                return Err("invalid HLASM argument address".into());
            }
            let at = list + i * parameter_width;
            if parameter_width == 8 {
                mem[at..at + 8].copy_from_slice(&(arg.map_or(0, |a| a as u64)).to_be_bytes());
            } else {
                let mut value = arg.map_or(0, |a| a as u32);
                if terminated && i + 1 == args.len() {
                    value |= 0x8000_0000;
                }
                put_word(mem, at, value)?;
            }
        }
        let targets: HashMap<usize, usize> = module
            .code
            .iter()
            .enumerate()
            .map(|(i, ins)| (base + ins.offset as usize, i))
            .collect();
        let local = |address: usize| {
            targets.contains_key(&address)
                || address
                    .checked_sub(base)
                    .and_then(|n| u32::try_from(n).ok())
                    .is_some_and(|n| super::machine::contains(module, n))
        };
        let mut pc = base + entry as usize;
        if !local(pc) {
            return Err("invalid HLASM entry".into());
        }
        for _ in 0..1_000_000 {
            let decoded;
            let fetched = if pc
                .checked_sub(base)
                .and_then(|n| u32::try_from(n).ok())
                .is_some_and(|n| super::machine::contains(module, n))
            {
                decoded = super::machine::fetch(module, mem, base, pc, 0)?;
                &decoded
            } else {
                &module.code[*targets
                    .get(&pc)
                    .ok_or("HLASM execution entered data or unmapped instruction storage")?]
            };
            let next_address = base
                + fetched.offset as usize
                + super::assemble::instruction_size(&fetched.op)
                    .ok_or("unknown instruction size")? as usize;
            let mut modified = None;
            if matches!(fetched.op.as_str(), "EX" | "EXRL") {
                let r = *fetched.registers.first().ok_or("EX register required")? as usize;
                if r > 15 {
                    return Err("invalid EX register".into());
                }
                let target = if fetched.op == "EXRL" {
                    relative_address(fetched, base, amode)?
                } else {
                    super::abi::effective(
                        fetched.address.as_ref().ok_or("EX target required")?,
                        &registers,
                        &high,
                        amode,
                    )?
                };
                if !target.is_multiple_of(2) {
                    return Err("EX target must be halfword aligned".into());
                }
                let machine_target = target
                    .checked_sub(base)
                    .and_then(|n| u32::try_from(n).ok())
                    .is_some_and(|n| super::machine::contains(module, n));
                let decoded_target;
                let original = if machine_target {
                    decoded_target = super::machine::fetch(module, mem, base, target, 0)?;
                    &decoded_target
                } else {
                    &module.code[*targets
                        .get(&target)
                        .ok_or("EX target is not a compiled instruction template")?]
                };
                if matches!(original.op.as_str(), "EX" | "EXRL") {
                    return Err("execute exception: nested EXECUTE".into());
                }
                if matches!(
                    original.op.as_str(),
                    "GETMAIN"
                        | "FREEMAIN"
                        | "STORAGE"
                        | "STIMER"
                        | "LINK"
                        | "XCTL"
                        | "LOAD"
                        | "DELETE"
                        | "WTO"
                        | "WAIT"
                        | "POST"
                        | "OPEN"
                        | "CLOSE"
                        | "GET"
                        | "PUT"
                        | "FREEPOOL"
                ) {
                    return Err("EX of service instruction templates is unsupported".into());
                }
                let instruction = if machine_target {
                    super::machine::fetch(
                        module,
                        mem,
                        base,
                        target,
                        if r == 0 { 0 } else { registers[r] as u8 },
                    )?
                } else {
                    let size = super::assemble::instruction_size(&original.op)
                        .ok_or("invalid EX target size")? as usize;
                    let bytes = &mem[range(mem, target, size)?];
                    let initial = module
                        .storage
                        .get(original.offset as usize..original.offset as usize + size)
                        .ok_or("invalid EX template storage")?;
                    if bytes
                        .iter()
                        .zip(initial)
                        .enumerate()
                        .any(|(n, (a, b))| n != 1 && a != b)
                    {
                        return Err(
                            "EX target changed outside its supported second-byte fields".into()
                        );
                    }
                    let mut instruction = original.clone();
                    super::template::apply(
                        &mut instruction,
                        bytes[1] | if r == 0 { 0 } else { registers[r] as u8 },
                    )?;
                    instruction
                };
                if matches!(instruction.op.as_str(), "EX" | "EXRL") {
                    return Err("execute exception: nested EXECUTE".into());
                }
                modified = Some(instruction);
            }
            let prepared = super::abi::prepare(
                modified.as_ref().unwrap_or(fetched),
                &registers,
                &high,
                amode,
            )?;
            let ins = prepared.as_ref();
            let mut branch_return_mode = None;
            let mut run = || -> Result<(Option<usize>, bool), String> {
                let reg = |n: usize| -> Result<usize, String> {
                    let r = *ins.registers.get(n).ok_or("missing register operand")? as usize;
                    if r > 15 {
                        return Err("invalid register operand".into());
                    }
                    Ok(r)
                };
                if matches!(
                    ins.op.as_str(),
                    "OPEN" | "CLOSE" | "GET" | "PUT" | "FREEPOOL"
                ) {
                    let reply = super::data_management::execute(
                        ins.data_service
                            .as_ref()
                            .ok_or("missing data service metadata")?,
                        &registers,
                        mem,
                        storage,
                        host,
                    )?;
                    if let Some(address) = reply.address {
                        registers[1] = u32::try_from(address)
                            .map_err(|_| "data buffer exceeds 31-bit addressing")?;
                        high[1] = 0;
                    }
                    if let Some(rc) = reply.rc {
                        registers[15] = rc as u32;
                    }
                    if reply.link {
                        registers[14] = next_address as u32 | 0x8000_0000;
                    }
                    return Ok((reply.branch, false));
                }
                if matches!(ins.op.as_str(), "WTO" | "WAIT" | "POST") {
                    super::system_services::execute(
                        ins.system_service
                            .as_ref()
                            .ok_or("missing system service metadata")?,
                        &mut registers,
                        mem,
                        storage,
                        host,
                    )?;
                    return Ok((None, false));
                }
                if matches!(ins.op.as_str(), "LINK" | "XCTL" | "LOAD" | "DELETE") {
                    let request = ins
                        .module_service
                        .as_ref()
                        .ok_or("missing module service metadata")?;
                    let service_name = super::services::module_name(request, &registers, mem)?;
                    if ins.op == "LINK" && amode == 64 && !request.extended {
                        return Err("LINK requires AMODE 24/31; use LINKX in AMODE 64".into());
                    }
                    let list_mode = match request.list_width {
                        Some(8) => 64,
                        Some(4) => 31,
                        Some(_) => return Err("invalid module parameter width".into()),
                        None => {
                            if request.extended || ins.op == "XCTL" {
                                if amode == 64 { 64 } else { 31 }
                            } else {
                                31
                            }
                        }
                    };
                    if list_mode == 64 && request.variable_list {
                        return Err("VL=1 is invalid with eight-byte parameter lists".into());
                    }
                    let current_list = super::abi::register(&registers, &high, 1, amode)?;
                    let known = original_parameters
                        .filter(|(address, _, _)| *address == current_list as u64);
                    if request.parameters.is_none()
                        && known.is_some_and(|(_, _, mode)| {
                            super::abi::parameter_width(mode)
                                != super::abi::parameter_width(list_mode)
                        })
                    {
                        return Err("inherited parameter width differs from PLIST4/PLIST8".into());
                    }
                    let count = if request.parameters.is_none() {
                        if let Some((_, count, _)) = known {
                            Some(count)
                        } else {
                            host.parameter_count(&service_name, mem, storage)?
                        }
                    } else {
                        None
                    };
                    let (name, parameters) = super::services::arguments(
                        request,
                        &registers,
                        mem,
                        args.is_empty().then_some(list),
                        count,
                        current_list,
                        list_mode,
                    )?;
                    if ins.op == "LOAD" {
                        let (entry, length) = host.load(&name, mem, storage)?;
                        registers[0] = entry;
                        high[0] = 0;
                        registers[1] = length;
                        if amode == 64 {
                            high[1] = 0;
                            high[15] = 0;
                        }
                        registers[15] = 0;
                        return Ok((None, false));
                    }
                    if ins.op == "DELETE" {
                        registers[15] = host.delete(&name, mem, storage)? as u32;
                        return Ok((None, false));
                    }
                    let variable_list = if list_mode == 64 {
                        false
                    } else if request.parameters.is_some() {
                        request.variable_list
                    } else if parameters.is_empty() {
                        true
                    } else {
                        let last = current_list + (parameters.len() - 1) * 4;
                        word(mem, last)? & 0x8000_0000 != 0
                    };
                    let rc = host.call(
                        super::AssemblerCall {
                            name: &name,
                            args: &parameters,
                            terminated: variable_list,
                            parameter_mode: list_mode,
                            allow_64: request.allow_64,
                            floating: Some(&mut floating),
                        },
                        mem,
                        storage,
                    )?;
                    registers[15] = rc as u32;
                    return Ok((None, ins.op == "XCTL"));
                }
                if conditional::execute(ins, &mut registers, &mut high, mem, cc, storage)?
                    || floating::execute(
                        ins,
                        &mut registers,
                        &mut high,
                        mem,
                        storage,
                        &mut floating,
                        &mut cc,
                    )?
                    || atomic::execute(
                        ins,
                        &mut registers,
                        &mut high,
                        mem,
                        &mut cc,
                        storage,
                        amode,
                    )?
                    || masked::execute(ins, &mut registers, &mut high, mem, storage, &mut cc)?
                    || edit::execute(ins, &mut registers, &mut high, mem, storage, &mut cc, amode)?
                    || long::execute(ins, &mut registers, &mut high, mem, storage, &mut cc, amode)?
                    || registers::execute(ins, &mut registers, &mut high, &mut cc)?
                    || bits::execute(ins, &mut registers, &mut high, mem, &mut cc)?
                    || memory::execute(ins, &mut registers, &mut high, mem, storage)?
                    || integer::execute(ins, &mut registers, &mut high, mem, &mut cc)?
                    || wide::execute(ins, &mut registers, &mut high, mem, storage, &mut cc)?
                {
                    return Ok((None, false));
                }
                let op = super::assemble::canonical_operation(&ins.op);
                let ea = || -> Result<usize, String> {
                    address(
                        ins.address.as_ref().ok_or("missing address operand")?,
                        &registers,
                    )
                };
                let mut branch = None;
                match op {
                    "GETMAIN" | "FREEMAIN" | "STORAGE" => {
                        use super::StorageOperand;
                        let service = ins
                            .storage_service
                            .as_ref()
                            .ok_or("missing storage service request")?;
                        if service.obtain
                            && !matches!(
                                &service.address,
                                StorageOperand::Memory(_) | StorageOperand::Register(0..=15)
                            )
                        {
                            return Err("invalid storage obtain destination".into());
                        }
                        let value = |operand: &StorageOperand| -> Result<u32, String> {
                            match operand {
                                StorageOperand::Constant(n) => Ok(*n),
                                StorageOperand::Register(r) => registers
                                    .get(*r as usize)
                                    .copied()
                                    .ok_or_else(|| "invalid storage register".into()),
                                StorageOperand::Memory(a) => word(mem, address(a, &registers)?),
                                StorageOperand::Address(a) => {
                                    u32::try_from(address(a, &registers)?)
                                        .map_err(|_| "storage address overflow".into())
                                }
                            }
                        };
                        let mut subpool = u8::try_from(value(&service.subpool)?)
                            .map_err(|_| "invalid storage subpool")?;
                        let mut length = service
                            .length
                            .as_ref()
                            .map(value)
                            .transpose()?
                            .map(|n| n as usize);
                        if service.packed_length
                            && let Some(raw) = length
                        {
                            subpool = (raw >> 24) as u8;
                            length = Some(raw & 0x00ff_ffff);
                        }
                        let destination = match &service.address {
                            StorageOperand::Memory(a) => Some(address(a, &registers)?),
                            _ => None,
                        };
                        if let Some(at) = destination {
                            range(mem, at, 4)?;
                        }
                        let return_code = service
                            .return_code
                            .as_ref()
                            .map(|operand| match operand {
                                StorageOperand::Memory(a) => address(a, &registers).map(Some),
                                StorageOperand::Register(r) if *r <= 15 => Ok(None),
                                _ => Err("invalid storage return-code destination".into()),
                            })
                            .transpose()?;
                        if let Some(Some(at)) = return_code {
                            range(mem, at, 4)?;
                        }
                        let release_address = if service.obtain {
                            0
                        } else {
                            let address = value(&service.address)?;
                            if amode == 64 && address > 0x7fff_ffff {
                                return Err("service parameter address must be below 2 GB".into());
                            }
                            address as usize & 0x7fff_ffff
                        };
                        let result = if service.obtain {
                            storage
                                .obtain(
                                    mem,
                                    length.ok_or("storage length required")?,
                                    subpool,
                                    service.below_16m,
                                    service.page,
                                )
                                .map(Some)
                        } else if length == Some(0) && release_address == 0 {
                            storage.release_subpool(mem, subpool).map(|()| None)
                        } else if let Some(length) = length {
                            storage
                                .release(mem, release_address, length, subpool)
                                .map(|()| None)
                        } else {
                            storage.release_subpool(mem, subpool).map(|()| None)
                        };
                        let rc = match result {
                            Ok(Some(at)) => {
                                registers[1] = at as u32;
                                if ins.op == "STORAGE" || amode == 64 {
                                    high[1] = 0;
                                }
                                if let Some(destination) = destination {
                                    storage.check_write(destination, 4)?;
                                    put_word(mem, destination, at as u32)?;
                                } else if let StorageOperand::Register(r) = service.address {
                                    registers[r as usize] = at as u32;
                                    if ins.op == "STORAGE" || amode == 64 {
                                        high[r as usize] = 0;
                                    }
                                } else {
                                    return Err("invalid storage obtain destination".into());
                                }
                                0
                            }
                            Ok(None) => 0,
                            Err(_) if service.conditional => 4,
                            Err(e) => return Err(e),
                        };
                        registers[15] = rc;
                        if let Some(Some(at)) = return_code {
                            storage.check_write(at, 4)?;
                            put_word(mem, at, rc)?;
                        } else if let Some(StorageOperand::Register(r)) = service.return_code {
                            registers[r as usize] = rc;
                        }
                    }
                    op if super::operation::relative(op).is_some() => {
                        use super::operation::RelativeKind;
                        let spec = super::operation::relative(op).unwrap();
                        let target = relative_address(ins, base, amode)? as u64;
                        let count = match spec.kind {
                            RelativeKind::Execute => {
                                return Err("execute exception: nested EXECUTE".into());
                            }
                            RelativeKind::Condition(_) => {
                                if ins.immediate > 15 {
                                    return Err("invalid branch mask".into());
                                }
                                if ins.immediate & (8 >> cc) != 0 {
                                    branch = Some(target as usize);
                                }
                                None
                            }
                            RelativeKind::Save => {
                                let r = reg(0)?;
                                if amode == 64 {
                                    wide::write(&mut registers, &mut high, r, next_address as u64);
                                } else {
                                    registers[r] =
                                        super::abi::tagged_entry(next_address as u64, amode)?
                                            as u32;
                                }
                                branch = Some(target as usize);
                                None
                            }
                            RelativeKind::LoadAddress => {
                                let r = reg(0)?;
                                if amode == 64 {
                                    wide::write(&mut registers, &mut high, r, target);
                                } else {
                                    registers[r] = target as u32;
                                }
                                None
                            }
                            RelativeKind::Count32 => {
                                let r = reg(0)?;
                                registers[r] = registers[r].wrapping_sub(1);
                                Some(u64::from(registers[r]))
                            }
                            RelativeKind::Count64 => {
                                let r = reg(0)?;
                                let value = wide::read(&registers, &high, r).wrapping_sub(1);
                                wide::write(&mut registers, &mut high, r, value);
                                Some(value)
                            }
                            RelativeKind::CountHigh => {
                                let r = reg(0)?;
                                high[r] = high[r].wrapping_sub(1);
                                Some(u64::from(high[r]))
                            }
                        };
                        if count.is_some_and(|value| value != 0) {
                            branch = Some(target as usize);
                        }
                    }
                    "BCTR" | "BCTGR" | "BCTG" => {
                        let r = reg(0)?;
                        let target = if op == "BCTG" {
                            Some(branch_address(
                                ins.address.as_ref().ok_or("missing branch address")?,
                                &registers,
                                amode,
                            )?)
                        } else {
                            let r2 = reg(1)?;
                            (r2 != 0).then_some(super::abi::register(&registers, &high, r2, amode)?)
                        };
                        let value = if op == "BCTR" {
                            registers[r] = registers[r].wrapping_sub(1);
                            u64::from(registers[r])
                        } else {
                            let value = wide::read(&registers, &high, r).wrapping_sub(1);
                            wide::write(&mut registers, &mut high, r, value);
                            value
                        };
                        if value != 0 {
                            branch = target;
                        }
                    }
                    "LH" | "IC" => {
                        let (r, a) = (reg(0)?, ea()?);
                        let len = if op == "LH" { 2 } else { 1 };
                        let bytes = &mem[range(mem, a, len)?];
                        registers[r] = if len == 2 {
                            i16::from_be_bytes([bytes[0], bytes[1]]) as i32 as u32
                        } else {
                            (registers[r] & 0xffff_ff00) | u32::from(bytes[0])
                        };
                    }
                    "STH" | "STC" => {
                        let (r, a) = (reg(0)?, ea()?);
                        let len = if op == "STH" { 2 } else { 1 };
                        let dest = range(mem, a, len)?;
                        storage.check_write(a, len)?;
                        mem[dest].copy_from_slice(&registers[r].to_be_bytes()[4 - len..]);
                    }
                    "LLC" | "LLH" => {
                        let r = reg(0)?;
                        let addr = ea()?;
                        registers[r] = if op == "LLC" {
                            u32::from(mem[range(mem, addr, 1)?][0])
                        } else {
                            u32::from(u16::from_be_bytes(
                                mem[range(mem, addr, 2)?].try_into().unwrap(),
                            ))
                        };
                    }
                    "L" => {
                        let (r, a) = (reg(0)?, ea()?);
                        registers[r] = word(mem, a)?;
                    }
                    "ST" => {
                        let (r, a) = (reg(0)?, ea()?);
                        storage.check_write(a, 4)?;
                        put_word(mem, a, registers[r])?;
                    }
                    "LA" => {
                        let (r, a) = (reg(0)?, ea()?);
                        if amode == 64 {
                            wide::write(&mut registers, &mut high, r, a as u64);
                        } else {
                            registers[r] =
                                u32::try_from(a).map_err(|_| "address exceeds fullword")?;
                        }
                    }
                    "LR" | "LTR" => {
                        let (a, b) = (reg(0)?, reg(1)?);
                        registers[a] = registers[b];
                        if op == "LTR" {
                            cc = signed_cc(registers[a]);
                        }
                    }
                    "LCR" | "LPR" | "LNR" => {
                        let (a, b) = (reg(0)?, reg(1)?);
                        let v = registers[b] as i32;
                        let (v, overflow) = match op {
                            "LCR" => v.overflowing_neg(),
                            "LPR" => v.overflowing_abs(),
                            _ => (if v > 0 { -v } else { v }, false),
                        };
                        registers[a] = v as u32;
                        cc = if overflow { 3 } else { signed_cc(v as u32) };
                    }
                    "A" | "S" | "C" | "CL" | "AL" | "SL" | "N" | "O" | "X" | "AH" | "SH" | "CH"
                    | "NR" | "OR" | "CLR" | "ALR" | "SLR" => {
                        let r = reg(0)?;
                        let v = if op.ends_with('R') {
                            registers[reg(1)?]
                        } else if op.ends_with('H') {
                            let a = ea()?;
                            let b = &mem[range(mem, a, 2)?];
                            i16::from_be_bytes([b[0], b[1]]) as i32 as u32
                        } else {
                            word(mem, ea()?)?
                        };
                        let a = registers[r];
                        match op {
                            "C" | "CH" => cc = ordering_cc((a as i32).cmp(&(v as i32))),
                            "CL" | "CLR" => cc = ordering_cc(a.cmp(&v)),
                            "N" | "NR" | "O" | "OR" | "X" => {
                                registers[r] = match op {
                                    "N" | "NR" => a & v,
                                    "O" | "OR" => a | v,
                                    _ => a ^ v,
                                };
                                cc = u8::from(registers[r] != 0);
                            }
                            "AL" | "ALR" | "SL" | "SLR" => {
                                let (n, carry) = if op.starts_with('A') {
                                    a.overflowing_add(v)
                                } else {
                                    let (n, borrow) = a.overflowing_sub(v);
                                    (n, !borrow)
                                };
                                registers[r] = n;
                                cc = u8::from(n != 0) + 2 * u8::from(carry);
                            }
                            _ => {
                                let (n, overflow) = if op.starts_with('A') {
                                    (a as i32).overflowing_add(v as i32)
                                } else {
                                    (a as i32).overflowing_sub(v as i32)
                                };
                                registers[r] = n as u32;
                                cc = if overflow { 3 } else { signed_cc(n as u32) };
                            }
                        }
                    }
                    "MH" | "M" | "MR" | "D" | "DR" => {
                        let r = reg(0)?;
                        let v = if op == "MR" || op == "DR" {
                            registers[reg(1)?] as i32
                        } else if op == "MH" {
                            let a = ea()?;
                            let b = &mem[range(mem, a, 2)?];
                            i32::from(i16::from_be_bytes([b[0], b[1]]))
                        } else {
                            word(mem, ea()?)? as i32
                        };
                        if op == "MH" {
                            registers[r] = (registers[r] as i32).wrapping_mul(v) as u32;
                        } else {
                            if !r.is_multiple_of(2) {
                                return Err("multiply/divide requires an even register pair".into());
                            }
                            if op.starts_with('M') {
                                let n = i64::from(registers[r + 1] as i32) * i64::from(v);
                                registers[r] = (n >> 32) as u32;
                                registers[r + 1] = n as u32;
                            } else {
                                let dividend = ((u64::from(registers[r]) << 32)
                                    | u64::from(registers[r + 1]))
                                    as i64;
                                let quotient = dividend
                                    .checked_div(i64::from(v))
                                    .and_then(|q| i32::try_from(q).ok())
                                    .ok_or("fixed-point divide exception")?;
                                registers[r] = (dividend % i64::from(v)) as u32;
                                registers[r + 1] = quotient as u32;
                            }
                        }
                    }
                    "SLL" | "SRL" | "SLA" | "SRA" => {
                        let (r, count) = (reg(0)?, (ea()? & 63) as u32);
                        let v = registers[r];
                        registers[r] = match op {
                            "SLL" => v.checked_shl(count).unwrap_or(0),
                            "SRL" => v.checked_shr(count).unwrap_or(0),
                            "SRA" => ((v as i32) >> count.min(31)) as u32,
                            _ => {
                                (v & 0x8000_0000)
                                    | (v.checked_shl(count).unwrap_or(0) & 0x7fff_ffff)
                            }
                        };
                        if op == "SRA" {
                            cc = signed_cc(registers[r]);
                        }
                        if op == "SLA" {
                            let shifted = i64::from(v as i32)
                                .checked_mul(1i64.checked_shl(count).unwrap_or(0));
                            let overflow = count >= 32 && v != 0
                                || shifted.is_none_or(|n| i32::try_from(n).is_err());
                            cc = if overflow { 3 } else { signed_cc(registers[r]) };
                        }
                    }
                    "SR" | "AR" | "XR" | "CR" => {
                        let (a, b) = (reg(0)?, reg(1)?);
                        if op == "CR" {
                            cc = ordering_cc((registers[a] as i32).cmp(&(registers[b] as i32)));
                        } else {
                            let (value, overflow) = match op {
                                "SR" => (registers[a] as i32).overflowing_sub(registers[b] as i32),
                                "AR" => (registers[a] as i32).overflowing_add(registers[b] as i32),
                                _ => ((registers[a] ^ registers[b]) as i32, false),
                            };
                            registers[a] = value as u32;
                            cc = if overflow {
                                3
                            } else if op == "XR" {
                                u8::from(value != 0)
                            } else {
                                signed_cc(value as u32)
                            };
                        }
                    }
                    "LHI" | "MHI" | "CHI" => {
                        let r = reg(0)?;
                        match op {
                            "LHI" => registers[r] = ins.immediate,
                            "MHI" => registers[r] = registers[r].wrapping_mul(ins.immediate),
                            _ => {
                                cc = ordering_cc((registers[r] as i32).cmp(&(ins.immediate as i32)))
                            }
                        }
                    }
                    "AHI" => {
                        let r = reg(0)?;
                        let (v, of) = (registers[r] as i32).overflowing_add(ins.immediate as i32);
                        registers[r] = v as u32;
                        cc = if of { 3 } else { signed_cc(v as u32) };
                    }
                    "STM" | "LM" => {
                        let (first, last, a) = (reg(0)?, reg(1)?, ea()?);
                        let count = (last + 16 - first) % 16 + 1;
                        range(mem, a, count * 4)?;
                        for i in 0..count {
                            let r = (first + i) % 16;
                            if op == "STM" {
                                storage.check_write(a + i * 4, 4)?;
                                put_word(mem, a + i * 4, registers[r])?;
                            } else {
                                registers[r] = word(mem, a + i * 4)?;
                            }
                        }
                    }
                    "MVI" | "CLI" | "NI" | "OI" | "XI" | "TM" => {
                        let a = ea()?;
                        range(mem, a, 1)?;
                        if ins.immediate > 255 {
                            return Err("invalid byte immediate".into());
                        }
                        if matches!(op, "MVI" | "NI" | "OI" | "XI") {
                            storage.check_write(a, 1)?;
                        }
                        if op == "MVI" {
                            mem[a] = ins.immediate as u8;
                        } else if op == "CLI" {
                            cc = ordering_cc(mem[a].cmp(&(ins.immediate as u8)));
                        } else if op == "TM" {
                            let masked = mem[a] & ins.immediate as u8;
                            cc = if masked == 0 {
                                0
                            } else if masked == ins.immediate as u8 {
                                3
                            } else {
                                1
                            };
                        } else {
                            mem[a] = match op {
                                "NI" => mem[a] & ins.immediate as u8,
                                "OI" => mem[a] | ins.immediate as u8,
                                _ => mem[a] ^ ins.immediate as u8,
                            };
                            cc = u8::from(mem[a] != 0);
                        }
                    }
                    "TRT" | "TRTR" => {
                        let a = ea()?;
                        let table = address(
                            ins.source.as_ref().ok_or("missing translation table")?,
                            &registers,
                        )?;
                        let length = ins.length as usize;
                        if !(1..=256).contains(&length) {
                            return Err("invalid translate-and-test length".into());
                        }
                        let first = if op == "TRTR" {
                            a.checked_sub(length - 1)
                                .ok_or("reverse translation address underflow")?
                        } else {
                            a
                        };
                        range(mem, first, length)?;
                        cc = 0;
                        for scanned in 0..length {
                            let at = if op == "TRTR" {
                                a - scanned
                            } else {
                                a + scanned
                            };
                            let function = table
                                .checked_add(usize::from(mem[at]))
                                .ok_or("translation table address overflow")?;
                            range(mem, function, 1)?;
                            let value = mem[function];
                            if value != 0 {
                                if amode == 64 {
                                    wide::write(&mut registers, &mut high, 1, at as u64);
                                } else {
                                    let mask = super::abi::mask(amode)? as u32;
                                    registers[1] = (registers[1] & !mask) | (at as u32 & mask);
                                }
                                registers[2] = (registers[2] & !255) | u32::from(value);
                                cc = if scanned + 1 == length { 2 } else { 1 };
                                break;
                            }
                        }
                    }
                    "SRP" => {
                        let a = ea()?;
                        let length = ins.length as usize;
                        if !(1..=16).contains(&length) || ins.immediate > 9 {
                            return Err("invalid SRP length or rounding digit".into());
                        }
                        let r = range(mem, a, length)?;
                        storage.check_write(a, length)?;
                        let n = packed(&mem[r.clone()])?;
                        let raw = branch_address(
                            ins.source.as_ref().ok_or("missing SRP shift")?,
                            &registers,
                            amode,
                        )? as u32
                            & 63;
                        let shift = if raw >= 32 {
                            raw as i32 - 64
                        } else {
                            raw as i32
                        };
                        let magnitude = n.unsigned_abs();
                        let digits = length as u32 * 2 - 1;
                        let (value, overflow) = if shift > 0 {
                            let shift = shift as u32;
                            if shift >= digits {
                                (0, magnitude != 0)
                            } else {
                                let limit = 10u128.pow(digits - shift);
                                ((magnitude % limit) * 10u128.pow(shift), magnitude >= limit)
                            }
                        } else if shift < 0 {
                            let shift = shift.unsigned_abs();
                            (
                                (magnitude + u128::from(ins.immediate) * 10u128.pow(shift - 1))
                                    / 10u128.pow(shift),
                                false,
                            )
                        } else {
                            (magnitude, false)
                        };
                        let result = if n < 0 {
                            -(value as i128)
                        } else {
                            value as i128
                        };
                        store_packed(&mut mem[r], result)?;
                        if overflow && n < 0 && result == 0 {
                            mem[a + length - 1] = (mem[a + length - 1] & 0xf0) | 0xd;
                        }
                        cc = if overflow {
                            3
                        } else if result == 0 {
                            0
                        } else if result < 0 {
                            1
                        } else {
                            2
                        };
                    }
                    "MVC" | "CLC" | "XC" | "NC" | "OC" | "MVN" | "MVZ" | "TR" => {
                        let a = ea()?;
                        let b = address(
                            ins.source.as_ref().ok_or("missing second address")?,
                            &registers,
                        )?;
                        let len = ins.length as usize;
                        if !(1..=256).contains(&len) {
                            return Err("invalid SS length".into());
                        }
                        let ra = range(mem, a, len)?;
                        if op != "CLC" {
                            storage.check_write(a, len)?;
                        }
                        let rb = if op == "TR" {
                            b..b
                        } else {
                            range(mem, b, len)?
                        };
                        if op == "CLC" {
                            cc = ordering_cc(mem[ra].cmp(&mem[rb]));
                        } else {
                            // MVC/XC are left-to-right operations; overlap deliberately
                            // observes bytes stored by an earlier iteration (fill idiom).
                            let mut nonzero = false;
                            for i in 0..len {
                                let value = match op {
                                    "XC" => mem[a + i] ^ mem[b + i],
                                    "NC" => mem[a + i] & mem[b + i],
                                    "OC" => mem[a + i] | mem[b + i],
                                    "MVN" => (mem[a + i] & 0xf0) | (mem[b + i] & 0x0f),
                                    "MVZ" => (mem[a + i] & 0x0f) | (mem[b + i] & 0xf0),
                                    "TR" => {
                                        let table = b + usize::from(mem[a + i]);
                                        range(mem, table, 1)?;
                                        mem[table]
                                    }
                                    _ => mem[b + i],
                                };
                                mem[a + i] = value;
                                nonzero |= value != 0;
                            }
                            if matches!(op, "XC" | "NC" | "OC") {
                                cc = u8::from(nonzero);
                            }
                        }
                    }
                    "CVB" | "CVD" => {
                        let (r, a) = (reg(0)?, ea()?);
                        let dest = range(mem, a, 8)?;
                        if op == "CVB" {
                            registers[r] = i32::try_from(packed(&mem[dest])?)
                                .map_err(|_| "CVB fixed-point overflow")?
                                as u32;
                        } else {
                            storage.check_write(a, 8)?;
                            store_packed(&mut mem[dest], i128::from(registers[r] as i32))?;
                        }
                    }
                    "PACK" | "UNPK" | "ZAP" | "AP" | "SP" | "CP" | "MP" | "DP" => {
                        let a = ea()?;
                        let b = address(
                            ins.source.as_ref().ok_or("missing source address")?,
                            &registers,
                        )?;
                        let (l1, l2) = (ins.length as usize, ins.source_length as usize);
                        if !(1..=16).contains(&l1) || !(1..=16).contains(&l2) {
                            return Err("decimal lengths must be 1..16".into());
                        }
                        let (ra, rb) = (range(mem, a, l1)?, range(mem, b, l2)?);
                        if op != "CP" {
                            storage.check_write(a, l1)?;
                        }
                        if op == "PACK" || op == "UNPK" {
                            let source = mem[rb].to_vec();
                            let mut nibbles = Vec::new();
                            if op == "PACK" {
                                for byte in source.iter().rev() {
                                    nibbles.push(byte & 15);
                                }
                                let sign = source[l2 - 1] >> 4;
                                let mut digits = vec![0; l1 * 2];
                                digits[l1 * 2 - 1] = sign;
                                for (i, n) in nibbles.into_iter().take(l1 * 2 - 1).enumerate() {
                                    digits[l1 * 2 - 2 - i] = n;
                                }
                                for (i, pair) in digits.as_chunks::<2>().0.iter().enumerate() {
                                    mem[a + i] = pair[0] * 16 + pair[1];
                                }
                            } else {
                                for byte in &source {
                                    nibbles.push(byte >> 4);
                                    nibbles.push(byte & 15);
                                }
                                let sign = nibbles.pop().unwrap();
                                mem[ra].fill(0xf0);
                                for (i, n) in nibbles.into_iter().rev().take(l1).enumerate() {
                                    mem[a + l1 - 1 - i] |= n;
                                }
                                mem[a + l1 - 1] = (sign << 4) | (mem[a + l1 - 1] & 15);
                            }
                        } else {
                            let right = packed(&mem[rb])?;
                            let left = if op == "ZAP" {
                                0
                            } else {
                                packed(&mem[ra.clone()])?
                            };
                            if op == "CP" {
                                cc = ordering_cc(left.cmp(&right));
                            } else if op == "DP" {
                                if l2 > 8 || l1 <= l2 {
                                    return Err("invalid decimal divide lengths".into());
                                }
                                let q =
                                    left.checked_div(right).ok_or("decimal divide exception")?;
                                let rem = left % right;
                                if decimal_overflow(q, l1 - l2) {
                                    return Err("decimal quotient overflow".into());
                                }
                                store_packed(&mut mem[a..a + l1 - l2], q)?;
                                store_packed(&mut mem[a + l1 - l2..a + l1], rem)?;
                            } else {
                                if op == "MP"
                                    && (l2 > 8
                                        || l1 <= l2
                                        || mem[a..a + l2].iter().any(|b| *b != 0))
                                {
                                    return Err("invalid decimal multiply operand".into());
                                }
                                let n = match op {
                                    "ZAP" => Some(right),
                                    "AP" => left.checked_add(right),
                                    "SP" => left.checked_sub(right),
                                    _ => left.checked_mul(right),
                                }
                                .ok_or("decimal arithmetic overflow")?;
                                let overflow = store_packed(&mut mem[ra], n)?;
                                if op != "MP" {
                                    cc = if overflow { 3 } else { ordering_cc(n.cmp(&0)) };
                                }
                            }
                        }
                    }
                    "SPM" => {
                        let value = registers[reg(0)?];
                        floating.program_mask = ((value >> 24) & 15) as u8;
                        cc = ((value >> 28) & 3) as u8;
                    }
                    "IPM" => {
                        let r = reg(0)?;
                        registers[r] = (registers[r] & 0x00ffffff)
                            | (u32::from(floating.program_mask) << 24)
                            | (u32::from(cc) << 28);
                    }
                    "SAM24" | "SAM31" | "SAM64" => {
                        amode = if op == "SAM24" {
                            24
                        } else if op == "SAM31" {
                            31
                        } else {
                            64
                        };
                        branch = Some((next_address as u64 & super::abi::mask(amode)?) as usize);
                    }
                    "BASSM" | "BSM" => {
                        let (r, s) = (reg(0)?, reg(1)?);
                        let raw = wide::read(&registers, &high, s);
                        let old = amode;
                        if op == "BASSM" {
                            let link = super::abi::tagged_entry(next_address as u64, old)?;
                            if old == 64 {
                                wide::write(&mut registers, &mut high, r, link);
                            } else {
                                registers[r] = link as u32;
                            }
                            if r == 14 {
                                branch_return_mode = Some(old);
                            }
                        } else if r != 0 {
                            if old == 64 {
                                let v = wide::read(&registers, &high, r) | 1;
                                wide::write(&mut registers, &mut high, r, v);
                            } else {
                                registers[r] = (registers[r] & !0x80000000)
                                    | if old == 31 { 0x80000000 } else { 0 };
                            }
                        }
                        if s != 0 {
                            amode = super::abi::branch_mode(raw);
                            branch = Some(
                                ((raw & super::abi::mask(amode)?)
                                    & if amode == 64 { !1 } else { u64::MAX })
                                    as usize,
                            );
                        }
                    }
                    "BALR" | "BASR" | "BAL" | "BAS" => {
                        let r = reg(0)?;
                        let target = if op.ends_with('R') {
                            let r2 = reg(1)?;
                            (r2 != 0).then_some(super::abi::register(&registers, &high, r2, amode)?)
                        } else {
                            Some(ea()?)
                        };
                        if amode == 64 {
                            wide::write(&mut registers, &mut high, r, next_address as u64);
                        } else {
                            registers[r] =
                                super::abi::tagged_entry(next_address as u64, amode)? as u32;
                            if amode == 24 && matches!(op, "BAL" | "BALR") {
                                let ilc = if modified.is_some() {
                                    super::assemble::instruction_size(&fetched.op).unwrap() / 2
                                } else {
                                    super::assemble::instruction_size(op).unwrap() / 2
                                };
                                registers[r] |= (ilc << 30)
                                    | (u32::from(cc) << 28)
                                    | (u32::from(floating.program_mask) << 24);
                            }
                        }
                        branch = target;
                    }
                    "BR" | "BCR" => {
                        if op == "BR" || (ins.immediate & (8 >> cc)) != 0 {
                            let r = reg(0)?;
                            if r != 0 {
                                branch = Some(super::abi::register(&registers, &high, r, amode)?);
                            }
                        }
                    }
                    "B" | "BE" | "BNE" | "BH" | "BL" => {
                        let taken = match op {
                            "B" => true,
                            "BE" => cc == 0,
                            "BNE" => cc != 0,
                            "BH" => cc == 2,
                            _ => cc == 1,
                        };
                        if taken {
                            branch = Some(ea()?);
                        }
                    }
                    "BC" => {
                        if ins.immediate > 15 {
                            return Err("invalid branch mask".into());
                        }
                        if ins.immediate & (8 >> cc) != 0 {
                            branch = Some(ea()?);
                        }
                    }
                    "BCT" => {
                        let (r, target) = (reg(0)?, ea()?);
                        registers[r] = registers[r].wrapping_sub(1);
                        if registers[r] != 0 {
                            branch = Some(target);
                        }
                    }
                    "STIMER" => {
                        let ticks = word(mem, ea()?)?;
                        std::thread::sleep(std::time::Duration::from_millis(u64::from(ticks) * 10));
                    }
                    _ => return Err(format!("unsupported instruction {}", ins.op)),
                }
                if branch == Some(return_target as usize) {
                    return Ok((None, true));
                }
                Ok((branch, false))
            };
            let outcome = run().and_then(|result| {
                let op = super::assemble::canonical_operation(&ins.op);
                if cc == 3
                    && floating.program_mask & 8 != 0
                    && matches!(
                        op,
                        "A" | "AR"
                            | "AH"
                            | "AHI"
                            | "S"
                            | "SR"
                            | "SH"
                            | "SLA"
                            | "SLDA"
                            | "LCR"
                            | "LPR"
                            | "AG"
                            | "AGR"
                            | "AGH"
                            | "AGHI"
                            | "AGFI"
                            | "AGF"
                            | "AGFR"
                            | "SG"
                            | "SGR"
                            | "SGF"
                            | "SGFR"
                            | "SLAG"
                            | "LCGR"
                            | "LPGR"
                    )
                {
                    return Err("fixed-point-overflow exception".into());
                }
                if cc == 3
                    && floating.program_mask & 4 != 0
                    && matches!(op, "AP" | "SP" | "ZAP" | "SRP")
                {
                    return Err("decimal-overflow exception".into());
                }
                Ok(result)
            });
            storage
                .verify_protection(mem)
                .map_err(|e| format!("{}:{}: {}: {e}", ins.file, ins.line, ins.op))?;
            let (branch, returned) = outcome.map_err(|e| {
                if e.starts_with('\0') {
                    e
                } else {
                    format!("{}:{}: {}: {e}", ins.file, ins.line, ins.op)
                }
            })?;
            if returned {
                return Ok(registers[15] as i32);
            }
            pc = match branch {
                Some(a) if local(a) => a,
                Some(a) => {
                    let mut state = super::Registers {
                        low: registers,
                        high,
                        cc,
                        floating: floating.clone(),
                        address_mode: amode,
                        return_mode: branch_return_mode,
                        parameter_list: original_parameters,
                    };
                    let resume = host.branch(a, &mut state, mem, storage).map_err(|error| {
                        if error.starts_with('\0') {
                            error
                        } else {
                            format!("{}:{}: {}: {error}", ins.file, ins.line, ins.op)
                        }
                    })?;
                    registers = state.low;
                    high = state.high;
                    cc = state.cc;
                    floating = state.floating;
                    amode = state.address_mode;
                    if resume == return_target as usize {
                        return Ok(registers[15] as i32);
                    }
                    if !local(resume) {
                        return Err("loaded entry returned to an unmapped instruction".into());
                    }
                    resume
                }
                None if local(next_address) => next_address,
                None => {
                    return Err(
                        "HLASM execution entered data or unmapped instruction storage".into(),
                    );
                }
            };
        }
        Err("HLASM instruction limit exceeded".into())
    })();
    storage.release_frame(mem, frame);
    if let Some(state) = call_state.as_mut() {
        **state = floating.clone();
    }
    if let Some(initial) = initial {
        initial.low = registers;
        initial.high = high;
        initial.cc = cc;
        initial.floating = floating;
        initial.address_mode = amode;
    }
    result
}

fn signed_cc(value: u32) -> u8 {
    ordering_cc((value as i32).cmp(&0))
}

fn packed(bytes: &[u8]) -> Result<i128, String> {
    let mut n = 0i128;
    for (i, b) in bytes.iter().enumerate() {
        if b >> 4 > 9 {
            return Err("invalid packed decimal digit".into());
        }
        n = n * 10 + i128::from(b >> 4);
        if i + 1 < bytes.len() {
            if b & 15 > 9 {
                return Err("invalid packed decimal digit".into());
            }
            n = n * 10 + i128::from(b & 15);
        } else {
            match b & 15 {
                0xb | 0xd => n = -n,
                0xa | 0xc | 0xe | 0xf => {}
                _ => return Err("invalid packed decimal sign".into()),
            }
        }
    }
    Ok(n)
}

fn decimal_overflow(n: i128, len: usize) -> bool {
    n.unsigned_abs() >= 10u128.pow((len * 2 - 1) as u32)
}

fn store_packed(bytes: &mut [u8], n: i128) -> Result<bool, String> {
    if bytes.is_empty() || bytes.len() > 16 {
        return Err("invalid packed decimal length".into());
    }
    let overflow = decimal_overflow(n, bytes.len());
    let mut value = n.unsigned_abs();
    let last = bytes.len() - 1;
    bytes[last] = ((value % 10) as u8) << 4 | if n < 0 { 0xd } else { 0xc };
    value /= 10;
    for b in bytes[..last].iter_mut().rev() {
        *b = (value % 10) as u8;
        value /= 10;
        *b |= ((value % 10) as u8) << 4;
        value /= 10;
    }
    Ok(overflow)
}
fn ordering_cc(order: std::cmp::Ordering) -> u8 {
    match order {
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Greater => 2,
    }
}

#[cfg(test)]
mod invocation_tests {
    use super::*;
    #[test]
    fn invalid_caller_or_parameter_modes_do_not_allocate_or_leak_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("own-entry.asm");
        std::fs::write(&path, "MAIN CSECT\n BR 14\n END\n").unwrap();
        let module = crate::hlasm::compile_file(&path, &[]).unwrap();
        let mut memory = vec![0; 128];
        memory.extend(module.storage_at(128).unwrap());
        let original = memory.clone();
        let mut storage = crate::hlasm::DynamicStorage::default();
        for (caller_mode, parameter_mode) in [(Some(7), None), (None, Some(7))] {
            let mut registers = crate::hlasm::Registers {
                return_mode: caller_mode,
                ..Default::default()
            };
            for _ in 0..3 {
                assert!(
                    execute_entry_with_host(
                        &module,
                        &mut memory,
                        128,
                        &[],
                        module.entry,
                        &mut storage,
                        &mut crate::hlasm::services::NoHost,
                        true,
                        parameter_mode,
                        Some(&mut registers),
                        None
                    )
                    .unwrap_err()
                    .contains("addressing mode")
                );
                assert_eq!(memory, original);
                assert_eq!(storage.allocated_bytes(), 0);
            }
        }
        assert_eq!(
            execute_entry(&module, &mut memory, 128, &[], module.entry, &mut storage).unwrap(),
            128
        );
        assert_eq!(memory, original);
        assert_eq!(storage.allocated_bytes(), 0);
    }
}

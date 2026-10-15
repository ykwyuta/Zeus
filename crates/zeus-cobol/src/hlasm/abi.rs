//! Addressing and invocation layouts shared by source execution and runtime
//! dispatch. Emulated addresses are run-unit offsets, never host pointers.
use super::{Address, Instruction, StorageOperand};
use std::borrow::Cow;

pub(crate) fn mask(mode: u8) -> Result<u64, String> {
    match mode {
        24 => Ok(0x00ff_ffff),
        31 => Ok(0x7fff_ffff),
        64 => Ok(u64::MAX),
        _ => Err("invalid addressing mode".into()),
    }
}
pub(crate) fn read(low: &[u32; 16], high: &[u32; 16], r: usize) -> u64 {
    u64::from(low[r]) | (u64::from(high[r]) << 32)
}
pub(crate) fn register(
    low: &[u32; 16],
    high: &[u32; 16],
    r: usize,
    mode: u8,
) -> Result<usize, String> {
    if r >= 16 {
        return Err("invalid address register".into());
    }
    let value = read(low, high, r) & mask(mode)?;
    usize::try_from(value).map_err(|_| "HLASM address exceeds host offset range".into())
}
pub(crate) fn effective(
    a: &Address,
    low: &[u32; 16],
    high: &[u32; 16],
    mode: u8,
) -> Result<usize, String> {
    let component = |r: u8| -> Result<u64, String> {
        if r == 0 {
            Ok(0)
        } else if r < 16 {
            Ok(read(low, high, usize::from(r)))
        } else {
            Err("invalid address register".into())
        }
    };
    let value = component(a.base)?
        .wrapping_add(component(a.index)?)
        .wrapping_add(a.displacement as u64)
        & mask(mode)?;
    usize::try_from(value).map_err(|_| "HLASM address exceeds host offset range".into())
}
fn resolved(at: usize) -> Result<Address, String> {
    Ok(Address {
        displacement: i64::try_from(at).map_err(|_| "HLASM address outside run-unit storage")?,
        base: 0,
        index: 0,
    })
}
pub(crate) fn prepare<'a>(
    ins: &'a Instruction,
    low: &[u32; 16],
    high: &[u32; 16],
    mode: u8,
) -> Result<Cow<'a, Instruction>, String> {
    if mode == 31 {
        return Ok(Cow::Borrowed(ins));
    }
    let mut ins = ins.clone();
    let fix = |a: &mut Address| -> Result<(), String> {
        *a = resolved(effective(a, low, high, mode)?)?;
        Ok(())
    };
    if super::operation::relative(&ins.op).is_none()
        && let Some(a) = &mut ins.address
    {
        if super::float::operations::classify(&ins.op).is_some() {
            *a = resolved(effective(a, low, high, mode)? & 4095)?;
        } else if super::float::operations::decimal_shift(&ins.op).is_some()
            || matches!(
                ins.op.as_str(),
                "SLL"
                    | "SRL"
                    | "SLA"
                    | "SRA"
                    | "SLDL"
                    | "SRDL"
                    | "SLDA"
                    | "SRDA"
                    | "SLLG"
                    | "SRLG"
                    | "SLAG"
                    | "SRAG"
                    | "RLL"
                    | "RLLG"
            )
        {
            *a = resolved(effective(a, low, high, mode)? & 63)?;
        } else {
            fix(a)?;
        }
    }
    if let Some(a) = &mut ins.source {
        fix(a)?;
    }
    let pointer = |operand: &mut StorageOperand| -> Result<(), String> {
        match operand {
            StorageOperand::Register(r) => {
                *operand =
                    StorageOperand::Address(resolved(register(low, high, usize::from(*r), mode)?)?)
            }
            StorageOperand::Address(a) | StorageOperand::Memory(a) => fix(a)?,
            _ => {}
        }
        Ok(())
    };
    if let Some(service) = &mut ins.module_service {
        if let super::ModuleName::Address(a) = &mut service.name {
            pointer(a)?;
        }
        if let Some(parameters) = &mut service.parameters {
            for p in parameters {
                pointer(p)?;
            }
        }
    }
    // Supervisor services accept 31-bit parameters even when invoked in
    // AMODE 64. Reject an out-of-range pointer before any 32-bit service
    // helper can discard its high half.
    let below_bar = |operand: &mut StorageOperand| -> Result<(), String> {
        pointer(operand)?;
        if let StorageOperand::Address(a) | StorageOperand::Memory(a) = operand
            && (a.displacement < 0 || a.displacement > 0x7fff_ffff)
        {
            return Err("service parameter address must be below 2 GB".into());
        }
        Ok(())
    };
    if let Some(service) = &mut ins.system_service {
        match service {
            super::SystemService::Write {
                text: super::MessageText::Address(a),
                ..
            } => below_bar(a)?,
            super::SystemService::Post { ecb, .. } => below_bar(ecb)?,
            super::SystemService::Wait { ecbs, .. } => below_bar(ecbs)?,
            _ => {}
        }
    }
    if let Some(service) = &mut ins.data_service {
        match service {
            super::DataService::Open(dcbs) => {
                for (dcb, _, _) in dcbs {
                    below_bar(dcb)?;
                }
            }
            super::DataService::Close(dcbs) => {
                for dcb in dcbs {
                    below_bar(dcb)?;
                }
            }
            super::DataService::Get { dcb, area } | super::DataService::Put { dcb, area } => {
                below_bar(dcb)?;
                if let Some(area) = area {
                    below_bar(area)?;
                }
            }
            super::DataService::FreePool(dcb) => below_bar(dcb)?,
        }
    }
    if let Some(service) = &mut ins.storage_service {
        if !service.obtain {
            match &mut service.address {
                StorageOperand::Register(_) | StorageOperand::Address(_) => {
                    below_bar(&mut service.address)?
                }
                StorageOperand::Memory(_) => below_bar(&mut service.address)?,
                _ => {}
            }
        }
        for operand in [&mut service.length, &mut service.return_code] {
            if let Some(StorageOperand::Memory(a) | StorageOperand::Address(a)) = operand {
                fix(a)?;
            }
        }
        for operand in [&mut service.address, &mut service.subpool] {
            if let StorageOperand::Memory(a) | StorageOperand::Address(a) = operand {
                fix(a)?;
            }
        }
    }
    Ok(Cow::Owned(ins))
}

pub(crate) fn save_length(mode: u8) -> usize {
    if mode == 64 { 144 } else { 72 }
}
pub(crate) fn parameter_width(mode: u8) -> usize {
    if mode == 64 { 8 } else { 4 }
}
pub(crate) fn return_sentinel(mode: u8) -> u64 {
    if mode == 64 {
        u64::MAX - 1
    } else if mode == 24 {
        0x00ff_fffe
    } else {
        0x7fff_fffe
    }
}

pub(crate) fn tagged_entry(address: u64, mode: u8) -> Result<u64, String> {
    Ok(match mode {
        64 => address | 1,
        31 => (address & mask(31)?) | 0x80000000,
        24 => address & mask(24)?,
        _ => return Err("invalid entry mode".into()),
    })
}
pub(crate) fn branch_mode(value: u64) -> u8 {
    if value & 1 != 0 {
        64
    } else if value & 0x80000000 != 0 {
        31
    } else {
        24
    }
}

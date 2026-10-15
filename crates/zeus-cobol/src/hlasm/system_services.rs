use super::execute::{address, word};
use super::{DynamicStorage, Host, MessageText, StorageOperand, SystemService};

#[derive(Debug, Clone)]
pub struct OperatorMessage {
    pub text: String,
    pub routes: Vec<u8>,
    pub descriptors: Vec<u8>,
}

fn scalar(value: &StorageOperand, registers: &[u32; 16]) -> Result<u32, String> {
    match value {
        StorageOperand::Constant(n) => Ok(*n),
        StorageOperand::Register(r) => registers
            .get(*r as usize)
            .copied()
            .ok_or_else(|| "invalid service register".into()),
        _ => Err("service value must be a constant or register".into()),
    }
}

fn pointer(value: &StorageOperand, registers: &[u32; 16]) -> Result<usize, String> {
    match value {
        StorageOperand::Address(a) => address(a, registers),
        StorageOperand::Register(r) => {
            Ok((scalar(&StorageOperand::Register(*r), registers)? & 0x7fff_ffff) as usize)
        }
        _ => Err("service pointer must be an address or register".into()),
    }
}

fn ecb(memory: &[u8], at: usize) -> Result<u32, String> {
    if !at.is_multiple_of(4) {
        return Err("ECB must be fullword aligned".into());
    }
    word(memory, at)
}

pub(crate) fn post(memory: &mut [u8], at: usize, code: u32) -> Result<(), String> {
    ecb(memory, at)?;
    if code > 0x3fff_ffff {
        return Err("POST completion code exceeds 30 bits".into());
    }
    memory[at..at + 4].copy_from_slice(&(0x4000_0000 | code).to_be_bytes());
    Ok(())
}

pub(crate) fn completed(events: &[usize], memory: &[u8]) -> Result<usize, String> {
    events.iter().try_fold(0, |count, at| {
        Ok(count + usize::from(ecb(memory, *at)? & 0x4000_0000 != 0))
    })
}

pub(super) fn execute(
    request: &SystemService,
    registers: &mut [u32; 16],
    memory: &mut Vec<u8>,
    storage: &mut DynamicStorage,
    host: &mut dyn Host,
) -> Result<(), String> {
    match request {
        SystemService::Write {
            text,
            routes,
            descriptors,
        } => {
            let bytes = match text {
                MessageText::Literal(text) => crate::ebcdic::encode(text),
                MessageText::Address(value) => {
                    let at = pointer(value, registers)?;
                    let end = at.checked_add(2).ok_or("WTO text address overflow")?;
                    let len = u16::from_be_bytes(
                        memory
                            .get(at..end)
                            .ok_or("WTO length outside run-unit storage")?
                            .try_into()
                            .unwrap(),
                    ) as usize;
                    memory
                        .get(end..end.checked_add(len).ok_or("WTO length overflow")?)
                        .ok_or("WTO text outside run-unit storage")?
                        .to_vec()
                }
            };
            if bytes.is_empty() || bytes.len() > 126 {
                return Err("WTO single-line text must contain 1..126 bytes".into());
            }
            registers[0] = host.message(OperatorMessage {
                text: crate::ebcdic::decode(&bytes),
                routes: routes.clone(),
                descriptors: descriptors.clone(),
            })?;
            registers[15] = 0;
        }
        SystemService::Post { ecb, code } => {
            let at = pointer(ecb, registers)?;
            storage.check_write(at, 4)?;
            post(memory, at, scalar(code, registers)?)?
        }
        SystemService::Wait { ecbs, list, events } => {
            let needed = scalar(events, registers)? as usize;
            if needed > 255 || (!list && needed != 1) {
                return Err("invalid WAIT event count".into());
            }
            let base = pointer(ecbs, registers)?;
            let mut addresses = vec![];
            if *list {
                if !base.is_multiple_of(4) {
                    return Err("ECBLIST must be fullword aligned".into());
                }
                loop {
                    if addresses.len() >= 4096 {
                        return Err("ECBLIST has no terminator within 4096 entries".into());
                    }
                    let value = word(
                        memory,
                        base.checked_add(addresses.len() * 4)
                            .ok_or("ECBLIST overflow")?,
                    )?;
                    addresses.push((value & 0x7fff_ffff) as usize);
                    if value & 0x8000_0000 != 0 {
                        break;
                    }
                }
            } else {
                addresses.push(base);
            }
            let unique: std::collections::HashSet<_> = addresses.iter().collect();
            if unique.len() != addresses.len() || needed > addresses.len() {
                return Err("invalid WAIT ECB list or count".into());
            }
            let done = completed(&addresses, memory)?;
            if done >= needed {
                return Ok(());
            }
            let waiting: Vec<_> = addresses
                .iter()
                .copied()
                .filter(|at| word(memory, *at).unwrap() & 0x4000_0000 == 0)
                .collect();
            for at in &waiting {
                storage.check_write(*at, 4)?;
            }
            for at in &waiting {
                let value = word(memory, *at)?;
                memory[*at..*at + 4].copy_from_slice(&(value | 0x8000_0000).to_be_bytes());
            }
            let result = storage
                .verify_protection(memory)
                .and_then(|_| host.wait(&addresses, needed, memory, storage));
            // Release registrations on completion and error; completion codes
            // survive so the application can inspect the posted ECBs.
            for at in waiting {
                let value = word(memory, at)?;
                memory[at..at + 4].copy_from_slice(&(value & 0x7fff_ffff).to_be_bytes());
            }
            result?;
            if completed(&addresses, memory)? < needed {
                return Err("WAIT host returned before enough events completed".into());
            }
        }
    }
    Ok(())
}

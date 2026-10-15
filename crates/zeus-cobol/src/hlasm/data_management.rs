//! Source-level QSAM descriptors; no IBM macro implementations are embedded.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RecordFormat {
    Fixed,
    Variable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AccessMode {
    Move,
    Locate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataControlBlock {
    pub offset: u32,
    pub ddname: String,
    pub record_format: RecordFormat,
    pub blocked: bool,
    pub record_length: u16,
    pub block_size: u16,
    pub get: Option<AccessMode>,
    pub put: Option<AccessMode>,
    pub end: Option<u32>,
    pub error: Option<u32>,
}

pub(super) const LENGTH: usize = 96;

pub(crate) enum Request {
    Open {
        dcb: usize,
        output: bool,
        extend: bool,
    },
    Close(usize),
    Get {
        dcb: usize,
        area: Option<usize>,
    },
    Put {
        dcb: usize,
        area: Option<usize>,
    },
    FreePool(usize),
}

#[derive(Default)]
pub(crate) struct Reply {
    pub address: Option<usize>,
    pub rc: Option<i32>,
    pub branch: Option<usize>,
    pub link: bool,
}

pub(super) fn execute(
    service: &super::DataService,
    registers: &[u32; 16],
    memory: &mut Vec<u8>,
    storage: &mut super::DynamicStorage,
    host: &mut dyn super::Host,
) -> Result<Reply, String> {
    use super::DataService;
    let pointer = |p: &super::StorageOperand| super::services::pointer(p, registers);
    match service {
        DataService::Open(entries) => {
            let mut rc = 0;
            for (dcb, output, extend) in entries {
                let reply = host.data(
                    Request::Open {
                        dcb: pointer(dcb)?,
                        output: *output,
                        extend: *extend,
                    },
                    memory,
                    storage,
                )?;
                rc = rc.max(reply.rc.unwrap_or(0));
            }
            Ok(Reply {
                rc: Some(rc),
                ..Default::default()
            })
        }
        DataService::Close(entries) => {
            for dcb in entries {
                host.data(Request::Close(pointer(dcb)?), memory, storage)?;
            }
            Ok(Reply {
                rc: Some(0),
                ..Default::default()
            })
        }
        DataService::Get { dcb, area } => host.data(
            Request::Get {
                dcb: pointer(dcb)?,
                area: area.as_ref().map(pointer).transpose()?,
            },
            memory,
            storage,
        ),
        DataService::Put { dcb, area } => host.data(
            Request::Put {
                dcb: pointer(dcb)?,
                area: area.as_ref().map(pointer).transpose()?,
            },
            memory,
            storage,
        ),
        DataService::FreePool(dcb) => host.data(Request::FreePool(pointer(dcb)?), memory, storage),
    }
}

pub(super) fn validate(module: &super::Module) -> Result<(), String> {
    if !module.dcbs.is_empty() && module.zeus_hlasm < 8 {
        return Err("DCB metadata requires HLASM module version 8".into());
    }
    let mut offsets = std::collections::HashSet::new();
    for dcb in &module.dcbs {
        if !offsets.insert(dcb.offset)
            || !dcb.offset.is_multiple_of(4)
            || dcb.offset as usize + LENGTH > module.storage.len()
            || !super::assemble::symbol_name(&dcb.ddname)
            || dcb.ddname.len() > 8
            || (dcb.get.is_none() && dcb.put.is_none())
            || dcb.record_length > 32760
            || dcb.block_size > 32760
            || (dcb.record_format == RecordFormat::Variable
                && dcb.record_length != 0
                && dcb.record_length < 4)
            || module.code.iter().any(|ins| {
                ins.offset < dcb.offset + LENGTH as u32
                    && dcb.offset
                        < ins.offset + super::assemble::instruction_size(&ins.op).unwrap_or(0)
            })
            || [dcb.end, dcb.error]
                .into_iter()
                .flatten()
                .any(|entry| !module.code.iter().any(|i| i.offset == entry))
        {
            return Err("invalid DCB metadata".into());
        }
    }
    Ok(())
}

pub(super) fn initialize(
    module: &super::Module,
    image: &mut [u8],
    base: usize,
) -> Result<(), String> {
    validate(module)?;
    for dcb in &module.dcbs {
        if base
            .checked_add(dcb.offset as usize)
            .and_then(|v| v.checked_add(LENGTH))
            .is_none_or(|v| v > 0x1000000)
        {
            return Err("DCB must reside below 16 MiB".into());
        }
        let data = &mut image[dcb.offset as usize..dcb.offset as usize + LENGTH];
        data[26] = 0x40; // DSORG=PS
        data[40..48].fill(0x40);
        let name = crate::ebcdic::encode(&dcb.ddname);
        data[40..40 + name.len()].copy_from_slice(&name);
        data[36] = (if dcb.record_format == RecordFormat::Fixed {
            0x80
        } else {
            0x40
        }) | if dcb.blocked { 0x10 } else { 0 };
        data[50] = dcb.get.map_or(0, |mode| {
            0x20 | if mode == AccessMode::Move { 0x10 } else { 8 }
        });
        data[51] = dcb.put.map_or(0, |mode| {
            0x20 | if mode == AccessMode::Move { 0x10 } else { 8 }
        });
        data[62..64].copy_from_slice(&dcb.block_size.to_be_bytes());
        data[82..84].copy_from_slice(&dcb.record_length.to_be_bytes());
        for (field, exit) in [(33, dcb.end), (57, dcb.error)] {
            if let Some(exit) = exit {
                let address = u32::try_from(base)
                    .ok()
                    .and_then(|v| v.checked_add(exit))
                    .filter(|v| *v <= 0xffffff)
                    .ok_or("DCB exit must reside below 16 MiB")?;
                data[field..field + 3].copy_from_slice(&address.to_be_bytes()[1..]);
            }
        }
    }
    Ok(())
}

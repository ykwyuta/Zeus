//! QSAM record operations over the existing DD and dataset formats.
use super::Machine;
use crate::{
    ebcdic,
    hlasm::{DataReply, DataRequest, DcbAccessMode as Access, DcbRecordFormat as Format},
};
use std::{collections::HashMap, fs::File, io::Write};
use zeus_ds::{format::RecordFormat, seq::SequentialDataset};

#[derive(Default)]
pub(super) struct Files {
    open: HashMap<usize, OpenFile>,
}

struct OpenFile {
    format: RecordFormat,
    logical: Format,
    lrecl: usize,
    access: Access,
    writer: Option<File>,
    records: Vec<Vec<u8>>,
    next: usize,
    end: Option<usize>,
    error: Option<usize>,
    buffer: Option<usize>,
    pending: bool,
}

fn bytes(memory: &[u8], at: usize, length: usize) -> Result<&[u8], String> {
    memory
        .get(at..at.checked_add(length).ok_or("I/O address overflow")?)
        .ok_or_else(|| "I/O area outside run-unit storage".into())
}

impl OpenFile {
    fn record(&self, memory: &[u8], area: usize) -> Result<Vec<u8>, String> {
        if self.logical == Format::Variable {
            let rdw = bytes(memory, area, 4)?;
            let length = u16::from_be_bytes(rdw[..2].try_into().unwrap()) as usize;
            if length < 4 || length > self.lrecl || rdw[2..] != [0, 0] {
                return Err("invalid QSAM variable record RDW".into());
            }
            Ok(bytes(memory, area + 4, length - 4)?.to_vec())
        } else {
            Ok(bytes(memory, area, self.lrecl)?.to_vec())
        }
    }
    fn write(&mut self, record: &[u8]) -> Result<(), String> {
        let writer = self
            .writer
            .as_mut()
            .ok_or("PUT requires OUTPUT or EXTEND")?;
        let mut encoded = vec![];
        if self.format == RecordFormat::Text {
            encoded.extend(ebcdic::to_host(record));
            encoded.push(b'\n');
        } else {
            zeus_ds::format::encode_into(self.format, record, 0x40, &mut encoded);
        }
        writer.write_all(&encoded).map_err(|e| e.to_string())
    }
    fn flush_pending(&mut self, memory: &[u8]) -> Result<(), String> {
        if self.pending {
            let record = self.record(memory, self.buffer.ok_or("missing locate buffer")?)?;
            self.write(&record)?;
            self.pending = false;
        }
        Ok(())
    }
}

impl Machine {
    fn open_assembler_file(
        &mut self,
        dcb: usize,
        output: bool,
        extend: bool,
    ) -> Result<DataReply, String> {
        if !dcb.is_multiple_of(4) || dcb >= 1 << 24 {
            return Err("QSAM DCB must be fullword aligned below 16 MiB".into());
        }
        self.hlasm_storage.check_write(dcb, 96)?;
        let data = bytes(&self.mem, dcb, 96)?;
        if self.hlasm_io.open.contains_key(&dcb) {
            return Err("QSAM DCB is already open".into());
        }
        let ddname = ebcdic::decode(&data[40..48]).trim_end().to_string();
        let Some(allocation) = self.dd.allocation(&[&ddname]) else {
            return Ok(DataReply {
                rc: Some(8),
                ..Default::default()
            });
        };
        let format = match data[36] & 0xc0 {
            0x80 => Format::Fixed,
            0x40 => Format::Variable,
            _ => return Err("unsupported QSAM DCB record format".into()),
        };
        let default_lrecl = usize::from(u16::from_be_bytes(data[82..84].try_into().unwrap()));
        let logical = format;
        let format = allocation.format.unwrap_or(if format == Format::Fixed {
            RecordFormat::Fixed(default_lrecl as u32)
        } else {
            RecordFormat::Variable
        });
        let logical = match format {
            RecordFormat::Fixed(_) => Format::Fixed,
            RecordFormat::Variable => Format::Variable,
            RecordFormat::Text => logical,
        };
        let lrecl = match format {
            RecordFormat::Fixed(n) => n as usize,
            _ => default_lrecl,
        };
        if lrecl == 0 || lrecl > 32760 || (logical == Format::Variable && lrecl < 4) {
            return Err("OPEN requires a valid DCB or DD record length".into());
        }
        let access = data[if output { 51 } else { 50 }];
        if access & 0x20 == 0 {
            return Err("DCB MACRF does not permit requested OPEN mode".into());
        }
        let access = match access & 0x18 {
            0x10 => Access::Move,
            8 => Access::Locate,
            _ => return Err("invalid QSAM DCB access mode".into()),
        };
        let exit = |at| {
            let value = u32::from_be_bytes([0, data[at], data[at + 1], data[at + 2]]) as usize;
            (value != 0).then_some(value)
        };
        let (end, error) = (exit(33), exit(57));
        let (writer, records) = if output {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(!extend)
                .append(extend)
                .open(&allocation.path)
                .map_err(|e| format!("OPEN {ddname}: {e}"))?;
            (Some(file), vec![])
        } else {
            let dataset = SequentialDataset::load(&allocation.path, format, 0x40)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("OPEN {ddname}: input dataset not found"))?;
            let records = if format == RecordFormat::Text {
                dataset
                    .records
                    .iter()
                    .map(|record| ebcdic::from_host(record))
                    .collect()
            } else {
                dataset.records
            };
            if format != RecordFormat::Text
                && logical == Format::Fixed
                && records.iter().any(|record| record.len() != lrecl)
            {
                return Err("incomplete fixed-length input record".into());
            }
            (None, records)
        };
        self.hlasm_io.open.insert(
            dcb,
            OpenFile {
                format,
                logical,
                lrecl,
                access,
                writer,
                records,
                next: 0,
                end,
                error,
                buffer: None,
                pending: false,
            },
        );
        self.hlasm_storage.pin(dcb, 96);
        self.mem[dcb + 48] |= 0x10;
        self.mem[dcb + 82..dcb + 84].copy_from_slice(&(lrecl as u16).to_be_bytes());
        Ok(DataReply {
            rc: Some(0),
            ..Default::default()
        })
    }

    fn close_assembler_file(&mut self, dcb: usize) -> Result<DataReply, String> {
        let Some(mut file) = self.hlasm_io.open.remove(&dcb) else {
            return Ok(DataReply::default());
        };
        // Restore the handle if flushing fails, so a later explicit close or
        // step teardown can still release its resources.
        let result = file.flush_pending(&self.mem).and_then(|_| {
            if let Some(writer) = &mut file.writer {
                writer.flush().map_err(|e| e.to_string())?;
            }
            self.hlasm_storage.check_write(dcb + 48, 1)?;
            *self
                .mem
                .get_mut(dcb + 48)
                .ok_or("closed DCB outside storage")? &= !0x10;
            Ok(())
        });
        if let Err(error) = result {
            self.hlasm_io.open.insert(dcb, file);
            return Err(error);
        }
        if let Some(buffer) = file.buffer {
            self.hlasm_storage.release_frame(&mut self.mem, buffer);
        }
        self.hlasm_storage.unpin(dcb);
        Ok(DataReply::default())
    }

    fn get_assembler_record(
        &mut self,
        dcb: usize,
        area: Option<usize>,
    ) -> Result<DataReply, String> {
        let file = self
            .hlasm_io
            .open
            .get_mut(&dcb)
            .ok_or("GET requires an open DCB")?;
        if file.writer.is_some() {
            return Err("GET requires INPUT mode".into());
        }
        if (file.access == Access::Move) != area.is_some() {
            return Err("GET area must match DCB move/locate mode".into());
        }
        let Some(record) = file.records.get(file.next) else {
            return Ok(DataReply {
                branch: Some(file.end.ok_or("end of data without an EODAD exit")?),
                ..Default::default()
            });
        };
        let mut value = vec![];
        if file.logical == Format::Variable {
            if record.len() + 4 > file.lrecl {
                return Err("input record exceeds DCB LRECL".into());
            }
            value.extend(((record.len() + 4) as u16).to_be_bytes());
            value.extend([0, 0]);
            value.extend(record);
        } else {
            if record.len() > file.lrecl {
                return Err("input record exceeds DCB LRECL".into());
            }
            value.extend(record);
            value.resize(file.lrecl, 0x40);
        }
        let target = if let Some(area) = area {
            area
        } else if let Some(buffer) = file.buffer {
            buffer
        } else {
            let buffer = self.hlasm_storage.frame(&mut self.mem, file.lrecl)?;
            file.buffer = Some(buffer);
            buffer
        };
        bytes(&self.mem, target, value.len())?;
        self.hlasm_storage.check_write(target, value.len())?;
        self.hlasm_storage.check_write(dcb + 76, 8)?;
        self.mem[target..target + value.len()].copy_from_slice(&value);
        self.mem[dcb + 76..dcb + 80].copy_from_slice(&(target as u32).to_be_bytes());
        if file.logical == Format::Variable {
            self.mem[dcb + 82..dcb + 84].copy_from_slice(&(value.len() as u16).to_be_bytes());
        }
        file.next += 1;
        Ok(DataReply {
            address: Some(target),
            ..Default::default()
        })
    }

    fn put_assembler_record(
        &mut self,
        dcb: usize,
        area: Option<usize>,
    ) -> Result<DataReply, String> {
        let file = self
            .hlasm_io
            .open
            .get_mut(&dcb)
            .ok_or("PUT requires an open DCB")?;
        if file.writer.is_none() {
            return Err("PUT requires OUTPUT or EXTEND mode".into());
        }
        if (file.access == Access::Move) != area.is_some() {
            return Err("PUT area must match DCB move/locate mode".into());
        }
        if let Some(area) = area {
            let record = file.record(&self.mem, area)?;
            file.write(&record)?;
            return Ok(DataReply::default());
        }
        file.flush_pending(&self.mem)?;
        let buffer = if let Some(buffer) = file.buffer {
            buffer
        } else {
            let buffer = self.hlasm_storage.frame(&mut self.mem, file.lrecl)?;
            self.mem[buffer..buffer + file.lrecl].fill(0x40);
            if file.logical == Format::Variable {
                self.mem[buffer..buffer + 4].copy_from_slice(&[0, 4, 0, 0]);
            }
            file.buffer = Some(buffer);
            buffer
        };
        self.hlasm_storage.check_write(dcb + 76, 4)?;
        self.mem[dcb + 76..dcb + 80].copy_from_slice(&(buffer as u32).to_be_bytes());
        file.pending = true;
        Ok(DataReply {
            address: Some(buffer),
            ..Default::default()
        })
    }

    pub(super) fn assembler_data(&mut self, request: DataRequest) -> Result<DataReply, String> {
        let dcb = match &request {
            DataRequest::Open { dcb, .. }
            | DataRequest::Get { dcb, .. }
            | DataRequest::Put { dcb, .. } => *dcb,
            DataRequest::Close(dcb) | DataRequest::FreePool(dcb) => *dcb,
        };
        let result = match request {
            DataRequest::Open {
                dcb,
                output,
                extend,
            } => self.open_assembler_file(dcb, output, extend),
            DataRequest::Close(dcb) => self.close_assembler_file(dcb),
            DataRequest::Get { dcb, area } => self.get_assembler_record(dcb, area),
            DataRequest::Put { dcb, area } => self.put_assembler_record(dcb, area),
            DataRequest::FreePool(dcb) => {
                if self.hlasm_io.open.contains_key(&dcb) {
                    Err("FREEPOOL requires a closed DCB".into())
                } else {
                    Ok(DataReply::default())
                }
            }
        };
        match result {
            Err(error) => {
                if let Some(target) = self.hlasm_io.open.get(&dcb).and_then(|file| file.error) {
                    Ok(DataReply {
                        rc: Some(8),
                        branch: Some(target),
                        link: true,
                        ..Default::default()
                    })
                } else {
                    Err(error)
                }
            }
            result => result,
        }
    }

    /// Close assembler datasets at the end of a run unit, flushing the last
    /// locate-mode PUT buffer and returning output errors to the caller.
    pub fn finish_assembler_io(&mut self) -> Result<(), String> {
        let addresses: Vec<_> = self.hlasm_io.open.keys().copied().collect();
        let mut failure = None;
        for address in addresses {
            if let Err(error) = self.close_assembler_file(address) {
                failure.get_or_insert(error);
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(super) fn close_assembler_range(&mut self, start: usize, end: usize) -> Result<(), String> {
        let addresses: Vec<_> = self
            .hlasm_io
            .open
            .keys()
            .copied()
            .filter(|at| (start..end).contains(at))
            .collect();
        for address in addresses {
            self.close_assembler_file(address)?;
        }
        Ok(())
    }
}

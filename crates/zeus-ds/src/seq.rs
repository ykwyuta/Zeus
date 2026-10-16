//! Sequential data sets.

use std::path::Path;

use crate::Result;
use crate::format::{self, RecordFormat};

/// Open-file I/O staging buffers. Decoded logical records remain in the
/// existing in-memory dataset model independently of this pool.
#[derive(Debug, Default)]
pub struct BufferPool {
    pub buffers: Vec<Vec<u8>>,
}

impl BufferPool {
    pub fn new(count: u32, size: usize) -> Result<Self> {
        if count == 0 || size == 0 {
            return Err(crate::DsError::Io("zero I/O buffer size/count".into()));
        }
        let mut buffers = Vec::new();
        buffers
            .try_reserve_exact(count as usize)
            .map_err(|e| crate::DsError::Io(e.to_string()))?;
        for _ in 0..count {
            let mut buffer = Vec::new();
            buffer
                .try_reserve_exact(size)
                .map_err(|e| crate::DsError::Io(e.to_string()))?;
            buffer.resize(size, 0);
            buffers.push(buffer);
        }
        Ok(Self { buffers })
    }

    pub fn read(&mut self, path: &Path) -> Result<Option<Vec<u8>>> {
        use std::io::Read;
        if self.buffers.is_empty() {
            return Err(crate::DsError::Io("I/O buffer pool is not open".into()));
        }
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(crate::DsError::Io(e.to_string())),
        };
        let mut out = Vec::new();
        loop {
            for buffer in &mut self.buffers {
                let n = file
                    .read(buffer)
                    .map_err(|e| crate::DsError::Io(e.to_string()))?;
                if n == 0 {
                    return Ok(Some(out));
                }
                out.extend_from_slice(&buffer[..n]);
            }
        }
    }

    pub fn write(&mut self, path: &Path, bytes: &[u8]) -> Result<()> {
        use std::io::Write;
        if self.buffers.is_empty() {
            return Err(crate::DsError::Io("I/O buffer pool is not open".into()));
        }
        let mut file =
            std::fs::File::create(path).map_err(|e| crate::DsError::Io(e.to_string()))?;
        let mut offset = 0;
        while offset < bytes.len() {
            for buffer in &mut self.buffers {
                let n = buffer.len().min(bytes.len() - offset);
                buffer[..n].copy_from_slice(&bytes[offset..offset + n]);
                file.write_all(&buffer[..n])
                    .map_err(|e| crate::DsError::Io(e.to_string()))?;
                offset += n;
                if offset == bytes.len() {
                    break;
                }
            }
        }
        file.flush().map_err(|e| crate::DsError::Io(e.to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct SequentialDataset {
    pub format: RecordFormat,
    /// Pad byte of fixed-length records.
    pub pad: u8,
    pub records: Vec<Vec<u8>>,
}

impl SequentialDataset {
    pub fn new(format: RecordFormat, pad: u8) -> SequentialDataset {
        SequentialDataset {
            format,
            pad,
            records: Vec::new(),
        }
    }

    /// Loads a data set; None when the file does not exist.
    pub fn load(path: &Path, format: RecordFormat, pad: u8) -> Result<Option<SequentialDataset>> {
        let Some(data) = format::read_file(path)? else {
            return Ok(None);
        };
        Ok(Some(SequentialDataset {
            format,
            pad,
            records: format::decode(format, &data)?,
        }))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for r in &self.records {
            format::encode_into(self.format, r, self.pad, &mut out);
        }
        out
    }

    pub fn store(&self, path: &Path) -> Result<()> {
        format::write_file(path, &self.to_bytes())
    }
}

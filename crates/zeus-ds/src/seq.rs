//! Sequential data sets.

use std::path::Path;

use crate::Result;
use crate::format::{self, RecordFormat};

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

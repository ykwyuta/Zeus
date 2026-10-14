use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Actual assembly inputs supplied by a CLI, JCL step or embedding application.
#[derive(Debug, Clone, Default)]
pub struct AssemblyDataSet {
    pub name: String,
    pub member: String,
    pub volume: String,
}

#[derive(Debug, Clone)]
pub struct AssemblyOptions {
    pub source_encoding: super::SourceEncoding,
    pub source_format: super::SourceFormat,
    pub sysparm: String,
    pub job: Option<String>,
    pub step: Option<String>,
    pub data_sets: BTreeMap<String, AssemblyDataSet>,
    pub library_data_sets: BTreeMap<PathBuf, AssemblyDataSet>,
    pub started: SystemTime,
}

impl Default for AssemblyOptions {
    fn default() -> Self {
        Self {
            source_encoding: super::SourceEncoding::default(),
            source_format: super::SourceFormat::default(),
            sysparm: String::new(),
            job: None,
            step: None,
            data_sets: BTreeMap::new(),
            library_data_sets: BTreeMap::new(),
            started: SystemTime::now(),
        }
    }
}

pub(super) fn clock(time: SystemTime) -> (String, String, String, String) {
    let time = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = time.as_secs();
    let (year, month, day) = crate::vm::civil_from_days((seconds / 86400) as i64);
    let (hour, minute, second) = (seconds / 3600 % 24, seconds / 60 % 60, seconds % 60);
    (
        format!("{year:04}{month:02}{day:02}"),
        format!("{month:02}/{day:02}/{:02}", year % 100),
        format!("{hour:02}.{minute:02}"),
        format!(
            "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{:06}",
            time.subsec_micros()
        ),
    )
}

pub(super) fn numeric_symbol(name: &str) -> bool {
    matches!(
        name,
        "&SYSDATC"
            | "&SYSNEST"
            | "&SYSNDX"
            | "&SYSSTMT"
            | "&SYSM_HSEV"
            | "&SYSM_SEV"
            | "&SYSOPT_DBCS"
            | "&SYSOPT_RENT"
            | "&SYSOPT_XOBJECT"
            | "&SYSOPT_CODEPAGE"
            | "&SYSCODEPAGE"
    )
}

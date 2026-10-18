//! Export sidefiles for Zeus program objects. This is a portable virtual
//! linkage format; it does not contain IBM object records or host addresses.
use crate::zir::Module;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Sidefile {
    pub format: String,
    pub module: String,
    pub exports: Vec<String>,
}
pub fn exports(module: &Module) -> Vec<String> {
    module
        .programs
        .iter()
        .filter(|p| p.parent.is_none() && p.exportall)
        .flat_map(|p| {
            std::iter::once(p.name.clone()).chain(p.entries.iter().map(|e| e.name.clone()))
        })
        .collect()
}
pub fn sidefile_bytes(module: &Module, path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let exports = exports(module);
    if exports.is_empty() {
        return Ok(None);
    }
    let sidefile = Sidefile {
        format: "ZEUS-DLL-1".into(),
        module: path
            .file_name()
            .ok_or_else(|| std::io::Error::other("DLL needs a module file name"))?
            .to_string_lossy()
            .into_owned(),
        exports,
    };
    serde_json::to_vec_pretty(&sidefile)
        .map(Some)
        .map_err(std::io::Error::other)
}
pub fn save_sidefile(module: &Module, path: &Path) -> std::io::Result<()> {
    if let Some(bytes) = sidefile_bytes(module, path)? {
        // Appending also supports extensionless load-library members.
        let mut side = path.as_os_str().to_os_string();
        side.push(".x");
        std::fs::write(side, bytes)?;
    }
    Ok(())
}

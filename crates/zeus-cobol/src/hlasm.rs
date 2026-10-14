//! Source-level HLASM subset for application subroutines. Modules contain
//! decoded instructions and Zeus relocations, not IBM object decks. Execution uses the caller's
//! storage and the 31-bit OS linkage convention, without unsafe host pointers.
mod assemble;
mod binding;
mod bit_constants;
mod classes;
mod constants;
mod context;
mod data_management;
mod diagnostics;
mod execute;
mod expression;
mod link;
mod macros;
mod operation;
mod services;
mod source;
mod source_refs;
mod storage;
mod system_services;
mod template;

use serde::{Deserialize, Serialize};
use std::path::Path;

pub use assemble::compile_file;
pub use binding::{
    Bindings, Label as BindingLabel, Psect, Quantity, QuantityKind, Section as BindingSection,
};
pub use context::{AssemblyDataSet, AssemblyOptions};
pub use data_management::{
    AccessMode as DcbAccessMode, DataControlBlock, RecordFormat as DcbRecordFormat,
};
pub(crate) use data_management::{Reply as DataReply, Request as DataRequest};
pub use diagnostics::{AssemblyReport, Diagnostic, compile_report, compile_report_with_options};
pub(crate) use execute::execute_entry_with_host;
pub use execute::{execute, execute_with_storage};
pub use link::{link_modules, link_with_libraries};
pub(crate) use services::{Host, Registers};
pub use source::{SourceEncoding, SourceFormat};
pub use source_refs::{ConstantReference, constant_references};
pub use storage::DynamicStorage;
pub use system_services::OperatorMessage;
pub(crate) use system_services::{completed as completed_events, post as post_event};

pub fn is_operation(op: &str) -> bool {
    assemble::directive(op)
        || assemble::instruction_size(op).is_some()
        || matches!(
            op,
            "COPY"
                | "MACRO"
                | "MEND"
                | "LCLA"
                | "LCLB"
                | "LCLC"
                | "GBLA"
                | "GBLB"
                | "GBLC"
                | "SETA"
                | "SETB"
                | "SETC"
                | "AIF"
                | "AGO"
                | "ANOP"
                | "MEXIT"
                | "MNOTE"
                | "ACTR"
                | "OPSYN"
        )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub zeus_hlasm: u32,
    pub name: String,
    #[serde(default = "default_alignment")]
    pub alignment: u32,
    #[serde(default)]
    pub instruction_templates: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_only: Vec<std::ops::Range<u32>>,
    #[serde(default = "default_rmode")]
    pub rmode: u8,
    pub storage: Vec<u8>,
    pub entry: u32,
    pub code: Vec<Instruction>,
    #[serde(default)]
    pub relocations: Vec<Relocation>,
    #[serde(default)]
    pub entries: std::collections::HashMap<String, u32>,
    /// Symbol name -> weak reference. Kept separately so EXTRN without an
    /// address constant still requests library inclusion during linking.
    #[serde(default)]
    pub externals: std::collections::HashMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dcbs: Vec<DataControlBlock>,
    #[serde(default, skip_serializing_if = "Bindings::is_empty")]
    pub bindings: Bindings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relocation {
    pub offset: u32,
    #[serde(
        default = "default_address_width",
        skip_serializing_if = "is_default_address_width"
    )]
    pub width: u8,
    pub target: u32,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub addend: i64,
    #[serde(default)]
    pub weak: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<RelocationExpression>,
}

impl Module {
    pub fn required_alignment(&self) -> Result<usize, String> {
        if !matches!(self.rmode, 24 | 31) {
            return Err("invalid HLASM RMODE".into());
        }
        if self.rmode == 24 && self.zeus_hlasm < 6 {
            return Err("RMODE 24 requires HLASM module version 6".into());
        }
        if !(8..=4096).contains(&self.alignment) || !self.alignment.is_power_of_two() {
            return Err("invalid HLASM module alignment".into());
        }
        Ok(self.alignment as usize)
    }
    pub(crate) fn validate_read_only(&self) -> Result<(), String> {
        if !self.read_only.is_empty() && self.zeus_hlasm < 6 {
            return Err("RSECT protection requires HLASM module version 6".into());
        }
        let mut previous_end = 0;
        for area in &self.read_only {
            if area.start < previous_end
                || area.start >= area.end
                || area.end as usize > self.storage.len()
            {
                return Err("invalid RSECT storage range".into());
            }
            previous_end = area.end;
        }
        Ok(())
    }
    pub(crate) fn validate_relative_references(&self, base: usize) -> Result<(), String> {
        for ins in &self.code {
            if let Some(expression) = &ins.relative_expression {
                let spec = operation::relative(&ins.op)
                    .ok_or("relative relocation on a non-relative instruction")?;
                if ins.relative_external.is_some() {
                    return Err("conflicting relative relocations".into());
                }
                let address = ins
                    .address
                    .as_ref()
                    .ok_or("relative relocation has no operand")?;
                if address.base != 0 || address.index != 0 {
                    return Err("relative relocation cannot use base/index registers".into());
                }
                let target = expression.target_at(base)?;
                let location = i64::try_from(base)
                    .ok()
                    .and_then(|b| b.checked_add(i64::from(ins.offset)))
                    .ok_or("relative relocation address overflow")?;
                operation::check_relative(
                    target
                        .checked_sub(location)
                        .ok_or("relative relocation overflow")?,
                    spec.bits,
                )
                .map_err(|e| {
                    format!(
                        "{}:{}: {}: relative expression: {e}",
                        ins.file, ins.line, ins.op
                    )
                })?;
            }
            if let Some(reference) = &ins.relative_external {
                let spec = operation::relative(&ins.op)
                    .ok_or("relative relocation on a non-relative instruction")?;
                if !reference.weak {
                    return Err(format!(
                        "unresolved external symbol {}; link the module before execution",
                        reference.symbol
                    ));
                }
                // An absent weak symbol has absolute address zero, independent
                // of the image load origin, just like an unresolved weak adcon.
                let location = i64::try_from(base)
                    .ok()
                    .and_then(|base| base.checked_add(i64::from(ins.offset)))
                    .ok_or("relative relocation address overflow")?;
                operation::check_relative(
                    location
                        .checked_neg()
                        .ok_or("relative relocation overflow")?,
                    spec.bits,
                )
                .map_err(|e| {
                    format!(
                        "{}:{}: {}: weak reference {}: {e}",
                        ins.file, ins.line, ins.op, reference.symbol
                    )
                })?;
            }
        }
        Ok(())
    }

    /// Instantiate address constants at the actual load origin. Do this once
    /// per load (and after CANCEL), rather than overwriting writable constants
    /// on every invocation.
    pub(crate) fn validate_origin(&self, base: usize) -> Result<(), String> {
        if self.rmode == 24
            && base
                .checked_add(self.storage.len())
                .is_none_or(|end| end > 1 << 24)
        {
            return Err("RMODE 24 module must reside below 16 MiB".into());
        }
        if !base.is_multiple_of(self.required_alignment()?) {
            return Err("HLASM load origin violates module alignment".into());
        }
        Ok(())
    }

    pub fn storage_at(&self, base: usize) -> Result<Vec<u8>, String> {
        self.validate_origin(base)?;
        self.validate_relative_references(base)?;
        let mut storage = self.storage.clone();
        if !self.instruction_templates {
            template::initialize(&self.code, &mut storage)?;
        }
        for r in &self.relocations {
            if r.width != 4 && self.zeus_hlasm < 7 {
                return Err("variable-width relocation requires module version 7".into());
            }
            let value = if let Some(expression) = &r.expression {
                if r.symbol.is_some() || r.target != 0 || r.addend != 0 || r.weak {
                    return Err("conflicting address relocations".into());
                }
                expression.value_at(base)? as u64
            } else if let Some(symbol) = &r.symbol {
                if !r.weak {
                    return Err(format!(
                        "unresolved external symbol {symbol}; link the module before execution"
                    ));
                }
                r.addend as u64
            } else {
                let value = i64::try_from(base)
                    .ok()
                    .and_then(|b| b.checked_add(i64::from(r.target)))
                    .and_then(|n| n.checked_add(r.addend))
                    .ok_or("address constant arithmetic overflow")?;
                value as u64
            };
            let at = r.offset as usize;
            if !(1..=8).contains(&r.width) {
                return Err("invalid address relocation width".into());
            }
            let end = at
                .checked_add(r.width as usize)
                .ok_or("invalid address relocation")?;
            storage
                .get_mut(at..end)
                .ok_or("invalid address relocation")?
                .copy_from_slice(&value.to_be_bytes()[8 - r.width as usize..]);
        }
        data_management::initialize(self, &mut storage, base)?;
        self.bindings.initialize(self, &mut storage, base)?;
        Ok(storage)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub displacement: i64,
    pub base: u8,
    pub index: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageOperand {
    Constant(u32),
    Register(u8),
    Memory(Address),
    Address(Address),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageService {
    pub obtain: bool,
    pub conditional: bool,
    pub length: Option<StorageOperand>,
    pub address: StorageOperand,
    pub subpool: StorageOperand,
    pub below_16m: bool,
    pub page: bool,
    #[serde(default)]
    pub packed_length: bool,
    pub return_code: Option<StorageOperand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instruction {
    pub offset: u32,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_service: Option<StorageService>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_service: Option<ModuleService>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_service: Option<SystemService>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_service: Option<DataService>,
    pub registers: Vec<u8>,
    pub address: Option<Address>,
    pub source: Option<Address>,
    pub length: u32,
    #[serde(default = "default_length")]
    pub source_length: u32,
    pub immediate: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_external: Option<RelativeExternal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_expression: Option<RelativeExpression>,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleName {
    Literal(String),
    Address(StorageOperand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleService {
    pub name: ModuleName,
    pub parameters: Option<Vec<StorageOperand>>,
    pub variable_list: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageText {
    Literal(String),
    Address(StorageOperand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemService {
    Write {
        text: MessageText,
        routes: Vec<u8>,
        descriptors: Vec<u8>,
    },
    Post {
        ecb: StorageOperand,
        code: StorageOperand,
    },
    Wait {
        ecbs: StorageOperand,
        list: bool,
        events: StorageOperand,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataService {
    Open(Vec<(StorageOperand, bool, bool)>),
    Close(Vec<StorageOperand>),
    Get {
        dcb: StorageOperand,
        area: Option<StorageOperand>,
    },
    Put {
        dcb: StorageOperand,
        area: Option<StorageOperand>,
    },
    FreePool(StorageOperand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativeExternal {
    pub symbol: String,
    pub addend: i64,
    #[serde(default)]
    pub weak: bool,
}

/// Linear target expression. Local addresses are relative to the image origin;
/// unresolved weak terms contribute zero without discarding the other terms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationExpression {
    pub addend: i64,
    pub local_coefficient: i64,
    pub terms: Vec<RelativeTerm>,
}

pub type RelativeExpression = RelocationExpression;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativeTerm {
    pub symbol: String,
    pub coefficient: i64,
    pub weak: bool,
}

impl RelocationExpression {
    fn value_at(&self, base: usize) -> Result<i64, String> {
        for term in &self.terms {
            if !term.weak {
                return Err(format!(
                    "unresolved external symbol {}; link the module before execution",
                    term.symbol
                ));
            }
        }
        i64::try_from(base)
            .ok()
            .and_then(|b| b.checked_mul(self.local_coefficient))
            .and_then(|n| n.checked_add(self.addend))
            .ok_or_else(|| "relocation expression overflow".into())
    }

    fn rebase(&mut self, origin: usize) -> Result<(), String> {
        self.addend = i64::try_from(origin)
            .ok()
            .and_then(|origin| origin.checked_mul(self.local_coefficient))
            .and_then(|n| n.checked_add(self.addend))
            .ok_or("relocation expression overflow")?;
        Ok(())
    }

    fn resolve(&mut self, entries: &std::collections::HashMap<String, u32>) -> Result<(), String> {
        let mut pending = Vec::new();
        for term in &self.terms {
            if let Some(&target) = entries.get(&term.symbol) {
                self.addend = i64::from(target)
                    .checked_mul(term.coefficient)
                    .and_then(|n| n.checked_add(self.addend))
                    .ok_or("relocation expression overflow")?;
                self.local_coefficient = self
                    .local_coefficient
                    .checked_add(term.coefficient)
                    .ok_or("relocation expression overflow")?;
            } else if term.weak {
                pending.push(term.clone());
            } else {
                return Err(format!("unresolved external symbol {}", term.symbol));
            }
        }
        self.terms = pending;
        Ok(())
    }

    fn target_at(&self, base: usize) -> Result<i64, String> {
        let mut coefficient = self.local_coefficient;
        for term in &self.terms {
            if !term.weak {
                return Err(format!(
                    "unresolved external symbol {}; link the module before execution",
                    term.symbol
                ));
            }
            coefficient = coefficient
                .checked_add(term.coefficient)
                .ok_or("relative relocation overflow")?;
        }
        if coefficient != 1 {
            return Err("relative expression must have net relocation coefficient one".into());
        }
        self.value_at(base)
    }
}

fn format_version(code: &[Instruction], relocations: &[Relocation]) -> u32 {
    if relocations.iter().any(|r| r.width != 4) {
        7
    } else if relocations.iter().any(|r| r.expression.is_some()) {
        4
    } else if code.iter().any(|i| i.relative_expression.is_some()) {
        3
    } else if code.iter().any(|i| i.relative_external.is_some()) {
        2
    } else {
        1
    }
}

fn default_address_width() -> u8 {
    4
}
fn is_default_address_width(width: &u8) -> bool {
    *width == 4
}

fn default_length() -> u32 {
    1
}

fn default_alignment() -> u32 {
    8
}
fn default_rmode() -> u8 {
    31
}

pub fn is_module(bytes: &[u8]) -> bool {
    bytes.starts_with(b"{\"zeus_hlasm\":")
}

pub fn module_bytes(module: &Module) -> Result<Vec<u8>, String> {
    serde_json::to_vec(module).map_err(|e| e.to_string())
}

pub fn load_module(bytes: &[u8]) -> Result<Module, String> {
    if !is_module(bytes) {
        return Err("not a Zeus HLASM module".into());
    }
    let module: Module = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if !matches!(module.zeus_hlasm, 1..=9) {
        return Err("unsupported HLASM module version".into());
    }
    module.required_alignment()?;
    data_management::validate(&module)?;
    module.bindings.validate(&module)?;
    if module
        .relocations
        .iter()
        .any(|r| !(1..=8).contains(&r.width) || (r.width != 4 && module.zeus_hlasm < 7))
    {
        return Err("invalid relocation width or module version".into());
    }
    module.validate_read_only()?;
    if module.alignment > 8 && module.zeus_hlasm < 5 {
        return Err("extended alignment requires HLASM module version 5".into());
    }
    if module.zeus_hlasm == 1 && module.code.iter().any(|i| i.relative_external.is_some()) {
        return Err("relative external relocation requires HLASM module version 2".into());
    }
    if module.zeus_hlasm < 3 && module.code.iter().any(|i| i.relative_expression.is_some()) {
        return Err("composite relative relocation requires HLASM module version 3".into());
    }
    if module.zeus_hlasm < 4 && module.relocations.iter().any(|r| r.expression.is_some()) {
        return Err("composite address relocation requires HLASM module version 4".into());
    }
    Ok(module)
}

pub fn save_module(module: &Module, path: &Path) -> Result<(), String> {
    std::fs::write(path, module_bytes(module)?).map_err(|e| format!("{}: {e}", path.display()))
}

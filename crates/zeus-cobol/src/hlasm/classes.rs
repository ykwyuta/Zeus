//! Source class attributes needed for separately protected PSECT data.
use super::assemble::{Symbols, operands};
#[derive(Clone)]
pub(super) struct Attributes {
    pub alignment: u32,
    pub read_only: bool,
    pub executable: bool,
    pub rmode: u8,
    pub no_load: bool,
    pub part: Option<String>,
    pub priority: u32,
    pub fill: u8,
}
impl Default for Attributes {
    fn default() -> Self {
        Self {
            alignment: 8,
            read_only: false,
            executable: true,
            rmode: 24,
            no_load: false,
            part: None,
            priority: 0,
            fill: 0,
        }
    }
}
pub(super) fn parse(text: &str, symbols: &Symbols) -> Result<Attributes, String> {
    let mut result = Attributes::default();
    for field in operands(&text.to_ascii_uppercase()) {
        match field.as_str() {
            "" => {}
            "READONLY" => result.read_only = true,
            "EXECUTABLE" => result.executable = true,
            "NOTEXECUTABLE" => result.executable = false,
            "NOLOAD" => result.no_load = true,
            "LOAD" | "DEFLOAD" => result.no_load = false,
            // Zeus owns a run-unit copy. These reuse declarations carry no
            // additional sharing or relocation promise in its source engine.
            "RENT" | "REUS" | "REFR" | "NOTREUS" | "REMOVABLE" => {}
            "RMODE(24)" => result.rmode = 24,
            "RMODE(31)" | "RMODE(ANY)" => result.rmode = 31,
            "MERGE" => {}
            _ if field.starts_with("PART(") && field.ends_with(')') => {
                let name = &field[5..field.len() - 1];
                if !super::assemble::symbol_name(name) {
                    return Err("invalid CATTR PART name".into());
                }
                result.part = Some(name.into());
            }
            _ if field.starts_with("PRIORITY(") && field.ends_with(')') => {
                result.priority = field[9..field.len() - 1]
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n <= 0x7fffffff)
                    .ok_or("CATTR PRIORITY must be 0..2147483647")?;
            }
            _ if field.starts_with("FILL(") && field.ends_with(')') => {
                result.fill = field[5..field.len() - 1]
                    .parse()
                    .map_err(|_| "CATTR FILL must be 0..255")?;
            }
            _ if field.starts_with("ALIGN(") && field.ends_with(')') => {
                let n =
                    super::expression::evaluate_early(&field[6..field.len() - 1], symbols)?.value;
                if ![0, 1, 2, 3, 4, 12].contains(&n) {
                    return Err("CATTR ALIGN exponent must be 0,1,2,3,4 or 12".into());
                }
                result.alignment = 1u32 << n;
            }
            _ => return Err(format!("unsupported CATTR attribute {field}")),
        }
    }
    if result.part.is_none()
        && (text.to_ascii_uppercase().contains("PRIORITY(")
            || text.to_ascii_uppercase().contains("FILL("))
    {
        return Err("CATTR PRIORITY/FILL require PART".into());
    }
    if result.part.is_some() && !result.no_load {
        return Err("initialized CATTR PART classes require GOFF-class binding, outside the current source engine".into());
    }
    Ok(result)
}

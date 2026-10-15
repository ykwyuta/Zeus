//! Per-operand assembly layout, before information is lost by image flattening.
use super::super::{
    Module,
    assemble::{self, Statement, Symbols},
    bit_constants, constants,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataValue {
    pub byte_length: u32,
    pub bit_length: u32,
    /// Nominal value of the first repetition, left-aligned for bit operands.
    pub data: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataOperand {
    pub offset: u32,
    pub bit_offset: u8,
    pub duplication: u32,
    pub type_attribute: u8,
    pub type_extension: String,
    pub assembler_type: String,
    pub program_type: u32,
    pub integer_attribute: i64,
    pub scale_attribute: i64,
    pub location_dependent: bool,
    pub values: Vec<DataValue>,
}
fn bits(image: &[u8], start: usize, count: usize) -> Result<Vec<u8>, String> {
    if start
        .checked_add(count)
        .is_none_or(|end| end > image.len() * 8)
    {
        return Err("data layout outside assembled image".into());
    }
    let mut result = vec![0; count.div_ceil(8)];
    for n in 0..count {
        if image[(start + n) / 8] & (0x80 >> ((start + n) % 8)) != 0 {
            result[n / 8] |= 0x80 >> (n % 8);
        }
    }
    Ok(result)
}
pub(super) fn layout(
    module: &Module,
    statement: &Statement,
    symbols: &Symbols,
) -> Result<Vec<DataOperand>, String> {
    let mut result = Vec::new();
    let mut cursor = statement.offset as usize * 8;
    let initialized = statement.op == "DC";
    for original in assemble::operands(&statement.args) {
        let (clean, _, assembler_type) = constants::annotated(&original)?;
        let (at, duplication) = constants::duplication(&clean, symbols)?;
        let once = &clean[at..];
        let (_, alignment, first_length) = assemble::storage_item(once, initialized, symbols)?;
        let bit_length = bit_constants::length(once, symbols)?.map(|v| v.0);
        if bit_length.is_none() {
            cursor = cursor.div_ceil(8).next_multiple_of(alignment as usize) * 8;
        }
        let start = cursor;
        let kind = once.as_bytes()[0].to_ascii_uppercase();
        let nominal = if let Some(address) = assemble::address_constants(once, symbols)? {
            let n = address.values.len().max(1);
            vec![(u32::from(address.width), 0); n]
        } else if let Some(bit_length) = bit_length {
            let field =
                bit_constants::assemble(once, initialized, symbols)?.ok_or("missing bit layout")?;
            vec![(0, bit_length); field.bits / bit_length as usize]
        } else if !matches!(kind, b'C' | b'G') {
            if let Some(quote) = constants::nominal_quote(once)? {
                let value = once[quote + 1..]
                    .strip_suffix('\'')
                    .ok_or("unterminated nominal")?;
                let mut nominal = Vec::new();
                for value in value.split(',') {
                    let spec = format!("{}'{}'", &once[..quote], value);
                    let (data, _, _) = assemble::storage_item(&spec, initialized, symbols)?;
                    nominal.push((data.len() as u32, 0));
                }
                nominal
            } else {
                vec![(first_length, 0)]
            }
        } else {
            let (data, _, _) = assemble::storage_item(once, initialized, symbols)?;
            vec![(data.len() as u32, 0)]
        };
        let (program_type, _) = constants::annotations(&original, symbols)?;
        let (scale_attribute, integer_attribute) =
            assemble::storage_numeric_attributes(&original, first_length, symbols)
                .unwrap_or_default();
        // Dummy sections have no bytes in the executable image. Their ordinary
        // nominal values still have a well-defined ADATA representation.
        let dummy_data = if initialized && duplication > 0 && !statement.section.is_empty() {
            if assemble::address_constants(once, symbols)?.is_some() {
                return Err("dummy-section address nominal requires relocation layout".into());
            }
            Some(assemble::storage_item(once, true, symbols)?.0)
        } else {
            None
        };
        let mut values = Vec::new();
        for (byte_length, bit_length) in nominal {
            let size = if bit_length == 0 {
                byte_length as usize * 8
            } else {
                bit_length as usize
            };
            let data = if statement.op == "DC" && duplication > 0 {
                if statement.section.is_empty() {
                    bits(&module.storage, cursor, size)?
                } else {
                    bits(
                        dummy_data.as_ref().ok_or("missing dummy nominal")?,
                        cursor - start,
                        size,
                    )?
                }
            } else {
                Vec::new()
            };
            values.push(DataValue {
                byte_length,
                bit_length,
                data,
            });
            cursor += size;
        }
        let per_copy = cursor - start;
        cursor = start + per_copy * duplication as usize;
        let location_dependent = initialized
            && duplication > 1
            && !matches!(kind, b'C' | b'G')
            && assemble::literal_has_location(once);
        result.push(DataOperand {
            offset: (start / 8) as u32,
            bit_offset: (start % 8) as u8,
            duplication,
            type_attribute: assemble::storage_type(&original),
            type_extension: assembler_type
                .chars()
                .nth(1)
                .map_or(String::new(), |c| c.to_string()),
            assembler_type,
            program_type: program_type.unwrap_or(0),
            integer_attribute,
            scale_attribute,
            location_dependent,
            values,
        });
    }
    Ok(result)
}

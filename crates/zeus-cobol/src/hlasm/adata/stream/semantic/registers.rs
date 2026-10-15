//! Architectural GPR references; floating registers never enter the GPR map.
use crate::hlasm::{
    AssemblyListing, Module,
    float::{convert, operations as fp},
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn collect(module: &Module, listing: &AssemblyListing) -> BTreeMap<u8, Vec<(u32, u8)>> {
    let mut result: BTreeMap<u8, Vec<(u32, u8)>> = BTreeMap::new();
    for i in &module.code {
        let Some(row) = listing.statements.iter().find(|s| {
            s.offset == i.offset && s.location.file == i.file && s.location.line == i.line
        }) else {
            continue;
        };
        let statement = row.location.statement as u32;
        let op = i.op.as_str();
        let mut refs = BTreeSet::new();
        for address in [&i.address, &i.source].into_iter().flatten() {
            if address.base != 0 {
                refs.insert((address.base, 0x40));
            }
            if address.index != 0 {
                refs.insert((address.index, 0xd5));
            }
        }
        let mut gprs = Vec::new();
        if op == "SFPC" || op == "EFPC" {
            gprs.push((0, if op == "EFPC" { 0xd4 } else { 0x40 }));
        } else if op == "PFPO" {
            refs.insert((0, 0x40));
            refs.insert((1, 0xd4));
        } else if let Some(spec) = convert::instruction(op) {
            gprs.push((
                usize::from(!spec.to_integer),
                if spec.to_integer { 0xd4 } else { 0x40 },
            ));
        } else if let Some(spec) = fp::decimal_packed(op) {
            gprs.push((
                usize::from(spec.to_decimal),
                if spec.to_decimal { 0x40 } else { 0xd4 },
            ));
        } else if fp::common(op).is_some() {
            match op {
                "LDGR" => gprs.push((1, 0x40)),
                "LGDR" => gprs.push((0, 0xd4)),
                _ => {}
            }
        } else if let Some(spec) = fp::decimal_bits(op) {
            match spec.kind {
                fp::DecimalBitKind::Exponent | fp::DecimalBitKind::Significance => {
                    gprs.push((0, 0xd4))
                }
                fp::DecimalBitKind::Insert => gprs.push((1, 0x40)),
                fp::DecimalBitKind::Reround => gprs.push((2, 0x40)),
                _ => {}
            }
        } else if fp::size(op).is_some()
            || matches!(
                op,
                "LER" | "LDR" | "LXR" | "LE" | "LD" | "STE" | "STD" | "PFPO"
            )
        {
        } else {
            for n in 0..i.registers.len() {
                let flag = if (op == "BR" || op == "BCR")
                    || n == 1 && matches!(op, "BASR" | "BALR" | "BCTR" | "BSM" | "BASSM")
                {
                    0xc2
                } else if n == 0
                    && !(op.starts_with("ST")
                        || op.starts_with('C')
                            && !matches!(op, "CS" | "CSY" | "CSG" | "CDS" | "CDSY" | "CDSG")
                        || matches!(op, "SPM" | "SFPC"))
                {
                    0xd4
                } else {
                    0x40
                };
                gprs.push((n, flag));
            }
        }
        for (n, flag) in gprs {
            if let Some(r) = i.registers.get(n) {
                refs.insert((*r, flag));
            }
        }
        if matches!(op, "LM" | "LMG" | "LMY" | "STM" | "STMG" | "STMY") && i.registers.len() == 2 {
            let mut r = i.registers[0];
            loop {
                refs.insert((r, if op.starts_with('L') { 0xd4 } else { 0x40 }));
                if r == i.registers[1] {
                    break;
                }
                r = (r + 1) & 15;
            }
        }
        // Pair operands imply a reference to their odd partner even though
        // the assembler syntax mentions only the even register.
        let pair_slots: &[usize] = match op {
            "MR" | "DR" | "MLR" | "MLGR" | "DLR" | "DLGR" | "DSGR" | "DSGFR" | "M" | "D" | "ML"
            | "MLG" | "DL" | "DLG" | "DSG" | "DSGF" | "FLOGR" | "LPQ" | "STPQ" | "SLDL"
            | "SRDL" | "SLDA" | "SRDA" => &[0],
            "MVCL" | "CLCL" | "MVCLE" | "CLCLE" | "CDS" | "CDSY" | "CDSG" => &[0, 1],
            _ => &[],
        };
        for &slot in pair_slots {
            if let Some(&r) = i.registers.get(slot) {
                refs.insert((
                    (r + 1) & 15,
                    if op == "STPQ" || slot == 1 && matches!(op, "CDS" | "CDSY" | "CDSG") {
                        0x40
                    } else {
                        0xd4
                    },
                ));
            }
        }
        let rank = |flag| match flag {
            0xd4 => 4,
            0xc2 => 3,
            0xd5 => 2,
            _ => 1,
        };
        let mut grouped: BTreeMap<u8, u8> = BTreeMap::new();
        for (r, flag) in refs {
            grouped
                .entry(r)
                .and_modify(|old| {
                    if rank(flag) > rank(*old) {
                        *old = flag;
                    }
                })
                .or_insert(flag);
        }
        for (r, flag) in grouped {
            result.entry(r).or_default().push((statement, flag));
        }
    }
    for (index, row) in listing.statements.iter().enumerate() {
        if !matches!(row.operation.as_str(), "USING" | "DROP") {
            continue;
        }
        let after = listing
            .statements
            .get(index + 1)
            .map_or(&row.usings, |next| &next.usings);
        let mut registers = BTreeSet::new();
        let operands = crate::hlasm::assemble::operands(&row.operands);
        for operand in operands.iter().skip(usize::from(row.operation == "USING")) {
            if let Some(r) = operand
                .parse::<u8>()
                .ok()
                .or_else(|| {
                    listing
                        .symbols
                        .get(operand)
                        .and_then(|s| u8::try_from(s.value).ok())
                })
                .filter(|r| *r < 16)
            {
                registers.insert(r);
            }
        }
        if row.operation == "USING" {
            for u in after {
                if u.label == row.label
                    && !row.usings.iter().any(|v| {
                        v.register == u.register && v.label == u.label && v.origin == u.origin
                    })
                {
                    registers.insert(u.register);
                }
            }
        } else {
            for u in &row.usings {
                if !after
                    .iter()
                    .any(|v| v.register == u.register && v.label == u.label && v.origin == u.origin)
                {
                    registers.insert(u.register);
                }
            }
        }
        for r in registers {
            result.entry(r).or_default().push((
                row.location.statement as u32,
                if row.operation == "USING" { 0xe4 } else { 0xc4 },
            ));
        }
    }
    for refs in result.values_mut() {
        refs.sort_unstable();
        refs.dedup();
    }
    result
}

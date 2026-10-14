//! BMS mapsets, IMS DBD and PSB sources, and CICS CSD definitions
//! (DFHCSDUP input).

use super::{Ref, Scanned};

/// The value of `KEY=value` in an assembler-style operand field, with its
/// column (1-based).
fn operand(line: &str, key: &str) -> Option<(String, usize)> {
    let u = line.to_ascii_uppercase();
    let mut from = 0;
    while let Some(p) = u[from..].find(key) {
        let at = from + p;
        let before = u[..at].chars().last();
        if before.is_none_or(|c| c == ',' || c == ' ' || c == '(')
            && u[at + key.len()..].starts_with('=')
        {
            let start = at + key.len() + 1;
            let rest = &u[start..];
            let rest_t = rest.trim_start_matches('(');
            let skip = rest.len() - rest_t.len();
            let end = rest_t.find([',', ' ', ')']).unwrap_or(rest_t.len());
            let v = rest_t[..end].trim_matches('\'').to_string();
            return (!v.is_empty()).then(|| (v, u[..start + skip].chars().count() + 1));
        }
        from = at + key.len();
    }
    None
}

pub fn bms(text: &str) -> Scanned {
    let mut s = Scanned::default();
    match zeus_bms::parse(text) {
        Ok(ms) => {
            s.name = Some(ms.name.to_ascii_uppercase());
            s.attrs.push(("mapset_name", ms.name.to_ascii_uppercase()));
            s.attrs.push((
                "maps",
                ms.maps
                    .iter()
                    .map(|m| m.name.to_ascii_uppercase())
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
            s.attrs.push(("parse_status", "OK".into()));
        }
        Err(e) => {
            // the label of DFHMSD
            if let Some(l) = text
                .lines()
                .find(|l| l.to_ascii_uppercase().contains("DFHMSD") && !l.starts_with(' '))
            {
                let n = l
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_uppercase();
                s.name = Some(n.clone());
                s.attrs.push(("mapset_name", n));
            }
            s.attrs.push(("parse_status", "ERROR".into()));
            s.attrs.push(("parse_message", e));
        }
    }
    s
}

pub fn dbd(text: &str) -> Scanned {
    let mut s = Scanned::default();
    for l in text.lines().filter(|l| !l.starts_with('*')) {
        let words: Vec<&str> = l.split_whitespace().collect();
        if words.first().is_some_and(|w| w.eq_ignore_ascii_case("DBD")) {
            if let Some((n, _)) = operand(l, "NAME") {
                s.name = Some(n.clone());
                s.attrs.push(("dbd_name", n));
            }
            if let Some((a, _)) = operand(l, "ACCESS") {
                s.attrs.push(("access", a));
            }
        }
    }
    s
}

pub fn psb(text: &str) -> Scanned {
    let mut s = Scanned::default();
    let mut pcbs = 0;
    for (i, l) in text.lines().enumerate() {
        if l.starts_with('*') {
            continue;
        }
        let u = l.to_ascii_uppercase();
        let words: Vec<&str> = u.split_whitespace().collect();
        let op_is = |w: &str| words.first() == Some(&w) || words.get(1) == Some(&w);
        if op_is("PCB") {
            pcbs += 1;
            if let Some((d, col)) = operand(l, "DBDNAME") {
                let mut r = Ref::new("PSB_DBD", &d, i + 1, col);
                if let Some((t, _)) = operand(l, "TYPE") {
                    r = r.attr("pcb_type", t);
                }
                if let Some((p, _)) = operand(l, "PROCOPT") {
                    r = r.attr("procopt", p);
                }
                s.refs.push(r);
            }
        }
        if op_is("PSBGEN") {
            if let Some((n, _)) = operand(l, "PSBNAME") {
                s.name = Some(n.clone());
                s.attrs.push(("psb_name", n));
            }
            if let Some((g, _)) = operand(l, "LANG") {
                s.attrs.push(("language", g));
            }
        }
    }
    s.attrs.push(("pcbs", pcbs.to_string()));
    s
}

/// DEFINE PROGRAM / TRANSACTION / MAPSET statements of a CSD.
pub fn csd(text: &str) -> Scanned {
    let mut s = Scanned::default();
    // words with their positions; `KEY(value)` as (key, value, col)
    struct Opt {
        key: String,
        value: String,
        line: usize,
        col: usize,
    }
    let mut stmts: Vec<Vec<Opt>> = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if l.trim_start().starts_with('*') {
            continue;
        }
        let u = l.to_ascii_uppercase();
        let b = u.as_bytes();
        let mut k = 0;
        while k < b.len() {
            if b[k].is_ascii_alphabetic() {
                let start = k;
                while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                    k += 1;
                }
                let key = u[start..k].to_string();
                let (value, col) = if b.get(k) == Some(&b'(') {
                    let vs = k + 1;
                    let ve = u[vs..].find(')').map_or(u.len(), |e| vs + e);
                    let v = u[vs..ve].trim().to_string();
                    k = ve + 1;
                    (v, vs + 1)
                } else {
                    (String::new(), start + 1)
                };
                if key == "DEFINE" {
                    stmts.push(Vec::new());
                }
                if let Some(cur) = stmts.last_mut() {
                    cur.push(Opt {
                        key,
                        value,
                        line: i + 1,
                        col,
                    });
                }
            } else {
                k += 1;
            }
        }
    }
    let mut groups: Vec<String> = Vec::new();
    let mut transactions: Vec<String> = Vec::new();
    let mut programs = 0;
    for st in &stmts {
        let get = |k: &str| st.iter().find(|o| o.key == k);
        let group = get("GROUP").map(|o| o.value.clone()).unwrap_or_default();
        if !group.is_empty() && !groups.contains(&group) {
            groups.push(group.clone());
        }
        let kind = st.get(1).map(|o| o.key.as_str()).unwrap_or("");
        let Some(def) = st.get(1) else { continue };
        match kind {
            "PROGRAM" => {
                programs += 1;
                s.refs.push(
                    Ref::new("CSD_PROGRAM", &def.value, def.line, def.col).attr("groups", group),
                );
            }
            "TRANSACTION" => {
                transactions.push(def.value.clone());
                if let Some(p) = get("PROGRAM") {
                    s.transactions.push((def.value.clone(), p.value.clone()));
                    s.refs.push(
                        Ref::new("CSD_TRANSACTION", &p.value, p.line, p.col)
                            .attr("transids", def.value.clone())
                            .attr("groups", group),
                    );
                }
            }
            "MAPSET" => {
                s.refs.push(
                    Ref::new("CSD_MAPSET", &def.value, def.line, def.col).attr("groups", group),
                );
            }
            _ => {}
        }
    }
    s.attrs.push(("groups", groups.join(" ")));
    s.attrs.push(("transactions", transactions.join(" ")));
    s.attrs.push(("programs", programs.to_string()));
    s
}

/// Application assembler sources, COPY members and macro invocations.
fn hlasm_argument_field(text: &str) -> &str {
    let mut quoted = false;
    for (at, c) in text.char_indices() {
        let attribute =
            at > 0 && b"LKNDTOSI".contains(&text.as_bytes()[at - 1].to_ascii_uppercase());
        if c == '\'' && (quoted || !attribute) {
            quoted = !quoted;
        }
        if c.is_whitespace() && !quoted {
            return &text[..at];
        }
    }
    text
}

pub fn hlasm(text: &str) -> super::Scanned {
    let mut out = super::Scanned::default();
    let mut prototype = false;
    let mut macros = std::collections::HashSet::new();
    let mut operation_aliases = std::collections::HashMap::<String, String>::new();
    let mut external_kinds = std::collections::HashMap::new();
    let mut local_symbols = std::collections::HashSet::new();
    let mut binding_aliases = std::collections::BTreeSet::new();
    for raw in text
        .lines()
        .filter(|l| !l.starts_with('*') && !l.starts_with(".*"))
    {
        let words: Vec<_> = raw.split_whitespace().collect();
        let at = usize::from(!raw.starts_with(char::is_whitespace));
        if at == 1
            && let Some(label) = words.first().filter(|name| !name.contains('&'))
        {
            local_symbols.insert(label.to_ascii_uppercase());
        }
        if at == 1
            && let Some(op) = words.get(1)
            && matches!(op.to_ascii_uppercase().as_str(), "DXD" | "DSECT" | "CATTR")
        {
            binding_aliases.insert(words[0].to_ascii_uppercase());
        }
        if words
            .get(at)
            .is_some_and(|op| op.eq_ignore_ascii_case("CATTR"))
            && let Some(part_at) = raw.to_ascii_uppercase().find("PART(")
            && let Some(end) = raw[part_at + 5..].find(')')
        {
            let name = raw[part_at + 5..part_at + 5 + end].to_ascii_uppercase();
            local_symbols.insert(name.clone());
            binding_aliases.insert(name);
        }
        if let (Some(op), Some(args)) = (words.get(at), words.get(at + 1))
            && matches!(op.to_ascii_uppercase().as_str(), "EXTRN" | "WXTRN")
        {
            for name in args.split(',') {
                external_kinds
                    .entry(name.to_ascii_uppercase())
                    .or_insert(op.eq_ignore_ascii_case("WXTRN"));
            }
        }
    }
    for (line, raw) in text.lines().enumerate() {
        if raw.is_empty() || raw.starts_with('*') || raw.starts_with(".*") {
            continue;
        }
        let mut words = raw.split_whitespace();
        let label = if raw.starts_with(char::is_whitespace) {
            None
        } else {
            words.next()
        };
        let Some(op) = words.next() else {
            continue;
        };
        let mut op = op.to_ascii_uppercase();
        let first = words.next().unwrap_or("");
        let argument_start = if first.is_empty() {
            raw.len()
        } else {
            raw.find(first).unwrap_or(raw.len())
        };
        let arg = hlasm_argument_field(&raw[argument_start..]);
        if op == "OPSYN" {
            if let Some(label) = label {
                let name = label.to_ascii_uppercase();
                if arg.is_empty() {
                    operation_aliases.remove(&name);
                } else {
                    let target = arg.to_ascii_uppercase();
                    let target = operation_aliases.get(&target).cloned().unwrap_or(target);
                    operation_aliases.insert(name, target);
                }
            }
            continue;
        }
        if let Some(target) = operation_aliases.get(&op) {
            op = target.clone();
        }
        if matches!(op.as_str(), "CSECT" | "RSECT" | "START") {
            if out.name.is_none() {
                out.name = label.map(str::to_ascii_uppercase);
            } else if let Some(label) = label {
                let label = label.to_ascii_uppercase();
                if out.name.as_ref() != Some(&label) && !out.aliases.contains(&label) {
                    out.aliases.push(label);
                }
            }
        }
        if op == "ENTRY" {
            out.aliases
                .extend(arg.split(',').map(str::to_ascii_uppercase));
        }
        if prototype {
            macros.insert(op.clone());
            prototype = false;
            continue;
        }
        if op == "MACRO" {
            prototype = true;
            continue;
        }
        if matches!(op.as_str(), "EXTRN" | "WXTRN") {
            for name in arg.split(',').filter(|n| !n.is_empty()) {
                let col = raw.find(name).map_or(1, |i| i + 1);
                out.refs.push(
                    super::Ref::new("HLASM_EXTERNAL", name, line + 1, col)
                        .attr("weak", if op == "WXTRN" { "true" } else { "false" }),
                );
            }
        }
        for reference in zeus_cobol::hlasm::constant_references(arg, op != "DC") {
            if reference.kind != 'V' && local_symbols.contains(&reference.name) {
                continue;
            }
            let mut r = super::Ref::new(
                "HLASM_EXTERNAL",
                &reference.name,
                line + 1,
                raw[..argument_start + reference.offset].chars().count() + 1,
            );
            r.dynamic = reference.name.contains('&');
            r.attrs.push((
                "weak",
                external_kinds
                    .get(&reference.name)
                    .copied()
                    .unwrap_or(false)
                    .to_string(),
            ));
            r.attrs.push(("constant_type", reference.kind.to_string()));
            out.refs.push(r);
        }
        if matches!(op.as_str(), "LINK" | "XCTL" | "LOAD") {
            if let Some((name, col)) = operand(raw, "EP") {
                out.refs
                    .push(super::Ref::new("CALL", &name, line + 1, col).attr("service", &op));
            } else if let Some((pointer, col)) = operand(raw, "EPLOC") {
                let mut reference =
                    super::Ref::new("CALL", &pointer, line + 1, col).attr("service", &op);
                reference.dynamic = true;
                out.refs.push(reference);
            }
        }
        let target = if op == "COPY" {
            Some(arg)
        } else if op != "MEND"
            && !macros.contains(&op)
            && !op.starts_with('&')
            && !zeus_cobol::hlasm::is_operation(&op)
        {
            Some(op.as_str())
        } else {
            None
        };
        if let Some(target) = target.filter(|t| !t.is_empty()) {
            let col = raw
                .find(if op == "COPY" { arg } else { op.as_str() })
                .map_or(1, |i| i + 1);
            out.refs
                .push(super::Ref::new("HLASM_COPY", target, line + 1, col));
        }
    }
    for alias in binding_aliases {
        if out.name.as_ref() != Some(&alias) && !out.aliases.contains(&alias) {
            out.aliases.push(alias);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hlasm_sections_entries_and_local_macro_operations() {
        let s = hlasm(
            "         MACRO\n         BUMP &R\n         GBLA &COUNT\n&COUNT   SETA &COUNT+1\n         AHI &R,1\n         MEND\nMAIN CSECT\n         BUMP 2\n         COPY REGS\nSECOND RSECT\n         ENTRY ALT,OTHER\n         END MAIN\n",
        );
        assert_eq!(s.name.as_deref(), Some("MAIN"));
        assert_eq!(s.aliases, ["SECOND", "ALT", "OTHER"]);
        assert_eq!(s.refs.len(), 1);
        assert_eq!(s.refs[0].target, "REGS");
    }

    #[test]
    fn hlasm_operation_aliases_do_not_create_false_macro_dependencies() {
        let s = hlasm(
            "LOAD OPSYN LGHI\nFETCH OPSYN COPY\nMAIN CSECT\n         LOAD 15,0\n         FETCH MEMBER\n         BR 14\n         END\n",
        );
        assert_eq!(s.name.as_deref(), Some("MAIN"));
        assert_eq!(s.refs.len(), 1);
        assert_eq!(s.refs[0].target, "MEMBER");
    }

    #[test]
    fn hlasm_extended_constants_and_services_keep_dependencies_and_source_columns() {
        let text = "MAIN CSECT\nREF DC (N+1)VDP(C'TAG ')L(8)(REMOTE),Q(LOCAL),JD(EXTD),QY(PART1),R(OTHER)\n L 2,=QDP(4)L(8)(EXT2)\nMSG DC C'V(FAKE) Q(FAKE)'\nLOCAL DSECT\nPRIVATE CATTR NOLOAD,PART(PART1)\n LINK EP=COBOL,PARAM=(VALUE)\n LOAD EP=PLISUB\n XCTL EPLOC=TARGET\n";
        let scanned = hlasm(text);
        let refs: Vec<_> = scanned
            .refs
            .iter()
            .map(|r| (r.edge, r.target.as_str(), r.dynamic))
            .collect();
        assert_eq!(
            refs,
            [
                ("HLASM_EXTERNAL", "REMOTE", false),
                ("HLASM_EXTERNAL", "EXTD", false),
                ("HLASM_EXTERNAL", "OTHER", false),
                ("HLASM_EXTERNAL", "EXT2", false),
                ("CALL", "COBOL", false),
                ("CALL", "PLISUB", false),
                ("CALL", "TARGET", true)
            ]
        );
        assert!(scanned.aliases.contains(&"LOCAL".into()));
        assert!(scanned.aliases.contains(&"PART1".into()));
        for r in &scanned.refs {
            let source = text.lines().nth(r.line - 1).unwrap();
            assert!(
                source[r.col - 1..].starts_with(&r.target),
                "{} {}:{}",
                r.target,
                r.line,
                r.col
            );
        }
        assert!(
            scanned.refs[0]
                .attrs
                .contains(&("constant_type", "V".into()))
        );
    }

    #[test]
    fn psb_and_csd() {
        let p = psb("         PCB    TYPE=DB,DBDNAME=CUSTDB,PROCOPT=A,KEYLEN=10
         PSBGEN LANG=COBOL,PSBNAME=CUSTPSB
");
        assert_eq!(p.name.as_deref(), Some("CUSTPSB"));
        assert_eq!((p.refs[0].target.as_str(), p.refs[0].col), ("CUSTDB", 33));
        assert!(p.refs[0].attrs.contains(&("procopt", "A".into())));
        let c = csd(" DEFINE TRANSACTION(CC00) GROUP(CARDDEMO) PROGRAM(COSGN00C)
 DEFINE PROGRAM(COSGN00C) GROUP(CARDDEMO)
 DEFINE MAPSET(COSGN00) GROUP(CARDDEMO)
");
        let refs: Vec<(&str, &str, usize, usize)> = c
            .refs
            .iter()
            .map(|r| (r.edge, r.target.as_str(), r.line, r.col))
            .collect();
        assert_eq!(
            refs,
            [
                ("CSD_TRANSACTION", "COSGN00C", 1, 51),
                ("CSD_PROGRAM", "COSGN00C", 2, 17),
                ("CSD_MAPSET", "COSGN00", 3, 16),
            ]
        );
        assert_eq!(
            c.transactions,
            [("CC00".to_string(), "COSGN00C".to_string())]
        );
    }
}

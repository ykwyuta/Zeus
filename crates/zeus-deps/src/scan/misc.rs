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

#[cfg(test)]
mod tests {
    use super::*;

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

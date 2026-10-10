//! Resource definitions in DFHCSDUP input form:
//! `DEFINE TRANSACTION(CC00) GROUP(G) PROGRAM(COSGN00C) ...`, spread over
//! lines; `*` lines are comments. Only the attributes Zeus uses are kept.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Csd {
    /// TRANSACTION -> PROGRAM.
    pub transactions: BTreeMap<String, String>,
    /// FILE -> DSNAME.
    pub files: BTreeMap<String, String>,
    pub programs: Vec<String>,
    pub mapsets: Vec<String>,
}

/// `KEYWORD(value)` and bare keywords of one statement.
fn attributes(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let c: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < c.len() {
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < c.len() && !c[i].is_whitespace() && c[i] != '(' {
            i += 1;
        }
        let key: String = c[start..i].iter().collect::<String>().to_ascii_uppercase();
        let mut value = String::new();
        if i < c.len() && c[i] == '(' {
            let mut depth = 0;
            i += 1;
            while i < c.len() {
                match c[i] {
                    '(' => depth += 1,
                    ')' if depth == 0 => break,
                    ')' => depth -= 1,
                    _ => {}
                }
                value.push(c[i]);
                i += 1;
            }
            i += 1;
        }
        if !key.is_empty() {
            out.push((key, value.trim().to_string()));
        }
    }
    out
}

impl Csd {
    pub fn parse(text: &str) -> Csd {
        let mut csd = Csd::default();
        let mut stmt = String::new();
        let flush = |stmt: &str, csd: &mut Csd| {
            let a = attributes(stmt);
            let get = |k: &str| {
                a.iter()
                    .find(|(n, _)| n == k)
                    .map(|(_, v)| v.to_ascii_uppercase())
            };
            if a.first().map(|(k, _)| k.as_str()) != Some("DEFINE") {
                return;
            }
            if let Some(t) = get("TRANSACTION") {
                if let Some(p) = get("PROGRAM") {
                    csd.transactions.insert(t, p);
                }
            } else if let Some(f) = get("FILE") {
                if let Some(d) = get("DSNAME") {
                    csd.files.insert(f, d);
                }
            } else if let Some(p) = get("PROGRAM") {
                csd.programs.push(p);
            } else if let Some(m) = get("MAPSET") {
                csd.mapsets.push(m);
            }
        };
        for line in text.lines() {
            let t = line.trim_end();
            if t.trim_start().starts_with('*') {
                continue;
            }
            let body: String = t.chars().take(72).collect();
            if body
                .trim_start()
                .to_ascii_uppercase()
                .starts_with("DEFINE ")
            {
                flush(&stmt, &mut csd);
                stmt.clear();
            }
            stmt.push(' ');
            stmt.push_str(&body);
        }
        flush(&stmt, &mut csd);
        csd
    }

    /// Adds the definitions of another CSD (later definitions win).
    pub fn merge(&mut self, other: Csd) {
        self.transactions.extend(other.transactions);
        self.files.extend(other.files);
        self.programs.extend(other.programs);
        self.mapsets.extend(other.mapsets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions() {
        let csd = Csd::parse(
            " DEFINE TRANSACTION(CC00) GROUP(CARDDEMO)
        PROGRAM(COSGN00C) TWASIZE(0) PROFILE(DFHCICST)
 DESCRIPTION(SIGN ON (LOGIN))
 DEFINE FILE(USRSEC) GROUP(CARDDEMO)
        DSNAME(AWS.M2.CARDDEMO.USRSEC.VSAM.KSDS) RLSACCESS(NO)
* comment
 DEFINE MAPSET(COSGN00) GROUP(CARDDEMO)",
        );
        assert_eq!(csd.transactions["CC00"], "COSGN00C");
        assert_eq!(csd.files["USRSEC"], "AWS.M2.CARDDEMO.USRSEC.VSAM.KSDS");
        assert_eq!(csd.mapsets, vec!["COSGN00"]);
    }
}

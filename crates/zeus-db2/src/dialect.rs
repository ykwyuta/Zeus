//! Db2 for z/OS SQL on Db2 for Linux, UNIX and Windows: statements and
//! clauses that only describe z/OS storage are dropped or rewritten.

use zeus_drda::sql::SqlCa;

/// Splits an SQL script (DSNTIAD / SPUFI input) into statements at `;`
/// outside literals; `--` and `/* */` comments are removed.
pub fn split_script(text: &str) -> Vec<String> {
    let text = strip_block_comments(text);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    for line in text.lines() {
        let mut l = line.to_string();
        if !quote && let Some(p) = find_comment(&l) {
            l.truncate(p);
        }
        for c in l.chars() {
            match c {
                '\'' => {
                    quote = !quote;
                    cur.push(c);
                }
                ';' if !quote => {
                    let s = cur.trim().to_string();
                    if !s.is_empty() {
                        out.push(s);
                    }
                    cur.clear();
                }
                _ => cur.push(c),
            }
        }
        cur.push(' ');
    }
    let s = cur.trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
    out
}

fn strip_block_comments(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut q = false;
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\'' {
            q = !q;
        }
        if !q && c == '/' && it.peek() == Some(&'*') {
            it.next();
            let mut prev = ' ';
            for d in it.by_ref() {
                if prev == '*' && d == '/' {
                    break;
                }
                prev = d;
            }
            out.push(' ');
            continue;
        }
        out.push(c);
    }
    out
}

fn find_comment(l: &str) -> Option<usize> {
    let mut q = false;
    let b = l.as_bytes();
    for i in 0..b.len() {
        match b[i] {
            b'\'' => q = !q,
            b'-' if !q && b.get(i + 1) == Some(&b'-') => return Some(i),
            _ => {}
        }
    }
    None
}

/// Words of a statement outside literals, upper-cased, with positions.
fn words(sql: &str) -> Vec<(usize, usize, String)> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\'' {
            i += 1;
            while i < b.len() && b[i] != b'\'' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'<' {
            let s = i;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || matches!(b[i], b'_' | b'.' | b'<' | b'>' | b'#' | b'@' | b'$'))
            {
                i += 1;
            }
            out.push((s, i, sql[s..i].to_ascii_uppercase()));
            continue;
        }
        i += 1;
    }
    out
}

/// The statement for Db2 LUW; None when it has no equivalent there and is
/// skipped (storage groups, databases, table spaces, ...).
pub fn translate(sql: &str) -> Option<String> {
    let w = words(sql);
    let kw = |k: usize| w.get(k).map(|x| x.2.as_str()).unwrap_or("");
    match (kw(0), kw(1)) {
        ("CREATE", "STOGROUP" | "DATABASE" | "TABLESPACE")
        | ("DROP", "STOGROUP" | "DATABASE" | "TABLESPACE")
        | ("ALTER", "STOGROUP" | "DATABASE" | "TABLESPACE")
        | ("COMMENT", _)
        | ("LABEL", _)
        | ("GRANT", _) => return None,
        ("SET", "CURRENT") if kw(2) == "SQLID" => {
            // the qualifier of unqualified names
            let rest = sql[w[2].1..].trim_start().trim_start_matches('=').trim();
            return Some(format!("SET CURRENT SCHEMA = {rest}"));
        }
        _ => {}
    }
    // drop z/OS storage clauses: CCSID EBCDIC/ASCII/UNICODE, IN db.ts,
    // COPY YES/NO, CLOSE YES/NO, BUFFERPOOL x, USING STOGROUP x,
    // PRIQTY/SECQTY n, ERASE YES/NO, FREEPAGE/PCTFREE n
    let mut cut: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    let ddl = matches!(kw(0), "CREATE" | "ALTER");
    while k < w.len() {
        let (s, _, ref t) = w[k];
        let end_of = |j: usize| w.get(j).map(|x| x.1);
        let mut take = |n: usize| {
            if let Some(e) = end_of(k + n) {
                cut.push((s, e));
            }
        };
        match t.as_str() {
            "CCSID" if matches!(kw(k + 1), "EBCDIC" | "ASCII" | "UNICODE") => take(1),
            "IN" if ddl && kw(0) == "CREATE" && kw(1) == "TABLE" && kw(k + 1).contains('.') => {
                take(1)
            }
            "COPY" | "CLOSE" | "ERASE" if ddl && matches!(kw(k + 1), "YES" | "NO") => take(1),
            "BUFFERPOOL" | "PRIQTY" | "SECQTY" | "FREEPAGE" if ddl => take(1),
            "USING" if ddl && kw(k + 1) == "STOGROUP" => take(2),
            // table options: [NOT] VOLATILE [CARDINALITY], AUDIT x, DATA
            // CAPTURE x
            "NOT" if ddl && kw(k + 1) == "VOLATILE" => {
                take(if kw(k + 2) == "CARDINALITY" { 2 } else { 1 })
            }
            "VOLATILE" if ddl && kw(k.wrapping_sub(1)) != "NOT" => {
                take(usize::from(kw(k + 1) == "CARDINALITY"))
            }
            "AUDIT" if ddl && matches!(kw(k + 1), "NONE" | "CHANGES" | "ALL") => take(1),
            "DATA" if ddl && kw(k + 1) == "CAPTURE" => take(2),
            _ => {}
        }
        k += 1;
    }
    let mut out = String::new();
    let mut last = 0;
    for (s, e) in cut {
        if s >= last {
            out.push_str(&sql[last..s]);
            last = e;
        }
    }
    out.push_str(&sql[last..]);
    Some(out)
}

/// Db2 for Linux warnings z/OS does not give for the same statement:
/// +605 when CREATE INDEX matches an index the server already made (the
/// one of the primary key); z/OS creates the index.
pub fn adjust_sqlca(sql: &str, ca: SqlCa) -> SqlCa {
    let w: Vec<String> = sql
        .split_whitespace()
        .take(3)
        .map(str::to_ascii_uppercase)
        .collect();
    let create_index =
        w.first().is_some_and(|f| f == "CREATE") && w.iter().skip(1).any(|x| x == "INDEX");
    if create_index && ca.sqlcode == 605 {
        return SqlCa::ok();
    }
    ca
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luw_warnings() {
        let w = SqlCa {
            sqlcode: 605,
            sqlstate: "01550".into(),
            ..SqlCa::default()
        };
        assert_eq!(
            adjust_sqlca("CREATE UNIQUE INDEX A.I ON A.T (C)", w.clone()).sqlcode,
            0
        );
        assert_eq!(adjust_sqlca("INSERT INTO A.T VALUES (1)", w).sqlcode, 605);
    }

    #[test]
    fn zos_ddl() {
        assert_eq!(translate("CREATE STOGROUP G VOLUMES ('*') VCAT X"), None);
        assert_eq!(
            translate(
                "CREATE TABLE B.C (N CHAR(4) NOT NULL) IN B.C NOT VOLATILE \
                 CARDINALITY AUDIT NONE DATA CAPTURE NONE"
            )
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
            "CREATE TABLE B.C (N CHAR(4) NOT NULL)"
        );
        assert_eq!(
            translate("CREATE TABLE GENAPP.CUSTOMER (ID INTEGER) CCSID EBCDIC IN GENAPP.GENATS00")
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "CREATE TABLE GENAPP.CUSTOMER (ID INTEGER)"
        );
        assert_eq!(
            translate("CREATE UNIQUE INDEX A.I ON A.T (C) CLUSTER COPY YES")
                .unwrap()
                .trim(),
            "CREATE UNIQUE INDEX A.I ON A.T (C) CLUSTER"
        );
        assert_eq!(
            translate("SET CURRENT SQLID='GENAPP'").unwrap(),
            "SET CURRENT SCHEMA = 'GENAPP'"
        );
        assert_eq!(
            translate("SELECT 'IN A.B' FROM T").unwrap(),
            "SELECT 'IN A.B' FROM T"
        );
    }

    #[test]
    fn scripts() {
        let s = split_script(
            "/* a; b */ SET A = 'x;y' ;\n-- note\nCREATE TABLE T (C INT) ; DROP TABLE U",
        );
        assert_eq!(
            s,
            vec!["SET A = 'x;y'", "CREATE TABLE T (C INT)", "DROP TABLE U"]
        );
    }
}

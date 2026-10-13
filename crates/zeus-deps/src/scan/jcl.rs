//! JCL jobs, procedures and INCLUDE members, read with the Zeus JCL
//! parser: EXEC PGM= and EXEC of procedures, INCLUDE MEMBER=, the
//! programs run by IKJEFT01 (DSN RUN PROGRAM()) and DFSRRC00 (PARM).
//! Procedures defined in the job (PROC ... PEND) are not references.

use zeus_jcl::parse::{Param, Stmt, Value};

use super::{Ref, Scanned};

fn keyword<'a>(params: &'a [Param], key: &str) -> Option<&'a Value> {
    params
        .iter()
        .find(|p| {
            p.key
                .as_deref()
                .is_some_and(|k| k.eq_ignore_ascii_case(key))
        })
        .map(|p| &p.value)
}

fn text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::List(l) => l
            .iter()
            .map(|p| match (&p.key, &p.value) {
                (Some(k), v) => format!("{k}={}", text(v)),
                (None, v) => text(v),
            })
            .collect::<Vec<_>>()
            .join(","),
    }
}

/// The line and column of `name` in the statement starting at `line`
/// (its continuation cards included), else the statement's line.
fn locate(lines: &[&str], line: usize, name: &str) -> (usize, usize) {
    let up = name.to_ascii_uppercase();
    for (k, l) in lines
        .iter()
        .enumerate()
        .skip(line.saturating_sub(1))
        .take(12)
    {
        let u = l.to_ascii_uppercase();
        let mut from = 0;
        while let Some(p) = u[from..].find(&up) {
            let at = from + p;
            let before = u[..at].chars().last();
            let after = u[at + up.len()..].chars().next();
            let boundary = |c: Option<char>| {
                c.is_none_or(|c| !c.is_ascii_alphanumeric() && !"#@$".contains(c))
            };
            if boundary(before) && boundary(after) {
                return (k + 1, u[..at].chars().count() + 1);
            }
            from = at + up.len();
        }
    }
    (line, 1)
}

pub fn scan(text_: &str, stem: &str) -> Scanned {
    let mut s = Scanned::default();
    let lines: Vec<&str> = text_.lines().collect();
    let stmts: Vec<Stmt> = match zeus_jcl::parse::parse(text_) {
        Ok(st) => {
            s.attrs.push(("parse_status", "OK".into()));
            st
        }
        Err(e) => {
            s.attrs.push(("parse_status", "ERROR".into()));
            s.attrs.push(("parse_message", e.to_string()));
            return s;
        }
    };
    let mut instream: Vec<String> = Vec::new();
    let mut steps: Vec<String> = Vec::new();
    let mut job = None;
    let mut proc_name = None;
    let mut first_op = true;
    // the program of the current step (for its in-stream SYSTSIN)
    let mut step: Option<(String, String)> = None;
    for st in &stmts {
        let op = st.op.to_ascii_uppercase();
        let at = |name: &str| locate(&lines, st.line, name);
        match op.as_str() {
            "JOB" => job = st.name.clone(),
            "PROC" => {
                if first_op {
                    proc_name = Some(st.name.clone().unwrap_or_else(|| stem.to_string()));
                } else if let Some(n) = &st.name {
                    instream.push(n.to_ascii_uppercase());
                }
            }
            "JCLLIB" => {
                if let Some(v) = keyword(&st.params, "ORDER") {
                    s.attrs.push(("jcllib", text(v)));
                }
            }
            "INCLUDE" => {
                if let Some(Value::Str(m)) = keyword(&st.params, "MEMBER") {
                    let (l, c) = at(m);
                    s.refs.push(Ref::new("JCL_INCLUDE", m, l, c));
                }
            }
            "EXEC" => {
                let name = st.name.clone().unwrap_or_default().to_ascii_uppercase();
                steps.push(name.clone());
                let pgm = keyword(&st.params, "PGM");
                let proc_ = keyword(&st.params, "PROC").or_else(|| {
                    st.params
                        .first()
                        .filter(|p| p.key.is_none())
                        .map(|p| &p.value)
                });
                let parm = keyword(&st.params, "PARM").map(text);
                if let Some(Value::Str(p)) = pgm {
                    let p = p.to_ascii_uppercase();
                    let (l, c) = at(&p);
                    let mut r = Ref::new("JCL_EXEC_PGM", &p, l, c).attr("steps", name.clone());
                    if p.starts_with('&') || p.starts_with('*') {
                        r.dynamic = true;
                        r.identifier = Some(p.clone());
                        r = r.attr("identifier", p.clone());
                    }
                    if let Some(parm) = &parm {
                        r = r.attr("parm", parm.clone());
                    }
                    s.refs.push(r);
                    // DFSRRC00 PARM='region,program,psb'
                    if p == "DFSRRC00"
                        && let Some(parm) = &parm
                    {
                        let parts: Vec<String> = parm
                            .split(',')
                            .map(|x| x.trim().trim_matches(['(', ')', '\'']).to_ascii_uppercase())
                            .collect();
                        let region = parts.first().cloned().unwrap_or_default();
                        if let Some(prog) = parts.get(1).filter(|x| !x.is_empty()) {
                            let (l, c) = at(prog);
                            s.refs.push(
                                Ref::new("JCL_IMS_PGM", prog, l, c)
                                    .attr("steps", name.clone())
                                    .attr("region", region.clone()),
                            );
                        }
                        if let Some(psb) = parts.get(2).filter(|x| !x.is_empty()) {
                            let (l, c) = at(psb);
                            s.refs.push(
                                Ref::new("JCL_IMS_PSB", psb, l, c)
                                    .attr("steps", name.clone())
                                    .attr("region", region.clone()),
                            );
                        }
                    }
                    step = Some((name.clone(), p));
                } else if let Some(Value::Str(p)) = proc_ {
                    let p = p.to_ascii_uppercase();
                    if !instream.contains(&p) {
                        let (l, c) = at(&p);
                        s.refs
                            .push(Ref::new("JCL_EXEC_PROC", &p, l, c).attr("steps", name.clone()));
                    }
                    step = None;
                }
            }
            "DD" => {
                let (Some((sname, pgm)), Some(data)) = (&step, &st.data) else {
                    continue;
                };
                let first = data_line(&lines, st.line, data);
                // DSN RUN PROGRAM(x) in the SYSTSIN of a TSO step
                if matches!(pgm.as_str(), "IKJEFT01" | "IKJEFT1A" | "IKJEFT1B") {
                    for (k, card) in data.iter().enumerate() {
                        let u = card.to_ascii_uppercase();
                        if let Some(p) = paren(&u, "PROGRAM") {
                            let (l, c) = locate(&lines, first + k, &p);
                            let mut r =
                                Ref::new("JCL_TSO_RUN", &p, l, c).attr("steps", sname.clone());
                            if let Some(plan) = paren(&u, "PLAN") {
                                r = r.attr("plan", plan);
                            }
                            s.refs.push(r);
                        }
                    }
                }
                // CSD definitions in the SYSIN of DFHCSDUP
                if pgm == "DFHCSDUP" {
                    let csd = super::misc::csd(&data.join("\n"));
                    for mut r in csd.refs {
                        r.line += first - 1;
                        s.refs.push(r.attr("steps", sname.clone()));
                    }
                    s.transactions.extend(csd.transactions);
                }
            }
            _ => {}
        }
        first_op = false;
    }
    let n = steps.len().to_string();
    if let Some(j) = job {
        s.name = Some(j.to_ascii_uppercase());
        s.attrs.push(("job_name", j.to_ascii_uppercase()));
    }
    if let Some(p) = proc_name {
        s.name = Some(p.to_ascii_uppercase());
        s.attrs.push(("proc_name", p.to_ascii_uppercase()));
    }
    s.attrs.push(("steps", n));
    s.attrs.push(("step_names", steps.join(" ")));
    if !instream.is_empty() {
        s.attrs.push(("instream_procs", instream.join(" ")));
    }
    s
}

/// The line of the first in-stream data card of the DD statement at
/// `line`.
fn data_line(lines: &[&str], line: usize, data: &[String]) -> usize {
    let first = data.first().map(|d| d.trim_end()).unwrap_or("");
    (line..lines.len().min(line + 12))
        .find(|&k| lines[k].trim_end().starts_with(first) && !lines[k].starts_with("//"))
        .map_or(line + 1, |k| k + 1)
}

/// The argument of `KEY(arg)` in a TSO command.
fn paren(u: &str, key: &str) -> Option<String> {
    let at = u.find(&format!("{key}("))?;
    let rest = &u[at + key.len() + 1..];
    let end = rest.find(')')?;
    let v = rest[..end].trim().trim_matches('\'').to_string();
    (!v.is_empty()).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_procs_and_programs() {
        let src = "//MYJOB    JOB (ACCT),'TEST'
//         JCLLIB ORDER=(MY.PROCLIB)
//LOCAL    PROC
//S1       EXEC PGM=INNERPGM
//         PEND
//STEP1    EXEC PGM=PROG1,PARM='/X'
//STEP2    EXEC MYPROC
//STEP3    EXEC LOCAL
//STEP4    EXEC PGM=IKJEFT01
//SYSTSIN  DD *
 DSN SYSTEM(DB2)
 RUN PROGRAM(DBPGM) PLAN(DBPLAN)
 END
/*
//STEP5    EXEC PGM=DFSRRC00,PARM='BMP,IMSPGM,IMSPSB'
//         INCLUDE MEMBER=COMMON
//CSD      EXEC PGM=DFHCSDUP
//SYSIN    DD *
 DEFINE TRANSACTION(TR01) GROUP(G1)
        PROGRAM(PROG1)
/*
";
        let s = scan(src, "MYJOB");
        assert_eq!(s.name.as_deref(), Some("MYJOB"));
        let refs: Vec<(&str, &str, usize, usize)> = s
            .refs
            .iter()
            .map(|r| (r.edge, r.target.as_str(), r.line, r.col))
            .collect();
        assert_eq!(
            refs,
            [
                ("JCL_EXEC_PGM", "INNERPGM", 4, 21),
                ("JCL_EXEC_PGM", "PROG1", 6, 21),
                ("JCL_EXEC_PROC", "MYPROC", 7, 17),
                ("JCL_EXEC_PGM", "IKJEFT01", 9, 21),
                ("JCL_TSO_RUN", "DBPGM", 12, 14),
                ("JCL_EXEC_PGM", "DFSRRC00", 15, 21),
                ("JCL_IMS_PGM", "IMSPGM", 15, 40),
                ("JCL_IMS_PSB", "IMSPSB", 15, 47),
                ("JCL_INCLUDE", "COMMON", 16, 27),
                ("JCL_EXEC_PGM", "DFHCSDUP", 17, 21),
                ("CSD_TRANSACTION", "PROG1", 20, 17),
            ]
        );
        assert_eq!(s.transactions, [("TR01".to_string(), "PROG1".to_string())]);
        assert!(s.attrs.contains(&("instream_procs", "LOCAL".into())));
        assert!(s.attrs.contains(&("steps", "7".into())));
    }
}

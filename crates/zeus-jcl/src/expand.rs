//! Job structure and procedure expansion.
//!
//! The statements of a job are grouped into steps. A step executing a
//! procedure (`EXEC PROC=name` or `EXEC name`) is replaced by the
//! procedure's steps, after symbolic parameter substitution: `&NAME` (a
//! following period is part of the reference) takes its value from the
//! EXEC statement, the PROC statement defaults or SET statements. DD
//! statements named `procstep.ddname` after the EXEC override (or add to)
//! the DD statements of that procedure step; overriding parameters
//! replace the ones with the same keyword, an empty value removes one.
//!
//! IF/THEN/ELSE/ENDIF constructs (in the job or in procedures) give each
//! step the conditions it runs under ([`Step::guards`]); INCLUDE
//! MEMBER=name statements are replaced by the member's statements, found
//! like procedures.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::cond;
use crate::parse::{self, JclError, Param, Stmt, Value};

/// EXEC keywords that are not symbolic parameters.
const EXEC_KEYWORDS: &[&str] = &[
    "PGM", "PROC", "PARM", "COND", "REGION", "TIME", "ACCT", "ADDRSPC", "DPRTY", "DYNAMNBR",
    "PERFORM", "RD", "MEMLIMIT", "CCSID", "TVSMSG", "TVSAMCOM",
];

#[derive(Debug, Clone)]
pub struct Step {
    /// Job step name (the EXEC statement of the job).
    pub name: String,
    /// Procedure step name when the step comes from a procedure.
    pub proc_step: Option<String>,
    pub exec: Stmt,
    pub dds: Vec<Stmt>,
    /// Enclosing IF statements (index into [`Job::ifs`]) and the branch
    /// (THEN: true, ELSE: false) the step is in.
    pub guards: Vec<(usize, bool)>,
}

#[derive(Debug, Clone, Default)]
pub struct Job {
    pub name: String,
    pub job: Option<Stmt>,
    /// JCLLIB ORDER: procedure libraries (data set names).
    pub jcllib: Vec<String>,
    /// JOBLIB DD statements.
    pub joblib: Vec<Stmt>,
    pub steps: Vec<Step>,
    /// Relational expressions of the IF statements.
    pub ifs: Vec<cond::Expr>,
}

/// Finds catalogued procedures: (procedure libraries of JCLLIB, name) ->
/// procedure text.
pub trait ProcLibrary {
    fn find(&self, jcllib: &[String], name: &str) -> Option<String>;
}

/// Replaces symbolic parameters in `s`.
pub fn substitute(s: &str, symbols: &HashMap<String, String>) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' {
            if b.get(i + 1) == Some(&b'&') {
                // &&name: temporary data set name
                out.push_str("&&");
                i += 2;
                continue;
            }
            let start = i + 1;
            let mut j = start;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b"@#$".contains(&b[j])) {
                j += 1;
            }
            let name = s[start..j].to_ascii_uppercase();
            if let Some(v) = symbols.get(&name) {
                out.push_str(v);
                if b.get(j) == Some(&b'.') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // copy up to the next '&' (whole UTF-8 characters)
        let next = s[i + 1..].find('&').map_or(s.len(), |p| i + 1 + p);
        out.push_str(&s[i..next]);
        i = next;
    }
    out
}

fn resubstitute(st: &Stmt, symbols: &HashMap<String, String>) -> Result<Stmt, JclError> {
    let operands = substitute(&st.operands, symbols);
    Ok(Stmt {
        params: parse::parse_operands(&operands, st.line)?,
        operands,
        ..st.clone()
    })
}

fn param_text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::List(l) => {
            let items: Vec<String> = l
                .iter()
                .map(|p| match &p.key {
                    Some(k) => format!("{k}={}", param_text(&p.value)),
                    None => param_text(&p.value),
                })
                .collect();
            format!("({})", items.join(","))
        }
    }
}

/// Applies an overriding DD statement to `dd`.
fn override_dd(dd: &mut Stmt, ov: &Stmt) {
    let mut params: Vec<Param> = dd.params.clone();
    // positional parameters (*, DATA, DUMMY) of the override replace the
    // original ones
    if ov.params.iter().any(|p| p.key.is_none()) {
        params.retain(|p| p.key.is_some());
    }
    for p in &ov.params {
        match &p.key {
            Some(k) => {
                params.retain(|q| q.key.as_deref() != Some(k.as_str()));
                if !matches!(&p.value, Value::Str(s) if s.is_empty()) {
                    params.push(p.clone());
                }
            }
            None => params.insert(0, p.clone()),
        }
    }
    dd.operands = params
        .iter()
        .map(|p| match &p.key {
            Some(k) => format!("{k}={}", param_text(&p.value)),
            None => param_text(&p.value),
        })
        .collect::<Vec<_>>()
        .join(",");
    dd.params = params;
    if ov.data.is_some() {
        dd.data = ov.data.clone();
    }
}

struct Expander<'a> {
    lib: &'a dyn ProcLibrary,
    instream: HashMap<String, Vec<Stmt>>,
    jcllib: Vec<String>,
    ifs: RefCell<Vec<cond::Expr>>,
}

/// IF/THEN/ELSE/ENDIF nesting while statements are read.
#[derive(Default)]
struct Nesting(Vec<(usize, bool)>);

impl Expander<'_> {
    fn proc_stmts(&self, name: &str, line: usize) -> Result<Vec<Stmt>, JclError> {
        if let Some(p) = self.instream.get(name) {
            return self.include(p.clone(), 0);
        }
        match self.lib.find(&self.jcllib, name) {
            Some(text) => self.include(parse::parse(&text)?, 0),
            None => Err(JclError {
                line,
                message: format!("procedure {name} not found"),
            }),
        }
    }

    /// Replaces the INCLUDE statements by the members' statements.
    fn include(&self, stmts: Vec<Stmt>, depth: usize) -> Result<Vec<Stmt>, JclError> {
        let mut out = Vec::new();
        for s in stmts {
            if s.op != "INCLUDE" {
                out.push(s);
                continue;
            }
            let fail = |m: String| JclError {
                line: s.line,
                message: m,
            };
            let member = s
                .get_str("MEMBER")
                .ok_or_else(|| fail("INCLUDE without MEMBER".into()))?
                .to_ascii_uppercase();
            if depth > 15 {
                return Err(fail("INCLUDE groups nested too deeply".into()));
            }
            let text = self
                .lib
                .find(&self.jcllib, &member)
                .ok_or_else(|| fail(format!("INCLUDE group {member} not found")))?;
            out.extend(self.include(parse::parse(&text)?, depth + 1)?);
        }
        Ok(out)
    }

    /// Handles an IF, ELSE or ENDIF statement; false for other statements.
    fn nesting(
        &self,
        s: &Stmt,
        n: &mut Nesting,
        symbols: &HashMap<String, String>,
    ) -> Result<bool, JclError> {
        let fail = |m: &str| JclError {
            line: s.line,
            message: m.into(),
        };
        match s.op.as_str() {
            "IF" => {
                let e = cond::parse(&substitute(&s.operands, symbols)).map_err(|m| fail(&m))?;
                let mut ifs = self.ifs.borrow_mut();
                ifs.push(e);
                n.0.push((ifs.len() - 1, true));
            }
            "ELSE" => match n.0.last_mut() {
                Some((_, b @ true)) => *b = false,
                _ => return Err(fail("ELSE without IF")),
            },
            "ENDIF" => {
                n.0.pop().ok_or_else(|| fail("ENDIF without IF"))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Expands one EXEC statement with its DD statements into steps.
    fn expand(
        &self,
        exec: &Stmt,
        dds: &[Stmt],
        symbols: &HashMap<String, String>,
        depth: usize,
    ) -> Result<Vec<Step>, JclError> {
        let exec = resubstitute(exec, symbols)?;
        let step_name = exec.name.clone().unwrap_or_default();
        let proc_name = match exec.get_str("PROC") {
            Some(p) => Some(p.to_ascii_uppercase()),
            None if exec.get("PGM").is_none() => exec
                .positional(0)
                .and_then(Value::as_str)
                .map(str::to_ascii_uppercase),
            None => None,
        };
        let Some(proc_name) = proc_name else {
            let dds = dds
                .iter()
                .map(|d| resubstitute(d, symbols))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(vec![Step {
                name: step_name,
                proc_step: None,
                exec,
                dds,
                guards: Vec::new(),
            }]);
        };
        if depth > 15 {
            return Err(JclError {
                line: exec.line,
                message: "procedures nested too deeply".into(),
            });
        }
        let body = self.proc_stmts(&proc_name, exec.line)?;
        // symbols: PROC defaults, then the EXEC overrides
        let mut syms = symbols.clone();
        if let Some(p) = body.iter().find(|s| s.op == "PROC") {
            for prm in &p.params {
                if let Some(k) = &prm.key {
                    syms.insert(k.clone(), param_text(&prm.value));
                }
            }
        }
        for prm in &exec.params {
            if let Some(k) = &prm.key
                && !EXEC_KEYWORDS.contains(&k.split('.').next().unwrap_or(""))
            {
                syms.insert(k.clone(), param_text(&prm.value));
            }
        }
        // the procedure's steps
        let mut out = Vec::new();
        let mut i = 0;
        let stmts: Vec<&Stmt> = body
            .iter()
            .filter(|s| s.op != "PROC" && s.op != "PEND")
            .collect();
        let mut nest = Nesting::default();
        while i < stmts.len() {
            let s = stmts[i];
            i += 1;
            if self.nesting(s, &mut nest, &syms)? || s.op != "EXEC" {
                continue;
            }
            let mut step_dds = Vec::new();
            while i < stmts.len()
                && !matches!(stmts[i].op.as_str(), "EXEC" | "IF" | "ELSE" | "ENDIF")
            {
                if stmts[i].op == "DD" {
                    step_dds.push(stmts[i].clone());
                }
                i += 1;
            }
            let pstep = s.name.clone().unwrap_or_default();
            // overrides for this procedure step, by name
            let mut sub = self.expand(s, &step_dds, &syms, depth + 1)?;
            for st in &mut sub {
                let target = st.proc_step.clone().unwrap_or_else(|| st.name.clone());
                for ov in dds {
                    let Some(n) = &ov.name else { continue };
                    let Some((ps, dd)) = n.split_once('.') else {
                        continue;
                    };
                    if ps != pstep && ps != target {
                        continue;
                    }
                    let ov = resubstitute(ov, symbols)?;
                    match st.dds.iter_mut().find(|d| d.name.as_deref() == Some(dd)) {
                        Some(d) => override_dd(d, &ov),
                        None => {
                            let mut add = ov.clone();
                            add.name = Some(dd.to_string());
                            st.dds.push(add);
                        }
                    }
                }
                // EXEC overrides: PARM.procstep / COND.procstep
                for prm in &exec.params {
                    let Some(k) = &prm.key else { continue };
                    if let Some((kw, ps)) = k.split_once('.')
                        && (ps == pstep || ps == target)
                        && EXEC_KEYWORDS.contains(&kw)
                    {
                        st.exec.params.retain(|q| q.key.as_deref() != Some(kw));
                        st.exec.params.push(Param {
                            key: Some(kw.to_string()),
                            value: prm.value.clone(),
                        });
                    }
                }
                if st.proc_step.is_none() {
                    st.proc_step = Some(st.name.clone());
                }
                st.guards.splice(0..0, nest.0.iter().copied());
                st.name = step_name.clone();
            }
            out.extend(sub);
        }
        Ok(out)
    }
}

/// Builds the job from its statements, expanding procedures.
pub fn build_job(stmts: &[Stmt], lib: &dyn ProcLibrary) -> Result<Job, JclError> {
    let mut job = Job::default();
    let mut symbols: HashMap<String, String> = HashMap::new();
    symbols.insert(
        "SYSUID".into(),
        std::env::var("USER")
            .unwrap_or_else(|_| "ZEUS".into())
            .to_ascii_uppercase(),
    );
    let mut ex = Expander {
        lib,
        instream: HashMap::new(),
        jcllib: Vec::new(),
        ifs: RefCell::new(Vec::new()),
    };
    let mut stmts = stmts.to_vec();
    let mut nest = Nesting::default();
    let mut i = 0;
    while i < stmts.len() {
        let s = stmts[i].clone();
        let s = &s;
        i += 1;
        if s.op == "INCLUDE" {
            let body = ex.include(vec![s.clone()], 0)?;
            stmts.splice(i - 1..i, body);
            i -= 1;
            continue;
        }
        if ex.nesting(s, &mut nest, &symbols)? {
            continue;
        }
        match s.op.as_str() {
            "JOB" => {
                job.name = s.name.clone().unwrap_or_default();
                job.job = Some(s.clone());
            }
            "JCLLIB" => {
                if let Some(v) = s.get("ORDER") {
                    ex.jcllib = v
                        .items()
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(str::to_ascii_uppercase)
                        .collect();
                }
            }
            "SET" => {
                for p in &s.params {
                    if let Some(k) = &p.key {
                        // the value may name symbols set before
                        let v = substitute(&param_text(&p.value), &symbols);
                        symbols.insert(k.clone(), v);
                    }
                }
            }
            "PROC" => {
                // in-stream procedure up to PEND
                let name = s.name.clone().unwrap_or_default();
                let mut body = vec![s.clone()];
                while i < stmts.len() && stmts[i].op != "PEND" {
                    body.push(stmts[i].clone());
                    i += 1;
                }
                i += 1;
                ex.instream.insert(name, body);
            }
            "DD" if s.name.as_deref() == Some("JOBLIB") => {
                // with its concatenated (unnamed) DD statements
                job.joblib.push(resubstitute(s, &symbols)?);
                while i < stmts.len() && stmts[i].op == "DD" && stmts[i].name.is_none() {
                    job.joblib.push(resubstitute(&stmts[i], &symbols)?);
                    i += 1;
                }
            }
            "EXEC" => {
                let mut dds = Vec::new();
                while i < stmts.len() && stmts[i].op == "DD" {
                    dds.push(stmts[i].clone());
                    i += 1;
                }
                let mut steps = ex.expand(s, &dds, &symbols, 0)?;
                for st in &mut steps {
                    st.guards.splice(0..0, nest.0.iter().copied());
                }
                job.steps.extend(steps);
            }
            _ => {}
        }
    }
    if !nest.0.is_empty() {
        return Err(JclError {
            line: stmts.last().map_or(0, |s| s.line),
            message: "IF without ENDIF".into(),
        });
    }
    job.jcllib = ex.jcllib;
    job.ifs = ex.ifs.into_inner();
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lib;
    impl ProcLibrary for Lib {
        fn find(&self, _: &[String], name: &str) -> Option<String> {
            (name == "REPROC").then(|| {
                "//REPROC PROC\n\
                 //PRC001 EXEC PGM=IDCAMS\n\
                 //FILEIN  DD DISP=SHR,DSN=NULLFILE\n\
                 //FILEOUT DD DISP=SHR,DSN=NULLFILE\n\
                 //SYSIN   DD DISP=SHR,DSN=&CNTLLIB(REPROCT)\n\
                 // PEND\n"
                    .to_string()
            })
        }
    }

    #[test]
    fn procedure_with_overrides() {
        let jcl = "\
//J JOB
//STEP05R EXEC PROC=REPROC,
// CNTLLIB=AWS.CNTL
//PRC001.FILEIN  DD DISP=SHR,
//        DSN=AWS.KSDS
//PRC001.FILEOUT DD DISP=(NEW,CATLG,DELETE),DSN=AWS.BKUP(+1)
//S2 EXEC PGM=IEFBR14
";
        let job = build_job(&parse::parse(jcl).unwrap(), &Lib).unwrap();
        assert_eq!(job.steps.len(), 2);
        let s = &job.steps[0];
        assert_eq!(s.name, "STEP05R");
        assert_eq!(s.proc_step.as_deref(), Some("PRC001"));
        assert_eq!(s.exec.get_str("PGM"), Some("IDCAMS"));
        let dd = |n: &str| s.dds.iter().find(|d| d.name.as_deref() == Some(n)).unwrap();
        assert_eq!(dd("FILEIN").get_str("DSN"), Some("AWS.KSDS"));
        assert_eq!(dd("FILEOUT").get_str("DSN"), Some("AWS.BKUP(+1)"));
        assert_eq!(dd("FILEOUT").get("DISP").unwrap().items().len(), 3);
        assert_eq!(dd("SYSIN").get_str("DSN"), Some("AWS.CNTL(REPROCT)"));
    }

    #[test]
    fn symbols_and_instream_proc() {
        let jcl = "\
//J JOB
// SET HLQ=MY
//P PROC X=A
//S EXEC PGM=&X
//D DD DSN=&HLQ..DATA
// PEND
//RUN EXEC P,X=B
";
        let job = build_job(&parse::parse(jcl).unwrap(), &Lib).unwrap();
        assert_eq!(job.steps[0].exec.get_str("PGM"), Some("B"));
        assert_eq!(job.steps[0].dds[0].get_str("DSN"), Some("MY.DATA"));
    }
}

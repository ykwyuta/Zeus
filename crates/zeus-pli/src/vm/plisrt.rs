//! PLISRTA, PLISRTB, PLISRTC and PLISRTD: the Sort program called from
//! PL/I (Programming Guide, "Using the Sort program").
//!
//! A: data set SORTIN to data set SORTOUT; B: input routine (E15) to
//! SORTOUT; C: SORTIN to output routine (E35); D: routine to routine. The
//! SORT statement gives the fields (FIELDS=(start,length,form,seq,...) or
//! FIELDS=(start,length,seq,...),FORMAT=form); the RECORD statement gives
//! TYPE=F or V and LENGTH=(l1). Zeus sorts the forms CH, BI, FI, PD and
//! ZD and refuses the others rather than ordering them as characters. The
//! sort is stable (records with equal keys keep their order, as with the
//! EQUALS option; without it the Programming Guide leaves the order open).
//!
//! E15 is called until it sets return code 8 with PLIRETC: 12 passes the
//! record it returns, 16 ends the sort with return code 16. E35 gets each
//! record as its argument and sets 4 to continue or 16 to stop. The return
//! code field is reset to zero after each call of a routine.

use super::*;
use zeus_ds::format::RecordFormat;

#[derive(Clone, Copy, PartialEq)]
enum Form {
    Ch,
    Bi,
    Fi,
    Pd,
    Zd,
}

struct Key {
    start: usize,
    len: usize,
    form: Form,
    descending: bool,
}

fn form(s: &str) -> Result<Form, String> {
    Ok(match s {
        "CH" => Form::Ch,
        "BI" => Form::Bi,
        "FI" => Form::Fi,
        "PD" => Form::Pd,
        "ZD" => Form::Zd,
        _ => return Err(format!("PLISRT: the sort field form {s} is not supported")),
    })
}

/// The text inside the parentheses after `key=`.
fn operand<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let at = s.find(key)? + key.len();
    let rest = &s[at..];
    if let Some(r) = rest.strip_prefix('(') {
        let end = r.find(')')?;
        Some(&r[..end])
    } else {
        Some(rest.split([',', ' ']).next().unwrap_or(""))
    }
}

fn keys(stmt: &str) -> Result<Vec<Key>, String> {
    let u = stmt.trim().to_ascii_uppercase();
    if !u.starts_with("SORT ") {
        return Err(format!("PLISRT: not a SORT statement: {stmt}"));
    }
    let fields = operand(&u, "FIELDS=").ok_or("PLISRT: SORT needs FIELDS=")?;
    let format = operand(&u, "FORMAT=");
    let parts: Vec<&str> = fields.split(',').map(str::trim).collect();
    let step = if format.is_some() { 3 } else { 4 };
    if parts.is_empty() || parts.len() % step != 0 {
        return Err(format!("PLISRT: bad FIELDS: {fields}"));
    }
    let mut out = Vec::new();
    for c in parts.chunks(step) {
        let start: usize = c[0].parse().map_err(|_| format!("PLISRT: bad start {}", c[0]))?;
        let len: usize = c[1].parse().map_err(|_| format!("PLISRT: bad length {}", c[1]))?;
        let (f, seq) = match format {
            Some(f) => (f, c[2]),
            None => (c[2], c[3]),
        };
        if start == 0 || len == 0 {
            return Err(format!("PLISRT: bad field {fields}"));
        }
        out.push(Key {
            start,
            len,
            form: form(f)?,
            descending: match seq {
                "A" => false,
                "D" => true,
                _ => return Err(format!("PLISRT: bad sequence {seq}")),
            },
        });
    }
    Ok(out)
}

/// TYPE=F or V and the record length.
fn record(stmt: &str) -> Result<(bool, usize), String> {
    let u = stmt.trim().to_ascii_uppercase();
    if !u.starts_with("RECORD ") {
        return Err(format!("PLISRT: not a RECORD statement: {stmt}"));
    }
    let var = match operand(&u, "TYPE=") {
        Some("F") => false,
        Some("V") | Some("D") => true,
        t => return Err(format!("PLISRT: bad record type {t:?}")),
    };
    let len = operand(&u, "LENGTH=")
        .and_then(|l| l.split(',').next())
        .and_then(|l| l.trim().parse().ok())
        .unwrap_or(0);
    Ok((var, len))
}

/// The value of a numeric field for ordering.
fn number(form: Form, b: &[u8]) -> i128 {
    match form {
        Form::Fi => {
            let mut v: i128 = if b.first().is_some_and(|&c| c & 0x80 != 0) { -1 } else { 0 };
            for &c in b {
                v = (v << 8) | i128::from(c);
            }
            v
        }
        Form::Pd => {
            let mut v: i128 = 0;
            for (k, &c) in b.iter().enumerate() {
                v = v * 10 + i128::from(c >> 4);
                if k + 1 < b.len() {
                    v = v * 10 + i128::from(c & 0x0F);
                }
            }
            match b.last().map(|c| c & 0x0F) {
                Some(0x0B | 0x0D) => -v,
                _ => v,
            }
        }
        Form::Zd => {
            let v = b.iter().fold(0i128, |v, &c| v * 10 + i128::from(c & 0x0F));
            match b.last().map(|c| c >> 4) {
                Some(0x0B | 0x0D) => -v,
                _ => v,
            }
        }
        _ => 0,
    }
}

fn compare(keys: &[Key], a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    for k in keys {
        let field = |r: &[u8]| {
            let s = (k.start - 1).min(r.len());
            let e = (k.start - 1 + k.len).min(r.len());
            r[s..e].to_vec()
        };
        let (x, y) = (field(a), field(b));
        let o = match k.form {
            Form::Ch | Form::Bi => x.cmp(&y),
            f => number(f, &x).cmp(&number(f, &y)),
        };
        let o = if k.descending { o.reverse() } else { o };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    std::cmp::Ordering::Equal
}

impl Machine {
    fn sort_dd(&self, dd: &str) -> Option<DdInfo> {
        self.env.dds.get(dd).cloned().or_else(|| {
            std::env::var_os(format!("DD_{dd}")).map(|path| DdInfo {
                path: path.into(),
                format: None,
                keys: None,
                sysout: false,
                relative: false,
            })
        })
    }

    pub(super) fn plisrt(
        &mut self,
        sort: &Expr,
        record_stmt: &Expr,
        rc: &Target,
        input: Option<&Expr>,
        output: Option<&Expr>,
        act: usize,
    ) -> R<()> {
        let text = |v: Value| match v {
            Value::Char(b) => zeus_cobol::ebcdic::decode(&b),
            _ => std::string::String::new(),
        };
        let sort_text = text(self.eval(sort, act)?);
        let record_text = text(self.eval(record_stmt, act)?);
        let keys = keys(&sort_text).or_else(err)?;
        let (var, len) = record(&record_text).or_else(err)?;
        let fmt = if var {
            RecordFormat::Variable
        } else {
            RecordFormat::Fixed(len as u32)
        };
        let mut failed = false;
        // the input: SORTIN or the E15 routine
        let mut recs: Vec<Vec<u8>> = Vec::new();
        match input {
            None => {
                let Some(dd) = self.sort_dd("SORTIN") else {
                    return err("PLISRT: no SORTIN data set");
                };
                let data = std::fs::read(&dd.path).unwrap_or_default();
                recs = match zeus_ds::format::decode(dd.format.unwrap_or(fmt), &data) {
                    Ok(r) => r,
                    Err(e) => return err(format!("PLISRT: SORTIN: {e}")),
                };
            }
            Some(e15) => loop {
                let callee = Callee::Var(e15.clone());
                let v = self.call(&callee, &[], act, None)?;
                let code = std::mem::replace(&mut self.rc, 0);
                match code {
                    12 => {
                        let mut r = match v {
                            Some(Value::Char(b)) => b,
                            _ => Vec::new(),
                        };
                        // F records: padded with blanks to the length
                        if !var && len > 0 {
                            r.resize(len, SPACE);
                        }
                        recs.push(r);
                    }
                    8 => break,
                    16 => {
                        failed = true;
                        break;
                    }
                    c => return err(format!("PLISRT: return code {c} from the input routine")),
                }
            },
        }
        // the keys of V records count the 4-byte length prefix
        let prefixed = |r: &[u8]| {
            if var {
                let mut p = ((r.len() + 4) as u16).to_be_bytes().to_vec();
                p.extend([0, 0]);
                p.extend_from_slice(r);
                p
            } else {
                r.to_vec()
            }
        };
        if !failed {
            recs.sort_by(|a, b| compare(&keys, &prefixed(a), &prefixed(b)));
            match output {
                None => {
                    let Some(dd) = self.sort_dd("SORTOUT") else {
                        return err("PLISRT: no SORTOUT data set");
                    };
                    let mut out = Vec::new();
                    for r in &recs {
                        zeus_ds::format::encode_into(dd.format.unwrap_or(fmt), r, SPACE, &mut out);
                    }
                    if let Err(e) = std::fs::write(&dd.path, out) {
                        return err(format!("PLISRT: SORTOUT: {e}"));
                    }
                }
                Some(e35) => {
                    for r in recs {
                        let arg = crate::ir::Arg::Dummy(crate::sema::expr::mk(
                            Ek::Char(r.clone()),
                            Ty::char(r.len() as u32),
                        ));
                        let callee = Callee::Var(e35.clone());
                        self.call(&callee, &[arg], act, None)?;
                        match std::mem::replace(&mut self.rc, 0) {
                            4 => {}
                            16 => {
                                failed = true;
                                break;
                            }
                            c => {
                                return err(format!(
                                    "PLISRT: return code {c} from the output routine"
                                ));
                            }
                        }
                    }
                }
            }
        }
        let code = Value::int(if failed { 16 } else { 0 });
        let l = self.target_loc(rc, act)?;
        self.assign_target(rc, l, code, &Ty::fixed_bin(31, 0), act)
    }
}

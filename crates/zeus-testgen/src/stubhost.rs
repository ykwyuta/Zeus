//! EXEC CICS / SQL / DLI in test runs: a host that runs no transaction
//! or database but takes what the statements give the program as
//! inputs. The `n`-th execution of the statement at line `l` of program
//! `P` sets its output options from `outputs["P.l.n.OPTION"]` (RESP,
//! INTO, SQLCODE, host variables, ...; unset: RESP and SQLCODE 0, the
//! areas as they are); in concolic runs they follow the variables
//! `X<P>.<l>.<n>.<OPTION>` (numeric) or `...#<i>` (bytes). What the
//! statements send out is logged (the expected results).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use zeus_cobol::concolic::VarKind;
use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall, ExecHost, ExecOutcome};
use zeus_cobol::vm::{Machine, RResult, RunError, data as vdata};
use zeus_cobol::zir::{DataType, ExecKind, NumType, NumUsage, SignPos};
use zeus_decimal::Rounding;

/// What an output option is set to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Out {
    /// A number (the integer of its digits).
    Num(i128),
    /// The leading bytes of the area (hexadecimal in case files).
    Hex(String),
}

/// The bytes of an area that follow inputs (at most).
pub const AREA_BYTES: usize = 1024;

/// CICS options whose areas the command sets.
const CICS_OUT: &[&str] = &[
    "INTO", "RESP", "RESP2", "ABSTIME", "YYYYMMDD", "YYMMDD", "MMDDYYYY", "DDMMYYYY", "DATE",
    "TIME", "USERID", "APPLID", "SYSID", "NETNAME", "OPID", "ITEM", "NUMITEMS", "TOKEN",
    "COMMAREA", "LENGTH", "CURSOR", "PROGRAM", "TERMID",
];

/// Commands for which an option in `CICS_OUT` is only sent.
fn sends(command: &str, option: &str) -> bool {
    match option {
        "COMMAREA" => command != "LINK",
        "LENGTH" => !matches!(
            command,
            "READ" | "READNEXT" | "READPREV" | "READQ" | "RECEIVE"
        ),
        "PROGRAM" | "TERMID" | "USERID" => command != "ASSIGN",
        _ => false,
    }
}

#[derive(Default)]
pub struct StubHost {
    pub outputs: BTreeMap<String, Out>,
    /// The statements executed with what they sent out.
    pub log: RefCell<Vec<String>>,
    counts: RefCell<HashMap<(String, u32), u32>>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .filter_map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

fn raw(t: &NumType, bytes: &[u8]) -> i128 {
    vdata::num_value(t, bytes)
        .rescale(t.scale.max(0) as u32, Rounding::Truncate)
        .map_or(0, |d| d.coef)
}

const FULLWORD: NumType = NumType {
    usage: NumUsage::Binary,
    digits: 9,
    scale: 0,
    signed: true,
    sign: SignPos::None,
};

impl StubHost {
    pub fn new(outputs: BTreeMap<String, Out>) -> Self {
        StubHost {
            outputs,
            ..Default::default()
        }
    }

    /// Sets an output area (from `outputs`, else `default` for numbers)
    /// and makes it follow its variables.
    fn output(
        &self,
        m: &mut Machine,
        key: &str,
        addr: usize,
        len: usize,
        num: Option<NumType>,
        default: Option<i128>,
    ) {
        let num = num.filter(|t| t.digits <= 18 && t.scale >= 0);
        match (self.outputs.get(key), &num) {
            (Some(Out::Num(x)), Some(t)) => {
                vdata::store_raw(t, &mut m.storage_mut()[addr..addr + len], *x);
            }
            (Some(Out::Hex(h)), _) => {
                let b = unhex(h);
                let l = b.len().min(len);
                m.storage_mut()[addr..addr + l].copy_from_slice(&b[..l]);
            }
            (None, Some(t)) => {
                if let Some(x) = default {
                    vdata::store_raw(t, &mut m.storage_mut()[addr..addr + len], x);
                }
            }
            _ => {}
        }
        if m.symbolic().is_none() {
            return;
        }
        let l = len.min(AREA_BYTES);
        m.sym_forget_area(addr, l);
        match num {
            Some(t) => {
                let value = raw(&t, &m.storage()[addr..addr + len]);
                let st = m.symbolic().unwrap();
                let v = st.var_named(
                    format!("X{key}"),
                    VarKind::Num {
                        digits: t.digits,
                        signed: t.signed,
                    },
                    value,
                );
                m.sym_bind_num(addr, len, t, v);
            }
            None => {
                for i in 0..l {
                    let b = m.storage()[addr + i];
                    let st = m.symbolic().unwrap();
                    let v = st.var_named(format!("X{key}#{}", i + 1), VarKind::Byte, i128::from(b));
                    m.sym_bind_byte(addr + i, v);
                }
            }
        }
    }

    fn area_num(a: &Area) -> Option<NumType> {
        match &a.ty {
            DataType::Numeric(t) => Some(*t),
            _ => None,
        }
    }

    fn cics(&self, m: &mut Machine, call: &ExecCall, key: &str) -> RResult<ExecOutcome> {
        let cmd = call.command.as_str();
        let mut sent = Vec::new();
        for (name, arg) in &call.options {
            match arg {
                Some(ExecArg::Area(a))
                    if CICS_OUT.contains(&name.as_str()) && !sends(cmd, name) =>
                {
                    let default = matches!(name.as_str(), "RESP" | "RESP2").then_some(0);
                    self.output(
                        m,
                        &format!("{key}.{name}"),
                        a.addr,
                        a.len,
                        Self::area_num(a),
                        default,
                    );
                }
                Some(ExecArg::Area(a)) => sent.push(format!(
                    "{name}={}",
                    hex(&m.storage()[a.addr..a.addr + a.len.min(AREA_BYTES)])
                )),
                Some(ExecArg::Alnum(b)) => sent.push(format!(
                    "{name}('{}')",
                    zeus_cobol::ebcdic::decode(b).trim_end()
                )),
                Some(ExecArg::Num(d)) => sent.push(format!("{name}({d})")),
                Some(ExecArg::Name(n)) => sent.push(format!("{name}({n})")),
                _ => sent.push(name.clone()),
            }
        }
        self.log.borrow_mut().push(
            format!("CICS {cmd} {}", sent.join(" "))
                .trim_end()
                .to_string(),
        );
        match cmd {
            "RETURN" | "XCTL" => Ok(ExecOutcome::Return),
            "ABEND" => {
                let code = match call.arg("ABCODE") {
                    Some(ExecArg::Alnum(b)) => zeus_cobol::ebcdic::decode(b),
                    _ => String::new(),
                };
                Err(RunError(format!("CICS abend {}", code.trim())))
            }
            _ => Ok(ExecOutcome::Continue),
        }
    }

    fn sql(&self, m: &mut Machine, call: &ExecCall, key: &str) -> RResult<ExecOutcome> {
        let text = call.sql.clone().unwrap_or_default();
        // the host variables after INTO (up to FROM) are set; the others sent
        let up = text.to_ascii_uppercase();
        let marks: Vec<usize> = up.match_indices('?').map(|(i, _)| i).collect();
        let into = up.find(" INTO ");
        let from = into.and_then(|i| up[i..].find(" FROM ").map(|f| f + i));
        let mut sent = Vec::new();
        for (k, (v, ind)) in call.host.iter().enumerate() {
            let at = marks.get(k).copied().unwrap_or(0);
            let out = into.is_some_and(|i| at > i && from.is_none_or(|f| at < f));
            if out {
                self.output(
                    m,
                    &format!("{key}.H{}", k + 1),
                    v.addr,
                    v.len,
                    Self::area_num(v),
                    None,
                );
                if let Some(i) = ind {
                    let n = i.len.min(2);
                    m.storage_mut()[i.addr..i.addr + n].fill(0);
                }
            } else {
                sent.push(hex(&m.storage()[v.addr..v.addr + v.len.min(AREA_BYTES)]));
            }
        }
        if let Some(ExecArg::Area(a)) = call.arg("SQLCA")
            && a.len >= 16
        {
            self.output(
                m,
                &format!("{key}.SQLCODE"),
                a.addr + 12,
                4,
                Some(FULLWORD),
                Some(0),
            );
        }
        self.log.borrow_mut().push(
            format!("SQL {} {}", text.trim(), sent.join(" "))
                .trim_end()
                .to_string(),
        );
        Ok(ExecOutcome::Continue)
    }
}

impl ExecHost for StubHost {
    fn exec(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let n = {
            let mut c = self.counts.borrow_mut();
            let n = c.entry((call.program.clone(), call.line)).or_insert(0);
            *n += 1;
            *n
        };
        let key = format!("{}.{}.{n}", call.program, call.line);
        match call.kind {
            ExecKind::Cics => self.cics(m, call, &key),
            ExecKind::Sql | ExecKind::SqlIms => self.sql(m, call, &key),
            ExecKind::Dli => {
                self.log.borrow_mut().push(format!("DLI {}", call.command));
                Ok(ExecOutcome::Continue)
            }
        }
    }
}

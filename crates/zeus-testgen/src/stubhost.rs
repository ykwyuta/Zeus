//! EXEC CICS / SQL / DLI in test runs: a host that runs no transaction
//! or database but takes what the statements give the program as
//! inputs. The `n`-th execution of the statement at line `l` of program
//! `P` sets its output options from `outputs["P.l.n.OPTION"]` (RESP,
//! INTO, SQLCODE, host variables `H<k>` and their indicators `I<k>`,
//! ...); a group area has a key per elementary item
//! (`P.l.n.INTO.ITEM`). Unset: RESP and SQLCODE are 0 (100, ENDFILE,
//! ITEMERR / QZERO, ENDDATA and NOTFND from the second execution of a SQL
//! FETCH, READNEXT, READQ, RETRIEVE and FETCH ANY on, so that loops end),
//! the areas stay as they are. In concolic runs the
//! outputs follow the variables `X<key>` (numeric) or `X<key>#<i>`
//! (bytes). What the statements send out is logged (the expected
//! results).
//!
//! Conditions of CICS commands without RESP go to the HANDLE CONDITION
//! label (or are ignored with IGNORE CONDITION or NOHANDLE); otherwise the
//! run ends with the condition. EIBRESP is set when the program has an
//! EIB. SQL statements go to their WHENEVER labels. With a HANDLE ABEND
//! LABEL, EXEC CICS ABEND and the abends a command may have
//! (`outputs["P.l.n.ABEND"]`) go to the label once.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use zeus_cobol::concolic::VarKind;
use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall, ExecHost, ExecOutcome};
use zeus_cobol::vm::{Machine, RResult, RunError, data as vdata};
use zeus_cobol::zir::{DataType, ExecKind, ExecVal, Loc, Module, NumType, NumUsage, Rel, SignPos};
use zeus_decimal::Rounding;

use crate::engine::items;

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
    "INTO",
    "RESP",
    "RESP2",
    "ABSTIME",
    "YYYYMMDD",
    "YYMMDD",
    "MMDDYYYY",
    "DDMMYYYY",
    "DATE",
    "TIME",
    "USERID",
    "APPLID",
    "SYSID",
    "NETNAME",
    "OPID",
    "ITEM",
    "NUMITEMS",
    "TOKEN",
    "COMMAREA",
    "LENGTH",
    "CURSOR",
    "PROGRAM",
    "TERMID",
    "COMPSTATUS",
    "ABCODE",
    "ANY",
    "CHILD",
    "CHANNEL",
    "FLENGTH",
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
        // the child a RUN starts, what FETCH ANY gets back, the length
        // GET CONTAINER gives
        "CHILD" => command != "RUN",
        "CHANNEL" => command != "FETCH",
        "ABCODE" => !matches!(command, "FETCH" | "ASSIGN"),
        "FLENGTH" => command != "GET",
        _ => false,
    }
}

/// CICS conditions (by EIBRESP) for HANDLE CONDITION.
const CONDITIONS: &[&str] = &[
    "ERROR",
    "EOF",
    "ENDINPT",
    "NOTFND",
    "DUPREC",
    "DUPKEY",
    "INVREQ",
    "IOERR",
    "NOSPACE",
    "NOTOPEN",
    "ENDFILE",
    "ILLOGIC",
    "LENGERR",
    "QZERO",
    "SIGNAL",
    "QBUSY",
    "ITEMERR",
    "PGMIDERR",
    "TRANSIDERR",
    "ENDDATA",
    "EXPIRED",
    "MAPFAIL",
    "QIDERR",
    "ENQBUSY",
    "DISABLED",
    "NOTAUTH",
    "USERIDERR",
    "FILENOTFOUND",
    "SYSIDERR",
    "TERMIDERR",
    "TSIOERR",
    "CHANNELERR",
    "CONTAINERERR",
];

fn condition(resp: i128) -> String {
    CONDITIONS
        .iter()
        .find(|c| zeus_cobol::front::cics::resp_value(c).map(i128::from) == Some(resp))
        .map_or_else(|| format!("RESP {resp}"), |c| c.to_string())
}

/// EIBRESP in DFHEIBLK.
const EIBRESP: usize = 76;

/// An elementary item of an output area: name (empty for the area
/// itself), offset in the area, length and numeric type.
type Item = (String, usize, usize, Option<NumType>);

#[derive(Default)]
pub struct StubHost {
    pub outputs: BTreeMap<String, Out>,
    /// The statements executed with what they sent out.
    pub log: RefCell<Vec<String>>,
    /// The module of the program run (for the items of group areas).
    pub module: Option<Rc<Module>>,
    /// The address of the EIB of the program run.
    pub eib: Option<usize>,
    counts: RefCell<HashMap<(String, u32), u32>>,
    /// HANDLE CONDITION: (program instance, condition) -> label (None:
    /// IGNORE CONDITION).
    handlers: RefCell<HashMap<(usize, String), Option<u32>>>,
    /// HANDLE ABEND LABEL: program instance -> label (taken once: CICS
    /// cancels the exit when it gets control).
    abend_exits: RefCell<HashMap<usize, u32>>,
    /// The code of the abend that went to an exit (ASSIGN ABCODE).
    abcode: RefCell<Option<String>>,
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

/// EIBRESP: PIC S9(8) COMP.
const EIBRESP_TYPE: NumType = NumType {
    usage: NumUsage::Binary,
    digits: 8,
    scale: 0,
    signed: true,
    sign: SignPos::None,
};

/// SQLCODE of the SQLCA Zeus includes: PIC S9(9) COMP-5 at offset 12.
const SQLCODE_TYPE: NumType = NumType {
    usage: NumUsage::Native,
    digits: 9,
    scale: 0,
    signed: true,
    sign: SignPos::None,
};

/// A numeric type a variable can follow.
fn followed(t: Option<NumType>) -> Option<NumType> {
    t.filter(|t| t.digits <= 18 && t.scale >= 0 && t.usage != NumUsage::National)
}

/// Where an operand of an EXEC statement comes from in its definition.
enum Operand {
    Option(usize),
    Host(usize),
}

/// The position of a SQL keyword outside quotes and parentheses.
fn find_kw(sql: &str, kw: &str, from: usize) -> Option<usize> {
    let up = sql.to_ascii_uppercase();
    let b = up.as_bytes();
    let (mut q, mut depth) = (false, 0i32);
    for i in 0..b.len() {
        match b[i] {
            b'\'' => q = !q,
            b'(' if !q => depth += 1,
            b')' if !q => depth -= 1,
            _ if !q && depth == 0 && i >= from && up[i..].starts_with(kw) => {
                let before = i == 0 || !b[i - 1].is_ascii_alphanumeric();
                let after = b
                    .get(i + kw.len())
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_');
                if before && after {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The host variable markers (`?`) outside quotes.
fn count_markers(s: &str) -> usize {
    let mut q = false;
    let mut n = 0;
    for c in s.chars() {
        match c {
            '\'' => q = !q,
            '?' if !q => n += 1,
            _ => {}
        }
    }
    n
}

/// Which host variables of a SQL statement it sets (as the Db2 host does).
fn sql_outputs(sql: &str, n: usize) -> Vec<bool> {
    let words: Vec<String> = sql
        .split_whitespace()
        .map(|w| w.to_ascii_uppercase())
        .collect();
    let w = |k: usize| words.get(k).map(String::as_str).unwrap_or("");
    let mut out = vec![false; n];
    match w(0) {
        "FETCH" => out.fill(true),
        "SET" if w(1) == "?" => {
            if let Some(o) = out.first_mut() {
                *o = true;
            }
        }
        "SELECT" | "VALUES" | "WITH" => {
            if let Some(at) = find_kw(sql, "INTO", 0) {
                let end = find_kw(sql, "FROM", at).unwrap_or(sql.len());
                let before = count_markers(&sql[..at]);
                let outs = count_markers(&sql[at..end]);
                for o in out.iter_mut().skip(before).take(outs) {
                    *o = true;
                }
            }
        }
        _ => {}
    }
    out
}

impl StubHost {
    pub fn new(outputs: BTreeMap<String, Out>) -> Self {
        StubHost {
            outputs,
            ..Default::default()
        }
    }

    /// The elementary items of an operand's area: those of the group it
    /// names, else the area itself.
    fn layout(&self, call: &ExecCall, operand: Operand, area: &Area) -> Vec<Item> {
        let whole = vec![(String::new(), 0, area.len, followed(num_of(area)))];
        if !matches!(area.ty, DataType::Group) {
            return whole;
        }
        let Some(code) = self
            .module
            .as_ref()
            .and_then(|m| m.programs.iter().find(|p| p.name == call.program))
        else {
            return whole;
        };
        let Some(def) = code.execs.iter().find(|e| {
            e.line == call.line
                && e.command == call.command
                && e.options.len() == call.options.len()
                && e.host.len() == call.host.len()
        }) else {
            return whole;
        };
        let loc: Option<&Loc> = match operand {
            Operand::Option(k) => match def.options.get(k).and_then(|o| o.arg.as_ref()) {
                Some(ExecVal::Area(l)) => Some(l),
                _ => None,
            },
            Operand::Host(k) => def.host.get(k).map(|h| &h.0),
        };
        let Some(d) = loc.and_then(|l| code.descs.get(l.desc as usize)) else {
            return whole;
        };
        let descs: Vec<_> = code.descs.iter().filter(|x| x.base == d.base).collect();
        // what a terminal sends is bytes, whatever the items' pictures
        let raw_input = call.command == "RECEIVE";
        let found: Vec<Item> = items(&descs, d.offset as usize, area.len)
            .into_iter()
            .filter(|i| i.1 + i.2 <= AREA_BYTES)
            .map(|(n, o, l, t)| {
                let t = followed(t).filter(|t| !(raw_input && t.usage == NumUsage::Display));
                (n, o, l, t)
            })
            .collect();
        if found.is_empty() { whole } else { found }
    }

    /// Sets an output area item by item (from `outputs`, else `default`
    /// for a numeric area) and makes it follow its variables.
    fn output(
        &self,
        m: &mut Machine,
        key: &str,
        area: &Area,
        layout: &[Item],
        default: Option<i128>,
    ) {
        for (name, off, len, num) in layout {
            let key = if name.is_empty() {
                key.to_string()
            } else {
                format!("{key}.{name}")
            };
            let addr = area.addr + off;
            match (self.outputs.get(&key), num) {
                (Some(Out::Num(x)), Some(t)) => {
                    vdata::store_raw(t, &mut m.storage_mut()[addr..addr + len], *x);
                }
                (Some(Out::Hex(h)), _) => {
                    let b = unhex(h);
                    let l = b.len().min(*len);
                    m.storage_mut()[addr..addr + l].copy_from_slice(&b[..l]);
                }
                (None, Some(t)) if name.is_empty() => {
                    if let Some(x) = default {
                        vdata::store_raw(t, &mut m.storage_mut()[addr..addr + len], x);
                    }
                }
                _ => {}
            }
            if m.symbolic().is_none_or(|st| st.full()) || *off >= AREA_BYTES {
                if m.symbolic().is_some() {
                    m.sym_forget_area(addr, *len);
                }
                continue;
            }
            let l = (*len).min(AREA_BYTES - off);
            m.sym_forget_area(addr, l);
            match num {
                Some(t) => {
                    let value = raw(t, &m.storage()[addr..addr + len]);
                    let st = m.symbolic().unwrap();
                    let v = st.var_named(
                        format!("X{key}"),
                        VarKind::Num {
                            digits: t.digits,
                            signed: t.signed,
                        },
                        value,
                    );
                    m.sym_bind_num(addr, *len, *t, v);
                }
                None => {
                    for i in 0..l {
                        let b = m.storage()[addr + i];
                        let st = m.symbolic().unwrap();
                        let v =
                            st.var_named(format!("X{key}#{}", i + 1), VarKind::Byte, i128::from(b));
                        m.sym_bind_byte(addr + i, v);
                    }
                }
            }
        }
    }

    /// Sets EIBRESP (following the variable of the response, if any).
    fn set_eibresp(&self, m: &mut Machine, key: &str, resp: i128) {
        let Some(e) = self.eib else { return };
        if e + EIBRESP + 4 > m.storage().len() {
            return;
        }
        let area = Area {
            addr: e + EIBRESP,
            len: 4,
            ty: DataType::Numeric(EIBRESP_TYPE),
        };
        vdata::store_raw(
            &EIBRESP_TYPE,
            &mut m.storage_mut()[area.addr..area.addr + 4],
            resp,
        );
        if let Some(st) = m.symbolic()
            && st.vars.iter().any(|v| v.name == format!("X{key}"))
        {
            self.output(
                m,
                key,
                &area,
                &[(String::new(), 0, 4, Some(EIBRESP_TYPE))],
                None,
            );
        }
    }

    /// The response of a command without RESP in concolic runs: a
    /// variable, and a decision for each condition with a HANDLE or
    /// IGNORE CONDITION (in order, as an EVALUATE would): the solver can
    /// make the command raise the condition the program handles. The
    /// decisions are not the module's branches (program `P#EXEC`).
    fn decide(&self, m: &mut Machine, call: &ExecCall, key: &str, resp: i128) {
        use zeus_cobol::concolic::{Branch, Pred, Step, Term};
        let mut values: Vec<i128> = self
            .handlers
            .borrow()
            .keys()
            .filter(|(i, c)| *i == call.instance && c != "ERROR")
            .filter_map(|(_, c)| zeus_cobol::front::cics::resp_value(c).map(i128::from))
            .collect();
        if values.is_empty() {
            return;
        }
        values.sort();
        values.dedup();
        values.insert(0, 0);
        let Some(st) = m.symbolic() else { return };
        let v = st.var_named(
            format!("X{key}"),
            VarKind::Num {
                digits: 8,
                signed: true,
            },
            resp,
        );
        for (i, c) in values.into_iter().enumerate() {
            let hit = resp == c;
            st.path.push(Step::Branch(Branch {
                program: format!("{}#EXEC", call.program),
                pc: call.line * 64 + i as u32,
                jumped: hit,
                pred: Some(Pred::Cmp(Term::Var(v), Rel::Eq, Term::Const(c))),
            }));
            if hit {
                break;
            }
        }
    }

    /// The SQLCODE of a statement with WHENEVER labels in concolic runs:
    /// a decision for each label (SQLERROR, NOT FOUND, SQLWARNING, in
    /// that order), so that the solver can take the program there.
    fn whenever(&self, m: &mut Machine, call: &ExecCall, key: &str, sqlcode: i128) {
        use zeus_cobol::concolic::{Branch, Pred, Step, Term};
        type Cond = (&'static str, fn(Term) -> Pred);
        let conds: Vec<Cond> = vec![
            ("SQLERROR", |t| Pred::Cmp(t, Rel::Lt, Term::Const(0))),
            ("NOTFOUND", |t| Pred::Cmp(t, Rel::Eq, Term::Const(100))),
            ("SQLWARNING", |t| Pred::Cmp(t, Rel::Gt, Term::Const(0))),
        ];
        let conds: Vec<_> = conds
            .into_iter()
            .filter(|(c, _)| matches!(call.arg(c), Some(ExecArg::Label(_))))
            .collect();
        if conds.is_empty() {
            return;
        }
        let Some(st) = m.symbolic() else { return };
        let v = st.var_named(
            format!("X{key}"),
            VarKind::Num {
                digits: 9,
                signed: true,
            },
            sqlcode,
        );
        for (i, (_, cond)) in conds.into_iter().enumerate() {
            let p = cond(Term::Var(v));
            let hit = p.eval(&|x| st.value(x)).unwrap_or(false);
            st.path.push(Step::Branch(Branch {
                program: format!("{}#EXEC", call.program),
                pc: call.line * 64 + 32 + i as u32,
                jumped: hit,
                pred: Some(p),
            }));
            if hit {
                break;
            }
        }
    }

    /// With a HANDLE ABEND exit, a command may abend (a deadlock, a
    /// file...): a decision (`P#EXEC`, from `outputs["P.l.n.ABEND"]`), the
    /// exit's label when it is taken.
    fn abend_here(&self, m: &mut Machine, call: &ExecCall, key: &str) -> Option<u32> {
        use zeus_cobol::concolic::{Branch, Pred, Step, Term};
        let l = *self.abend_exits.borrow().get(&call.instance)?;
        let first = |s: &str| {
            s.split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_uppercase()
        };
        let can = match call.kind {
            ExecKind::Cics => !matches!(
                call.command.as_str(),
                "HANDLE" | "IGNORE" | "ASSIGN" | "ABEND" | "RETURN" | "XCTL" | "PUSH" | "POP"
            ),
            ExecKind::Sql | ExecKind::SqlIms => !matches!(
                first(call.sql.as_deref().unwrap_or("")).as_str(),
                "DECLARE" | "INCLUDE" | "WHENEVER" | "BEGIN" | "END"
            ),
            ExecKind::Dli => false,
        };
        if !can {
            return None;
        }
        let akey = format!("{key}.ABEND");
        let hit = matches!(self.outputs.get(&akey), Some(Out::Num(x)) if *x != 0);
        if let Some(st) = m.symbolic()
            && !st.full()
        {
            let v = st.var_named(
                format!("X{akey}"),
                VarKind::Num {
                    digits: 1,
                    signed: false,
                },
                i128::from(hit),
            );
            st.path.push(Step::Branch(Branch {
                program: format!("{}#EXEC", call.program),
                pc: call.line * 64 + 63,
                jumped: hit,
                pred: Some(Pred::Cmp(Term::Var(v), Rel::Ne, Term::Const(0))),
            }));
        }
        if !hit {
            return None;
        }
        self.abend_exits.borrow_mut().remove(&call.instance);
        *self.abcode.borrow_mut() = None;
        self.log
            .borrow_mut()
            .push(format!("ABEND at line {}", call.line));
        Some(l)
    }

    fn cics(&self, m: &mut Machine, call: &ExecCall, key: &str, n: u32) -> RResult<ExecOutcome> {
        let cmd = call.command.as_str();
        if matches!(cmd, "HANDLE" | "IGNORE") && call.has("CONDITION") {
            let mut h = self.handlers.borrow_mut();
            for (name, arg) in &call.options {
                if name == "CONDITION" {
                    continue;
                }
                let k = (call.instance, name.clone());
                match arg {
                    Some(ExecArg::Label(l)) if cmd == "HANDLE" => {
                        h.insert(k, Some(*l));
                    }
                    _ if cmd == "IGNORE" => {
                        h.insert(k, None);
                    }
                    _ => {
                        h.remove(&k);
                    }
                }
            }
            self.log.borrow_mut().push(format!("CICS {cmd} CONDITION"));
            return Ok(ExecOutcome::Continue);
        }
        if cmd == "HANDLE" && call.has("ABEND") {
            let mut x = self.abend_exits.borrow_mut();
            match call.arg("LABEL") {
                Some(ExecArg::Label(l)) => {
                    x.insert(call.instance, *l);
                }
                _ => {
                    x.remove(&call.instance);
                }
            }
            self.log.borrow_mut().push(format!("CICS {cmd} ABEND"));
            return Ok(ExecOutcome::Continue);
        }
        // browses and queue reads end from the second time on
        let end = match cmd {
            "READNEXT" | "READPREV" if n > 1 => Some("ENDFILE"),
            "READQ" if n > 1 && call.has("TD") => Some("QZERO"),
            "READQ" if n > 1 => Some("ITEMERR"),
            "RETRIEVE" if n > 1 => Some("ENDDATA"),
            // no child left to fetch (RESP2 1)
            "FETCH" if n > 1 && call.has("ANY") => Some("NOTFND"),
            _ => None,
        };
        let default_resp2 = i128::from(end == Some("NOTFND"));
        let default_resp = end
            .and_then(zeus_cobol::front::cics::resp_value)
            .map_or(0, i128::from);
        let mut sent = Vec::new();
        let mut resp = None;
        for (k, (name, arg)) in call.options.iter().enumerate() {
            match arg {
                Some(ExecArg::Area(a))
                    // every option of ASSIGN receives
                    if (cmd == "ASSIGN" || CICS_OUT.contains(&name.as_str()))
                        && !sends(cmd, name) =>
                {
                    // the code of the abend the exit was given
                    if cmd == "ASSIGN"
                        && name == "ABCODE"
                        && let Some(c) = self.abcode.borrow().as_ref()
                    {
                        let mut b = zeus_cobol::ebcdic::encode(c);
                        b.resize(a.len, 0x40);
                        m.storage_mut()[a.addr..a.addr + a.len].copy_from_slice(&b);
                    }
                    let default = match name.as_str() {
                        "RESP" => Some(default_resp),
                        "RESP2" => Some(default_resp2),
                        _ => None,
                    };
                    let layout = self.layout(call, Operand::Option(k), a);
                    self.output(m, &format!("{key}.{name}"), a, &layout, default);
                    if name == "RESP" {
                        resp = Some(match num_of(a) {
                            Some(t) => raw(&t, &m.storage()[a.addr..a.addr + a.len]),
                            None => default_resp,
                        });
                    }
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
        let rkey = format!("{key}.RESP");
        let handled = resp.is_some();
        let resp = resp.unwrap_or_else(|| match self.outputs.get(&rkey) {
            Some(Out::Num(x)) => *x,
            _ => default_resp,
        });
        if !handled && !matches!(cmd, "RETURN" | "XCTL" | "ABEND") {
            if call.has("NOHANDLE") {
                // the program can only look at EIBRESP
                if let Some(st) = m.symbolic() {
                    st.var_named(
                        format!("X{rkey}"),
                        VarKind::Num {
                            digits: 8,
                            signed: true,
                        },
                        resp,
                    );
                }
            } else {
                self.decide(m, call, &rkey, resp);
            }
        }
        self.set_eibresp(m, &rkey, resp);
        match cmd {
            "RETURN" | "XCTL" => return Ok(ExecOutcome::Return),
            "ABEND" => {
                let code = match call.arg("ABCODE") {
                    Some(ExecArg::Alnum(b)) => zeus_cobol::ebcdic::decode(b),
                    _ => String::new(),
                };
                if let Some(l) = self.abend_exits.borrow_mut().remove(&call.instance) {
                    *self.abcode.borrow_mut() = Some(code);
                    return Ok(ExecOutcome::Goto(l));
                }
                return Err(RunError(format!("CICS abend {}", code.trim())));
            }
            _ => {}
        }
        if resp == 0 || handled || call.has("NOHANDLE") {
            return Ok(ExecOutcome::Continue);
        }
        let cond = condition(resp);
        let h = self.handlers.borrow();
        match h
            .get(&(call.instance, cond.clone()))
            .or_else(|| h.get(&(call.instance, "ERROR".to_string())))
        {
            Some(Some(l)) => Ok(ExecOutcome::Goto(*l)),
            Some(None) => Ok(ExecOutcome::Continue),
            None => Err(RunError(format!(
                "CICS condition {cond} (line {})",
                call.line
            ))),
        }
    }

    /// EXEC DLI: INTO (item by item) and DIBSTAT are inputs (DIBSTAT
    /// spaces, from the second execution of a GN / GNP on GB / GE); what
    /// the command sends (FROM, WHERE values, ...) is logged.
    fn dli(&self, m: &mut Machine, call: &ExecCall, key: &str, n: u32) -> RResult<ExecOutcome> {
        let cmd = call.command.as_str();
        let mut sent = Vec::new();
        for (k, (name, arg)) in call.options.iter().enumerate() {
            match (name.as_str(), arg) {
                ("DIB", _) => {}
                ("INTO", Some(ExecArg::Area(a))) => {
                    let layout = self.layout(call, Operand::Option(k), a);
                    self.output(m, &format!("{key}.INTO"), a, &layout, None);
                }
                (_, Some(ExecArg::Area(a))) => sent.push(format!(
                    "{name}={}",
                    hex(&m.storage()[a.addr..a.addr + a.len.min(AREA_BYTES)])
                )),
                (_, Some(ExecArg::Alnum(b))) => sent.push(format!(
                    "{name}('{}')",
                    zeus_cobol::ebcdic::decode(b).trim_end()
                )),
                (_, Some(ExecArg::Num(d))) => sent.push(format!("{name}({d})")),
                (_, Some(ExecArg::Name(x))) => sent.push(format!("{name}({x})")),
                _ => sent.push(name.clone()),
            }
        }
        for (v, _) in &call.host {
            sent.push(hex(&m.storage()[v.addr..v.addr + v.len.min(AREA_BYTES)]));
        }
        if let Some(ExecArg::Area(dib)) = call.arg("DIB")
            && dib.len >= 4
        {
            let status = Area {
                addr: dib.addr + 2,
                len: 2,
                ty: DataType::Alphanumeric { just: false },
            };
            let skey = format!("{key}.DIBSTAT");
            let end = match cmd {
                "GN" | "GHN" if n > 1 => Some("GB"),
                "GNP" | "GHNP" if n > 1 => Some("GE"),
                _ => None,
            };
            if !self.outputs.contains_key(&skey) {
                let s = zeus_cobol::ebcdic::encode(end.unwrap_or("  "));
                m.storage_mut()[status.addr..status.addr + 2].copy_from_slice(&s);
            }
            self.output(m, &skey, &status, &[(String::new(), 0, 2, None)], None);
        }
        self.log.borrow_mut().push(
            format!("DLI {cmd} {}", sent.join(" "))
                .trim_end()
                .to_string(),
        );
        Ok(ExecOutcome::Continue)
    }

    fn sql(&self, m: &mut Machine, call: &ExecCall, key: &str, n: u32) -> RResult<ExecOutcome> {
        let text = call.sql.clone().unwrap_or_default();
        let first = text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        if matches!(
            first.as_str(),
            "DECLARE" | "INCLUDE" | "WHENEVER" | "BEGIN" | "END"
        ) {
            return Ok(ExecOutcome::Continue);
        }
        let outs = sql_outputs(&text, call.host.len());
        let mut sent = Vec::new();
        for (k, (v, ind)) in call.host.iter().enumerate() {
            if outs[k] {
                let layout = self.layout(call, Operand::Host(k), v);
                self.output(m, &format!("{key}.H{}", k + 1), v, &layout, None);
                // the null indicator: 0 (not null) unless set
                if let Some(i) = ind {
                    let n = i.len.min(2);
                    m.storage_mut()[i.addr..i.addr + n].fill(0);
                    let whole = [(String::new(), 0, n, followed(num_of(i)))];
                    self.output(m, &format!("{key}.I{}", k + 1), i, &whole, None);
                }
            } else {
                sent.push(hex(&m.storage()[v.addr..v.addr + v.len.min(AREA_BYTES)]));
            }
        }
        // a cursor's rows end from the second FETCH on
        let default = if first == "FETCH" && n > 1 { 100 } else { 0 };
        let ckey = format!("{key}.SQLCODE");
        let ca = call
            .options
            .iter()
            .position(|(n, _)| n == "SQLCA")
            .and_then(|k| match &call.options[k].1 {
                Some(ExecArg::Area(a)) if a.len >= 16 => Some((k, a)),
                _ => None,
            });
        let sqlcode = match ca {
            Some((k, a)) => {
                // SQLCODE as the program's SQLCA describes it
                let (off, len, t) = self
                    .layout(call, Operand::Option(k), a)
                    .into_iter()
                    .find(|i| i.0 == "SQLCODE")
                    .and_then(|i| Some((i.1, i.2, i.3?)))
                    .unwrap_or((12, 4, SQLCODE_TYPE));
                let code = Area {
                    addr: a.addr + off,
                    len,
                    ty: DataType::Numeric(t),
                };
                self.output(
                    m,
                    &ckey,
                    &code,
                    &[(String::new(), 0, len, Some(t))],
                    Some(default),
                );
                raw(&t, &m.storage()[code.addr..code.addr + len])
            }
            _ => match self.outputs.get(&ckey) {
                Some(Out::Num(x)) => *x,
                _ => default,
            },
        };
        self.log.borrow_mut().push(
            format!(
                "SQL {} {}",
                text.split_whitespace().collect::<Vec<_>>().join(" "),
                sent.join(" ")
            )
            .trim_end()
            .to_string(),
        );
        self.whenever(m, call, &ckey, sqlcode);
        let whenever = if sqlcode < 0 {
            "SQLERROR"
        } else if sqlcode == 100 {
            "NOTFOUND"
        } else if sqlcode > 0 {
            "SQLWARNING"
        } else {
            return Ok(ExecOutcome::Continue);
        };
        match call.arg(whenever) {
            Some(ExecArg::Label(l)) => Ok(ExecOutcome::Goto(*l)),
            _ => Ok(ExecOutcome::Continue),
        }
    }
}

fn num_of(a: &Area) -> Option<NumType> {
    match &a.ty {
        DataType::Numeric(t) => Some(*t),
        _ => None,
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
        if let Some(l) = self.abend_here(m, call, &key) {
            return Ok(ExecOutcome::Goto(l));
        }
        match call.kind {
            ExecKind::Cics => self.cics(m, call, &key, n),
            ExecKind::Sql | ExecKind::SqlIms => self.sql(m, call, &key, n),
            ExecKind::Dli => self.dli(m, call, &key, n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_host_variables_set() {
        assert_eq!(
            sql_outputs(
                "SELECT A, (SELECT X INTO Y FROM U) INTO ?, ? FROM T WHERE K = ?",
                3
            ),
            [true, true, false]
        );
        assert_eq!(sql_outputs("FETCH C1 INTO ?, ?", 2), [true, true]);
        assert_eq!(
            sql_outputs("UPDATE T SET A = ? WHERE K = ?", 2),
            [false, false]
        );
        assert_eq!(sql_outputs("SET ? = CURRENT DATE", 1), [true]);
    }

    #[test]
    fn conditions_by_resp() {
        assert_eq!(condition(13), "NOTFND");
        assert_eq!(condition(20), "ENDFILE");
    }
}

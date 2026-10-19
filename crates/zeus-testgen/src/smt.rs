//! An SMT solver for the conditions the built-in search cannot solve: Z3
//! loaded at run time (`libz3`), given the conditions as SMT-LIB text
//! through `Z3_eval_smtlib2_string`. Nothing of Z3 is needed to build;
//! without the library the built-in solver works alone.
//!
//! The library is `ZTESTGEN_Z3` (a path), else `libz3` on the search path
//! of the system, else the one of the Python package `z3-solver` (user or
//! system site-packages).
//!
//! This is the only module with unsafe code: the calls of the Z3 C API.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::fmt::Write;
use std::path::PathBuf;
use std::time::Duration;

use zeus_cobol::concolic::{Pred, Term, Var, VarKind};
use zeus_cobol::zir::Rel;

use crate::solver::Outcome;

type Ctx = *mut c_void;

/// Z3 and a context of it.
pub struct Z3 {
    ctx: Ctx,
    eval: unsafe extern "C" fn(Ctx, *const c_char) -> *const c_char,
    del_context: unsafe extern "C" fn(Ctx),
    _lib: libloading::Library,
    /// Time per call.
    pub time: Duration,
}

// SAFETY: a context is used by one thread at a time (each solver thread
// has its own Z3).
unsafe impl Send for Z3 {}

unsafe extern "C" fn ignore_error(_: Ctx, _: u32) {}

/// The places the library is looked for.
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("ZTESTGEN_Z3") {
        out.push(PathBuf::from(p));
    }
    let name = if cfg!(windows) {
        "libz3.dll"
    } else if cfg!(target_os = "macos") {
        "libz3.dylib"
    } else {
        "libz3.so"
    };
    out.push(PathBuf::from(name));
    // the Python package z3-solver
    let mut sites = Vec::new();
    if let Some(a) = std::env::var_os("APPDATA") {
        sites.push(PathBuf::from(a).join("Python"));
    }
    if let Some(h) = std::env::var_os("HOME") {
        sites.push(PathBuf::from(h).join(".local/lib"));
    }
    for base in sites {
        let Ok(dirs) = std::fs::read_dir(&base) else {
            continue;
        };
        for d in dirs.flatten() {
            out.push(d.path().join("site-packages/z3/lib").join(name));
        }
    }
    out
}

impl Z3 {
    /// Z3 when its library is found.
    pub fn load() -> Option<Z3> {
        candidates().into_iter().find_map(|p| Self::open(p).ok())
    }

    fn open(path: PathBuf) -> Result<Z3, libloading::Error> {
        // SAFETY: the library is Z3, whose C API these signatures are.
        unsafe {
            let lib = libloading::Library::new(&path)?;
            let mk_config: libloading::Symbol<unsafe extern "C" fn() -> *mut c_void> =
                lib.get(b"Z3_mk_config\0")?;
            let mk_context: libloading::Symbol<unsafe extern "C" fn(*mut c_void) -> Ctx> =
                lib.get(b"Z3_mk_context\0")?;
            let del_config: libloading::Symbol<unsafe extern "C" fn(*mut c_void)> =
                lib.get(b"Z3_del_config\0")?;
            let set_handler: libloading::Symbol<
                unsafe extern "C" fn(Ctx, Option<unsafe extern "C" fn(Ctx, u32)>),
            > = lib.get(b"Z3_set_error_handler\0")?;
            let eval = *lib.get(b"Z3_eval_smtlib2_string\0")?;
            let del_context = *lib.get(b"Z3_del_context\0")?;
            let cfg = mk_config();
            let ctx = mk_context(cfg);
            del_config(cfg);
            // errors come back as text, the process does not exit
            set_handler(ctx, Some(ignore_error));
            Ok(Z3 {
                ctx,
                eval,
                del_context,
                _lib: lib,
                time: Duration::from_millis(400),
            })
        }
    }

    /// Runs an SMT-LIB script: its output.
    fn run(&mut self, script: &str) -> String {
        let Ok(text) = CString::new(script) else {
            return String::new();
        };
        // SAFETY: the context is alive; Z3 owns the returned string until
        // the next call, it is copied at once.
        unsafe {
            let out = (self.eval)(self.ctx, text.as_ptr());
            if out.is_null() {
                return String::new();
            }
            CStr::from_ptr(out).to_string_lossy().into_owned()
        }
    }

    /// Solves `cons` over `vars`: values for the variables in the
    /// conditions, the others from `start`.
    pub fn solve(&mut self, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome {
        let mut used = Vec::new();
        for c in cons {
            c.vars(&mut used);
        }
        used.sort();
        used.dedup();
        let mut s = String::from("(reset)\n");
        let _ = writeln!(s, "(set-option :timeout {})", self.time.as_millis());
        for &v in &used {
            let _ = writeln!(s, "(declare-const v{v} Int)");
            let (lo, hi) = match vars[v as usize].kind {
                VarKind::Byte => (0, 255),
                VarKind::Num { digits, signed } => {
                    let max = 10i128
                        .checked_pow(digits.min(36))
                        .map_or(i128::MAX, |m| m - 1);
                    (if signed { -max } else { 0 }, max)
                }
            };
            let _ = writeln!(s, "(assert (<= {} v{v} {}))", int(lo), int(hi));
        }
        for c in cons {
            let _ = writeln!(s, "(assert {})", pred(c));
        }
        s.push_str("(check-sat)\n");
        if !used.is_empty() {
            s.push_str("(get-value (");
            for &v in &used {
                let _ = write!(s, " v{v}");
            }
            s.push_str("))\n");
        }
        let out = self.run(&s);
        let mut lines = out.lines();
        match lines.next().map(str::trim) {
            Some("sat") => {}
            Some("unsat") => return Outcome::Unsat,
            _ => return Outcome::Unknown,
        }
        let mut values = start.to_vec();
        let rest: String = lines.collect::<Vec<_>>().join(" ");
        for (v, x) in values_of(&rest) {
            if let Some(slot) = values.get_mut(v as usize) {
                *slot = x;
            }
        }
        Outcome::Sat(values)
    }
}

impl Drop for Z3 {
    fn drop(&mut self) {
        // SAFETY: the context was made by this library and is not used after.
        unsafe { (self.del_context)(self.ctx) }
    }
}

/// `((v3 12) (v7 (- 4)))` -> [(3, 12), (7, -4)].
fn values_of(text: &str) -> Vec<(u32, i128)> {
    let t = text.replace('(', " ( ").replace(')', " ) ");
    let toks: Vec<&str> = t.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let Some(n) = toks[i].strip_prefix('v')
            && let Ok(v) = n.parse::<u32>()
        {
            let (x, used) = match toks.get(i + 1..i + 4) {
                Some(["(", "-", x]) => (x.parse::<i128>().ok().map(|x| -x), 4),
                _ => (toks.get(i + 1).and_then(|x| x.parse::<i128>().ok()), 1),
            };
            if let Some(x) = x {
                out.push((v, x));
            }
            i += used;
        }
        i += 1;
    }
    out
}

fn int(x: i128) -> String {
    if x < 0 {
        format!("(- {})", x.unsigned_abs())
    } else {
        x.to_string()
    }
}

/// Division truncated toward zero by a positive constant.
fn tdiv(a: &str, k: i128) -> String {
    format!("(ite (>= {a} 0) (div {a} {k}) (- (div (- {a}) {k})))")
}

fn term(t: &Term) -> String {
    match t {
        Term::Const(c) => int(*c),
        Term::Var(v) => format!("v{v}"),
        Term::Add(a, b) => format!("(+ {} {})", term(a), term(b)),
        Term::Sub(a, b) => format!("(- {} {})", term(a), term(b)),
        Term::Mul(a, b) => format!("(* {} {})", term(a), term(b)),
        Term::Neg(a) => format!("(- {})", term(a)),
        Term::Abs(a) => {
            let a = term(a);
            format!("(ite (>= {a} 0) {a} (- {a}))")
        }
        Term::Div(a, k) => tdiv(&term(a), *k),
        Term::Rem(a, k) => {
            let a = term(a);
            format!("(- {a} (* {k} {}))", tdiv(&a, *k))
        }
        Term::If(p, a, b) => format!("(ite {} {} {})", pred(p), term(a), term(b)),
    }
}

fn cmp(a: String, r: Rel, b: String) -> String {
    match r {
        Rel::Eq => format!("(= {a} {b})"),
        Rel::Ne => format!("(not (= {a} {b}))"),
        Rel::Lt => format!("(< {a} {b})"),
        Rel::Le => format!("(<= {a} {b})"),
        Rel::Gt => format!("(> {a} {b})"),
        Rel::Ge => format!("(>= {a} {b})"),
    }
}

/// Byte strings compared as EBCDIC (the bytes' values in order), the
/// shorter one padded with spaces.
fn bytes(a: &[Term], r: Rel, b: &[Term]) -> String {
    let n = a.len().max(b.len());
    let at = |s: &[Term], i: usize| s.get(i).map_or_else(|| "64".to_string(), term);
    let eq_all = || {
        let parts: Vec<String> = (0..n)
            .map(|i| format!("(= {} {})", at(a, i), at(b, i)))
            .collect();
        format!("(and true {})", parts.join(" "))
    };
    // a < b: the first byte that differs is smaller
    let less = |x: &[Term], y: &[Term]| {
        let mut e = "false".to_string();
        for i in (0..n).rev() {
            let (p, q) = (at(x, i), at(y, i));
            e = format!("(or (< {p} {q}) (and (= {p} {q}) {e}))");
        }
        e
    };
    match r {
        Rel::Eq => eq_all(),
        Rel::Ne => format!("(not {})", eq_all()),
        Rel::Lt => less(a, b),
        Rel::Gt => less(b, a),
        Rel::Le => format!("(not {})", less(b, a)),
        Rel::Ge => format!("(not {})", less(a, b)),
    }
}

fn pred(p: &Pred) -> String {
    match p {
        Pred::Bool(b) => b.to_string(),
        Pred::Cmp(a, r, b) => cmp(term(a), *r, term(b)),
        Pred::Bytes(a, r, b) => bytes(a, *r, b),
        Pred::In(t, ranges) => {
            let t = term(t);
            let parts: Vec<String> = ranges
                .iter()
                .map(|(lo, hi)| format!("(<= {} {t} {})", int(*lo), int(*hi)))
                .collect();
            format!("(or false {})", parts.join(" "))
        }
        Pred::Not(p) => format!("(not {})", pred(p)),
        Pred::And(ps) => format!(
            "(and true {})",
            ps.iter().map(pred).collect::<Vec<_>>().join(" ")
        ),
        Pred::Or(ps) => format!(
            "(or false {})",
            ps.iter().map(pred).collect::<Vec<_>>().join(" ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_values() {
        assert_eq!(values_of("((v3 12)\n (v7 (- 4)))"), [(3, 12), (7, -4)]);
    }

    #[test]
    fn truncated_division_and_bytes() {
        let Some(mut z3) = Z3::load() else {
            eprintln!("Z3 not found: skipped");
            return;
        };
        let vars = vec![
            Var {
                name: "N".into(),
                kind: VarKind::Num {
                    digits: 5,
                    signed: true,
                },
                value: 0,
            },
            Var {
                name: "B".into(),
                kind: VarKind::Byte,
                value: 0x40,
            },
        ];
        // N / 7 (truncated) = -3 and N % 7 = -2: N = -23
        let cons = vec![
            Pred::Cmp(
                Term::Div(Box::new(Term::Var(0)), 7),
                Rel::Eq,
                Term::Const(-3),
            ),
            Pred::Cmp(
                Term::Rem(Box::new(Term::Var(0)), 7),
                Rel::Eq,
                Term::Const(-2),
            ),
            Pred::Bytes(vec![Term::Var(1)], Rel::Eq, vec![Term::Const(0xC1)]),
        ];
        match z3.solve(&cons, &vars, &[0, 0x40]) {
            Outcome::Sat(v) => assert_eq!(v, [-23, 0xC1]),
            o => panic!("{o:?}"),
        }
        let none = vec![Pred::Cmp(Term::Var(0), Rel::Gt, Term::Const(99_999))];
        assert!(matches!(z3.solve(&none, &vars, &[0, 0]), Outcome::Unsat));
    }
}

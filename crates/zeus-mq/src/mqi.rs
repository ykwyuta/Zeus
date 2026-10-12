//! The MQI calls of COBOL programs (CALL 'MQOPEN' USING ...): host
//! programs of the COBOL run time. Each converts the z/OS structures to
//! the native ones, calls the client library and converts back.

use std::cell::RefCell;

use zeus_cobol::ebcdic;

use crate::ffi::{Cr, Library};
use crate::layout::{GMO, Layout, MD, OD, PMO, be, ne, set_ne};

pub const MQCC_FAILED: i32 = 2;
pub const MQRC_ENVIRONMENT_ERROR: i32 = 2012;
pub const MQRC_HCONN_ERROR: i32 = 2018;
pub const MQRC_OD_ERROR: i32 = 2044;
pub const MQRC_MD_ERROR: i32 = 2026;
pub const MQRC_PMO_ERROR: i32 = 2173;
pub const MQRC_GMO_ERROR: i32 = 2186;
pub const MQRC_BUFFER_LENGTH_ERROR: i32 = 2005;
/// MQCCSI_Q_MGR: the queue manager's CCSID (here: the program's code page).
const MQCCSI_Q_MGR: i32 = 0;
/// Offset of CodedCharSetId in MQMD.
const MD_CCSID: usize = 28;

#[derive(Default)]
struct State {
    lib: Option<Result<Library, String>>,
    /// The connection of calls with the default handle (0), as the CICS
    /// adapter gives one.
    default: Option<i32>,
    /// Connections made by the run unit (ended with it).
    conns: Vec<i32>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Runs `f` with the client library (loaded on first use).
fn with_lib<T>(f: impl FnOnce(&Library) -> T) -> Result<T, String> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let lib = s.lib.get_or_insert_with(|| {
            let r = Library::load();
            if let Err(e) = &r {
                eprintln!("ZEUS MQ: {e}");
            }
            r
        });
        match lib {
            Ok(l) => Ok(f(l)),
            Err(e) => Err(e.clone()),
        }
    })
}

/// The server of ZEUS_MQ (`mq://user:password@host:port/QMGR/CHANNEL`):
/// MQSERVER, the queue manager, the user and the password.
pub struct Server {
    pub mqserver: String,
    pub qmgr: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

impl Server {
    pub fn parse(url: &str) -> Option<Server> {
        let rest = url.strip_prefix("mq://")?;
        let (auth, rest) = match rest.rsplit_once('@') {
            Some((a, r)) => (Some(a), r),
            None => (None, rest),
        };
        let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = hostport.rsplit_once(':').unwrap_or((hostport, "1414"));
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let qmgr = parts.next().unwrap_or("").to_string();
        let channel = parts.next().unwrap_or("SYSTEM.DEF.SVRCONN");
        let (user, password) = match auth.map(|a| a.split_once(':')) {
            Some(Some((u, p))) => (Some(u.to_string()), Some(p.to_string())),
            _ => (auth.map(str::to_string), None),
        };
        Some(Server {
            mqserver: format!("{channel}/TCP/{host}({port})"),
            qmgr,
            user,
            password,
        })
    }

    fn from_env() -> Option<Server> {
        Server::parse(&std::env::var("ZEUS_MQ").ok()?)
    }
}

/// Connects to `qmgr` (blank: the queue manager of ZEUS_MQ).
fn connect(qmgr: &str) -> (i32, Cr) {
    let server = Server::from_env();
    if let Some(s) = &server
        && std::env::var_os("MQSERVER").is_none()
    {
        crate::ffi::set_mqserver(&s.mqserver);
    }
    let name = if qmgr.trim().is_empty() {
        server.as_ref().map(|s| s.qmgr.clone()).unwrap_or_default()
    } else {
        qmgr.trim().to_string()
    };
    let mut q = [b' '; 48];
    for (d, s) in q.iter_mut().zip(name.bytes()) {
        *d = s;
    }
    let user = server
        .as_ref()
        .and_then(|s| s.user.clone())
        .unwrap_or_default();
    let password = server
        .as_ref()
        .and_then(|s| s.password.clone())
        .unwrap_or_default();
    let mut cno = [0u8; 200];
    let mut csp = [0u8; 56];
    let creds = (!user.is_empty()).then_some((&mut csp, user.as_bytes(), password.as_bytes()));
    crate::ffi::client_options(&mut cno, creds);
    match with_lib(|l| l.connx(&q, &mut cno)) {
        Ok((h, cr)) => {
            if cr.0 != MQCC_FAILED {
                STATE.with(|s| s.borrow_mut().conns.push(h));
            }
            (h, cr)
        }
        Err(_) => (0, (MQCC_FAILED, MQRC_ENVIRONMENT_ERROR)),
    }
}

/// The native connection of a program's handle: 0 (MQHC_DEF_HCONN) is
/// the default connection, made on first use.
fn hconn(h: i32) -> Result<i32, Cr> {
    match h {
        0 => {
            if let Some(d) = STATE.with(|s| s.borrow().default) {
                return Ok(d);
            }
            let (d, cr) = connect("");
            if cr.0 == MQCC_FAILED {
                return Err(cr);
            }
            STATE.with(|s| s.borrow_mut().default = Some(d));
            Ok(d)
        }
        -1 => Err((MQCC_FAILED, MQRC_HCONN_ERROR)),
        h => Ok(h),
    }
}

/// Ends the connections of the run unit: their units of work are
/// committed (or backed out) and they are disconnected.
pub fn end_run(commit: bool) {
    let conns = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.default = None;
        std::mem::take(&mut s.conns)
    });
    if conns.is_empty() {
        return;
    }
    let _ = with_lib(|l| {
        for h in conns {
            if commit {
                l.cmit(h);
            } else {
                l.back(h);
            }
            l.disc(h);
        }
    });
}

/// Commits (or backs out) the units of work of the run unit's
/// connections (a CICS SYNCPOINT).
pub fn syncpoint(commit: bool) {
    let conns = STATE.with(|s| s.borrow().conns.clone());
    if conns.is_empty() {
        return;
    }
    let _ = with_lib(|l| {
        for h in conns {
            if commit {
                l.cmit(h);
            } else {
                l.back(h);
            }
        }
    });
}

// ---- access to the program's arguments --------------------------------

struct Args<'a> {
    mem: &'a mut [u8],
    addrs: &'a [Option<usize>],
}

impl Args<'_> {
    fn addr(&self, i: usize, len: usize) -> Result<usize, String> {
        match self.addrs.get(i).copied().flatten() {
            Some(a) if a + len <= self.mem.len() => Ok(a),
            _ => Err(format!(
                "MQ call: parameter {} missing or out of storage",
                i + 1
            )),
        }
    }

    fn int(&self, i: usize) -> Result<i32, String> {
        Ok(be(self.mem, self.addr(i, 4)?))
    }

    fn set_int(&mut self, i: usize, v: i32) -> Result<(), String> {
        let a = self.addr(i, 4)?;
        self.mem[a..a + 4].copy_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn bytes(&self, i: usize, len: usize) -> Result<&[u8], String> {
        let a = self.addr(i, len)?;
        Ok(&self.mem[a..a + len])
    }

    fn bytes_mut(&mut self, i: usize, len: usize) -> Result<&mut [u8], String> {
        let a = self.addr(i, len)?;
        Ok(&mut self.mem[a..a + len])
    }

    /// Completion and reason codes into the arguments `i` and `i + 1`.
    fn done(&mut self, i: usize, cr: Cr) -> Result<i32, String> {
        self.set_int(i, cr.0)?;
        self.set_int(i + 1, cr.1)?;
        Ok(0)
    }

    /// A structure argument in native form (None: not this structure).
    fn structure(&self, i: usize, l: &Layout) -> Result<Option<(Vec<u8>, usize)>, String> {
        let head = self.bytes(i, 8)?;
        if !l.is(head) {
            return Ok(None);
        }
        let v = l.version(head);
        let zos = self.bytes(i, l.len(v))?;
        Ok(Some((l.to_native(zos, v), v)))
    }

    fn put_back(&mut self, i: usize, l: &Layout, native: &[u8]) -> Result<(), String> {
        let n = native.len();
        let zos = self.bytes_mut(i, n)?;
        l.to_zos(native, zos);
        Ok(())
    }
}

/// The 48-character queue manager name of argument `i` as text.
fn name48(a: &Args, i: usize) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&ebcdic::decode_latin1(a.bytes(i, 48)?)).into_owned())
}

/// MQCCSI_Q_MGR in a message descriptor stands for the code page of the
/// program's data.
fn own_ccsid(md: &mut [u8]) {
    if ne(md, MD_CCSID) == MQCCSI_Q_MGR {
        set_ne(md, MD_CCSID, zeus_cobol::codepage::active().ccsid as i32);
    }
}

macro_rules! try_cr {
    ($a:expr, $at:expr, $e:expr) => {
        match $e {
            Ok(v) => v,
            Err(cr) => return $a.done($at, cr),
        }
    };
}

/// MQCONN(QMgrName, Hconn, CompCode, Reason)
pub fn mqconn(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let (h, cr) = connect(&name48(&a, 0)?);
    a.set_int(1, h)?;
    a.done(2, cr)
}

/// MQCONNX(QMgrName, ConnectOpts, Hconn, CompCode, Reason): the options
/// are those of the client connection of ZEUS_MQ.
pub fn mqconnx(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let (h, cr) = connect(&name48(&a, 0)?);
    a.set_int(2, h)?;
    a.done(3, cr)
}

/// MQDISC(Hconn, CompCode, Reason)
pub fn mqdisc(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 1, hconn(a.int(0)?));
    let cr = with_lib(|l| l.disc(h)).unwrap_or((MQCC_FAILED, MQRC_ENVIRONMENT_ERROR));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.conns.retain(|&c| c != h);
        if s.default == Some(h) {
            s.default = None;
        }
    });
    a.set_int(0, -1)?;
    a.done(1, cr)
}

/// MQOPEN(Hconn, ObjDesc, Options, Hobj, CompCode, Reason)
pub fn mqopen(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 4, hconn(a.int(0)?));
    let Some((mut od, _)) = a.structure(1, &OD)? else {
        return a.done(4, (MQCC_FAILED, MQRC_OD_ERROR));
    };
    let options = a.int(2)?;
    let (o, cr) = with_lib(|l| l.open(h, &mut od, options))
        .unwrap_or((0, (MQCC_FAILED, MQRC_ENVIRONMENT_ERROR)));
    a.put_back(1, &OD, &od)?;
    a.set_int(3, o)?;
    a.done(4, cr)
}

/// MQCLOSE(Hconn, Hobj, Options, CompCode, Reason)
pub fn mqclose(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 3, hconn(a.int(0)?));
    let (o, options) = (a.int(1)?, a.int(2)?);
    let (o, cr) =
        with_lib(|l| l.close(h, o, options)).unwrap_or((o, (MQCC_FAILED, MQRC_ENVIRONMENT_ERROR)));
    a.set_int(1, o)?;
    a.done(3, cr)
}

/// MQPUT(Hconn, Hobj, MsgDesc, PutMsgOpts, BufferLength, Buffer,
/// CompCode, Reason)
pub fn mqput(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 6, hconn(a.int(0)?));
    let o = a.int(1)?;
    let Some((mut md, _)) = a.structure(2, &MD)? else {
        return a.done(6, (MQCC_FAILED, MQRC_MD_ERROR));
    };
    let Some((mut pmo, _)) = a.structure(3, &PMO)? else {
        return a.done(6, (MQCC_FAILED, MQRC_PMO_ERROR));
    };
    let n = a.int(4)?;
    if n < 0 {
        return a.done(6, (MQCC_FAILED, MQRC_BUFFER_LENGTH_ERROR));
    }
    own_ccsid(&mut md);
    let data = a.bytes(5, n as usize)?.to_vec();
    let cr = with_lib(|l| l.put(h, o, &mut md, &mut pmo, &data))
        .unwrap_or((MQCC_FAILED, MQRC_ENVIRONMENT_ERROR));
    a.put_back(2, &MD, &md)?;
    a.put_back(3, &PMO, &pmo)?;
    a.done(6, cr)
}

/// MQPUT1(Hconn, ObjDesc, MsgDesc, PutMsgOpts, BufferLength, Buffer,
/// CompCode, Reason)
pub fn mqput1(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 6, hconn(a.int(0)?));
    let Some((mut od, _)) = a.structure(1, &OD)? else {
        return a.done(6, (MQCC_FAILED, MQRC_OD_ERROR));
    };
    let Some((mut md, _)) = a.structure(2, &MD)? else {
        return a.done(6, (MQCC_FAILED, MQRC_MD_ERROR));
    };
    let Some((mut pmo, _)) = a.structure(3, &PMO)? else {
        return a.done(6, (MQCC_FAILED, MQRC_PMO_ERROR));
    };
    let n = a.int(4)?;
    if n < 0 {
        return a.done(6, (MQCC_FAILED, MQRC_BUFFER_LENGTH_ERROR));
    }
    own_ccsid(&mut md);
    let data = a.bytes(5, n as usize)?.to_vec();
    let cr = with_lib(|l| l.put1(h, &mut od, &mut md, &mut pmo, &data))
        .unwrap_or((MQCC_FAILED, MQRC_ENVIRONMENT_ERROR));
    a.put_back(1, &OD, &od)?;
    a.put_back(2, &MD, &md)?;
    a.put_back(3, &PMO, &pmo)?;
    a.done(6, cr)
}

/// MQGET(Hconn, Hobj, MsgDesc, GetMsgOpts, BufferLength, Buffer,
/// DataLength, CompCode, Reason)
pub fn mqget(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 7, hconn(a.int(0)?));
    let o = a.int(1)?;
    let Some((mut md, _)) = a.structure(2, &MD)? else {
        return a.done(7, (MQCC_FAILED, MQRC_MD_ERROR));
    };
    let Some((mut gmo, _)) = a.structure(3, &GMO)? else {
        return a.done(7, (MQCC_FAILED, MQRC_GMO_ERROR));
    };
    let n = a.int(4)?;
    if n < 0 {
        return a.done(7, (MQCC_FAILED, MQRC_BUFFER_LENGTH_ERROR));
    }
    // MQGMO_CONVERT converts to the code page of the program's data
    own_ccsid(&mut md);
    let mut buf = vec![0u8; n as usize];
    let (len, cr) = with_lib(|l| l.get(h, o, &mut md, &mut gmo, &mut buf))
        .unwrap_or((0, (MQCC_FAILED, MQRC_ENVIRONMENT_ERROR)));
    let k = (len.max(0) as usize).min(buf.len());
    a.bytes_mut(5, n as usize)?[..k].copy_from_slice(&buf[..k]);
    a.put_back(2, &MD, &md)?;
    a.put_back(3, &GMO, &gmo)?;
    a.set_int(6, len)?;
    a.done(7, cr)
}

/// MQCMIT(Hconn, CompCode, Reason)
pub fn mqcmit(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 1, hconn(a.int(0)?));
    let cr = with_lib(|l| l.cmit(h)).unwrap_or((MQCC_FAILED, MQRC_ENVIRONMENT_ERROR));
    a.done(1, cr)
}

/// MQBACK(Hconn, CompCode, Reason)
pub fn mqback(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 1, hconn(a.int(0)?));
    let cr = with_lib(|l| l.back(h)).unwrap_or((MQCC_FAILED, MQRC_ENVIRONMENT_ERROR));
    a.done(1, cr)
}

/// MQINQ(Hconn, Hobj, SelectorCount, Selectors, IntAttrCount, IntAttrs,
/// CharAttrLength, CharAttrs, CompCode, Reason)
pub fn mqinq(mem: &mut [u8], addrs: &[Option<usize>]) -> Result<i32, String> {
    let mut a = Args { mem, addrs };
    let h = try_cr!(a, 8, hconn(a.int(0)?));
    let o = a.int(1)?;
    let (ns, ni, nc) = (
        a.int(2)?.max(0) as usize,
        a.int(4)?.max(0) as usize,
        a.int(6)?.max(0) as usize,
    );
    let sel = a.bytes(3, ns * 4)?;
    let selectors: Vec<i32> = (0..ns).map(|k| be(sel, k * 4)).collect();
    let mut ints = vec![0i32; ni];
    let mut chars = vec![b' '; nc];
    let cr = with_lib(|l| l.inq(h, o, &selectors, &mut ints, &mut chars))
        .unwrap_or((MQCC_FAILED, MQRC_ENVIRONMENT_ERROR));
    if ni > 0 {
        let out = a.bytes_mut(5, ni * 4)?;
        for (k, v) in ints.iter().enumerate() {
            out[k * 4..k * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
    }
    if nc > 0 {
        let out = a.bytes_mut(7, nc)?;
        for (d, s) in out.iter_mut().zip(&chars) {
            *d = ebcdic::from_latin1(*s);
        }
    }
    a.done(8, cr)
}

/// The MQI calls by name.
pub const CALLS: &[(&str, zeus_cobol::vm::NativeFn)] = &[
    ("MQCONN", mqconn),
    ("MQCONNX", mqconnx),
    ("MQDISC", mqdisc),
    ("MQOPEN", mqopen),
    ("MQCLOSE", mqclose),
    ("MQPUT", mqput),
    ("MQPUT1", mqput1),
    ("MQGET", mqget),
    ("MQCMIT", mqcmit),
    ("MQBACK", mqback),
    ("MQINQ", mqinq),
];

#[cfg(test)]
mod tests {
    use super::Server;

    #[test]
    fn server_url() {
        let s = Server::parse("mq://app:secret@mqhost:1414/QM1/DEV.APP.SVRCONN").unwrap();
        assert_eq!(s.mqserver, "DEV.APP.SVRCONN/TCP/mqhost(1414)");
        assert_eq!(s.qmgr, "QM1");
        assert_eq!(s.user.as_deref(), Some("app"));
        assert_eq!(s.password.as_deref(), Some("secret"));
        let s = Server::parse("mq://localhost/QM2").unwrap();
        assert_eq!(s.mqserver, "SYSTEM.DEF.SVRCONN/TCP/localhost(1414)");
        assert!(s.user.is_none());
    }
}

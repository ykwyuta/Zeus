//! MQCB and MQCTL: asynchronous consumption by programs the run unit
//! calls back. MQCB registers a message consumer (a queue handle, its
//! MQMD and MQGMO) or an event handler, named by CallbackName; MQCTL
//! START_WAIT gets the consumers' messages and calls each consumer with
//! (Hconn, MQMD, MQGMO, Buffer, MQCBC) until a callback issues MQCTL STOP
//! or no consumer is left. START delivers the messages there are and
//! returns (the run unit has a single thread).

use std::cell::RefCell;
use std::time::{Duration, Instant};

use zeus_cobol::ebcdic;
use zeus_cobol::interlang::XRef;
use zeus_cobol::vm::Machine;

use crate::layout::{GMO, MD, be, ne, set_ne};
use crate::mqi::{
    MQCC_FAILED, MQRC_CALLBACK_LINK_ERROR, MQRC_CALLBACK_TYPE_ERROR, MQRC_CBD_ERROR,
    MQRC_ENVIRONMENT_ERROR, MQRC_GMO_ERROR, MQRC_MD_ERROR, MQRC_NO_MSG_AVAILABLE,
    MQRC_OPERATION_ERROR, hconn, own_ccsid, wait, with_lib,
};

const MQOP_START: i32 = 1;
const MQOP_START_WAIT: i32 = 2;
const MQOP_STOP: i32 = 4;
const MQOP_REGISTER: i32 = 256;
const MQOP_DEREGISTER: i32 = 512;
const MQOP_SUSPEND: i32 = 65536;
const MQOP_RESUME: i32 = 131072;

const MQCBT_MESSAGE_CONSUMER: i32 = 1;
const MQCBT_EVENT_HANDLER: i32 = 2;

const MQCBDO_START_CALL: i32 = 1;
const MQCBDO_STOP_CALL: i32 = 4;
const MQCBDO_REGISTER_CALL: i32 = 256;
const MQCBDO_DEREGISTER_CALL: i32 = 512;

const MQCBCT_START_CALL: i32 = 1;
const MQCBCT_STOP_CALL: i32 = 2;
const MQCBCT_REGISTER_CALL: i32 = 3;
const MQCBCT_DEREGISTER_CALL: i32 = 4;
const MQCBCT_EVENT_CALL: i32 = 5;
const MQCBCT_MSG_REMOVED: i32 = 6;
const MQCBCT_MSG_NOT_REMOVED: i32 = 7;

const MQGMO_WAIT: i32 = 1;
const MQGMO_BROWSE_FIRST: i32 = 16;
const MQGMO_BROWSE_NEXT: i32 = 32;
/// MQCBD_FULL_MSG_LENGTH: the largest message delivered then.
const FULL_LENGTH: usize = 4 * 1024 * 1024;

/// Offsets of MQGMO Options and WaitInterval.
const GMO_OPTIONS: usize = 8;
const GMO_WAIT_INTERVAL: usize = 12;

/// The length of MQCBC version 1.
const CBC_LEN: usize = 48;

#[derive(Clone)]
struct Consumer {
    hconn: i32,
    hobj: i32,
    /// The z/OS MQMD and MQGMO given to MQCB.
    md: Vec<u8>,
    gmo: Vec<u8>,
    program: String,
    area: u32,
    max_len: usize,
    options: i32,
    event: bool,
    suspended: bool,
    since: Instant,
}

#[derive(Default)]
struct State {
    consumers: Vec<Consumer>,
    stop: bool,
    connection_area: u32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn int(m: &Machine, a: Option<usize>) -> Result<i32, String> {
    match a {
        Some(a) if a + 4 <= m.storage().len() => Ok(be(m.storage(), a)),
        _ => Err("MQ call: parameter missing or out of storage".into()),
    }
}

fn set_int(m: &mut Machine, a: Option<usize>, v: i32) {
    if let Some(a) = a
        && a + 4 <= m.storage().len()
    {
        m.storage_mut()[a..a + 4].copy_from_slice(&v.to_be_bytes());
    }
}

fn done(m: &mut Machine, addrs: &[Option<usize>], at: usize, cc: i32, rc: i32) -> i32 {
    set_int(m, addrs.get(at).copied().flatten(), cc);
    set_int(m, addrs.get(at + 1).copied().flatten(), rc);
    0
}

/// A copy of the z/OS structure at `a` of `layout` (None: not one).
fn structure(m: &Machine, a: Option<usize>, l: &crate::layout::Layout) -> Option<Vec<u8>> {
    let a = a?;
    let mem = m.storage();
    let head = mem.get(a..a + 8)?;
    if !l.is(head) {
        return None;
    }
    let n = l.len(l.version(head));
    mem.get(a..a + n).map(<[u8]>::to_vec)
}

/// MQCB(Hconn, Operation, CallbackDesc, Hobj, MsgDesc, GetMsgOpts,
/// CompCode, Reason)
pub fn mqcb(m: &mut Machine, addrs: &[Option<usize>]) -> Result<i32, String> {
    let a = |k: usize| addrs.get(k).copied().flatten();
    let h = int(m, a(0))?;
    if let Err((cc, rc)) = hconn(h) {
        return Ok(done(m, addrs, 6, cc, rc));
    }
    let op = int(m, a(1))?;
    let hobj = a(3).map_or(Ok(0), |x| int(m, Some(x)))?;
    match op {
        MQOP_REGISTER => {
            let Some(cbd) = a(2).and_then(|x| m.storage().get(x..x + 156).map(<[u8]>::to_vec))
            else {
                return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_CBD_ERROR));
            };
            let id: Vec<u8> = cbd[..4].iter().map(|&b| ebcdic::to_latin1(b)).collect();
            if id != b"CBD " {
                return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_CBD_ERROR));
            }
            let ty = be(&cbd, 8);
            let event = match ty {
                MQCBT_MESSAGE_CONSUMER => false,
                MQCBT_EVENT_HANDLER => true,
                _ => return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_CALLBACK_TYPE_ERROR)),
            };
            let program = String::from_utf8_lossy(&ebcdic::decode_latin1(&cbd[24..152]))
                .trim()
                .to_ascii_uppercase();
            if program.is_empty() {
                return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_CALLBACK_LINK_ERROR));
            }
            let (md, gmo) = if event {
                (Vec::new(), Vec::new())
            } else {
                let Some(md) = structure(m, a(4), &MD) else {
                    return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_MD_ERROR));
                };
                let Some(gmo) = structure(m, a(5), &GMO) else {
                    return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_GMO_ERROR));
                };
                (md, gmo)
            };
            let max = be(&cbd, 152);
            let c = Consumer {
                hconn: h,
                hobj: if event { 0 } else { hobj },
                md,
                gmo,
                program,
                area: be(&cbd, 16) as u32,
                max_len: if max < 0 { FULL_LENGTH } else { max as usize },
                options: be(&cbd, 12),
                event,
                suspended: false,
                since: Instant::now(),
            };
            let call = c.options & MQCBDO_REGISTER_CALL != 0;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.consumers
                    .retain(|x| !(x.hconn == h && x.event == event && x.hobj == c.hobj));
                s.consumers.push(c.clone());
            });
            if call {
                deliver(m, &mut None, &c, MQCBCT_REGISTER_CALL, (0, 0), None)?;
            }
        }
        MQOP_DEREGISTER | MQOP_SUSPEND | MQOP_RESUME => {
            let found: Vec<Consumer> = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let mut out = Vec::new();
                for c in s.consumers.iter_mut() {
                    if c.hconn == h && c.hobj == hobj {
                        c.suspended = op == MQOP_SUSPEND;
                        out.push(c.clone());
                    }
                }
                if op == MQOP_DEREGISTER {
                    s.consumers.retain(|c| !(c.hconn == h && c.hobj == hobj));
                }
                out
            });
            if found.is_empty() {
                return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_CALLBACK_LINK_ERROR));
            }
            if op == MQOP_DEREGISTER {
                for c in found
                    .iter()
                    .filter(|c| c.options & MQCBDO_DEREGISTER_CALL != 0)
                {
                    deliver(m, &mut None, c, MQCBCT_DEREGISTER_CALL, (0, 0), None)?;
                }
            }
        }
        _ => return Ok(done(m, addrs, 6, MQCC_FAILED, MQRC_OPERATION_ERROR)),
    }
    Ok(done(m, addrs, 6, 0, 0))
}

/// MQCTL(Hconn, Operation, ControlOpts, CompCode, Reason)
pub fn mqctl(m: &mut Machine, addrs: &[Option<usize>]) -> Result<i32, String> {
    let a = |k: usize| addrs.get(k).copied().flatten();
    let h = int(m, a(0))?;
    if let Err((cc, rc)) = hconn(h) {
        return Ok(done(m, addrs, 3, cc, rc));
    }
    let op = int(m, a(1))?;
    match op {
        MQOP_START | MQOP_START_WAIT => {
            let area = a(2)
                .and_then(|x| m.storage().get(x + 16..x + 20).map(|b| be(b, 0) as u32))
                .unwrap_or(0);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.stop = false;
                s.connection_area = area;
            });
            let cr = run(m, h, op == MQOP_START_WAIT)?;
            Ok(done(m, addrs, 3, cr.0, cr.1))
        }
        MQOP_STOP => {
            STATE.with(|s| s.borrow_mut().stop = true);
            Ok(done(m, addrs, 3, 0, 0))
        }
        MQOP_SUSPEND | MQOP_RESUME => {
            STATE.with(|s| {
                for c in s.borrow_mut().consumers.iter_mut().filter(|c| c.hconn == h) {
                    c.suspended = op == MQOP_SUSPEND;
                }
            });
            Ok(done(m, addrs, 3, 0, 0))
        }
        _ => Ok(done(m, addrs, 3, MQCC_FAILED, MQRC_OPERATION_ERROR)),
    }
}

fn consumers(h: i32) -> Vec<Consumer> {
    STATE.with(|s| {
        s.borrow()
            .consumers
            .iter()
            .filter(|c| c.hconn == h)
            .cloned()
            .collect()
    })
}

fn stopped() -> bool {
    STATE.with(|s| s.borrow().stop)
}

/// The consumers' loop of MQCTL START / START_WAIT.
fn run(m: &mut Machine, h: i32, wait_more: bool) -> Result<(i32, i32), String> {
    let mut scratch = None;
    for c in consumers(h)
        .iter()
        .filter(|c| c.options & MQCBDO_START_CALL != 0)
    {
        deliver(m, &mut scratch, c, MQCBCT_START_CALL, (0, 0), None)?;
    }
    let mut result = (0, 0);
    loop {
        let active: Vec<Consumer> = consumers(h)
            .into_iter()
            .filter(|c| !c.event && !c.suspended)
            .collect();
        if stopped() || active.is_empty() {
            break;
        }
        let mut any = false;
        for c in active {
            if stopped() {
                break;
            }
            let Ok(native_h) = hconn(c.hconn) else {
                break;
            };
            // the message, without waiting
            let mv = MD.version(&c.md);
            let gv = GMO.version(&c.gmo);
            let mut md = MD.to_native(&c.md, 0, mv);
            let mut gmo = GMO.to_native(&c.gmo, 0, gv);
            own_ccsid(&mut md.bytes);
            let options = ne(&gmo.bytes, GMO_OPTIONS);
            set_ne(&mut gmo.bytes, GMO_OPTIONS, options & !MQGMO_WAIT);
            let mut buf = vec![0u8; c.max_len];
            let (len, cr) =
                with_lib(|l| l.get(native_h, c.hobj, &mut md.bytes, &mut gmo.bytes, &mut buf))
                    .unwrap_or((0, (MQCC_FAILED, MQRC_ENVIRONMENT_ERROR)));
            set_ne(&mut gmo.bytes, GMO_OPTIONS, options);
            if cr.1 == MQRC_NO_MSG_AVAILABLE {
                // the wait interval passed without a message
                let interval = ne(&gmo.bytes, GMO_WAIT_INTERVAL);
                if options & MQGMO_WAIT != 0
                    && interval >= 0
                    && c.since.elapsed() >= Duration::from_millis(interval as u64)
                {
                    touch(&c);
                    deliver(m, &mut scratch, &c, MQCBCT_EVENT_CALL, cr, None)?;
                    any = true;
                }
                continue;
            }
            any = true;
            touch(&c);
            let mut zmd = c.md.clone();
            MD.to_zos(&md, &mut zmd, 0, mv);
            let mut zgmo = c.gmo.clone();
            GMO.to_zos(&gmo, &mut zgmo, 0, gv);
            if cr.0 == MQCC_FAILED {
                deliver(m, &mut scratch, &c, MQCBCT_EVENT_CALL, cr, None)?;
                if matches!(cr.1, 2009 | 2161 | 2162 | 2202 | 2203) {
                    result = cr;
                    STATE.with(|s| s.borrow_mut().stop = true);
                }
                continue;
            }
            let browse = options & (MQGMO_BROWSE_FIRST | MQGMO_BROWSE_NEXT) != 0;
            let ty = if browse {
                MQCBCT_MSG_NOT_REMOVED
            } else {
                MQCBCT_MSG_REMOVED
            };
            let k = (len.max(0) as usize).min(buf.len());
            deliver(
                m,
                &mut scratch,
                &c,
                ty,
                cr,
                Some((&zmd, &zgmo, &buf[..k], len)),
            )?;
        }
        if !any {
            if !wait_more {
                break;
            }
            wait(Duration::from_millis(50));
        }
    }
    for c in consumers(h)
        .iter()
        .filter(|c| c.options & MQCBDO_STOP_CALL != 0)
    {
        deliver(m, &mut scratch, c, MQCBCT_STOP_CALL, (0, 0), None)?;
    }
    STATE.with(|s| s.borrow_mut().stop = false);
    Ok(result)
}

/// Resets the wait interval of a consumer.
fn touch(c: &Consumer) {
    STATE.with(|s| {
        for x in s.borrow_mut().consumers.iter_mut() {
            if x.hconn == c.hconn && x.hobj == c.hobj && x.event == c.event {
                x.since = Instant::now();
            }
        }
    });
}

/// A message for a consumer: its MQMD and MQGMO (z/OS forms), data and
/// length.
type Delivery<'a> = (&'a [u8], &'a [u8], &'a [u8], i32);

/// Calls a consumer or event handler: (Hconn, MQMD, MQGMO, Buffer,
/// MQCBC) in storage of the run unit.
fn deliver(
    m: &mut Machine,
    scratch: &mut Option<(usize, usize)>,
    c: &Consumer,
    call_type: i32,
    cr: (i32, i32),
    msg: Option<Delivery>,
) -> Result<(), String> {
    let (md, gmo, data, len) = match msg {
        Some((md, gmo, data, len)) => (md.to_vec(), gmo.to_vec(), data.to_vec(), len),
        None => (c.md.clone(), c.gmo.clone(), Vec::new(), 0),
    };
    let area = STATE.with(|s| s.borrow().connection_area);
    let mut cbc = Vec::with_capacity(CBC_LEN);
    cbc.extend(ebcdic::encode("CBC "));
    for v in [
        1,
        call_type,
        c.hobj,
        c.area as i32,
        area as i32,
        cr.0,
        cr.1,
        0,
        len,
        c.max_len as i32,
        0,
    ] {
        cbc.extend(v.to_be_bytes());
    }
    let parts: [&[u8]; 5] = [&c.hconn.to_be_bytes(), &md, &gmo, &data, &cbc];
    // the arguments in a scratch area of the run unit, reused while it
    // is large enough
    let need: usize = parts.iter().map(|p| p.len().div_ceil(8) * 8 + 8).sum();
    let base = match *scratch {
        Some((a, cap)) if cap >= need => a,
        _ => {
            let a = m.getmain(need, 0);
            *scratch = Some((a, need));
            a
        }
    };
    let mut at = base;
    let mut refs = Vec::new();
    for p in parts {
        m.storage_mut()[at..at + p.len()].copy_from_slice(p);
        refs.push(XRef {
            addr: Some(at),
            len: p.len(),
            ..Default::default()
        });
        at += p.len().div_ceil(8) * 8 + 8;
    }
    let r = m.call_program(&c.program, &refs)?;
    if r.is_none() {
        return Err(format!("MQCTL: callback program {} not found", c.program));
    }
    // the callback may change its CallbackArea
    let cbc_at = refs[4].addr.unwrap_or(0);
    let new_area = be(m.storage(), cbc_at + 16) as u32;
    STATE.with(|s| {
        for x in s.borrow_mut().consumers.iter_mut() {
            if x.hconn == c.hconn && x.hobj == c.hobj && x.event == c.event {
                x.area = new_area;
            }
        }
    });
    Ok(())
}

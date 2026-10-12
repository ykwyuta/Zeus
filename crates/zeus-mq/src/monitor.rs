//! Messages of a queue for the region and the tools: the trigger monitor
//! of a CICS region (CKTI) reads the trigger messages (MQTM) the queue
//! manager puts on an initiation queue, in the z/OS form a program
//! RETRIEVEs (EBCDIC characters, big-endian integers); `zmq` puts and gets
//! text messages. The MQI calls are those of COBOL programs, on structures
//! built here in a work area.

use zeus_cobol::ebcdic;

use crate::mqi;

const MQOO_INPUT_SHARED: i32 = 2;
const MQOO_FAIL_IF_QUIESCING: i32 = 8192;
const MQGMO_NO_SYNCPOINT: i32 = 4;
const MQGMO_CONVERT: i32 = 16384;
const MQGMO_FAIL_IF_QUIESCING: i32 = 8192;
const MQRC_NO_MSG_AVAILABLE: i32 = 2033;
/// The length of MQTM.
pub const MQTM_LEN: usize = 684;
/// Offset of ApplId in MQTM.
pub const MQTM_APPLID: usize = 172;

/// A work area with the arguments of the calls at fixed offsets.
struct Work {
    mem: Vec<u8>,
}

const HCONN: usize = 0;
const OPTIONS: usize = 4;
const HOBJ: usize = 8;
const CC: usize = 12;
const RC: usize = 16;
const BUFLEN: usize = 20;
const DATALEN: usize = 24;
const QMGR: usize = 32;
const OD: usize = 80;
const MD: usize = 256;
const GMO: usize = 592;
const PMO: usize = 600;
const BUF: usize = 760;
/// The largest message read.
const MAX_MESSAGE: usize = 65536;

fn text(s: &str, len: usize) -> Vec<u8> {
    let mut b = ebcdic::encode(s);
    b.resize(len, ebcdic::SPACE);
    b
}

impl Work {
    fn new() -> Work {
        Work {
            mem: vec![0; BUF + MAX_MESSAGE],
        }
    }

    fn int(&self, at: usize) -> i32 {
        i32::from_be_bytes([
            self.mem[at],
            self.mem[at + 1],
            self.mem[at + 2],
            self.mem[at + 3],
        ])
    }

    fn set(&mut self, at: usize, v: i32) {
        self.mem[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }

    fn put(&mut self, at: usize, b: &[u8]) {
        self.mem[at..at + b.len()].copy_from_slice(b);
    }

    fn result(&self) -> (i32, i32) {
        (self.int(CC), self.int(RC))
    }

    /// MQMD version 1 for a get: any message, the program's code page.
    fn md(&mut self) {
        self.mem[MD..MD + 324].fill(0);
        self.put(MD, &text("MD", 4));
        self.set(MD + 4, 1);
        self.set(MD + 12, 8); // MsgType: datagram
        self.set(MD + 16, -1); // Expiry: unlimited
        self.set(MD + 24, 785); // Encoding: z/OS
        self.put(MD + 32, &text("", 8)); // Format
        self.set(MD + 40, -1); // Priority
        self.set(MD + 44, 2); // Persistence: as the queue
        self.put(MD + 100, &text("", 48 + 48 + 12));
        self.put(MD + 240, &text("", 32));
        self.put(MD + 276, &text("", 28 + 8 + 8 + 4));
    }
}

/// The trigger messages on initiation queue `initq` (none when there is
/// no queue manager): each one MQTM as a z/OS program sees it.
pub fn trigger_messages(initq: &str) -> Result<Vec<Vec<u8>>, String> {
    get_messages(initq, 0)
}

type Call = fn(&mut [u8], &[Option<usize>]) -> Result<i32, String>;

fn call(w: &mut Work, f: Call, args: &[usize]) -> Result<(i32, i32), String> {
    let a: Vec<Option<usize>> = args.iter().map(|&x| Some(x)).collect();
    f(&mut w.mem, &a).map(|_| w.result())
}

/// Connects and opens `queue` with `options`; the work area.
fn open(queue: &str, options: i32) -> Result<Work, String> {
    let mut w = Work::new();
    w.put(QMGR, &text("", 48));
    let (cc, rc) = call(&mut w, mqi::mqconn, &[QMGR, HCONN, CC, RC])?;
    if cc == mqi::MQCC_FAILED {
        return Err(format!("MQCONN failed: reason {rc}"));
    }
    // MQOD version 1 for the queue
    w.put(OD, &text("OD", 4));
    w.set(OD + 4, 1);
    w.set(OD + 8, 1); // a queue
    w.put(OD + 12, &text(queue, 48));
    w.put(OD + 60, &text("", 48 + 48 + 12));
    w.set(OPTIONS, options);
    let (cc, rc) = call(&mut w, mqi::mqopen, &[HCONN, OD, OPTIONS, HOBJ, CC, RC])?;
    if cc == mqi::MQCC_FAILED {
        let _ = call(&mut w, mqi::mqdisc, &[HCONN, CC, RC]);
        return Err(format!("MQOPEN {queue} failed: reason {rc}"));
    }
    Ok(w)
}

fn close(mut w: Work) {
    w.set(OPTIONS, 0);
    let _ = call(&mut w, mqi::mqclose, &[HCONN, HOBJ, OPTIONS, CC, RC]);
    let _ = call(&mut w, mqi::mqdisc, &[HCONN, CC, RC]);
}

/// Puts a message (MQFMT_STRING in the code page) on `queue`, with the
/// queue for replies when `reply_to` is given.
pub fn put_message(queue: &str, data: &[u8], reply_to: Option<&str>) -> Result<(), String> {
    const MQOO_OUTPUT: i32 = 16;
    const MQPMO_NO_SYNCPOINT: i32 = 4;
    let mut w = open(queue, MQOO_OUTPUT | MQOO_FAIL_IF_QUIESCING)?;
    w.md();
    w.put(MD + 32, &text("MQSTR", 8));
    if let Some(q) = reply_to {
        w.put(MD + 100, &text(q, 48));
    }
    w.mem[PMO..PMO + 128].fill(0);
    w.put(PMO, &text("PMO", 4));
    w.set(PMO + 4, 1);
    w.set(PMO + 8, MQPMO_NO_SYNCPOINT);
    w.put(PMO + 32, &text("", 96));
    let n = data.len().min(MAX_MESSAGE);
    w.put(BUF, &data[..n]);
    w.set(BUFLEN, n as i32);
    let r = call(
        &mut w,
        mqi::mqput,
        &[HCONN, HOBJ, MD, PMO, BUFLEN, BUF, CC, RC],
    );
    let r = match r {
        Ok((cc, rc)) if cc == mqi::MQCC_FAILED => Err(format!("MQPUT {queue} failed: reason {rc}")),
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    };
    close(w);
    r
}

/// The messages on `queue` (taken off it), converted to the code page;
/// with `wait` (milliseconds) more than 0, also those arriving until none
/// has come for that long.
pub fn get_messages(initq: &str, wait: i32) -> Result<Vec<Vec<u8>>, String> {
    const MQGMO_WAIT: i32 = 1;
    let mut w = open(initq, MQOO_INPUT_SHARED | MQOO_FAIL_IF_QUIESCING)?;
    let mut out = Vec::new();
    let mut error = None;
    loop {
        w.md();
        w.mem[GMO..GMO + 72].fill(0);
        w.put(GMO, &text("GMO", 4));
        w.set(GMO + 4, 1);
        let mut options = MQGMO_NO_SYNCPOINT | MQGMO_CONVERT | MQGMO_FAIL_IF_QUIESCING;
        if wait > 0 {
            options |= MQGMO_WAIT;
            w.set(GMO + 12, wait);
        }
        w.set(GMO + 8, options);
        w.put(GMO + 24, &text("", 48));
        w.set(BUFLEN, MAX_MESSAGE as i32);
        let (cc, rc) = call(
            &mut w,
            mqi::mqget,
            &[HCONN, HOBJ, MD, GMO, BUFLEN, BUF, DATALEN, CC, RC],
        )?;
        if cc == mqi::MQCC_FAILED {
            if rc != MQRC_NO_MSG_AVAILABLE {
                error = Some(format!("MQGET {initq} failed: reason {rc}"));
            }
            break;
        }
        let n = (w.int(DATALEN).max(0) as usize).min(MAX_MESSAGE);
        out.push(w.mem[BUF..BUF + n].to_vec());
    }
    close(w);
    match error {
        Some(e) if out.is_empty() => Err(e),
        _ => Ok(out),
    }
}

/// The transaction a trigger message starts (its ApplId).
pub fn transaction(mqtm: &[u8]) -> String {
    let b = mqtm.get(MQTM_APPLID..MQTM_APPLID + 256).unwrap_or(&[]);
    let t: String = ebcdic::decode_latin1(b)
        .iter()
        .map(|&c| c as char)
        .collect();
    t.split_whitespace()
        .next()
        .unwrap_or("")
        .chars()
        .take(4)
        .collect()
}

/// The queue a trigger message is for (its QName).
pub fn queue(mqtm: &[u8]) -> String {
    let b = mqtm.get(8..56).unwrap_or(&[]);
    let t: String = ebcdic::decode_latin1(b)
        .iter()
        .map(|&c| c as char)
        .collect();
    t.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mqtm_fields() {
        let mut tm = text("TM", MQTM_LEN);
        tm[8..56].copy_from_slice(&text("CARD.DEMO.REQUEST.DATE", 48));
        tm[MQTM_APPLID..MQTM_APPLID + 256].copy_from_slice(&text("CDRD extra", 256));
        assert_eq!(queue(&tm), "CARD.DEMO.REQUEST.DATE");
        assert_eq!(transaction(&tm), "CDRD");
    }
}

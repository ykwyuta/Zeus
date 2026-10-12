//! The IBM MQ C client library (libmqic_r), loaded at run time. This is
//! the only module with unsafe code: the calls pass buffers laid out as
//! the native C structures (see `layout`).
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};

type Long = i32;
type ConnX = unsafe extern "C" fn(*const u8, *mut u8, *mut Long, *mut Long, *mut Long);
type Disc = unsafe extern "C" fn(*mut Long, *mut Long, *mut Long);
type Open = unsafe extern "C" fn(Long, *mut u8, Long, *mut Long, *mut Long, *mut Long);
type Close = unsafe extern "C" fn(Long, *mut Long, Long, *mut Long, *mut Long);
type Put =
    unsafe extern "C" fn(Long, Long, *mut u8, *mut u8, Long, *const u8, *mut Long, *mut Long);
type Put1 =
    unsafe extern "C" fn(Long, *mut u8, *mut u8, *mut u8, Long, *const u8, *mut Long, *mut Long);
type Get = unsafe extern "C" fn(
    Long,
    Long,
    *mut u8,
    *mut u8,
    Long,
    *mut u8,
    *mut Long,
    *mut Long,
    *mut Long,
);
type Sync = unsafe extern "C" fn(Long, *mut Long, *mut Long);
type Inq = unsafe extern "C" fn(
    Long,
    Long,
    Long,
    *const Long,
    Long,
    *mut Long,
    Long,
    *mut u8,
    *mut Long,
    *mut Long,
);

/// Completion code and reason code of a call.
pub type Cr = (i32, i32);

/// The entry points of the client library.
pub struct Library {
    _lib: libloading::Library,
    connx: ConnX,
    disc: Disc,
    open: Open,
    close: Close,
    put: Put,
    put1: Put1,
    get: Get,
    cmit: Sync,
    back: Sync,
    inq: Inq,
}

/// Candidate files of the client library: ZEUS_MQ_LIB (the library or
/// its directory), the MQ installation, and the dynamic loader's path.
fn candidates() -> Vec<PathBuf> {
    let name = "libmqic_r.so";
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("ZEUS_MQ_LIB") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            v.push(p.join(name));
        } else {
            v.push(p);
        }
    }
    if let Ok(p) = std::env::var("MQ_INSTALLATION_PATH") {
        v.push(Path::new(&p).join("lib64").join(name));
    }
    v.push(PathBuf::from("/opt/mqm/lib64").join(name));
    v.push(PathBuf::from(name));
    v
}

impl Library {
    pub fn load() -> Result<Library, String> {
        let mut errors = Vec::new();
        for c in candidates() {
            if c.components().count() > 1 && !c.exists() {
                continue;
            }
            // SAFETY: loading the IBM MQ client runs its initialisers,
            // which have no preconditions.
            match unsafe { libloading::Library::new(&c) } {
                Ok(lib) => return Library::bind(lib).map_err(|e| format!("{}: {e}", c.display())),
                Err(e) => errors.push(format!("{}: {e}", c.display())),
            }
        }
        Err(format!(
            "the IBM MQ client library was not found ({})",
            errors.join("; ")
        ))
    }

    fn bind(lib: libloading::Library) -> Result<Library, libloading::Error> {
        // SAFETY: the symbols are the MQI functions with the C signatures
        // declared above.
        unsafe {
            Ok(Library {
                connx: *lib.get::<ConnX>(b"MQCONNX\0")?,
                disc: *lib.get::<Disc>(b"MQDISC\0")?,
                open: *lib.get::<Open>(b"MQOPEN\0")?,
                close: *lib.get::<Close>(b"MQCLOSE\0")?,
                put: *lib.get::<Put>(b"MQPUT\0")?,
                put1: *lib.get::<Put1>(b"MQPUT1\0")?,
                get: *lib.get::<Get>(b"MQGET\0")?,
                cmit: *lib.get::<Sync>(b"MQCMIT\0")?,
                back: *lib.get::<Sync>(b"MQBACK\0")?,
                inq: *lib.get::<Inq>(b"MQINQ\0")?,
                _lib: lib,
            })
        }
    }

    /// MQCONNX: `qmgr` is the 48-byte name; `cno` a native MQCNO whose
    /// pointers (client credentials) stay valid during the call.
    pub fn connx(&self, qmgr: &[u8; 48], cno: &mut [u8]) -> (i32, Cr) {
        let (mut h, mut cc, mut rc) = (0, 0, 0);
        // SAFETY: the name is 48 bytes, the MQCNO buffer has the size of
        // its version and the outputs are valid integers.
        unsafe { (self.connx)(qmgr.as_ptr(), cno.as_mut_ptr(), &mut h, &mut cc, &mut rc) };
        (h, (cc, rc))
    }

    pub fn disc(&self, h: i32) -> Cr {
        let (mut h, mut cc, mut rc) = (h, 0, 0);
        // SAFETY: integer arguments only.
        unsafe { (self.disc)(&mut h, &mut cc, &mut rc) };
        (cc, rc)
    }

    pub fn open(&self, h: i32, od: &mut [u8], options: i32) -> (i32, Cr) {
        let (mut o, mut cc, mut rc) = (0, 0, 0);
        // SAFETY: `od` is a native MQOD of its version's length.
        unsafe { (self.open)(h, od.as_mut_ptr(), options, &mut o, &mut cc, &mut rc) };
        (o, (cc, rc))
    }

    pub fn close(&self, h: i32, o: i32, options: i32) -> (i32, Cr) {
        let (mut o, mut cc, mut rc) = (o, 0, 0);
        // SAFETY: integer arguments only.
        unsafe { (self.close)(h, &mut o, options, &mut cc, &mut rc) };
        (o, (cc, rc))
    }

    pub fn put(&self, h: i32, o: i32, md: &mut [u8], pmo: &mut [u8], data: &[u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: native MQMD and MQPMO buffers; the message is `data`.
        unsafe {
            (self.put)(
                h,
                o,
                md.as_mut_ptr(),
                pmo.as_mut_ptr(),
                data.len() as i32,
                data.as_ptr(),
                &mut cc,
                &mut rc,
            )
        };
        (cc, rc)
    }

    pub fn put1(&self, h: i32, od: &mut [u8], md: &mut [u8], pmo: &mut [u8], data: &[u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: native MQOD, MQMD and MQPMO buffers; the message is `data`.
        unsafe {
            (self.put1)(
                h,
                od.as_mut_ptr(),
                md.as_mut_ptr(),
                pmo.as_mut_ptr(),
                data.len() as i32,
                data.as_ptr(),
                &mut cc,
                &mut rc,
            )
        };
        (cc, rc)
    }

    /// MQGET into `buf`; the length of the message.
    pub fn get(&self, h: i32, o: i32, md: &mut [u8], gmo: &mut [u8], buf: &mut [u8]) -> (i32, Cr) {
        let (mut n, mut cc, mut rc) = (0, 0, 0);
        // SAFETY: native MQMD and MQGMO buffers; the library writes at
        // most `buf.len()` bytes.
        unsafe {
            (self.get)(
                h,
                o,
                md.as_mut_ptr(),
                gmo.as_mut_ptr(),
                buf.len() as i32,
                buf.as_mut_ptr(),
                &mut n,
                &mut cc,
                &mut rc,
            )
        };
        (n, (cc, rc))
    }

    pub fn cmit(&self, h: i32) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: integer arguments only.
        unsafe { (self.cmit)(h, &mut cc, &mut rc) };
        (cc, rc)
    }

    pub fn back(&self, h: i32) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: integer arguments only.
        unsafe { (self.back)(h, &mut cc, &mut rc) };
        (cc, rc)
    }

    /// MQINQ: `ints` and `chars` receive the attributes.
    pub fn inq(&self, h: i32, o: i32, selectors: &[i32], ints: &mut [i32], chars: &mut [u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: the counts are the lengths of the slices passed.
        unsafe {
            (self.inq)(
                h,
                o,
                selectors.len() as i32,
                selectors.as_ptr(),
                ints.len() as i32,
                ints.as_mut_ptr(),
                chars.len() as i32,
                chars.as_mut_ptr(),
                &mut cc,
                &mut rc,
            )
        };
        (cc, rc)
    }
}

/// Fills a native MQCNO (version 5) for a client connection with the
/// MQCSP `csp` (user and password). The MQCNO points at `csp`, and the
/// MQCSP at `user` and `password`: all must live until MQCONNX returns.
pub fn client_options(cno: &mut [u8; 200], csp: Option<(&mut [u8; 56], &[u8], &[u8])>) {
    cno[0..4].copy_from_slice(b"CNO ");
    cno[4..8].copy_from_slice(&5i32.to_ne_bytes());
    // MQCNO_CLIENT_BINDING
    cno[8..12].copy_from_slice(&2048i32.to_ne_bytes());
    // ConnTag: MQCT_NONE (nulls); ConnectionId: MQCONNID_NONE
    if let Some((c, user, pw)) = csp {
        c[0..4].copy_from_slice(b"CSP ");
        c[4..8].copy_from_slice(&1i32.to_ne_bytes());
        // MQCSP_AUTH_USER_ID_AND_PWD
        c[8..12].copy_from_slice(&1i32.to_ne_bytes());
        c[16..24].copy_from_slice(&(user.as_ptr() as usize).to_ne_bytes());
        c[28..32].copy_from_slice(&(user.len() as i32).to_ne_bytes());
        c[40..48].copy_from_slice(&(pw.as_ptr() as usize).to_ne_bytes());
        c[52..56].copy_from_slice(&(pw.len() as i32).to_ne_bytes());
        cno[192..200].copy_from_slice(&(c.as_ptr() as usize).to_ne_bytes());
    }
}

/// Sets MQSERVER (the client channel) for the library, which reads it
/// when it connects.
pub fn set_mqserver(value: &str) {
    // SAFETY: called on the run unit's thread before the first MQCONNX;
    // no other thread reads the environment at that time.
    unsafe { std::env::set_var("MQSERVER", value) };
}

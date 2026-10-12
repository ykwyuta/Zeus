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

type Set = unsafe extern "C" fn(
    Long,
    Long,
    Long,
    *const Long,
    Long,
    *const Long,
    Long,
    *const u8,
    *mut Long,
    *mut Long,
);
type Sub = unsafe extern "C" fn(Long, *mut u8, *mut Long, *mut Long, *mut Long, *mut Long);
type SubRq = unsafe extern "C" fn(Long, Long, Long, *mut u8, *mut Long, *mut Long);
type CrtMh = unsafe extern "C" fn(Long, *mut u8, *mut i64, *mut Long, *mut Long);
type DltMh = unsafe extern "C" fn(Long, *mut i64, *mut u8, *mut Long, *mut Long);
type SetMp = unsafe extern "C" fn(
    Long,
    i64,
    *mut u8,
    *mut u8,
    *mut u8,
    Long,
    Long,
    *const u8,
    *mut Long,
    *mut Long,
);
type InqMp = unsafe extern "C" fn(
    Long,
    i64,
    *mut u8,
    *mut u8,
    *mut u8,
    *mut Long,
    Long,
    *mut u8,
    *mut Long,
    *mut Long,
    *mut Long,
);
type DltMp = unsafe extern "C" fn(Long, i64, *mut u8, *mut u8, *mut Long, *mut Long);

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
    set: Set,
    sub: Sub,
    subrq: SubRq,
    crtmh: CrtMh,
    dltmh: DltMh,
    setmp: SetMp,
    inqmp: InqMp,
    dltmp: DltMp,
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
                set: *lib.get::<Set>(b"MQSET\0")?,
                sub: *lib.get::<Sub>(b"MQSUB\0")?,
                subrq: *lib.get::<SubRq>(b"MQSUBRQ\0")?,
                crtmh: *lib.get::<CrtMh>(b"MQCRTMH\0")?,
                dltmh: *lib.get::<DltMh>(b"MQDLTMH\0")?,
                setmp: *lib.get::<SetMp>(b"MQSETMP\0")?,
                inqmp: *lib.get::<InqMp>(b"MQINQMP\0")?,
                dltmp: *lib.get::<DltMp>(b"MQDLTMP\0")?,
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

impl Library {
    /// MQSET: the attributes of `selectors` to `ints` and `chars`.
    pub fn set(&self, h: i32, o: i32, selectors: &[i32], ints: &[i32], chars: &[u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: the counts are the lengths of the slices passed.
        unsafe {
            (self.set)(
                h,
                o,
                selectors.len() as i32,
                selectors.as_ptr(),
                ints.len() as i32,
                ints.as_ptr(),
                chars.len() as i32,
                chars.as_ptr(),
                &mut cc,
                &mut rc,
            )
        };
        (cc, rc)
    }

    /// MQSUB: `sd` a native MQSD whose strings stay valid during the call;
    /// the object handle in and out, the subscription handle out.
    pub fn sub(&self, h: i32, sd: &mut [u8], hobj: &mut i32) -> (i32, Cr) {
        let (mut hsub, mut cc, mut rc) = (0, 0, 0);
        // SAFETY: the MQSD buffer has the size of its version.
        unsafe { (self.sub)(h, sd.as_mut_ptr(), hobj, &mut hsub, &mut cc, &mut rc) };
        (hsub, (cc, rc))
    }

    /// MQSUBRQ with a native MQSRO.
    pub fn subrq(&self, h: i32, hsub: i32, action: i32, sro: &mut [u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: the MQSRO buffer has the size of its version.
        unsafe { (self.subrq)(h, hsub, action, sro.as_mut_ptr(), &mut cc, &mut rc) };
        (cc, rc)
    }

    /// MQCRTMH: a message handle.
    pub fn crtmh(&self, h: i32, cmho: &mut [u8]) -> (i64, Cr) {
        let (mut hmsg, mut cc, mut rc) = (0i64, 0, 0);
        // SAFETY: the MQCMHO buffer has the size of its version.
        unsafe { (self.crtmh)(h, cmho.as_mut_ptr(), &mut hmsg, &mut cc, &mut rc) };
        (hmsg, (cc, rc))
    }

    /// MQDLTMH: the handle becomes MQHM_UNUSABLE_HMSG.
    pub fn dltmh(&self, h: i32, hmsg: &mut i64, dmho: &mut [u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: the MQDMHO buffer has the size of its version.
        unsafe { (self.dltmh)(h, hmsg, dmho.as_mut_ptr(), &mut cc, &mut rc) };
        (cc, rc)
    }

    /// MQSETMP: `name` a native MQCHARV, `value` the property's value.
    #[allow(clippy::too_many_arguments)]
    pub fn setmp(
        &self,
        h: i32,
        hmsg: i64,
        smpo: &mut [u8],
        name: &mut [u8],
        pd: &mut [u8],
        ty: i32,
        value: &[u8],
    ) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: the structures have the sizes of their versions and the
        // value length is the slice's.
        unsafe {
            (self.setmp)(
                h,
                hmsg,
                smpo.as_mut_ptr(),
                name.as_mut_ptr(),
                pd.as_mut_ptr(),
                ty,
                value.len() as i32,
                value.as_ptr(),
                &mut cc,
                &mut rc,
            )
        };
        (cc, rc)
    }

    /// MQINQMP: the property's type in and out, its value into `value`;
    /// returns its length.
    #[allow(clippy::too_many_arguments)]
    pub fn inqmp(
        &self,
        h: i32,
        hmsg: i64,
        impo: &mut [u8],
        name: &mut [u8],
        pd: &mut [u8],
        ty: &mut i32,
        value: &mut [u8],
    ) -> (i32, Cr) {
        let (mut len, mut cc, mut rc) = (0, 0, 0);
        // SAFETY: the structures have the sizes of their versions and the
        // value length is the slice's.
        unsafe {
            (self.inqmp)(
                h,
                hmsg,
                impo.as_mut_ptr(),
                name.as_mut_ptr(),
                pd.as_mut_ptr(),
                ty,
                value.len() as i32,
                value.as_mut_ptr(),
                &mut len,
                &mut cc,
                &mut rc,
            )
        };
        (len, (cc, rc))
    }

    /// MQDLTMP.
    pub fn dltmp(&self, h: i32, hmsg: i64, dmpo: &mut [u8], name: &mut [u8]) -> Cr {
        let (mut cc, mut rc) = (0, 0);
        // SAFETY: the structures have the sizes of their versions.
        unsafe {
            (self.dltmp)(
                h,
                hmsg,
                dmpo.as_mut_ptr(),
                name.as_mut_ptr(),
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

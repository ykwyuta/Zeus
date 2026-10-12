//! IBM MQ interface definitions for COBOL (COPY CMQV, CMQODV, CMQMDV,
//! CMQPMOV, CMQGMOV, CMQTMV, CMQSDV, CMQSROV, CMQCMHOV, CMQDMHOV,
//! CMQDMPOV, CMQSMPOV, CMQIMPOV, CMQPDV, CMQCBDV, CMQCBCV, CMQCTLOV and
//! their list forms ...L without initial values). Written for Zeus from
//! the documented MQI structure layouts (31-bit: 4-byte pointers) and
//! constant values, with the z/OS values of the platform-dependent ones
//! (MQENC_NATIVE 785). The structures are those of the versions the Zeus
//! MQ interface converts: MQOD 4, MQMD 2, MQPMO 3, MQGMO 4, MQTM 1 and
//! version 1 of the others.
//!
//! The same definitions in PL/I ([`pli_member`]): %INCLUDE CMQP (the
//! constants, as named constants with `_` for `-`), CMQEPP (the MQI
//! entries) and a member per structure (CMQODP, CMQMDP, CMQPMOP, ...)
//! that declares it as a level-1 structure with its initial values.

/// A data item of a member: name, PICTURE (None: a group; "CHARV": an
/// MQCHARV; "POINTER": a pointer) and initial value.
type Item = (u8, &'static str, Option<&'static str>, Option<&'static str>);

const L: Option<&str> = Some("S9(9) BINARY");
/// MQINT64 (message handles).
const L64: Option<&str> = Some("S9(18) BINARY");
const P: Option<&str> = Some("POINTER");
const V: Option<&str> = Some("CHARV");

fn text(items: &[Item], values: bool) -> String {
    let mut s = String::new();
    for &(level, name, pic, value) in items {
        item(&mut s, level, name, pic, value, values);
    }
    s
}

fn item(
    s: &mut String,
    level: u8,
    name: &str,
    pic: Option<&str>,
    value: Option<&str>,
    values: bool,
) {
    let indent = " ".repeat(10 + (level as usize / 5 - 2) * 2);
    match pic {
        None => s.push_str(&format!("{indent}{level} {name}.\n")),
        // an MQCHARV
        Some("CHARV") => {
            s.push_str(&format!("{indent}{level} {name}.\n"));
            item(s, level + 5, &format!("{name}-VSPTR"), P, None, values);
            for (n, v) in [
                ("VSOFFSET", "0"),
                ("VSBUFSIZE", "0"),
                ("VSLENGTH", "0"),
                ("VSCCSID", "-3"),
            ] {
                item(s, level + 5, &format!("{name}-{n}"), L, Some(v), values);
            }
        }
        Some("POINTER") => {
            s.push_str(&format!("{indent}{level} {name}\n"));
            if values {
                s.push_str(&format!("{indent}    POINTER VALUE NULL.\n"));
            } else {
                s.push_str(&format!("{indent}    POINTER.\n"));
            }
        }
        Some(p) => {
            s.push_str(&format!("{indent}{level} {name}\n"));
            match value.filter(|_| values) {
                Some(v) => s.push_str(&format!("{indent}    PIC {p} VALUE {v}.\n")),
                None => s.push_str(&format!("{indent}    PIC {p}.\n")),
            }
        }
    }
}

const MQOD: &[Item] = &[
    (10, "MQOD", None, None),
    (15, "MQOD-STRUCID", Some("X(4)"), Some("'OD  '")),
    (15, "MQOD-VERSION", L, Some("1")),
    (15, "MQOD-OBJECTTYPE", L, Some("1")),
    (15, "MQOD-OBJECTNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQOD-OBJECTQMGRNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQOD-DYNAMICQNAME", Some("X(48)"), Some("'CSQ.*'")),
    (15, "MQOD-ALTERNATEUSERID", Some("X(12)"), Some("SPACES")),
    // version 2
    (15, "MQOD-RECSPRESENT", L, Some("0")),
    (15, "MQOD-KNOWNDESTCOUNT", L, Some("0")),
    (15, "MQOD-UNKNOWNDESTCOUNT", L, Some("0")),
    (15, "MQOD-INVALIDDESTCOUNT", L, Some("0")),
    (15, "MQOD-OBJECTRECOFFSET", L, Some("0")),
    (15, "MQOD-RESPONSERECOFFSET", L, Some("0")),
    (15, "MQOD-OBJECTRECPTR", P, None),
    (15, "MQOD-RESPONSERECPTR", P, None),
    // version 3
    (
        15,
        "MQOD-ALTERNATESECURITYID",
        Some("X(40)"),
        Some("LOW-VALUES"),
    ),
    (15, "MQOD-RESOLVEDQNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQOD-RESOLVEDQMGRNAME", Some("X(48)"), Some("SPACES")),
    // version 4
    (15, "MQOD-OBJECTSTRING", V, None),
    (15, "MQOD-SELECTIONSTRING", V, None),
    (15, "MQOD-RESOBJECTSTRING", V, None),
    (15, "MQOD-RESOLVEDTYPE", L, Some("0")),
];

const MQMD: &[Item] = &[
    (10, "MQMD", None, None),
    (15, "MQMD-STRUCID", Some("X(4)"), Some("'MD  '")),
    (15, "MQMD-VERSION", L, Some("1")),
    (15, "MQMD-REPORT", L, Some("0")),
    (15, "MQMD-MSGTYPE", L, Some("8")),
    (15, "MQMD-EXPIRY", L, Some("-1")),
    (15, "MQMD-FEEDBACK", L, Some("0")),
    (15, "MQMD-ENCODING", L, Some("785")),
    (15, "MQMD-CODEDCHARSETID", L, Some("0")),
    (15, "MQMD-FORMAT", Some("X(8)"), Some("SPACES")),
    (15, "MQMD-PRIORITY", L, Some("-1")),
    (15, "MQMD-PERSISTENCE", L, Some("2")),
    (15, "MQMD-MSGID", Some("X(24)"), Some("LOW-VALUES")),
    (15, "MQMD-CORRELID", Some("X(24)"), Some("LOW-VALUES")),
    (15, "MQMD-BACKOUTCOUNT", L, Some("0")),
    (15, "MQMD-REPLYTOQ", Some("X(48)"), Some("SPACES")),
    (15, "MQMD-REPLYTOQMGR", Some("X(48)"), Some("SPACES")),
    (15, "MQMD-USERIDENTIFIER", Some("X(12)"), Some("SPACES")),
    (
        15,
        "MQMD-ACCOUNTINGTOKEN",
        Some("X(32)"),
        Some("LOW-VALUES"),
    ),
    (15, "MQMD-APPLIDENTITYDATA", Some("X(32)"), Some("SPACES")),
    (15, "MQMD-PUTAPPLTYPE", L, Some("0")),
    (15, "MQMD-PUTAPPLNAME", Some("X(28)"), Some("SPACES")),
    (15, "MQMD-PUTDATE", Some("X(8)"), Some("SPACES")),
    (15, "MQMD-PUTTIME", Some("X(8)"), Some("SPACES")),
    (15, "MQMD-APPLORIGINDATA", Some("X(4)"), Some("SPACES")),
    (15, "MQMD-GROUPID", Some("X(24)"), Some("LOW-VALUES")),
    (15, "MQMD-MSGSEQNUMBER", L, Some("1")),
    (15, "MQMD-OFFSET", L, Some("0")),
    (15, "MQMD-MSGFLAGS", L, Some("0")),
    (15, "MQMD-ORIGINALLENGTH", L, Some("-1")),
];

const MQPMO: &[Item] = &[
    (10, "MQPMO", None, None),
    (15, "MQPMO-STRUCID", Some("X(4)"), Some("'PMO '")),
    (15, "MQPMO-VERSION", L, Some("1")),
    (15, "MQPMO-OPTIONS", L, Some("0")),
    (15, "MQPMO-TIMEOUT", L, Some("-1")),
    (15, "MQPMO-CONTEXT", L, Some("0")),
    (15, "MQPMO-KNOWNDESTCOUNT", L, Some("0")),
    (15, "MQPMO-UNKNOWNDESTCOUNT", L, Some("0")),
    (15, "MQPMO-INVALIDDESTCOUNT", L, Some("0")),
    (15, "MQPMO-RESOLVEDQNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQPMO-RESOLVEDQMGRNAME", Some("X(48)"), Some("SPACES")),
    // version 2
    (15, "MQPMO-RECSPRESENT", L, Some("0")),
    (15, "MQPMO-PUTMSGRECFIELDS", L, Some("0")),
    (15, "MQPMO-PUTMSGRECOFFSET", L, Some("0")),
    (15, "MQPMO-RESPONSERECOFFSET", L, Some("0")),
    (15, "MQPMO-PUTMSGRECPTR", P, None),
    (15, "MQPMO-RESPONSERECPTR", P, None),
    // version 3
    (15, "MQPMO-ORIGINALMSGHANDLE", L64, Some("0")),
    (15, "MQPMO-NEWMSGHANDLE", L64, Some("0")),
    (15, "MQPMO-ACTION", L, Some("0")),
    (15, "MQPMO-PUBLEVEL", L, Some("9")),
];

const MQGMO: &[Item] = &[
    (10, "MQGMO", None, None),
    (15, "MQGMO-STRUCID", Some("X(4)"), Some("'GMO '")),
    (15, "MQGMO-VERSION", L, Some("1")),
    (15, "MQGMO-OPTIONS", L, Some("0")),
    (15, "MQGMO-WAITINTERVAL", L, Some("0")),
    (15, "MQGMO-SIGNAL1", L, Some("0")),
    (15, "MQGMO-SIGNAL2", L, Some("0")),
    (15, "MQGMO-RESOLVEDQNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQGMO-MATCHOPTIONS", L, Some("3")),
    (15, "MQGMO-GROUPSTATUS", Some("X"), Some("SPACE")),
    (15, "MQGMO-SEGMENTSTATUS", Some("X"), Some("SPACE")),
    (15, "MQGMO-SEGMENTATION", Some("X"), Some("SPACE")),
    (15, "MQGMO-RESERVED1", Some("X"), Some("SPACE")),
    (15, "MQGMO-MSGTOKEN", Some("X(16)"), Some("LOW-VALUES")),
    (15, "MQGMO-RETURNEDLENGTH", L, Some("-1")),
    // version 4
    (15, "MQGMO-RESERVED2", L, Some("0")),
    (15, "MQGMO-MSGHANDLE", L64, Some("0")),
];

const MQSD: &[Item] = &[
    (10, "MQSD", None, None),
    (15, "MQSD-STRUCID", Some("X(4)"), Some("'SD  '")),
    (15, "MQSD-VERSION", L, Some("1")),
    (15, "MQSD-OPTIONS", L, Some("0")),
    (15, "MQSD-OBJECTNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQSD-ALTERNATEUSERID", Some("X(12)"), Some("SPACES")),
    (
        15,
        "MQSD-ALTERNATESECURITYID",
        Some("X(40)"),
        Some("LOW-VALUES"),
    ),
    (15, "MQSD-SUBEXPIRY", L, Some("-1")),
    (15, "MQSD-OBJECTSTRING", V, None),
    (15, "MQSD-SUBNAME", V, None),
    (15, "MQSD-SUBUSERDATA", V, None),
    (15, "MQSD-SUBCORRELID", Some("X(24)"), Some("LOW-VALUES")),
    (15, "MQSD-PUBPRIORITY", L, Some("-3")),
    (
        15,
        "MQSD-PUBACCOUNTINGTOKEN",
        Some("X(32)"),
        Some("LOW-VALUES"),
    ),
    (
        15,
        "MQSD-PUBAPPLIDENTITYDATA",
        Some("X(32)"),
        Some("SPACES"),
    ),
    (15, "MQSD-SELECTIONSTRING", V, None),
    (15, "MQSD-SUBLEVEL", L, Some("1")),
    (15, "MQSD-RESOBJECTSTRING", V, None),
];

const MQSRO: &[Item] = &[
    (10, "MQSRO", None, None),
    (15, "MQSRO-STRUCID", Some("X(4)"), Some("'SRO '")),
    (15, "MQSRO-VERSION", L, Some("1")),
    (15, "MQSRO-OPTIONS", L, Some("0")),
    (15, "MQSRO-NUMPUBS", L, Some("0")),
];

const MQCMHO: &[Item] = &[
    (10, "MQCMHO", None, None),
    (15, "MQCMHO-STRUCID", Some("X(4)"), Some("'CMHO'")),
    (15, "MQCMHO-VERSION", L, Some("1")),
    (15, "MQCMHO-OPTIONS", L, Some("0")),
];

const MQDMHO: &[Item] = &[
    (10, "MQDMHO", None, None),
    (15, "MQDMHO-STRUCID", Some("X(4)"), Some("'DMHO'")),
    (15, "MQDMHO-VERSION", L, Some("1")),
    (15, "MQDMHO-OPTIONS", L, Some("0")),
];

const MQDMPO: &[Item] = &[
    (10, "MQDMPO", None, None),
    (15, "MQDMPO-STRUCID", Some("X(4)"), Some("'DMPO'")),
    (15, "MQDMPO-VERSION", L, Some("1")),
    (15, "MQDMPO-OPTIONS", L, Some("0")),
];

const MQSMPO: &[Item] = &[
    (10, "MQSMPO", None, None),
    (15, "MQSMPO-STRUCID", Some("X(4)"), Some("'SMPO'")),
    (15, "MQSMPO-VERSION", L, Some("1")),
    (15, "MQSMPO-OPTIONS", L, Some("0")),
    (15, "MQSMPO-VALUEENCODING", L, Some("785")),
    (15, "MQSMPO-VALUECCSID", L, Some("-3")),
];

const MQIMPO: &[Item] = &[
    (10, "MQIMPO", None, None),
    (15, "MQIMPO-STRUCID", Some("X(4)"), Some("'IMPO'")),
    (15, "MQIMPO-VERSION", L, Some("1")),
    (15, "MQIMPO-OPTIONS", L, Some("0")),
    (15, "MQIMPO-REQUESTEDENCODING", L, Some("785")),
    (15, "MQIMPO-REQUESTEDCCSID", L, Some("-3")),
    (15, "MQIMPO-RETURNEDENCODING", L, Some("785")),
    (15, "MQIMPO-RETURNEDCCSID", L, Some("0")),
    (15, "MQIMPO-RESERVED1", L, Some("0")),
    (15, "MQIMPO-RETURNEDNAME", V, None),
    (15, "MQIMPO-TYPESTRING", Some("X(8)"), Some("SPACES")),
];

const MQPD: &[Item] = &[
    (10, "MQPD", None, None),
    (15, "MQPD-STRUCID", Some("X(4)"), Some("'PD  '")),
    (15, "MQPD-VERSION", L, Some("1")),
    (15, "MQPD-OPTIONS", L, Some("0")),
    (15, "MQPD-SUPPORT", L, Some("1")),
    (15, "MQPD-CONTEXT", L, Some("0")),
    (15, "MQPD-COPYOPTIONS", L, Some("22")),
];

const MQCBD: &[Item] = &[
    (10, "MQCBD", None, None),
    (15, "MQCBD-STRUCID", Some("X(4)"), Some("'CBD '")),
    (15, "MQCBD-VERSION", L, Some("1")),
    (15, "MQCBD-CALLBACKTYPE", L, Some("1")),
    (15, "MQCBD-OPTIONS", L, Some("0")),
    (15, "MQCBD-CALLBACKAREA", P, None),
    (15, "MQCBD-CALLBACKFUNCTION", P, None),
    (15, "MQCBD-CALLBACKNAME", Some("X(128)"), Some("SPACES")),
    (15, "MQCBD-MAXMSGLENGTH", L, Some("-1")),
];

const MQCBC: &[Item] = &[
    (10, "MQCBC", None, None),
    (15, "MQCBC-STRUCID", Some("X(4)"), Some("'CBC '")),
    (15, "MQCBC-VERSION", L, Some("1")),
    (15, "MQCBC-CALLTYPE", L, Some("0")),
    (15, "MQCBC-HOBJ", L, Some("0")),
    (15, "MQCBC-CALLBACKAREA", P, None),
    (15, "MQCBC-CONNECTIONAREA", P, None),
    (15, "MQCBC-COMPCODE", L, Some("0")),
    (15, "MQCBC-REASON", L, Some("0")),
    (15, "MQCBC-STATE", L, Some("0")),
    (15, "MQCBC-DATALENGTH", L, Some("0")),
    (15, "MQCBC-BUFFERLENGTH", L, Some("0")),
    (15, "MQCBC-FLAGS", L, Some("0")),
];

const MQCTLO: &[Item] = &[
    (10, "MQCTLO", None, None),
    (15, "MQCTLO-STRUCID", Some("X(4)"), Some("'CTLO'")),
    (15, "MQCTLO-VERSION", L, Some("1")),
    (15, "MQCTLO-OPTIONS", L, Some("0")),
    (15, "MQCTLO-RESERVED", L, Some("-1")),
    (15, "MQCTLO-CONNECTIONAREA", P, None),
];

const MQTM: &[Item] = &[
    (10, "MQTM", None, None),
    (15, "MQTM-STRUCID", Some("X(4)"), Some("'TM  '")),
    (15, "MQTM-VERSION", L, Some("1")),
    (15, "MQTM-QNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQTM-PROCESSNAME", Some("X(48)"), Some("SPACES")),
    (15, "MQTM-TRIGGERDATA", Some("X(64)"), Some("SPACES")),
    (15, "MQTM-APPLTYPE", L, Some("0")),
    (15, "MQTM-APPLID", Some("X(256)"), Some("SPACES")),
    (15, "MQTM-ENVDATA", Some("X(128)"), Some("SPACES")),
    (15, "MQTM-USERDATA", Some("X(128)"), Some("SPACES")),
];

/// Named constants (CMQV): MQLONG values, and character values.
const INTS: &[(&str, i32)] = &[
    // completion and reason codes
    ("MQCC-OK", 0),
    ("MQCC-WARNING", 1),
    ("MQCC-FAILED", 2),
    ("MQCC-UNKNOWN", -1),
    ("MQRC-NONE", 0),
    ("MQRC-ALREADY-CONNECTED", 2002),
    ("MQRC-BUFFER-LENGTH-ERROR", 2005),
    ("MQRC-CONNECTION-BROKEN", 2009),
    ("MQRC-DATA-LENGTH-ERROR", 2010),
    ("MQRC-ENVIRONMENT-ERROR", 2012),
    ("MQRC-EXPIRY-ERROR", 2013),
    ("MQRC-HCONN-ERROR", 2018),
    ("MQRC-HOBJ-ERROR", 2019),
    ("MQRC-MD-ERROR", 2026),
    ("MQRC-MSG-TYPE-ERROR", 2029),
    ("MQRC-NO-MSG-AVAILABLE", 2033),
    ("MQRC-NOT-AUTHORIZED", 2035),
    ("MQRC-NOT-OPEN-FOR-INPUT", 2037),
    ("MQRC-NOT-OPEN-FOR-OUTPUT", 2039),
    ("MQRC-OBJECT-IN-USE", 2042),
    ("MQRC-OD-ERROR", 2044),
    ("MQRC-OPTIONS-ERROR", 2046),
    ("MQRC-PERSISTENCE-ERROR", 2047),
    ("MQRC-PRIORITY-ERROR", 2050),
    ("MQRC-PUT-INHIBITED", 2051),
    ("MQRC-Q-FULL", 2053),
    ("MQRC-Q-MGR-NAME-ERROR", 2058),
    ("MQRC-Q-MGR-NOT-AVAILABLE", 2059),
    ("MQRC-GET-INHIBITED", 2016),
    ("MQRC-TRUNCATED-MSG-ACCEPTED", 2079),
    ("MQRC-TRUNCATED-MSG-FAILED", 2080),
    ("MQRC-UNKNOWN-OBJECT-NAME", 2085),
    ("MQRC-FORMAT-ERROR", 2110),
    ("MQRC-NOT-CONVERTED", 2119),
    ("MQRC-Q-MGR-QUIESCING", 2161),
    ("MQRC-Q-MGR-STOPPING", 2162),
    ("MQRC-PMO-ERROR", 2173),
    ("MQRC-GMO-ERROR", 2186),
    ("MQRC-CONNECTION-QUIESCING", 2202),
    ("MQRC-CONNECTION-STOPPING", 2203),
    ("MQRC-UNKNOWN-REMOTE-Q-MGR", 2087),
    ("MQRC-CALL-IN-PROGRESS", 2219),
    ("MQRC-NOT-OPEN-FOR-SET", 2040),
    ("MQRC-SD-ERROR", 2424),
    ("MQRC-NO-SUBSCRIPTION", 2428),
    ("MQRC-SUBSCRIPTION-IN-USE", 2429),
    ("MQRC-SRO-ERROR", 2438),
    ("MQRC-PROPERTY-NAME-ERROR", 2442),
    ("MQRC-CBD-ERROR", 2444),
    ("MQRC-HMSG-ERROR", 2460),
    ("MQRC-CMHO-ERROR", 2461),
    ("MQRC-DMHO-ERROR", 2462),
    ("MQRC-SMPO-ERROR", 2463),
    ("MQRC-IMPO-ERROR", 2464),
    ("MQRC-PROPERTY-VALUE-TOO-BIG", 2469),
    ("MQRC-PROPERTY-NOT-AVAILABLE", 2471),
    ("MQRC-DMPO-ERROR", 2481),
    ("MQRC-PD-ERROR", 2482),
    ("MQRC-CALLBACK-TYPE-ERROR", 2483),
    ("MQRC-CALLBACK-LINK-ERROR", 2487),
    ("MQRC-OPERATION-ERROR", 2488),
    // connection and object handles
    ("MQHC-DEF-HCONN", 0),
    ("MQHC-UNUSABLE-HCONN", -1),
    ("MQHO-NONE", 0),
    ("MQHO-UNUSABLE-HOBJ", -1),
    // object types
    ("MQOT-Q", 1),
    ("MQOT-NAMELIST", 2),
    ("MQOT-PROCESS", 3),
    ("MQOT-STORAGE-CLASS", 4),
    ("MQOT-Q-MGR", 5),
    ("MQOT-CHANNEL", 6),
    ("MQOT-TOPIC", 8),
    // open options
    ("MQOO-BIND-AS-Q-DEF", 0),
    ("MQOO-INPUT-AS-Q-DEF", 1),
    ("MQOO-INPUT-SHARED", 2),
    ("MQOO-INPUT-EXCLUSIVE", 4),
    ("MQOO-BROWSE", 8),
    ("MQOO-OUTPUT", 16),
    ("MQOO-INQUIRE", 32),
    ("MQOO-SET", 64),
    ("MQOO-SAVE-ALL-CONTEXT", 128),
    ("MQOO-PASS-IDENTITY-CONTEXT", 256),
    ("MQOO-PASS-ALL-CONTEXT", 512),
    ("MQOO-SET-IDENTITY-CONTEXT", 1024),
    ("MQOO-SET-ALL-CONTEXT", 2048),
    ("MQOO-ALTERNATE-USER-AUTHORITY", 4096),
    ("MQOO-FAIL-IF-QUIESCING", 8192),
    ("MQOO-BIND-ON-OPEN", 16384),
    ("MQOO-BIND-NOT-FIXED", 32768),
    // close options
    ("MQCO-NONE", 0),
    ("MQCO-DELETE", 1),
    ("MQCO-DELETE-PURGE", 2),
    // put message options
    ("MQPMO-NONE", 0),
    ("MQPMO-SYNCPOINT", 2),
    ("MQPMO-NO-SYNCPOINT", 4),
    ("MQPMO-DEFAULT-CONTEXT", 32),
    ("MQPMO-NEW-MSG-ID", 64),
    ("MQPMO-NEW-CORREL-ID", 128),
    ("MQPMO-PASS-IDENTITY-CONTEXT", 256),
    ("MQPMO-PASS-ALL-CONTEXT", 512),
    ("MQPMO-SET-IDENTITY-CONTEXT", 1024),
    ("MQPMO-SET-ALL-CONTEXT", 2048),
    ("MQPMO-ALTERNATE-USER-AUTHORITY", 4096),
    ("MQPMO-FAIL-IF-QUIESCING", 8192),
    ("MQPMO-NO-CONTEXT", 16384),
    ("MQPMO-LOGICAL-ORDER", 32768),
    // get message options
    ("MQGMO-NONE", 0),
    ("MQGMO-NO-WAIT", 0),
    ("MQGMO-WAIT", 1),
    ("MQGMO-SYNCPOINT", 2),
    ("MQGMO-NO-SYNCPOINT", 4),
    ("MQGMO-SET-SIGNAL", 8),
    ("MQGMO-BROWSE-FIRST", 16),
    ("MQGMO-BROWSE-NEXT", 32),
    ("MQGMO-ACCEPT-TRUNCATED-MSG", 64),
    ("MQGMO-MARK-SKIP-BACKOUT", 128),
    ("MQGMO-MSG-UNDER-CURSOR", 256),
    ("MQGMO-LOCK", 512),
    ("MQGMO-UNLOCK", 1024),
    ("MQGMO-BROWSE-MSG-UNDER-CURSOR", 2048),
    ("MQGMO-SYNCPOINT-IF-PERSISTENT", 4096),
    ("MQGMO-FAIL-IF-QUIESCING", 8192),
    ("MQGMO-CONVERT", 16384),
    ("MQGMO-LOGICAL-ORDER", 32768),
    ("MQGMO-COMPLETE-MSG", 65536),
    ("MQGMO-ALL-MSGS-AVAILABLE", 131072),
    ("MQGMO-ALL-SEGMENTS-AVAILABLE", 262144),
    // match options
    ("MQMO-NONE", 0),
    ("MQMO-MATCH-MSG-ID", 1),
    ("MQMO-MATCH-CORREL-ID", 2),
    ("MQMO-MATCH-GROUP-ID", 4),
    ("MQMO-MATCH-MSG-SEQ-NUMBER", 8),
    ("MQMO-MATCH-OFFSET", 16),
    ("MQMO-MATCH-MSG-TOKEN", 32),
    // wait interval, expiry, priority, persistence
    ("MQWI-UNLIMITED", -1),
    ("MQEI-UNLIMITED", -1),
    ("MQPRI-PRIORITY-AS-Q-DEF", -1),
    ("MQPER-NOT-PERSISTENT", 0),
    ("MQPER-PERSISTENT", 1),
    ("MQPER-PERSISTENCE-AS-Q-DEF", 2),
    // message types, report options, feedback
    ("MQMT-REQUEST", 1),
    ("MQMT-REPLY", 2),
    ("MQMT-REPORT", 4),
    ("MQMT-DATAGRAM", 8),
    ("MQRO-NONE", 0),
    ("MQRO-PASS-MSG-ID", 128),
    ("MQRO-PASS-CORREL-ID", 64),
    ("MQRO-COPY-MSG-ID-TO-CORREL-ID", 0),
    ("MQRO-NEW-MSG-ID", 0),
    ("MQFB-NONE", 0),
    // encoding and character sets
    ("MQENC-NATIVE", 785),
    ("MQCCSI-Q-MGR", 0),
    ("MQCCSI-INHERIT", -2),
    ("MQCCSI-EMBEDDED", -1),
    // structure versions
    ("MQOD-VERSION-1", 1),
    ("MQOD-VERSION-2", 2),
    ("MQOD-VERSION-3", 3),
    ("MQOD-VERSION-4", 4),
    ("MQOD-CURRENT-VERSION", 4),
    ("MQMD-VERSION-1", 1),
    ("MQMD-VERSION-2", 2),
    ("MQMD-CURRENT-VERSION", 2),
    ("MQPMO-VERSION-1", 1),
    ("MQPMO-VERSION-2", 2),
    ("MQPMO-VERSION-3", 3),
    ("MQPMO-CURRENT-VERSION", 3),
    ("MQGMO-VERSION-1", 1),
    ("MQGMO-VERSION-2", 2),
    ("MQGMO-VERSION-3", 3),
    ("MQGMO-VERSION-4", 4),
    ("MQGMO-CURRENT-VERSION", 4),
    ("MQSD-VERSION-1", 1),
    ("MQSD-CURRENT-VERSION", 1),
    ("MQSRO-VERSION-1", 1),
    ("MQCBD-VERSION-1", 1),
    ("MQCBC-VERSION-1", 1),
    ("MQCTLO-VERSION-1", 1),
    ("MQCMHO-VERSION-1", 1),
    ("MQDMHO-VERSION-1", 1),
    ("MQSMPO-VERSION-1", 1),
    ("MQIMPO-VERSION-1", 1),
    ("MQPD-VERSION-1", 1),
    ("MQDMPO-VERSION-1", 1),
    // publish/subscribe
    ("MQSO-NONE", 0),
    ("MQSO-NON-DURABLE", 0),
    ("MQSO-ALTER", 1),
    ("MQSO-CREATE", 2),
    ("MQSO-RESUME", 4),
    ("MQSO-DURABLE", 8),
    ("MQSO-GROUP-SUB", 16),
    ("MQSO-MANAGED", 32),
    ("MQSO-SET-IDENTITY-CONTEXT", 64),
    ("MQSO-FIXED-USERID", 256),
    ("MQSO-ANY-USERID", 512),
    ("MQSO-PUBLICATIONS-ON-REQUEST", 2048),
    ("MQSO-NEW-PUBLICATIONS-ONLY", 4096),
    ("MQSO-FAIL-IF-QUIESCING", 8192),
    ("MQSO-ALTERNATE-USER-AUTHORITY", 262144),
    ("MQSO-WILDCARD-CHAR", 1048576),
    ("MQSO-WILDCARD-TOPIC", 2097152),
    ("MQSO-SET-CORREL-ID", 4194304),
    ("MQSO-SCOPE-QMGR", 67108864),
    ("MQSR-ACTION-PUBLICATION", 1),
    ("MQCO-KEEP-SUB", 4),
    ("MQCO-REMOVE-SUB", 8),
    ("MQPMO-SUPPRESS-REPLYTO", 134217728),
    ("MQPMO-RETAIN", 2097152),
    ("MQPRI-PRIORITY-AS-PUBLISHED", -3),
    ("MQOT-NONE", 0),
    // message handles and properties
    ("MQHM-NONE", 0),
    ("MQHM-UNUSABLE-HMSG", -1),
    ("MQCMHO-DEFAULT-VALIDATION", 0),
    ("MQCMHO-NO-VALIDATION", 1),
    ("MQCMHO-VALIDATE", 2),
    ("MQDMHO-NONE", 0),
    ("MQDMPO-NONE", 0),
    ("MQSMPO-NONE", 0),
    ("MQSMPO-SET-FIRST", 0),
    ("MQSMPO-SET-PROP-UNDER-CURSOR", 1),
    ("MQSMPO-SET-PROP-AFTER-CURSOR", 2),
    ("MQSMPO-APPEND-PROPERTY", 4),
    ("MQSMPO-SET-PROP-BEFORE-CURSOR", 8),
    ("MQIMPO-NONE", 0),
    ("MQIMPO-INQ-FIRST", 0),
    ("MQIMPO-CONVERT-TYPE", 2),
    ("MQIMPO-QUERY-LENGTH", 4),
    ("MQIMPO-INQ-NEXT", 8),
    ("MQIMPO-INQ-PROP-UNDER-CURSOR", 16),
    ("MQIMPO-CONVERT-VALUE", 32),
    ("MQPD-NONE", 0),
    ("MQPD-SUPPORT-OPTIONAL", 1),
    ("MQPD-NO-CONTEXT", 0),
    ("MQPD-USER-CONTEXT", 1),
    ("MQCOPY-DEFAULT", 22),
    ("MQTYPE-AS-SET", 0),
    ("MQTYPE-NULL", 2),
    ("MQTYPE-BOOLEAN", 4),
    ("MQTYPE-BYTE-STRING", 8),
    ("MQTYPE-INT8", 16),
    ("MQTYPE-INT16", 32),
    ("MQTYPE-INT32", 64),
    ("MQTYPE-LONG", 64),
    ("MQTYPE-INT64", 128),
    ("MQTYPE-FLOAT32", 256),
    ("MQTYPE-FLOAT64", 512),
    ("MQTYPE-STRING", 1024),
    ("MQVS-NULL-TERMINATED", -1),
    ("MQCCSI-APPL", -3),
    ("MQGMO-PROPERTIES-AS-Q-DEF", 0),
    ("MQGMO-PROPERTIES-FORCE-MQRFH2", 33554432),
    ("MQGMO-PROPERTIES-COMPATIBILITY", 268435456),
    ("MQGMO-PROPERTIES-IN-HANDLE", 134217728),
    ("MQGMO-NO-PROPERTIES", 67108864),
    // asynchronous consumption
    ("MQOP-START", 1),
    ("MQOP-START-WAIT", 2),
    ("MQOP-STOP", 4),
    ("MQOP-REGISTER", 256),
    ("MQOP-DEREGISTER", 512),
    ("MQOP-SUSPEND", 65536),
    ("MQOP-RESUME", 131072),
    ("MQCBT-MESSAGE-CONSUMER", 1),
    ("MQCBT-EVENT-HANDLER", 2),
    ("MQCBDO-NONE", 0),
    ("MQCBDO-START-CALL", 1),
    ("MQCBDO-STOP-CALL", 4),
    ("MQCBDO-REGISTER-CALL", 256),
    ("MQCBDO-DEREGISTER-CALL", 512),
    ("MQCBDO-FAIL-IF-QUIESCING", 8192),
    ("MQCBCT-START-CALL", 1),
    ("MQCBCT-STOP-CALL", 2),
    ("MQCBCT-REGISTER-CALL", 3),
    ("MQCBCT-DEREGISTER-CALL", 4),
    ("MQCBCT-EVENT-CALL", 5),
    ("MQCBCT-MSG-REMOVED", 6),
    ("MQCBCT-MSG-NOT-REMOVED", 7),
    ("MQCBD-FULL-MSG-LENGTH", -1),
    ("MQCTLO-NONE", 0),
    // MQSET attributes
    ("MQIA-TRIGGER-CONTROL", 24),
    ("MQIA-TRIGGER-MSG-PRIORITY", 26),
    ("MQIA-TRIGGER-TYPE", 28),
    ("MQIA-TRIGGER-DEPTH", 29),
    ("MQCA-TRIGGER-DATA", 2023),
    ("MQQA-GET-ALLOWED", 0),
    ("MQQA-GET-INHIBITED", 1),
    ("MQQA-PUT-ALLOWED", 0),
    ("MQQA-PUT-INHIBITED", 1),
    ("MQTC-OFF", 0),
    ("MQTC-ON", 1),
    ("MQTM-VERSION-1", 1),
    // application types
    ("MQAT-CICS", 1),
    ("MQAT-MVS", 2),
    ("MQAT-ZOS", 2),
    // inquire selectors
    ("MQIA-CURRENT-Q-DEPTH", 3),
    ("MQIA-MAX-Q-DEPTH", 15),
    ("MQIA-MAX-MSG-LENGTH", 13),
    ("MQIA-Q-TYPE", 20),
    ("MQIA-INHIBIT-GET", 9),
    ("MQIA-INHIBIT-PUT", 10),
    ("MQIA-OPEN-INPUT-COUNT", 17),
    ("MQIA-OPEN-OUTPUT-COUNT", 18),
    ("MQCA-Q-NAME", 2016),
    ("MQCA-Q-DESC", 2013),
    ("MQCA-Q-MGR-NAME", 2015),
];

const CHARS: &[(&str, &str, &str)] = &[
    ("MQFMT-NONE", "X(8)", "SPACES"),
    ("MQFMT-STRING", "X(8)", "'MQSTR   '"),
    ("MQFMT-TRIGGER", "X(8)", "'MQTRIG  '"),
    ("MQFMT-XMIT-Q-HEADER", "X(8)", "'MQXMIT  '"),
    ("MQFMT-DEAD-LETTER-HEADER", "X(8)", "'MQDEAD  '"),
    ("MQFMT-RF-HEADER-2", "X(8)", "'MQHRF2  '"),
    ("MQMI-NONE", "X(24)", "LOW-VALUES"),
    ("MQCI-NONE", "X(24)", "LOW-VALUES"),
    ("MQGI-NONE", "X(24)", "LOW-VALUES"),
    ("MQACT-NONE", "X(32)", "LOW-VALUES"),
    ("MQMTOK-NONE", "X(16)", "LOW-VALUES"),
];

fn cmqv() -> String {
    let mut s = String::new();
    for (n, v) in INTS {
        s.push_str(&format!(
            "          10 {n}\n              PIC S9(9) BINARY VALUE {v}.\n"
        ));
    }
    for (n, p, v) in CHARS {
        s.push_str(&format!(
            "          10 {n}\n              PIC {p} VALUE {v}.\n"
        ));
    }
    s
}

/// Source text of an MQ member.
pub fn member(name: &str) -> Option<String> {
    let (items, values) = match name {
        "CMQV" => return Some(cmqv()),
        "CMQODV" => (MQOD, true),
        "CMQODL" => (MQOD, false),
        "CMQMDV" => (MQMD, true),
        "CMQMDL" => (MQMD, false),
        "CMQPMOV" => (MQPMO, true),
        "CMQPMOL" => (MQPMO, false),
        "CMQGMOV" => (MQGMO, true),
        "CMQGMOL" => (MQGMO, false),
        "CMQTMV" => (MQTM, true),
        "CMQTML" => (MQTM, false),
        "CMQSDV" => (MQSD, true),
        "CMQSDL" => (MQSD, false),
        "CMQSROV" => (MQSRO, true),
        "CMQSROL" => (MQSRO, false),
        "CMQCMHOV" => (MQCMHO, true),
        "CMQCMHOL" => (MQCMHO, false),
        "CMQDMHOV" => (MQDMHO, true),
        "CMQDMHOL" => (MQDMHO, false),
        "CMQDMPOV" => (MQDMPO, true),
        "CMQDMPOL" => (MQDMPO, false),
        "CMQSMPOV" => (MQSMPO, true),
        "CMQSMPOL" => (MQSMPO, false),
        "CMQIMPOV" => (MQIMPO, true),
        "CMQIMPOL" => (MQIMPO, false),
        "CMQPDV" => (MQPD, true),
        "CMQPDL" => (MQPD, false),
        "CMQCBDV" => (MQCBD, true),
        "CMQCBDL" => (MQCBD, false),
        "CMQCBCV" => (MQCBC, true),
        "CMQCBCL" => (MQCBC, false),
        "CMQCTLOV" => (MQCTLO, true),
        "CMQCTLOL" => (MQCTLO, false),
        _ => return None,
    };
    Some(text(items, values))
}

/// The PL/I form of a COBOL PICTURE and VALUE: the attributes and the
/// INITIAL value.
fn pli_attrs(pic: &str, value: Option<&str>) -> (String, Option<String>) {
    if pic == "S9(9) BINARY" {
        return ("FIXED BIN(31)".into(), value.map(str::to_string));
    }
    if pic == "S9(18) BINARY" {
        return ("FIXED BIN(63)".into(), value.map(str::to_string));
    }
    let n: usize = match pic.strip_prefix("X(") {
        Some(r) => r.trim_end_matches(')').parse().unwrap_or(1),
        None => 1,
    };
    let init = value.map(|v| match v {
        "SPACES" | "SPACE" => "''".to_string(),
        "LOW-VALUES" => format!("(({n})'00'X)"),
        v => v.to_string(),
    });
    (format!("CHAR({n})"), init)
}

fn pli_line(s: &mut String, text: String) {
    s.push_str(&format!(" {text}\n"));
}

/// A structure member in PL/I: `DCL 1 MQMD, 3 STRUCID CHAR(4)
/// INIT('MD  '), ...;` (the prefix of the COBOL names dropped).
fn pli_struct(items: &[Item]) -> String {
    let mut s = String::new();
    let prefix = format!("{}-", items[0].1);
    pli_line(&mut s, format!("DCL 1 {},", items[0].1));
    let n = items.len();
    for (i, &(_, name, pic, value)) in items.iter().enumerate().skip(1) {
        let name = name.trim_start_matches(&prefix).replace('-', "");
        let end = if i + 1 == n { ";" } else { "," };
        match pic {
            Some("CHARV") => {
                pli_line(&mut s, format!("    3 {name},"));
                pli_line(&mut s, "      5 VSPTR POINTER INIT(SYSNULL()),".into());
                for f in ["VSOFFSET", "VSBUFSIZE", "VSLENGTH"] {
                    pli_line(&mut s, format!("      5 {f} FIXED BIN(31) INIT(0),"));
                }
                pli_line(
                    &mut s,
                    format!("      5 VSCCSID FIXED BIN(31) INIT(-3){end}"),
                );
            }
            Some("POINTER") => {
                pli_line(&mut s, format!("    3 {name} POINTER INIT(SYSNULL()){end}"));
            }
            Some(p) => {
                let (attrs, init) = pli_attrs(p, value);
                match init {
                    Some(v) => {
                        pli_line(&mut s, format!("    3 {name} {attrs}"));
                        pli_line(&mut s, format!("        INIT({v}){end}"));
                    }
                    None => pli_line(&mut s, format!("    3 {name} {attrs}{end}")),
                }
            }
            None => pli_line(&mut s, format!("    3 {name}{end}")),
        }
    }
    s
}

/// The MQI calls (CMQEPP).
const ENTRIES: &[&str] = &[
    "MQBACK", "MQCB", "MQCLOSE", "MQCMIT", "MQCONN", "MQCONNX", "MQCRTMH", "MQCTL", "MQDISC",
    "MQDLTMH", "MQDLTMP", "MQGET", "MQINQ", "MQINQMP", "MQOPEN", "MQPUT", "MQPUT1", "MQSET",
    "MQSETMP", "MQSUB", "MQSUBRQ",
];

/// Source text of an MQ include member for PL/I.
pub fn pli_member(name: &str) -> Option<String> {
    let items = match name {
        "CMQP" => {
            let mut s = String::new();
            for (n, v) in INTS {
                let n = n.replace('-', "_");
                pli_line(&mut s, format!("DCL {n}"));
                pli_line(&mut s, format!("    FIXED BIN(31) VALUE({v});"));
            }
            for (n, p, v) in CHARS {
                let (attrs, init) = pli_attrs(p, Some(v));
                let n = n.replace('-', "_");
                pli_line(&mut s, format!("DCL {n}"));
                pli_line(
                    &mut s,
                    format!("    {attrs} VALUE({});", init.unwrap_or_default()),
                );
            }
            return Some(s);
        }
        "CMQEPP" => {
            let mut s = String::new();
            for e in ENTRIES {
                pli_line(
                    &mut s,
                    format!("DCL {e} ENTRY EXTERNAL OPTIONS(ASM INTER);"),
                );
            }
            return Some(s);
        }
        "CMQODP" => MQOD,
        "CMQMDP" => MQMD,
        "CMQPMOP" => MQPMO,
        "CMQGMOP" => MQGMO,
        "CMQTMP" => MQTM,
        "CMQSDP" => MQSD,
        "CMQSROP" => MQSRO,
        "CMQCMHOP" => MQCMHO,
        "CMQDMHOP" => MQDMHO,
        "CMQDMPOP" => MQDMPO,
        "CMQSMPOP" => MQSMPO,
        "CMQIMPOP" => MQIMPO,
        "CMQPDP" => MQPD,
        "CMQCBDP" => MQCBD,
        "CMQCBCP" => MQCBC,
        "CMQCTLOP" => MQCTLO,
        _ => return None,
    };
    Some(pli_struct(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_fit_the_source_margins() {
        for m in [
            "CMQV", "CMQODV", "CMQMDL", "CMQPMOV", "CMQGMOV", "CMQTML", "CMQSDV", "CMQSROV",
            "CMQCMHOV", "CMQDMHOV", "CMQDMPOV", "CMQSMPOV", "CMQIMPOV", "CMQPDV", "CMQCBDV",
            "CMQCBCL", "CMQCTLOV",
        ] {
            for l in member(m).unwrap().lines() {
                assert!(l.len() <= 72, "{m}: {l}");
                assert!(l.starts_with("          "), "{m}: {l}");
            }
        }
        for m in ["CMQP", "CMQEPP", "CMQODP", "CMQMDP", "CMQSDP", "CMQCBCP"] {
            for l in pli_member(m).unwrap().lines() {
                assert!(l.len() <= 72, "{m}: {l}");
            }
        }
    }
}

//! IBM MQ interface definitions for COBOL (COPY CMQV, CMQODV, CMQMDV,
//! CMQPMOV, CMQGMOV, CMQTMV and their list forms ...L without initial
//! values). Written for Zeus from the documented MQI structure layouts
//! and constant values, with the z/OS values of the platform-dependent
//! ones (MQENC_NATIVE 785). The structures are those of the versions the
//! Zeus MQ interface converts: MQOD 1, MQMD 2, MQPMO 1, MQGMO 3, MQTM 1.

/// A data item of a member: name, PICTURE (None: a group) and initial
/// value.
type Item = (u8, &'static str, Option<&'static str>, Option<&'static str>);

const L: Option<&str> = Some("S9(9) BINARY");

fn text(items: &[Item], values: bool) -> String {
    let mut s = String::new();
    for (level, name, pic, value) in items {
        let indent = " ".repeat(10 + (*level as usize / 5 - 2) * 2);
        match pic {
            None => s.push_str(&format!("{indent}{level} {name}.\n")),
            Some(p) => {
                s.push_str(&format!("{indent}{level} {name}\n"));
                match value.filter(|_| values) {
                    Some(v) => s.push_str(&format!("{indent}    PIC {p} VALUE {v}.\n")),
                    None => s.push_str(&format!("{indent}    PIC {p}.\n")),
                }
            }
        }
    }
    s
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
    (15, "MQMD-ACCOUNTINGTOKEN", Some("X(32)"), Some("LOW-VALUES")),
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
    ("MQOD-CURRENT-VERSION", 1),
    ("MQMD-VERSION-1", 1),
    ("MQMD-VERSION-2", 2),
    ("MQMD-CURRENT-VERSION", 2),
    ("MQPMO-VERSION-1", 1),
    ("MQPMO-CURRENT-VERSION", 1),
    ("MQGMO-VERSION-1", 1),
    ("MQGMO-VERSION-2", 2),
    ("MQGMO-VERSION-3", 3),
    ("MQGMO-CURRENT-VERSION", 3),
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
        s.push_str(&format!("          10 {n}\n              PIC S9(9) BINARY VALUE {v}.\n"));
    }
    for (n, p, v) in CHARS {
        s.push_str(&format!("          10 {n}\n              PIC {p} VALUE {v}.\n"));
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
        _ => return None,
    };
    Some(text(items, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_fit_the_source_margins() {
        for m in ["CMQV", "CMQODV", "CMQMDL", "CMQPMOV", "CMQGMOV", "CMQTML"] {
            for l in member(m).unwrap().lines() {
                assert!(l.len() <= 72, "{m}: {l}");
                assert!(l.starts_with("          "), "{m}: {l}");
            }
        }
    }
}

//! What the CICS translator does to the COBOL text around EXEC CICS:
//! `DFHRESP(condition)` and `DFHVALUE(cvda)` become numeric literals.
//!
//! The RESP values are the EIBRESP numbers of the CICS conditions as
//! documented for the RESP option. CVDA values are those Zeus's CICS
//! runtime uses for the few CVDAs the applications name.

use super::ast::{DataRef, Program};
use super::lexer::{Tok, Token};
use crate::diag::{Diag, Result};

/// The EXEC interface block, as the translator inserts it in the
/// LINKAGE SECTION of a CICS program (field names and formats as
/// documented for EIB fields).
const DFHEIBLK: &str = "
01 DFHEIBLK.
   02 EIBTIME PIC S9(7) COMP-3.
   02 EIBDATE PIC S9(7) COMP-3.
   02 EIBTRNID PIC X(4).
   02 EIBTASKN PIC S9(7) COMP-3.
   02 EIBTRMID PIC X(4).
   02 DFHEIGDI PIC S9(4) COMP.
   02 EIBCPOSN PIC S9(4) COMP.
   02 EIBCALEN PIC S9(4) COMP.
   02 EIBAID PIC X(1).
   02 EIBFN PIC X(2).
   02 EIBRCODE PIC X(6).
   02 EIBDS PIC X(8).
   02 EIBREQID PIC X(8).
   02 EIBRSRCE PIC X(8).
   02 EIBSYNC PIC X(1).
   02 EIBFREE PIC X(1).
   02 EIBRECV PIC X(1).
   02 EIBFIL01 PIC X(1).
   02 EIBATT PIC X(1).
   02 EIBEOC PIC X(1).
   02 EIBFMH PIC X(1).
   02 EIBCOMPL PIC X(1).
   02 EIBSIG PIC X(1).
   02 EIBCONF PIC X(1).
   02 EIBERR PIC X(1).
   02 EIBERRCD PIC X(4).
   02 EIBSYNRB PIC X(1).
   02 EIBNODAT PIC X(1).
   02 EIBRESP PIC S9(8) COMP.
   02 EIBRESP2 PIC S9(8) COMP.
   02 EIBRLDBK PIC X(1).
";

/// The DL/I interface block the translator inserts in the WORKING-STORAGE
/// of a program with EXEC DLI commands.
const DLZDIB: &str = "
01 DLZDIB.
   02 DIBVER PIC X(2).
   02 DIBSTAT PIC X(2).
   02 DIBSEGM PIC X(8).
   02 DIBFIL01 PIC X(1).
   02 DIBFIL02 PIC X(1).
   02 DIBSEGLV PIC X(2).
   02 DIBKFBL PIC S9(4) COMP.
   02 DIBDBDNM PIC X(8).
   02 DIBDBORG PIC X(8).
   02 DIBFIL03 PIC X(6).
";

/// Whether the source contains EXEC blocks of `kind` (CICS, DLI).
pub fn uses(tokens: &[Token], kind: &str) -> bool {
    tokens
        .iter()
        .any(|t| matches!(&t.tok, Tok::Exec(k, _) if k == kind))
}

/// Adds the DL/I interface block to a program with EXEC DLI commands.
pub fn insert_dib(prog: &mut Program) -> Result<()> {
    let mut dib = super::parser::data_entries_from(DLZDIB)?;
    dib.append(&mut prog.data.working);
    prog.data.working = dib;
    Ok(())
}

/// What the translator adds to a CICS program: DFHEIBLK and (when the
/// program does not describe it) DFHCOMMAREA in the LINKAGE SECTION, and
/// both as the first parameters of PROCEDURE DIVISION USING.
pub fn insert_eib(prog: &mut Program) -> Result<()> {
    let link = &mut prog.data.linkage;
    let has = |l: &[super::ast::DataEntry], n: &str| {
        l.iter()
            .any(|e| e.level == 1 && e.name.as_deref() == Some(n))
    };
    let mut add = super::parser::data_entries_from(DFHEIBLK)?;
    if !has(link, "DFHCOMMAREA") {
        add.extend(super::parser::data_entries_from(
            "01 DFHCOMMAREA PIC X(1).",
        )?);
    }
    if !has(link, "DFHEIBLK") {
        add.append(link);
        *link = add;
    }
    let pos = prog.procedure.pos;
    let r = |n: &str| DataRef {
        name: n.to_string(),
        qualifiers: Vec::new(),
        subscripts: Vec::new(),
        refmod: None,
        pos,
    };
    let using = &mut prog.procedure.using;
    if !using.iter().any(|u| u.name == "DFHCOMMAREA") {
        using.insert(0, r("DFHCOMMAREA"));
    }
    if !using.iter().any(|u| u.name == "DFHEIBLK") {
        using.insert(0, r("DFHEIBLK"));
    }
    Ok(())
}

/// EIBRESP value of a CICS condition.
pub fn resp_value(name: &str) -> Option<u32> {
    const RESP: &[(&str, u32)] = &[
        ("NORMAL", 0),
        ("ERROR", 1),
        ("RDATT", 2),
        ("WRBRK", 3),
        ("EOF", 4),
        ("EODS", 5),
        ("EOC", 6),
        ("INBFMH", 7),
        ("ENDINPT", 8),
        ("NONVAL", 9),
        ("NOSTART", 10),
        ("TERMIDERR", 11),
        ("FILENOTFOUND", 12),
        ("DSIDERR", 12),
        ("NOTFND", 13),
        ("DUPREC", 14),
        ("DUPKEY", 15),
        ("INVREQ", 16),
        ("IOERR", 17),
        ("NOSPACE", 18),
        ("NOTOPEN", 19),
        ("ENDFILE", 20),
        ("ILLOGIC", 21),
        ("LENGERR", 22),
        ("QZERO", 23),
        ("SIGNAL", 24),
        ("QBUSY", 25),
        ("ITEMERR", 26),
        ("PGMIDERR", 27),
        ("TRANSIDERR", 28),
        ("ENDDATA", 29),
        ("INVTSREQ", 30),
        ("EXPIRED", 31),
        ("RETPAGE", 32),
        ("RTEFAIL", 33),
        ("RTESOME", 34),
        ("TSIOERR", 35),
        ("MAPFAIL", 36),
        ("INVERRTERM", 37),
        ("INVMPSZ", 38),
        ("IGREQID", 39),
        ("OVERFLOW", 40),
        ("INVLDC", 41),
        ("NOSTG", 42),
        ("JIDERR", 43),
        ("QIDERR", 44),
        ("NOJBUFSP", 45),
        ("DSSTAT", 46),
        ("SELNERR", 47),
        ("FUNCERR", 48),
        ("UNEXPIN", 49),
        ("NOPASSBKRD", 50),
        ("NOPASSBKWR", 51),
        ("SEGIDERR", 52),
        ("SYSIDERR", 53),
        ("ISCINVREQ", 54),
        ("ENQBUSY", 55),
        ("ENVDEFERR", 56),
        ("IGREQCD", 57),
        ("SESSIONERR", 58),
        ("SYSBUSY", 59),
        ("SESSBUSY", 60),
        ("NOTALLOC", 61),
        ("CBIDERR", 62),
        ("INVEXITREQ", 63),
        ("INVPARTNSET", 64),
        ("INVPARTN", 65),
        ("PARTNFAIL", 66),
        ("USERIDERR", 69),
        ("NOTAUTH", 70),
        ("VOLIDERR", 71),
        ("SUPPRESSED", 72),
        ("RESIDERR", 75),
        ("NOSPOOL", 80),
        ("TERMERR", 81),
        ("ROLLEDBACK", 82),
        ("END", 83),
        ("DISABLED", 84),
        ("ALLOCERR", 85),
        ("STRELERR", 86),
        ("OPENERR", 87),
        ("SPOLBUSY", 88),
        ("SPOLERR", 89),
        ("NODEIDERR", 90),
        ("TASKIDERR", 91),
        ("TCIDERR", 92),
        ("DSNNOTFOUND", 93),
        ("LOADING", 94),
        ("MODELIDERR", 95),
        ("OUTDESCRERR", 96),
        ("PARTNERIDERR", 97),
        ("PROFILEIDERR", 98),
        ("NETNAMEIDERR", 99),
        ("LOCKED", 100),
        ("RECORDBUSY", 101),
        ("UOWNOTFOUND", 102),
        ("UOWLNOTFOUND", 103),
        ("LINKABEND", 104),
        ("CHANGED", 105),
        ("PROCESSBUSY", 106),
        ("ACTIVITYBUSY", 107),
        ("PROCESSERR", 108),
        ("ACTIVITYERR", 109),
        ("CONTAINERERR", 110),
        ("EVENTERR", 111),
        ("TOKENERR", 112),
        ("NOTFINISHED", 113),
        ("POOLERR", 114),
        ("TIMERERR", 115),
        ("SYMBOLERR", 116),
        ("TEMPLATERR", 117),
        ("NOTSUPERUSER", 118),
        ("CSDERR", 119),
        ("DUPRES", 120),
        ("RESUNAVAIL", 121),
        ("CHANNELERR", 122),
        ("CCSIDERR", 123),
        ("TIMEDOUT", 124),
        ("CODEPAGEERR", 125),
        ("INCOMPLETE", 126),
        ("APPNOTFOUND", 127),
        ("BUSY", 128),
    ];
    RESP.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// CVDA value used by the Zeus CICS runtime.
pub fn cvda_value(name: &str) -> Option<u32> {
    const CVDA: &[(&str, u32)] = &[
        ("NORMAL", 1),
        ("ABEND", 2),
        ("SECERROR", 3),
        ("UCTRAN", 10),
        ("NOUCTRAN", 11),
        ("TRANIDONLY", 12),
        ("ENABLED", 20),
        ("DISABLED", 21),
        ("OPEN", 22),
        ("CLOSED", 23),
    ];
    CVDA.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// Replaces `DFHRESP(name)` and `DFHVALUE(name)` by numeric literals.
pub fn translate(tokens: Vec<Token>) -> Result<Vec<Token>> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        let f = t.word().filter(|w| *w == "DFHRESP" || *w == "DFHVALUE");
        if let (Some(f), Some(Tok::LParen), Some(Tok::Word(name)), Some(Tok::RParen)) = (
            f,
            tokens.get(i + 1).map(|t| &t.tok),
            tokens.get(i + 2).map(|t| &t.tok),
            tokens.get(i + 3).map(|t| &t.tok),
        ) {
            let v = if f == "DFHRESP" {
                resp_value(name)
            } else {
                cvda_value(name)
            }
            .ok_or_else(|| Diag::error(t.pos, format!("unknown {f} value {name}")))?;
            out.push(Token {
                tok: Tok::Num(v.to_string()),
                pos: t.pos,
            });
            i += 4;
            continue;
        }
        out.push(t.clone());
        i += 1;
    }
    Ok(out)
}

//! Units of work: changes to recoverable files (LOG(UNDO) clusters,
//! RECOVERY files) are backed out by SYNCPOINT ROLLBACK and abends; other
//! files keep them.

use std::cell::RefCell;
use std::rc::Rc;

use zeus_cics::csd::Csd;
use zeus_cics::region::{self, Event, Region, Terminal, aid};
use zeus_cobol::front::FrontOptions;
use zeus_ds::catalog::{Catalog, Entry, EntryKind, VsamOrg};

const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. UOWT.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 REC PIC X(10).
PROCEDURE DIVISION.
    MOVE 'AAAA111111' TO REC
    PERFORM WRITE-ALL
    EXEC CICS SYNCPOINT ROLLBACK END-EXEC
    MOVE 'BBBB222222' TO REC
    PERFORM WRITE-ALL
    EXEC CICS SYNCPOINT END-EXEC
    MOVE 'CCCC333333' TO REC
    PERFORM WRITE-ALL
    EXEC CICS ABEND ABCODE('UOWT') END-EXEC
    GOBACK.
WRITE-ALL.
    EXEC CICS WRITE FILE('RFILE') FROM(REC) RIDFLD(REC) END-EXEC
    EXEC CICS WRITE FILE('CFILE') FROM(REC) RIDFLD(REC) END-EXEC
    EXEC CICS WRITE FILE('NFILE') FROM(REC) RIDFLD(REC) END-EXEC.
";

const CSD: &str = " DEFINE TRANSACTION(UOWT) GROUP(T) PROGRAM(UOWT)
 DEFINE FILE(RFILE) GROUP(T) DSNAME(T.LOGGED)
 DEFINE FILE(CFILE) GROUP(T) DSNAME(T.RECOVERY) RECOVERY(BACKOUTONLY)
 DEFINE FILE(NFILE) GROUP(T) DSNAME(T.PLAIN) RECOVERY(NONE)
";

fn ksds(name: &str, log: Option<&str>) -> Entry {
    Entry {
        name: name.into(),
        kind: EntryKind::Cluster {
            org: VsamOrg::Ksds,
            keys: Some((4, 0)),
            record_size: (10, 10),
            aixs: vec![],
            log: log.map(str::to_string),
        },
    }
}

#[test]
fn backout_of_recoverable_files() {
    let dir = tempfile::tempdir().unwrap();
    let loadlib = dir.path().join("loadlib");
    std::fs::create_dir(&loadlib).unwrap();
    let src = dir.path().join("UOWT.cbl");
    let text: String = PROGRAM.lines().map(|l| format!("       {l}\n")).collect();
    std::fs::write(&src, text).unwrap();
    let module = zeus_cobol::compile_file(&src, &FrontOptions::default()).unwrap();
    zeus_cobol::save_module(&module, &loadlib.join("UOWT.zo")).unwrap();

    let cat_dir = dir.path().join("catalog");
    let mut catalog = Catalog::open(&cat_dir).unwrap();
    catalog.define(ksds("T.LOGGED", Some("UNDO"))).unwrap();
    catalog.define(ksds("T.RECOVERY", None)).unwrap();
    catalog.define(ksds("T.PLAIN", Some("NONE"))).unwrap();
    catalog.save().unwrap();

    let region = Rc::new(RefCell::new(Region::new(
        Csd::parse(CSD),
        catalog,
        vec![loadlib],
        vec![],
    )));
    let term = Rc::new(RefCell::new(Terminal::new("T001")));
    term.borrow_mut().screen.type_at(1, 1, "UOWT").unwrap();
    let e = region::attention(&region, &term, aid::ENTER);
    assert!(
        matches!(e, Event::Abended { ref code, .. } if code == "UOWT"),
        "{e:?}"
    );

    let catalog = Catalog::open(&cat_dir).unwrap();
    let keys = |dsn: &str| -> Vec<String> {
        catalog
            .read_records(dsn)
            .unwrap()
            .iter()
            .map(|r| {
                zeus_cobol::ebcdic::decode_latin1(&r[..4])
                    .iter()
                    .map(|&b| b as char)
                    .collect()
            })
            .collect()
    };
    // the rolled back and the abended writes are gone, the committed one stays
    assert_eq!(keys("T.LOGGED"), ["BBBB"]);
    assert_eq!(keys("T.RECOVERY"), ["BBBB"]);
    assert_eq!(keys("T.PLAIN"), ["AAAA", "BBBB", "CCCC"]);
}

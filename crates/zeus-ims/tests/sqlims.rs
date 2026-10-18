//! Own test definitions, derived from the published IMS SQL data model.
use zeus_cobol::codepage;
use zeus_decimal::Dec;
use zeus_ims::{
    Database,
    catalog::{parse_dbd, parse_psb},
    dli::{ReadOp, Session, Status, WriteOp},
    sql::SqlValue,
};

const DBD: &str = " DBD NAME=SQLTEST,ACCESS=(HDAM,OSAM),ENCODING=CP037\n SEGM NAME=ROOT,PARENT=0,BYTES=16\n FIELD NAME=(ID,SEQ,U),START=1,BYTES=4,TYPE=F,DATATYPE=INT\n FIELD NAME=NAME,START=5,BYTES=8,TYPE=C,DATATYPE=CHAR\n FIELD NAME=AMOUNT,START=13,BYTES=4,TYPE=C,DATATYPE=DECIMAL(7,2)\n SEGM NAME=CHILD,PARENT=ROOT,BYTES=8\n FIELD NAME=(ID,SEQ,U),START=1,BYTES=4,TYPE=F,DATATYPE=INT\n FIELD NAME=VALUE,START=5,BYTES=4,TYPE=F,DATATYPE=INT\n DBDGEN\n END\n";
const PSB: &str = " PCB TYPE=DB,DBDNAME=SQLTEST,PROCOPT=A,PCBNAME=APP\n SENSEG NAME=ROOT,PARENT=0\n SENSEG NAME=CHILD,PARENT=ROOT\n PSBGEN PSBNAME=SQLPSB,LANG=COBOL\n END\n";

fn setup() -> (tempfile::TempDir, Database, Session) {
    let dir = tempfile::tempdir().unwrap();
    let database =
        Database::open(dir.path().join("test.wal"), vec![parse_dbd(DBD).unwrap()]).unwrap();
    let session = Session::new(database.clone(), parse_psb(PSB).unwrap()).unwrap();
    (dir, database, session)
}
fn number(n: i64) -> SqlValue {
    SqlValue::Number(Dec::from_i64(n))
}
fn insert(session: &mut Session, id: i64, name: &str, amount: &str) {
    session
        .execute_sql(
            "INSERT INTO APP.ROOT (ID,NAME,AMOUNT) VALUES (?,?,?)",
            &[
                number(id),
                SqlValue::Text(name.into()),
                SqlValue::Number(Dec::parse(amount).unwrap()),
            ],
        )
        .unwrap();
}

#[test]
fn sql_and_dli_share_uncommitted_records_and_rollback() {
    let (_dir, db, mut s) = setup();
    insert(&mut s, 1, "first", "123.45");
    assert_eq!(db.count(), 0);
    let read = s.read(0, ReadOp::Gu, &[b"ROOT    "]).unwrap();
    assert_eq!(read.status, Status::Ok);
    assert_eq!(&read.record.unwrap().data[..4], &1i32.to_be_bytes());
    let result = s
        .execute_sql(
            "SELECT ID,NAME,AMOUNT FROM APP.ROOT WHERE ID=?",
            &[number(1)],
        )
        .unwrap();
    assert_eq!(
        result.rows[0],
        vec![
            number(1),
            SqlValue::Text("first".into()),
            SqlValue::Number(Dec::parse("123.45").unwrap())
        ]
    );
    s.rollback();
    assert!(
        s.execute_sql("SELECT * FROM ROOT", &[])
            .unwrap()
            .rows
            .is_empty()
    );
}

#[test]
fn update_commit_and_delete_are_visible_to_dli() {
    let (_dir, db, mut s) = setup();
    insert(&mut s, 1, "old", "0");
    assert_eq!(
        s.execute_sql("UPDATE APP.ROOT SET NAME='new' WHERE ID=1", &[])
            .unwrap()
            .affected,
        1
    );
    s.commit().unwrap();
    assert_eq!(db.count(), 1);
    let mut read = Session::new(db.clone(), parse_psb(PSB).unwrap()).unwrap();
    let record = read.read(0, ReadOp::Gu, &[]).unwrap().record.unwrap();
    assert_eq!(
        codepage::get(37).unwrap().decode(&record.data[4..12]),
        "new     "
    );
    assert_eq!(
        s.execute_sql("DELETE FROM ROOT WHERE ID=1", &[])
            .unwrap()
            .affected,
        1
    );
    s.rollback();
    assert_eq!(
        s.execute_sql("SELECT COUNT(*) FROM ROOT", &[])
            .unwrap()
            .rows,
        vec![vec![number(1)]]
    );
}

#[test]
fn aggregate_grouping_order_and_ebcdic_comparison() {
    let (_dir, _db, mut s) = setup();
    insert(&mut s, 1, "A", "1.25");
    insert(&mut s, 2, "A", "2.75");
    insert(&mut s, 3, "a", "9.00");
    let r = s
        .execute_sql(
            "SELECT NAME,SUM(AMOUNT),COUNT(*) FROM ROOT GROUP BY NAME ORDER BY NAME",
            &[],
        )
        .unwrap();
    // Lowercase sorts before uppercase in EBCDIC.
    assert_eq!(
        r.rows[0],
        vec![SqlValue::Text("a".into()), number(9), number(1)]
    );
    assert_eq!(
        r.rows[1],
        vec![SqlValue::Text("A".into()), number(4), number(2)]
    );
    assert_eq!(
        s.execute_sql("SELECT MIN(ID),MAX(ID),AVG(AMOUNT) FROM ROOT", &[])
            .unwrap()
            .rows[0],
        vec![
            number(1),
            number(3),
            SqlValue::Number(Dec::parse("4.33").unwrap())
        ]
    );
    assert_eq!(
        s.execute_sql("SELECT NAME,SUM(AMOUNT) FROM ROOT", &[])
            .unwrap_err()
            .state,
        "42803"
    );
}

#[test]
fn failed_statement_preserves_prior_unit_of_work() {
    let (_dir, _db, mut s) = setup();
    insert(&mut s, 1, "short", "1");
    insert(&mut s, 2, "longname", "2");
    // Conversion is prevalidated for every affected row before changes commit.
    assert_eq!(
        s.execute_sql("UPDATE ROOT SET NAME='too long for field'", &[])
            .unwrap_err()
            .state,
        "22001"
    );
    assert_eq!(
        s.execute_sql("SELECT NAME FROM ROOT ORDER BY ID", &[])
            .unwrap()
            .rows,
        vec![
            vec![SqlValue::Text("short".into())],
            vec![SqlValue::Text("longname".into())]
        ]
    );
    assert_eq!(
        s.execute_sql("INSERT INTO ROOT (ID) VALUES (1)", &[])
            .unwrap_err()
            .state,
        "23505"
    );
    assert_eq!(
        s.execute_sql("SELECT COUNT(*) FROM ROOT", &[])
            .unwrap()
            .rows,
        vec![vec![number(2)]]
    );
}

#[test]
fn validates_metadata_for_empty_results_and_parameter_counts() {
    let (_dir, _db, mut s) = setup();
    for sql in [
        "SELECT MISSING FROM ROOT",
        "SELECT * FROM ROOT WHERE MISSING=1",
        "SELECT * FROM ROOT ORDER BY MISSING",
        "UPDATE ROOT SET MISSING=1",
        "DELETE FROM ROOT WHERE MISSING=1",
    ] {
        assert_eq!(s.execute_sql(sql, &[]).unwrap_err().state, "23614", "{sql}");
    }
    assert_eq!(
        s.execute_sql("SELECT * FROM ROOT WHERE ID=?", &[])
            .unwrap_err()
            .state,
        "07001"
    );
    assert_eq!(
        s.execute_sql("SELECT * FROM SQLTEST.ROOT", &[])
            .unwrap_err()
            .state,
        "3F000"
    );
}

#[test]
fn sql_joins_hierarchical_path_and_delete_cascades() {
    let (_dir, db, mut s) = setup();
    insert(&mut s, 1, "parent", "0");
    let parent = s.read(0, ReadOp::Gu, &[]).unwrap().record.unwrap();
    let mut bytes = 7i32.to_be_bytes().to_vec();
    bytes.extend(22i32.to_be_bytes());
    assert_eq!(
        s.write(0, WriteOp::Isrt, "CHILD", Some(&bytes), &[])
            .unwrap()
            .status,
        Status::Ok
    );
    let r = s
        .execute_sql(
            "SELECT ROOT.NAME,CHILD.ID,CHILD.VALUE FROM APP.ROOT,APP.CHILD WHERE CHILD.ID=7",
            &[],
        )
        .unwrap();
    assert_eq!(
        r.rows,
        vec![vec![SqlValue::Text("parent".into()), number(7), number(22)]]
    );
    assert!(s.execute_sql("SELECT ID FROM ROOT,CHILD", &[]).is_err());
    s.commit().unwrap();
    assert_eq!(db.count(), 2);
    s.execute_sql("DELETE FROM ROOT WHERE ID=1", &[]).unwrap();
    s.commit().unwrap();
    assert_eq!(db.count(), 0);
    assert!(db.record(parent.id).is_none());
}

#[test]
fn checks_pcb_procopt_and_sensitivity() {
    let (_dir, db, _s) = setup();
    let psb = parse_psb(&PSB.replace("PROCOPT=A", "PROCOPT=G")).unwrap();
    let mut s = Session::new(db, psb).unwrap();
    assert_eq!(
        s.execute_sql("INSERT INTO ROOT (ID) VALUES (1)", &[])
            .unwrap_err()
            .state,
        "42501"
    );
    s.execute_sql("SELECT * FROM ROOT", &[]).unwrap();
}

#[test]
fn child_sql_insert_resolves_virtual_foreign_key_and_missing_parent() {
    let (_dir, _db, mut s) = setup();
    insert(&mut s, 1, "one", "0");
    insert(&mut s, 2, "two", "0");
    s.execute_sql("INSERT INTO CHILD (ID,VALUE,ROOT_ID) VALUES (7,23,2)", &[])
        .unwrap();
    assert_eq!(
        s.execute_sql(
            "SELECT ROOT.NAME,CHILD.VALUE,CHILD.ROOT_ID FROM ROOT,CHILD",
            &[]
        )
        .unwrap()
        .rows,
        vec![vec![SqlValue::Text("two".into()), number(23), number(2)]]
    );
    assert_eq!(
        s.execute_sql("INSERT INTO CHILD (ID,ROOT_ID) VALUES (8,99)", &[])
            .unwrap_err()
            .state,
        "23503"
    );
    assert_eq!(
        s.execute_sql("UPDATE CHILD SET ROOT_ID=1", &[])
            .unwrap_err()
            .state,
        "23504"
    );
    assert_eq!(
        s.execute_sql("SELECT VALUE FROM CHILD WHERE ROOT_ID=1", &[])
            .unwrap()
            .rows
            .len(),
        0
    );
}

#[test]
fn sql_external_names_and_zoned_decimal_marshaling() {
    let dir = tempfile::tempdir().unwrap();
    let dbd = parse_dbd(" DBD NAME=RAWDB,ENCODING=CP037,ACCESS=(HDAM,OSAM)\n SEGM NAME=RAWSEG,EXTERNALNAME=ACCOUNT,PARENT=0,BYTES=9\n FIELD NAME=(RAWKEY,SEQ,U),EXTERNALNAME=ID,START=1,BYTES=4,\n               TYPE=F,DATATYPE=INT\n FIELD NAME=RAWQTY,EXTERNALNAME=QTY,START=5,BYTES=5,\n               TYPE=C,DATATYPE=DECIMAL(5,2)\n DFSMARSH INTERNALTYPECONVERTER=ZONEDDECIMAL,ISSIGNED=Y\n DBDGEN\n END\n").unwrap();
    let psb = parse_psb(" PCB TYPE=DB,DBDNAME=RAWDB,PROCOPT=A,PCBNAME=RAWPCB,EXTERNALNAME=VIEW\n SENSEG NAME=RAWSEG,PARENT=0\n PSBGEN PSBNAME=RAWPSB,LANG=COBOL\n END\n").unwrap();
    let db = Database::open(dir.path().join("data.wal"), vec![dbd]).unwrap();
    let mut s = Session::new(db, psb).unwrap();
    s.execute_sql("INSERT INTO VIEW.ACCOUNT (ID,QTY) VALUES (1,-12.34)", &[])
        .unwrap();
    let row = s.read(0, ReadOp::Gu, &[]).unwrap().record.unwrap();
    assert_eq!(&row.data[4..], &[0xf0, 0xf1, 0xf2, 0xf3, 0xd4]);
    let result = s
        .execute_sql("SELECT ACCOUNT.QTY FROM VIEW.ACCOUNT WHERE ID=1", &[])
        .unwrap();
    assert_eq!(result.columns[0].name, "QTY");
    assert_eq!(
        result.rows,
        vec![vec![SqlValue::Number(Dec::parse("-12.34").unwrap())]]
    );
}

#[test]
fn sql_statement_rollback_undoes_earlier_rows_without_losing_prior_calls() {
    let (_dir, _db, mut s) = setup();
    insert(&mut s, 1, "one", "1");
    let mut corrupt = 2i32.to_be_bytes().to_vec();
    corrupt.extend(codepage::get(37).unwrap().encode("two     "));
    corrupt.extend([0xff; 4]);
    s.write(0, WriteOp::Isrt, "ROOT", Some(&corrupt), &[])
        .unwrap();
    assert_eq!(
        s.execute_sql("UPDATE ROOT SET NAME='changed' WHERE AMOUNT>=0", &[])
            .unwrap_err()
            .state,
        "22018"
    );
    assert_eq!(
        s.execute_sql("SELECT NAME FROM ROOT ORDER BY ID", &[])
            .unwrap()
            .rows,
        vec![
            vec![SqlValue::Text("one".into())],
            vec![SqlValue::Text("two".into())]
        ]
    );
}

#[test]
fn fetch_reports_truncation_original_length_and_null_indicators() {
    assert_eq!(
        run_cobol_with_data(
            "
EXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC
MOVE 'INSERT INTO ROOT (ID,NAME) VALUES (1,''test'')' TO STMT
EXEC SQLIMS EXECUTE IMMEDIATE :STMT END-EXEC
MOVE 'SELECT NAME FROM ROOT' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS OPEN C END-EXEC
EXEC SQLIMS FETCH C INTO :SMALL-NAME:IND END-EXEC
IF SQLIMSCODE NOT = 0 OR SQLIMSWARN1 NOT = 'W'
 OR SQLIMSWARN0 NOT = 'W' OR IND NOT = 8
 OR SMALL-NAME NOT = 'tes'
 MOVE 1 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS CLOSE C END-EXEC
MOVE 'SELECT SUM(AMOUNT) FROM ROOT WHERE ID=99' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS OPEN C END-EXEC
MOVE 17 TO ID-VALUE
EXEC SQLIMS FETCH C INTO :ID-VALUE:IND END-EXEC
IF SQLIMSCODE NOT = 0 OR IND NOT = -1 OR ID-VALUE NOT = 17
 MOVE 2 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS CLOSE C END-EXEC
MOVE 0 TO RETURN-CODE GOBACK.",
            true,
            "01 SMALL-NAME PIC X(3)."
        ),
        0
    );
}

fn run_cobol(body: &str, enabled: bool) -> i32 {
    run_cobol_with_data(body, enabled, "")
}

fn run_cobol_with_data(body: &str, enabled: bool, declarations: &str) -> i32 {
    use zeus_ims::runtime::{self, BatchHost, Ims, Library, Schedule};
    let dir = tempfile::tempdir().unwrap();
    let library = Library::from_sources([
        ("SQLTEST".into(), DBD.into()),
        ("SQLPSB".into(), PSB.into()),
    ])
    .unwrap();
    let ims = Ims::open(&dir.path().join("ims.wal"), library).unwrap();
    let mut sched = Schedule::new(&ims, "SQLPSB", true, &Default::default()).unwrap();
    if enabled {
        sched.enable_sqlims();
    }
    let source = format!(
        "identification division.\nprogram-id. SQLRUN.\ndata division.\nworking-storage section.\nEXEC SQLIMS INCLUDE SQLIMSCA END-EXEC.\n01 STMT PIC X(160).\n01 ID-VALUE PIC S9(9) COMP-5 VALUE 1.\n01 NAME-VALUE PIC X(8) VALUE 'first'.\n01 IND PIC S9(4) COMP-5.\n{declarations}\nprocedure division.\n{body}\n"
    );
    let mut fixed = String::new();
    for line in source.lines() {
        let mut rest = line;
        while rest.len() > 65 {
            let mut quote = None;
            let mut split = 0;
            for (i, c) in rest.char_indices().take_while(|(i, _)| *i <= 65) {
                if matches!(c, '\'' | '"') {
                    if quote == Some(c) {
                        quote = None;
                    } else if quote.is_none() {
                        quote = Some(c);
                    }
                }
                if c.is_whitespace() && quote.is_none() {
                    split = i;
                }
            }
            assert!(split > 0, "test source token exceeds reference format");
            fixed.push_str(&format!("       {}\n", &rest[..split]));
            rest = rest[split..].trim_start();
        }
        fixed.push_str(&format!("       {rest}\n"));
    }
    let path = dir.path().join("sqlrun.cbl");
    std::fs::write(&path, fixed).unwrap();
    let module = zeus_cobol::compile_file(
        &path,
        &zeus_cobol::front::FrontOptions {
            compiler_options: "SQLIMS".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(Vec::new());
    machine.set_exec_host(std::rc::Rc::new(BatchHost { sql: None }));
    runtime::start_batch(sched, Vec::new());
    let result = machine.run_main(module);
    runtime::end_batch(result.is_ok()).unwrap();
    result.unwrap()
}

#[test]
fn cobol_dynamic_sql_end_to_end_and_whenever() {
    let body = "
EXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC
EXEC SQLIMS WHENEVER SQLERROR GO TO FAILED END-EXEC
MOVE 'INSERT INTO APP.ROOT (ID,NAME) VALUES (?,?)' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS EXECUTE S USING :ID-VALUE, :NAME-VALUE END-EXEC
IF SQLIMSERRD(3) NOT = 1 MOVE 21 TO RETURN-CODE GOBACK END-IF
MOVE 'SELECT ID,NAME FROM APP.ROOT WHERE ID=?' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS OPEN C USING :ID-VALUE END-EXEC
MOVE 0 TO ID-VALUE
MOVE SPACES TO NAME-VALUE
EXEC SQLIMS FETCH C INTO :ID-VALUE, :NAME-VALUE:IND END-EXEC
IF ID-VALUE NOT = 1 OR NAME-VALUE NOT = 'first' OR IND NOT = 0
 MOVE 22 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS WHENEVER NOT FOUND GO TO DONE END-EXEC
EXEC SQLIMS FETCH C INTO :ID-VALUE, :NAME-VALUE END-EXEC
MOVE 23 TO RETURN-CODE
GOBACK.
DONE.
EXEC SQLIMS CLOSE C END-EXEC
MOVE 0 TO RETURN-CODE
GOBACK.
FAILED.
DISPLAY SQLIMSCODE SQLIMSSTATE SQLIMSERRMC
MOVE 99 TO RETURN-CODE
GOBACK.
";
    assert_eq!(run_cobol(body, true), 0);
}

#[test]
fn rejects_sqlims_in_dli_region_without_forwarding_to_db2() {
    assert_eq!(
        run_cobol(
            "MOVE 'DELETE FROM APP.ROOT' TO STMT\nEXEC SQLIMS PREPARE S FROM :STMT END-EXEC\nIF SQLIMSCODE NOT < 0 OR SQLIMSSTATE NOT = '0A000' MOVE 1 TO RETURN-CODE GOBACK END-IF\nMOVE 0 TO RETURN-CODE\nGOBACK.",
            false
        ),
        0
    );
}

#[test]
fn enforces_single_open_cursor_and_syncpoint_closes_it() {
    assert_eq!(
        run_cobol(
            "
EXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC
MOVE 'SELECT COUNT(*) FROM ROOT' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS OPEN C END-EXEC
EXEC SQLIMS OPEN C END-EXEC
IF SQLIMSSTATE NOT = '24502' MOVE 1 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
IF SQLIMSSTATE NOT = '24506' MOVE 2 TO RETURN-CODE GOBACK END-IF
EXEC DLI SYNC END-EXEC
EXEC SQLIMS FETCH C INTO :ID-VALUE END-EXEC
IF SQLIMSSTATE NOT = '24501' MOVE 3 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS OPEN C END-EXEC
EXEC SQLIMS FETCH C INTO :ID-VALUE END-EXEC
IF SQLIMSCODE NOT = 0 OR ID-VALUE NOT = 0 MOVE 4 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS CLOSE C END-EXEC
MOVE 0 TO RETURN-CODE GOBACK.",
            true
        ),
        0
    );
}

#[test]
fn describe_and_descriptor_fetch_use_published_layout() {
    assert_eq!(
        run_cobol_with_data(
            "
EXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC
MOVE 'INSERT INTO ROOT (ID,NAME) VALUES (7,''example'')' TO STMT
EXEC SQLIMS EXECUTE IMMEDIATE :STMT END-EXEC
IF SQLIMSCODE NOT = 0 MOVE 1 TO RETURN-CODE GOBACK END-IF
MOVE 'SELECT ID,NAME FROM ROOT' TO STMT
MOVE 0 TO SQLIMSN
EXEC SQLIMS PREPARE S INTO :SQLIMSDA FROM :STMT END-EXEC
IF SQLIMSCODE NOT = 239 OR SQLIMSD NOT = 2
 MOVE 2 TO RETURN-CODE GOBACK END-IF
MOVE 2 TO SQLIMSN
EXEC SQLIMS DESCRIBE OUTPUT S INTO :SQLIMSDA END-EXEC
IF SQLIMSCODE NOT = 0 OR SQLIMSDABC NOT = 104
 OR SQLIMSTYPE(1) NOT = 496 OR SQLIMSLEN(1) NOT = 4
 OR SQLIMSTYPE(2) NOT = 452 OR SQLIMSLEN(2) NOT = 8
 OR SQLIMSNAMEC(2) NOT = 'NAME'
 MOVE 3 TO RETURN-CODE GOBACK END-IF
SET SQLIMSDATA(1) TO ADDRESS OF ID-VALUE
SET SQLIMSDATA(2) TO ADDRESS OF NAME-VALUE
EXEC SQLIMS OPEN C END-EXEC
EXEC SQLIMS FETCH C INTO DESCRIPTOR :SQLIMSDA END-EXEC
IF SQLIMSCODE NOT = 0 OR ID-VALUE NOT = 7
 OR NAME-VALUE NOT = 'example'
 MOVE 4 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS CLOSE C END-EXEC
MOVE 0 TO RETURN-CODE GOBACK.",
            true,
            "EXEC SQLIMS INCLUDE SQLIMSDA END-EXEC."
        ),
        0
    );
}

#[test]
fn expands_host_structures_and_indicator_arrays() {
    assert_eq!(
        run_cobol_with_data(
            "
EXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC
MOVE 5 TO REC-ID MOVE 'record' TO REC-NAME
MOVE 'INSERT INTO ROOT (ID,NAME) VALUES (?,?)' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS EXECUTE S USING :REC END-EXEC
IF SQLIMSCODE NOT = 0 MOVE 1 TO RETURN-CODE GOBACK END-IF
MOVE 'SELECT ID,NAME FROM ROOT' TO STMT
EXEC SQLIMS PREPARE S FROM :STMT END-EXEC
EXEC SQLIMS OPEN C END-EXEC
MOVE 0 TO REC-ID MOVE SPACES TO REC-NAME
MOVE -1 TO REC-IND(1) REC-IND(2)
EXEC SQLIMS FETCH C INTO :REC:REC-IND END-EXEC
IF SQLIMSCODE NOT = 0 OR REC-ID NOT = 5
 OR REC-NAME NOT = 'record'
 OR REC-IND(1) NOT = 0 OR REC-IND(2) NOT = 0
 MOVE 2 TO RETURN-CODE GOBACK END-IF
EXEC SQLIMS CLOSE C END-EXEC
MOVE 0 TO RETURN-CODE GOBACK.",
            true,
            "01 REC.\n 02 REC-ID PIC S9(9) COMP-5.\n 02 REC-NAME PIC X(8).\n01 REC-INDS.\n 02 REC-IND PIC S9(4) COMP-5 OCCURS 2."
        ),
        0
    );
}

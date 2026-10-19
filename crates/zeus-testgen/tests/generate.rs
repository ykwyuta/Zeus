//! `ztestgen` on a program with EVALUATE, 88 levels, string and numeric
//! comparisons, ON SIZE ERROR, an ACCEPTed line and a loop: every
//! feasible branch direction covered, the infeasible one found, and the
//! cases replayed by `ztest` with the same coverage measured.

use std::path::Path;
use std::process::Command;

#[test]
fn covers_the_sample() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/SAMPLE.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        text.contains("C1: 17 of 18 branch directions")
            && text.contains("100.0% of the feasible (1 infeasible)"),
        "{text}"
    );
    // the infeasible direction: NOT ON SIZE ERROR after LK-AMOUNT > 1000.50
    assert!(
        text.contains("SAMPLE line 44 (instruction ") && text.contains(") jump: infeasible"),
        "{text}"
    );
    let report = std::fs::read_to_string(out.join("report.txt")).unwrap();
    assert_eq!(report, text);
    // a case for the user ADMIN001
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(
        cases
            .iter()
            .any(|c| c.contains("\"LK-USER\": \"ADMIN001\"")),
        "{cases:?}"
    );
    assert!(cases.iter().any(|c| c.contains("\"LOOP\"")), "{cases:?}");
    // replayed with coverage: the same branches
    let cov = tmp.path().join("cov");
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg("--coverage")
        .arg(&cov)
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(text.contains(", 0 failed"), "{text}");
    let progs = zeus_cobol::coverage::load_dir(&cov).unwrap();
    let (_, branches, _) = zeus_cobol::coverage::summary(&progs[0]);
    assert_eq!((branches.hit, branches.total), (17, 18));
}

#[test]
fn a_changed_program_fails_its_cases() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/SAMPLE.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(o.status.success());
    let changed = tmp.path().join("SAMPLE.cbl");
    let text = std::fs::read_to_string(&src)
        .unwrap()
        .replace("'DENIED'", "'REFUSED'");
    std::fs::write(&changed, text).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&changed)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{text}");
    assert!(text.contains("1 failed"), "{text}");
}

/// A program reading a sequential file to its end and writing a report
/// file: the records and their number are inputs (AT END, FILE STATUS
/// '10'), the records written are expected results.
#[test]
fn covers_a_file_program() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/FILEPGM.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(
        text.contains("C1: 16 of 16 branch directions (100.0%)"),
        "{text}"
    );
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    // an empty input file, a large debit reported, the overdrawn balance
    assert!(
        cases.iter().any(|c| c.contains("\"records\": []")),
        "{cases:?}"
    );
    assert!(
        cases.iter().any(|c| c.contains("\"TR-TYPE\": \"D\"")),
        "{cases:?}"
    );
    assert!(
        cases.iter().any(|c| c.contains("\"return_code\": 4")),
        "{cases:?}"
    );
    // the report file is compared on replay
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    let changed = tmp.path().join("FILEPGM.cbl");
    let text = std::fs::read_to_string(&src)
        .unwrap()
        .replace("'LARGE DEBIT'", "'BIG DEBIT'");
    std::fs::write(&changed, text).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&changed)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{text}");
    assert!(text.contains("file RPT-FILE: records"), "{text}");
}

/// A program on an indexed file (random READ with INVALID KEY, REWRITE,
/// WRITE with a duplicate key, START and READ NEXT): the records and
/// their keys are inputs, the data set's final records expected results.
#[test]
fn covers_an_indexed_file_program() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/IDXPGM.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(
        text.contains("C1: 24 of 24 branch directions (100.0%)"),
        "{text}"
    );
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    // the final records of the data set are expected results
    assert!(
        cases.iter().any(|c| c.contains("\"name\": \"ACCT-FILE\"")),
        "{cases:?}"
    );
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    // a new account written with another status: the data set differs
    let changed = tmp.path().join("IDXPGM.cbl");
    let text = std::fs::read_to_string(&src)
        .unwrap()
        .replace("MOVE 'A' TO AC-STATUS", "MOVE 'X' TO AC-STATUS");
    std::fs::write(&changed, text).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&changed)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{text}");
    assert!(text.contains("file ACCT-FILE: records"), "{text}");
}

/// I/O statements made to fail (OPEN, READ: FILE STATUS 35 and 30) and a
/// program called that is not there (a stub whose output argument is an
/// input): every branch direction, the faults and the stub's output in
/// the cases, replayed the same.
#[test]
fn covers_faults_and_stubs() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/ENVPGM.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(
        text.contains("C1: 8 of 8 branch directions (100.0%)"),
        "{text}"
    );
    assert!(
        text.contains("stubs (programs called that ran as stubs): CHKSUB"),
        "{text}"
    );
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    let all = cases.concat();
    assert!(all.contains("\"statement\": \"OPEN\""), "{all}");
    assert!(all.contains("\"statement\": \"READ\""), "{all}");
    assert!(all.contains("OPEN FAILED 35"), "{all}");
    // the stub returns 'OK' and a score above 500
    assert!(all.contains("\"hex\": \"D6D2"), "{all}");
    assert!(all.contains("\"HIGH\""), "{all}");
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
}

/// Runs `ztestgen` on a test program and replays its cases with `ztest`:
/// the report and the case files.
fn generate_and_replay(name: &str) -> (String, String) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/programs/{name}.cbl"));
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(
        o.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let replay = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{replay}");
    (text, cases.concat())
}

/// EXEC CICS: what RECEIVE and READ give the program (INTO items, RESP)
/// and the conditions a HANDLE CONDITION catches (ENDFILE of READNEXT)
/// are inputs; every branch direction, replayed the same.
#[test]
fn covers_cics_commands() {
    let (text, all) = generate_and_replay("CICSPGM");
    assert!(
        text.contains("C1: 18 of 18 branch directions (100.0%)"),
        "{text}"
    );
    // RECEIVE INTO: the command word, item by item
    assert!(all.contains("\"CICSPGM.25.1.INTO.IN-CMD\""), "{all}");
    // READ: NOTFND, and a balance above 1000 (COMP-3 item of INTO)
    assert!(all.contains("\"num\": 13"), "{all}");
    assert!(all.contains("\"CICSPGM.43.1.INTO.AC-BAL\""), "{all}");
    assert!(all.contains("\"RICH"), "{all}");
    // READNEXT without RESP: six records without ENDFILE, and ENDFILE to
    // the HANDLE CONDITION label
    assert!(all.contains("\"CICSPGM.66.6.RESP\""), "{all}");
    assert!(all.contains("\"LIST FULL\""), "{all}");
    assert!(all.contains("\"LISTED 00"), "{all}");
    // what the program sends
    assert!(
        all.contains("CICS RETURN TRANSID('ACCT') COMMAREA="),
        "{all}"
    );
}

/// EXEC SQL: SELECT INTO host variables and SQLCODE, a cursor FETCHed
/// until SQLCODE 100, WHENEVER SQLERROR: every branch direction and the
/// WHENEVER label.
#[test]
fn covers_sql_statements() {
    let (text, all) = generate_and_replay("SQLPGM");
    assert!(
        text.contains("C1: 12 of 12 branch directions (100.0%)"),
        "{text}"
    );
    assert!(all.contains("\"SQLPGM.18.1.SQLCODE\""), "{all}");
    assert!(all.contains("\"GOLD"), "{all}");
    assert!(all.contains("\"NO CUSTOMER\""), "{all}");
    assert!(all.contains("\"MANY 002\""), "{all}");
    // WHENEVER SQLERROR
    assert!(all.contains("\"CURSOR FAILED\""), "{all}");
    assert!(
        all.contains("SQL SELECT NAME, CREDIT_LIMIT INTO ?, ? FROM CUST WHERE ID = ?"),
        "{all}"
    );
}

/// INSPECT (TALLYING, REPLACING, BEFORE INITIAL), a JUSTIFIED RIGHT
/// move, subscripts that follow the inputs (one and two dimensions), an
/// LE service's feedback (CEEDAYS), a FILE STATUS that is not numeric
/// (VSAM) and a DL/I call (CBLTDLI) read until GB: every branch direction.
#[test]
fn covers_text_le_vsam_and_dli_calls() {
    let (text, all) = generate_and_replay("TEXTPGM");
    assert!(
        text.contains("C1: 24 of 24 branch directions (100.0%)"),
        "{text}"
    );
    for out in [
        "\"BAD OPTION\"",
        "\"ADMIN ONLY\"",
        "\"OPTION ",
        "\"NO OPTION\"",
        "\"DATE OK\"",
        "\"BAD DATE\"",
        "\"DATE ERROR ",
        "\"VSAM ERROR\"",
        "\"CREDIT",
        "\"SEGMENTS 00",
        "\"ZEROS BEFORE DASH\"",
        "\"GRID Z\"",
    ] {
        assert!(all.contains(out), "{out}: {all}");
    }
    assert!(all.contains("\"status\": \"F910\""), "{all}");
    assert!(all.contains("\"program\": \"CEEDAYS\""), "{all}");
    assert!(all.contains("\"program\": \"CBLTDLI.GN\""), "{all}");
}

/// EXEC DLI: INTO and DIBSTAT of GU and GNP are inputs; GNP ends with GE
/// from its second execution on.
#[test]
fn covers_dli_commands() {
    let (text, all) = generate_and_replay("DLIPGM");
    assert!(
        text.contains("C1: 14 of 14 branch directions (100.0%)"),
        "{text}"
    );
    for out in [
        "\"HIGH LIMIT\"",
        "\"NO SUMMARY\"",
        "\"DLI ERROR",
        "\"DECLINED\"",
        "\"DETAILS 00",
    ] {
        assert!(all.contains(out), "{out}: {all}");
    }
    assert!(all.contains("\"DLIPGM.15.1.INTO.SUM-LIMIT\""), "{all}");
}

/// UNSTRING DELIMITED BY ',' into three fields, NUMVAL of the third,
/// STRING, and UNSTRING DELIMITED BY '::': the solver adds and moves the
/// delimiters (AUTH,X12345,..., ...::END), every branch direction.
#[test]
fn covers_unstring_string_and_numval() {
    let (text, all) = generate_and_replay("SPLITPGM");
    assert!(
        text.contains("C1: 10 of 10 branch directions (100.0%)"),
        "{text}"
    );
    assert!(all.contains("\"LK-MSG\": \"AUTH,X12345"), "{all}");
    // a delimiter of two characters
    assert!(all.contains("\"DOUBLE COLON\""), "{all}");
    assert!(all.contains("\"KNOWN CODE\""), "{all}");
    assert!(all.contains("\"LARGE "), "{all}");
    assert!(all.contains("\"SMALL "), "{all}");
    assert!(all.contains("\"NO CODE\""), "{all}");
}

/// A parameter read through its length (`PARM(1:PARM-LEN)`) into numeric
/// items, and records of comma separated numbers: data shaped like the
/// fields (each a number, a comma between them) starts the search.
#[test]
fn covers_a_parameter_and_comma_separated_records() {
    let (text, all) = generate_and_replay("LOADPGM");
    assert!(
        text.contains("C1: 10 of 10 branch directions (100.0%)"),
        "{text}"
    );
    assert!(all.contains("\"NO START\""), "{all}");
    assert!(all.contains("\"BAD RANGE\""), "{all}");
    assert!(all.contains("\"BULK "), "{all}");
    assert!(all.contains("\"SMALL "), "{all}");
}

/// HANDLE ABEND LABEL: EXEC CICS ABEND and an abend of a command go to
/// the label, ASSIGN ABCODE gives the code; a reference modification
/// with a length received in the COMMAREA, a COMP-2 item from NUMVAL.
#[test]
fn covers_abend_exits_and_received_lengths() {
    let (text, all) = generate_and_replay("ABNDPGM");
    assert!(
        text.contains("C1: 10 of 10 branch directions (100.0%)"),
        "{text}"
    );
    assert!(all.contains("\"READ FAILED\""), "{all}");
    assert!(all.contains("\"DEADLOCK\""), "{all}");
    assert!(all.contains("\"OTHER ABEND\""), "{all}");
    assert!(all.contains("\"BIG AMOUNT\""), "{all}");
    assert!(all.contains(".ABEND\""), "{all}");
}

/// A seed written by hand (items by name only) is the first run; the
/// cases of a generation given back as seeds cover at once what they
/// covered.
#[test]
fn seeds_start_the_search() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/SAMPLE.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let seeds = tmp.path().join("seeds");
    std::fs::create_dir_all(&seeds).unwrap();
    std::fs::write(
        seeds.join("admin.json"),
        r#"{ "params": [ { "fields": { "LK-CODE": "AD", "LK-AMOUNT": "12.50", "LK-USER": "ADMIN001" } } ] }"#,
    )
    .unwrap();
    let ztestgen = |args: &[&str], out: &Path| {
        let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
            .args(args)
            .arg("--out")
            .arg(out)
            .arg(&src)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(
            o.status.success(),
            "{text}{}",
            String::from_utf8_lossy(&o.stderr)
        );
        text
    };
    // one run: the seed
    let one = tmp.path().join("one");
    ztestgen(&["--runs", "1", "--seeds", seeds.to_str().unwrap()], &one);
    let case = std::fs::read_to_string(one.join("case-001.json")).unwrap();
    assert!(case.contains("\"LK-USER\": \"ADMIN001\""), "{case}");
    assert!(case.contains("\"LK-AMOUNT\": \"12.50\""), "{case}");
    // a full generation, then its cases as seeds with as many runs as
    // there are cases: the same coverage
    let full = tmp.path().join("full");
    let text = ztestgen(&[], &full);
    let n = std::fs::read_dir(&full)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .count();
    let again = tmp.path().join("again");
    let text2 = ztestgen(
        &["--runs", &n.to_string(), "--seeds", full.to_str().unwrap()],
        &again,
    );
    let c1 = |t: &str| {
        t.lines()
            .find(|l| l.starts_with("C1:"))
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(
        c1(&text).split(',').next(),
        c1(&text2).split(',').next(),
        "{text}\n{text2}"
    );
}

/// A flag's values along the control flow: `IF WS-EOF = 'N'` inside
/// `PERFORM UNTIL WS-EOF = 'Y'` is never false, and a number's range:
/// `IF WS-I = 5` in `PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I >
/// WS-MAX` (VALUE 3) is never true (proved, not counted); the directions
/// on a flag set under input conditions are covered.
#[test]
fn proves_impossible_directions() {
    let (text, all) = generate_and_replay("FLAGPGM");
    assert!(
        text.contains("C1: 14 of 16 branch directions")
            && text.contains("100.0% of the feasible (2 infeasible)"),
        "{text}"
    );
    assert!(
        text.contains("FLAGPGM line 19 (instruction ")
            && text.contains("jump: impossible (static analysis"),
        "{text}"
    );
    // WS-I is 1 to 3 in the loop: IF WS-I = 5 is never true
    assert!(
        text.contains("FLAGPGM line 27 (instruction ") && text.contains("next: impossible"),
        "{text}"
    );
    assert!(all.contains("\"ERROR XX\""), "{all}");
    assert!(all.contains("\"NO CODE\""), "{all}");
}

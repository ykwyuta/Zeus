//! IBM Enterprise COBOL language-reference regression cases.
//! References and scope: docs/proposal/0014-cobol-ibm-conformance.md.

use std::{cell::RefCell, io::Write, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine};

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn compile(src: &str) -> Result<zeus_cobol::zir::Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("IBM.cbl");
    let fixed: String = src.lines().map(|l| format!("       {l}\n")).collect();
    assert!(src.lines().all(|l| l.chars().count() <= 65));
    std::fs::write(&path, fixed).unwrap();
    zeus_cobol::compile_file(&path, &FrontOptions::default())
}

fn run(src: &str) -> String {
    let module = compile(src).unwrap();
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(module).unwrap();
    String::from_utf8(out.0.borrow().clone()).unwrap()
}

#[test]
fn exit_perform_exits_innermost_inline_loop() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9 VALUE 0.
PROCEDURE DIVISION.
    PERFORM 2 TIMES
        PERFORM 3 TIMES
            ADD 1 TO N
            EXIT PERFORM
            ADD 5 TO N
        END-PERFORM
    END-PERFORM
    DISPLAY N
    STOP RUN."#),
        "2\n"
    );
}

#[test]
fn exit_perform_cycle_runs_increment_and_test() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9.
01 N PIC 99 VALUE 0.
PROCEDURE DIVISION.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
        ADD I TO N
        EXIT PERFORM CYCLE
        ADD 10 TO N
    END-PERFORM
    DISPLAY N "/" I
    STOP RUN."#),
        "06/4\n"
    );
}

#[test]
fn exit_perform_cycle_test_after_and_once() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9 VALUE 0.
PROCEDURE DIVISION.
    PERFORM WITH TEST AFTER UNTIL N = 3
        ADD 1 TO N
        EXIT PERFORM CYCLE
        ADD 5 TO N
    END-PERFORM
    PERFORM
        DISPLAY N
        EXIT PERFORM CYCLE
        DISPLAY "BAD"
    END-PERFORM
    EXIT PERFORM
    DISPLAY "OK"
    STOP RUN."#),
        "3\nOK\n"
    );
}

#[test]
fn set_false_uses_declared_value_and_true_uses_first_range_bound() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 FLAG PIC 9 VALUE 9.
   88 ACTIVE VALUE 1 THRU 3 WHEN SET TO FALSE IS 0.
PROCEDURE DIVISION.
    SET ACTIVE TO TRUE
    DISPLAY FLAG
    SET ACTIVE TO FALSE
    IF ACTIVE DISPLAY "BAD" ELSE DISPLAY FLAG END-IF
    MOVE 8 TO FLAG
    IF ACTIVE DISPLAY "BAD" ELSE DISPLAY "OK" END-IF
    STOP RUN."#),
        "1\n0\nOK\n"
    );
}

#[test]
fn set_false_requires_false_value() {
    let err = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 FLAG PIC 9.
   88 ACTIVE VALUE 1.
PROCEDURE DIVISION.
    SET ACTIVE TO FALSE.
    STOP RUN."#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("WHEN SET TO FALSE"), "{err}");
}

#[test]
fn initialize_national_category_does_not_select_alphanumeric() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 A PIC X VALUE "A".
   02 N PIC N VALUE N"B".
PROCEDURE DIVISION.
    INITIALIZE R REPLACING NATIONAL DATA BY N"C"
    DISPLAY A N
    INITIALIZE R REPLACING ALPHANUMERIC DATA BY "D"
    DISPLAY A N
    STOP RUN."#),
        "AC\nDC\n"
    );
}

#[test]
fn initialize_float_and_filler_group() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 FILLER.
      03 A PIC X VALUE "A".
   02 FILLER PIC X VALUE "B".
   02 F COMP-1 VALUE 7.
01 N PIC 9.
PROCEDURE DIVISION.
    INITIALIZE R
    MOVE F TO N
    DISPLAY "[" R(1:2) "]" N
    STOP RUN."#),
        "[ B]0\n"
    );
}

#[test]
fn initialize_rejects_duplicate_categories() {
    let err = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC X.
PROCEDURE DIVISION.
    INITIALIZE A REPLACING ALPHANUMERIC BY "A"
                           ALPHANUMERIC BY "B".
    STOP RUN."#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("duplicate"), "{err}");
}

#[test]
fn perform_until_exit_and_times_cycle() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9 VALUE 0.
PROCEDURE DIVISION.
    PERFORM 3 TIMES
        ADD 1 TO N
        EXIT PERFORM CYCLE
        ADD 5 TO N
    END-PERFORM
    PERFORM UNTIL EXIT
        ADD 1 TO N
        EXIT PERFORM
    END-PERFORM
    DISPLAY N
    STOP RUN."#),
        "4\n"
    );
    assert!(compile("ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n    PERFORM TEST AFTER UNTIL EXIT CONTINUE END-PERFORM.")
        .unwrap_err().to_string().contains("UNTIL EXIT cannot"));
}

#[test]
fn end_program_name_and_separator_are_checked() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
    assert!(compile(&format!("{src}END PROGRAM \"t\".")).is_ok());
    assert!(
        compile(&format!("{src}END PROGRAM WRONG."))
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );
    assert!(
        compile(&format!("{src}END PROGRAM T"))
            .unwrap_err()
            .to_string()
            .contains("period expected")
    );
}

#[test]
fn replace_partial_words_respects_edges_order_and_literals() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
REPLACE LEADING ==OLD-== BY ==NEW-==
        TRAILING ==-OLD== BY ==-NEW==.
01 OLD-A PIC X VALUE "A".
01 B-OLD PIC X VALUE "B".
REPLACE OFF.
PROCEDURE DIVISION.
    DISPLAY NEW-A B-NEW "OLD-A"
    STOP RUN."#),
        "ABOLD-A\n"
    );
}

#[test]
fn copy_replacing_partial_word_and_deletion() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("REC.cpy"),
        "       01 OLD-A PIC X VALUE \"A\".\n",
    )
    .unwrap();
    let path = dir.path().join("T.cbl");
    std::fs::write(
        &path,
        r#"       ID DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY REC REPLACING LEADING ==OLD-== BY ====.
       PROCEDURE DIVISION.
           DISPLAY A
           STOP RUN.
"#,
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(module).unwrap();
    assert_eq!(&*out.0.borrow(), b"A\n");
}

#[test]
fn initialize_value_replacing_default_precedence_and_filler() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 A PIC X VALUE "A".
   02 B PIC X.
   02 FILLER PIC X VALUE "F".
   02 N PIC 9 VALUE 7.
   02 M PIC 9.
PROCEDURE DIVISION.
    MOVE "ZZZ99" TO R
    INITIALIZE R WITH FILLER ALL TO VALUE
        THEN REPLACING ALPHANUMERIC BY "B"
        THEN TO DEFAULT
    DISPLAY R
    MOVE "ZZZ99" TO R
    INITIALIZE R ALPHANUMERIC TO VALUE
    DISPLAY R
    INITIALIZE R TO DEFAULT
    DISPLAY "[" R "]"
    STOP RUN."#),
        "ABF70\nAZZ99\n[  Z00]\n"
    );
}

#[test]
fn initialize_value_initializes_every_table_occurrence() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 ROW OCCURS 2 TIMES.
      03 A PIC X VALUE "A".
      03 B PIC 9 VALUE 3.
PROCEDURE DIVISION.
    MOVE "ZZZZ" TO R
    INITIALIZE R ALL VALUE
    DISPLAY R
    INITIALIZE ROW(2)
    DISPLAY "[" R "]"
    STOP RUN."#),
        "A3A3\n[A3 0]\n"
    );
}

#[test]
fn initialize_excludes_redefinitions_and_index_pointer_storage() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 A PIC X VALUE "A".
   02 B REDEFINES A PIC 9.
   02 P USAGE POINTER.
   02 I USAGE INDEX.
01 Q USAGE POINTER.
PROCEDURE DIVISION.
    SET P TO ADDRESS OF R
    SET Q TO P
    SET I TO 7
    INITIALIZE R
    IF A = SPACE AND P = Q AND I = 7
        DISPLAY "OK"
    ELSE DISPLAY "BAD" END-IF
    STOP RUN."#),
        "OK\n"
    );
}

#[test]
fn initialize_egcs_is_dbcs_category_and_duplicate_alias_is_invalid() {
    assert!(
        compile(
            r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R PIC G USAGE DISPLAY-1.
PROCEDURE DIVISION.
    INITIALIZE R REPLACING EGCS BY SPACE.
    STOP RUN."#
        )
        .is_ok()
    );
    let err = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R PIC G USAGE DISPLAY-1.
PROCEDURE DIVISION.
    INITIALIZE R REPLACING EGCS BY SPACE DBCS BY SPACE.
    STOP RUN."#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn malformed_replace_and_condition_value_are_diagnosed() {
    for directive in [
        "REPLACE ==A== BY ==B==",
        "REPLACE OFF",
        "REPLACE ==== BY ==A==.",
        "REPLACE LEADING ==A B== BY ==C==.",
    ] {
        assert!(compile(directive).is_err(), "{directive}");
    }
    assert!(
        compile(
            r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 FLAG PIC 9.
   88 ACTIVE VALUE WHEN SET TO FALSE IS 0.
PROCEDURE DIVISION.
    SET ACTIVE TO TRUE.
    STOP RUN."#
        )
        .unwrap_err()
        .to_string()
        .contains("requires a value")
    );
}

#[test]
fn replace_scope_ends_at_separately_compiled_program() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
REPLACE =="X"== BY =="Y"==.
PROCEDURE DIVISION.
    DISPLAY "X"
    CALL "U"
    STOP RUN.
END PROGRAM T.
ID DIVISION.
PROGRAM-ID. U.
PROCEDURE DIVISION.
    DISPLAY "X"
    GOBACK.
END PROGRAM U."#),
        "Y\nX\n"
    );
}

#[test]
fn replace_remains_active_across_nested_programs() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
REPLACE =="X"== BY =="Y"==.
PROCEDURE DIVISION.
    CALL "U"
    STOP RUN.
ID DIVISION.
PROGRAM-ID. U.
PROCEDURE DIVISION.
    DISPLAY "X"
    GOBACK.
END PROGRAM U.
END PROGRAM T."#),
        "Y\n"
    );
}

#[test]
fn local_storage_value_is_reinitialized_without_resetting_working_storage() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
PROCEDURE DIVISION.
    CALL "U"
    CALL "U"
    CANCEL "U"
    CALL "U"
    STOP RUN.
END PROGRAM T.
ID DIVISION.
PROGRAM-ID. U.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 W PIC 9 VALUE 1.
LOCAL-STORAGE SECTION.
01 L PIC 9 VALUE 2.
PROCEDURE DIVISION.
    DISPLAY W L
    ADD 1 TO W L
    GOBACK.
END PROGRAM U."#),
        "12\n22\n12\n"
    );
}

#[test]
fn local_storage_cannot_be_external() {
    assert!(
        compile(
            r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
LOCAL-STORAGE SECTION.
01 L EXTERNAL PIC X.
PROCEDURE DIVISION.
    STOP RUN."#
        )
        .unwrap_err()
        .to_string()
        .contains("LOCAL-STORAGE cannot specify EXTERNAL")
    );
}

#[test]
fn local_storage_is_reset_for_external_language_calls_and_serialized_modules() {
    let module = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
PROCEDURE DIVISION.
    CALL "U"
    STOP RUN.
END PROGRAM T.
ID DIVISION.
PROGRAM-ID. U.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 W PIC 9 VALUE 1.
LOCAL-STORAGE SECTION.
01 L PIC 9 VALUE 2.
PROCEDURE DIVISION.
    DISPLAY W L
    ADD 1 TO W L
    GOBACK.
END PROGRAM U."#,
    )
    .unwrap();
    let bytes = zeus_cobol::module_bytes(&module).unwrap();
    let module = zeus_cobol::load_module_bytes(&bytes).unwrap();
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(module).unwrap();
    assert!(vm.call_program("U", &[]).unwrap().is_some());
    assert!(vm.call_program("U", &[]).unwrap().is_some());
    assert_eq!(&*out.0.borrow(), b"12\n22\n32\n");
}

#[test]
fn listing_directives_do_not_affect_program_or_sentence_scope() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
    EJECT
    TITLE "IBM 'listing'".
DATA DIVISION.
WORKING-STORAGE SECTION.
    SKIP1.
01 N PIC 9 VALUE 1.
PROCEDURE DIVISION.
    IF N = 1
        NEXT SENTENCE
    END-IF
    SKIP2
    DISPLAY "BAD".
    TITLE 'A ''quoted'' title'
    SKIP3.
    DISPLAY "EJECT TITLE SKIP1"
    STOP RUN."#),
        "EJECT TITLE SKIP1\n"
    );
    assert!(
        compile(
            "ID DIVISION.\nPROGRAM-ID. T.\n    TITLE \"broken\nPROCEDURE DIVISION.\n    STOP RUN."
        )
        .is_err()
    );
}

#[test]
fn initialize_to_value_restores_justified_initial_layout() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC X(3) JUSTIFIED RIGHT VALUE "A".
PROCEDURE DIVISION.
    DISPLAY "[" A "]"
    MOVE "Z" TO A
    DISPLAY "[" A "]"
    INITIALIZE A ALL TO VALUE
    DISPLAY "[" A "]"
    STOP RUN."#),
        "[A  ]\n[  Z]\n[A  ]\n"
    );
}

// C02: exercise the stored DBCS bytes, rather than only accepting the clause.
#[test]
fn initialize_dbcs_and_egcs_replace_only_dbcs_storage() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 A PIC X VALUE "A".
   02 D PIC G(2) DISPLAY-1.
   02 N PIC N VALUE N"B".
01 RAW REDEFINES R.
   02 RAW-A PIC X.
   02 RAW-D PIC X(4).
   02 RAW-N PIC X(2).
PROCEDURE DIVISION.
    MOVE X"42434445" TO RAW-D
    INITIALIZE R REPLACING DBCS BY SPACE
    IF RAW-D = X"40404040" AND A = "A" AND N = N"B"
        DISPLAY "DBCS OK"
    ELSE DISPLAY "BAD" END-IF
    MOVE X"46474849" TO RAW-D
    INITIALIZE R REPLACING EGCS BY SPACE
    IF RAW-D = X"40404040" AND A = "A" AND N = N"B"
        DISPLAY "EGCS OK"
    ELSE DISPLAY "BAD" END-IF
    STOP RUN."#),
        "DBCS OK\nEGCS OK\n"
    );
}

// C03: both IBM floating-point storage lengths are numeric receivers.
#[test]
fn initialize_numeric_replacement_includes_both_float_usages() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 F COMP-1 VALUE 7.
   02 D COMP-2 VALUE 8.
   02 A PIC X VALUE "A".
01 NF PIC 9.
01 ND PIC 9.
PROCEDURE DIVISION.
    INITIALIZE R REPLACING NUMERIC BY 3
    MOVE F TO NF
    MOVE D TO ND
    DISPLAY NF ND A
    INITIALIZE R
    MOVE F TO NF
    MOVE D TO ND
    DISPLAY NF ND "[" A "]"
    STOP RUN."#),
        "33A\n00[ ]\n"
    );
}

// C04: the COBOL 4.2 form initializes every combination below the group.
#[test]
fn initialize_plain_form_visits_nested_tables_and_preserves_fillers() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 ROW OCCURS 2 TIMES.
      03 COL OCCURS 2 TIMES.
         04 A PIC X VALUE "A".
         04 N PIC 9 VALUE 7.
         04 FILLER PIC X VALUE "F".
PROCEDURE DIVISION.
    INITIALIZE R
    DISPLAY "[" R "]"
    MOVE "Z" TO A(2, 2)
    MOVE 9 TO N(2, 2)
    INITIALIZE COL(2, 2)
    DISPLAY "[" R "]"
    STOP RUN."#),
        "[ 0F 0F 0F 0F]\n[ 0F 0F 0F 0F]\n"
    );
}

// C05: both the containing and contained programs require their own marker.
#[test]
fn end_program_checks_nested_names_and_requires_containing_marker() {
    let src = r#"ID DIVISION.
PROGRAM-ID. OUTER-P.
PROCEDURE DIVISION.
    STOP RUN.
ID DIVISION.
PROGRAM-ID. INNER-P.
PROCEDURE DIVISION.
    GOBACK.
END PROGRAM INNER-P.
"#;
    assert!(
        compile(src)
            .unwrap_err()
            .to_string()
            .contains("END PROGRAM required")
    );
    assert!(compile(&format!("{src}END PROGRAM OUTER-P.")).is_ok());
    assert!(
        compile(&src.replace("END PROGRAM INNER-P.", "END PROGRAM OUTER-P."))
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );
}

// C08: reentry through a nested program does not reset its working storage.
#[test]
fn local_storage_nested_reentry_reinitializes_groups_and_tables() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
PROCEDURE DIVISION.
    CALL "U"
    CALL "U"
    STOP RUN.
ID DIVISION.
PROGRAM-ID. U.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 W PIC 9 VALUE 1.
LOCAL-STORAGE SECTION.
01 R.
   02 N PIC 9 VALUE 2 OCCURS 2 TIMES.
PROCEDURE DIVISION.
    DISPLAY W R
    ADD 1 TO W N(1) N(2)
    GOBACK.
END PROGRAM U.
END PROGRAM T."#),
        "122\n222\n"
    );
}

// C10: drive all three functions through compilation and VM execution.
// Choose a stable window end, independently of the machine's execution year.
#[test]
fn date_conversion_functions_use_runtime_year_and_preserve_date_components() {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400;
    let year = zeus_cobol::vm::civil_from_days(days as i64).0;
    let src = format!(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 W PIC S9(5) COMP VALUE {}.
01 Y PIC 9(4).
01 D PIC 9(8).
01 J PIC 9(7).
PROCEDURE DIVISION.
    MOVE FUNCTION YEAR-TO-YYYY(18 W) TO Y
    DISPLAY Y
    MOVE FUNCTION YEAR-TO-YYYY(19 W) TO Y
    DISPLAY Y
    MOVE FUNCTION DATE-TO-YYYYMMDD(980101 W) TO D
    DISPLAY D
    MOVE FUNCTION DAY-TO-YYYYDDD(98001 W) TO J
    DISPLAY J
    STOP RUN."#,
        2018 - year
    );
    assert_eq!(run(&src), "2018\n1919\n19980101\n1998001\n");
}

// X01: CYCLE applies to the nearest loop and still carries nested VARYINGs.
#[test]
fn exit_perform_cycle_handles_after_varying_and_nested_loops() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9.
01 J PIC 9.
01 N PIC 99 VALUE 0.
PROCEDURE DIVISION.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2
        AFTER J FROM 1 BY 1 UNTIL J > 3
        PERFORM 2 TIMES
            ADD 1 TO N
            EXIT PERFORM CYCLE
            ADD 50 TO N
        END-PERFORM
        EXIT PERFORM CYCLE
        ADD 50 TO N
    END-PERFORM
    DISPLAY N
    STOP RUN."#),
        "12\n"
    );
}

#[test]
fn exit_perform_cycle_test_after_varying_tests_before_increment() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9.
01 N PIC 99 VALUE 0.
PROCEDURE DIVISION.
    PERFORM TEST AFTER VARYING I FROM 1 BY 1 UNTIL I = 3
        ADD I TO N
        EXIT PERFORM CYCLE
        ADD 50 TO N
    END-PERFORM
    DISPLAY N "/" I
    EXIT PERFORM CYCLE
    DISPLAY "OK"
    STOP RUN."#),
        "06/3\nOK\n"
    );
}

// X02: GOBACK escapes an out-of-line UNTIL EXIT; EXIT PARAGRAPH does not.
#[test]
fn perform_until_exit_out_of_line_reenters_until_goback() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9 VALUE 0.
PROCEDURE DIVISION.
MAIN-P.
    PERFORM LOOP-P UNTIL EXIT
    DISPLAY "BAD"
    STOP RUN.
LOOP-P.
    ADD 1 TO N
    DISPLAY N
    IF N = 2 GOBACK END-IF
    EXIT PARAGRAPH.
"#),
        "1\n2\n"
    );
}

// X03: WHEN, SET, TO and IS are optional noise words in the FALSE phrase.
#[test]
fn condition_false_phrase_allows_ibm_optional_words() {
    for phrase in [
        "FALSE 0",
        "WHEN FALSE IS 0",
        "SET TO FALSE 0",
        "TO FALSE 0",
        "WHEN SET TO FALSE IS 0",
    ] {
        let src = format!(
            r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 F PIC 9 VALUE 1.
   88 ACTIVE VALUE 1 {phrase}.
PROCEDURE DIVISION.
    SET ACTIVE TO FALSE
    DISPLAY F
    STOP RUN."#
        );
        assert_eq!(run(&src), "0\n", "{phrase}");
    }
}

#[test]
fn set_false_accepts_multiple_subscripted_condition_names() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 F PIC X VALUE "Y" OCCURS 2 TIMES.
      88 ACTIVE VALUE "Y" WHEN SET TO FALSE "N".
PROCEDURE DIVISION.
    SET ACTIVE(1) ACTIVE(2) TO FALSE
    DISPLAY R
    SET ACTIVE(2) TO TRUE
    DISPLAY R
    STOP RUN."#),
        "NN\nNY\n"
    );
}

#[test]
fn condition_false_value_requires_a_literal_even_if_not_executed() {
    let err = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 OTHER-VALUE PIC 9 VALUE 0.
01 F PIC 9.
   88 ACTIVE VALUE 1 WHEN SET TO FALSE OTHER-VALUE.
PROCEDURE DIVISION.
    STOP RUN."#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("literal"), "{err}");
}

// X04: INITIALIZE's general receiver rules also apply to its new phrases.
#[test]
fn initialize_to_value_rejects_renames_receiver() {
    let err = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 A PIC X VALUE "A".
   02 B PIC X VALUE "B".
66 ALIAS-R RENAMES A THRU B.
PROCEDURE DIVISION.
    INITIALIZE ALIAS-R ALL TO VALUE THEN TO DEFAULT.
    STOP RUN."#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("RENAMES"), "{err}");
}

// X05: pictures must keep their lexer context after partial replacement.
#[test]
fn partial_word_replace_preserves_picture_token_and_hyphen_fragments() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
REPLACE LEADING ==X== BY ==9==.
01 N PIC X(3) VALUE 123.
REPLACE OFF.
REPLACE TRAILING ==-== BY ==-ITEM==.
01 A- PIC X VALUE "A".
REPLACE OFF.
PROCEDURE DIVISION.
    ADD 1 TO N
    DISPLAY N A-ITEM
    STOP RUN."#),
        "124A\n"
    );
    let err = compile(
        r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
REPLACE LEADING ==X== BY ====.
01 A PIC X VALUE "A".
REPLACE OFF.
PROCEDURE DIVISION.
    STOP RUN."#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("picture string expected"), "{err}");
}

#[test]
fn partial_word_replace_requires_pseudotext_delimiters() {
    for src in [
        "REPLACE LEADING ==A== BY B.",
        "REPLACE TRAILING A BY ==B==.",
    ] {
        let program =
            format!("ID DIVISION.\nPROGRAM-ID. T.\n{src}\nPROCEDURE DIVISION.\n    STOP RUN.");
        let err = compile(&program).unwrap_err();
        assert!(err.to_string().contains("pseudo-text delimiters"), "{err}");
    }
}

#[test]
fn perform_until_exit_rejects_test_phrases_and_varying() {
    for perform in [
        "PERFORM TEST BEFORE UNTIL EXIT CONTINUE END-PERFORM",
        "PERFORM TEST AFTER UNTIL EXIT CONTINUE END-PERFORM",
        "PERFORM WITH TEST BEFORE UNTIL EXIT CONTINUE END-PERFORM",
        "PERFORM WITH TEST AFTER UNTIL EXIT CONTINUE END-PERFORM",
        "PERFORM VARYING I FROM 1 BY 1 UNTIL EXIT END-PERFORM",
    ] {
        let src = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 I PIC 9.\nPROCEDURE DIVISION.\n    {perform}.\n    STOP RUN."
        );
        // Keep long statement forms inside the fixed-format text area.
        let src = src.replace(" CONTINUE END-PERFORM", "\n    CONTINUE END-PERFORM");
        assert!(compile(&src).is_err(), "{perform}");
    }
}

#[test]
fn set_false_preserves_national_and_symbolic_literal_values() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
SPECIAL-NAMES.
    SYMBOLIC CHARACTERS FALSE-CHAR IS 214.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 F PIC X VALUE "Y".
   88 ACTIVE VALUE "Y" FALSE FALSE-CHAR.
01 N PIC N VALUE N"Y".
   88 N-ACTIVE VALUE N"Y" FALSE N"N".
PROCEDURE DIVISION.
    SET ACTIVE N-ACTIVE TO FALSE
    DISPLAY F N
    STOP RUN."#),
        "NN\n"
    );
}

#[test]
fn initialize_value_phrases_select_categories_and_reject_invalid_receivers() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 A PIC X VALUE "A".
   02 N PIC 9 VALUE 3.
   02 FILLER PIC X VALUE "F".
PROCEDURE DIVISION.
    MOVE "Z9Z" TO R
    INITIALIZE R ALPHANUMERIC NUMERIC TO VALUE
    DISPLAY R
    INITIALIZE R FILLER ALL VALUE
    DISPLAY R
    STOP RUN."#),
        "A3Z\nA3F\n"
    );
    for (declaration, receiver) in [
        ("01 P USAGE POINTER.", "P"),
        ("01 I USAGE INDEX.", "I"),
        ("01 F PIC 9.\n   88 ACTIVE VALUE 1.", "ACTIVE"),
    ] {
        let src = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{declaration}\nPROCEDURE DIVISION.\n    INITIALIZE {receiver} ALL TO VALUE.\n    STOP RUN."
        );
        let err = compile(&src).unwrap_err();
        assert!(err.to_string().contains("INITIALIZE receiver"), "{err}");
    }
}

#[test]
fn copy_partial_replacement_handles_picture_and_does_not_cascade() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("REC.cpy"),
        "       01 OLD-N PIC X(3) VALUE 123.\n",
    )
    .unwrap();
    let path = dir.path().join("T.cbl");
    std::fs::write(
        &path,
        r#"       ID DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY REC REPLACING LEADING ==OLD-== BY ==NEW-==
               LEADING ==NEW-== BY ==BAD-==
               LEADING ==X== BY ==9==.
       PROCEDURE DIVISION.
           ADD 1 TO NEW-N
           DISPLAY NEW-N
           STOP RUN.
"#,
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(module).unwrap();
    assert_eq!(&*out.0.borrow(), b"124\n");
}

#[test]
fn set_false_global_condition_keeps_declaring_program_symbolic_literal() {
    assert_eq!(
        run(r#"ID DIVISION.
PROGRAM-ID. T.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
SPECIAL-NAMES.
    SYMBOLIC CHARACTERS FALSE-CHAR IS 214.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 F GLOBAL PIC X VALUE "Y".
   88 ACTIVE VALUE "Y" FALSE FALSE-CHAR.
PROCEDURE DIVISION.
    CALL "U"
    DISPLAY F
    STOP RUN.
ID DIVISION.
PROGRAM-ID. U.
PROCEDURE DIVISION.
    SET ACTIVE TO FALSE
    GOBACK.
END PROGRAM U.
END PROGRAM T."#),
        "N\n"
    );
}

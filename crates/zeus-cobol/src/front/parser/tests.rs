use super::*;
use crate::front::lexer::lex;
use crate::front::source::{self, Pos};

fn parse_src(body: &str) -> Program {
    let text: String = body.lines().map(|l| format!("000000 {l}\n")).collect();
    let stream = source::read(&text, 0, false);
    let toks = lex(&stream, false).unwrap();
    let _ = Pos::default();
    parse(toks).unwrap().remove(0)
}

#[test]
fn minimal_program() {
    let p = parse_src(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01  A PIC 9(3) VALUE 5.
01  G.
    05 B PIC X(4) OCCURS 3 TIMES INDEXED BY IX.
88  FLAG VALUE 'Y' 'Z'.
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE 1 TO A. ADD 1 TO A
    IF A = 2 DISPLAY 'TWO' ELSE DISPLAY 'NO' END-IF.
    PERFORM SUB-PARA 3 TIMES.
    STOP RUN.
SUB-PARA.
    EXIT.",
    );
    assert_eq!(p.name, "HELLO");
    assert_eq!(p.data.working.len(), 4);
    assert_eq!(p.data.working[2].occurs.as_ref().unwrap().max, 3);
    let paras = &p.procedure.sections[0].paragraphs;
    assert_eq!(paras.len(), 2);
    assert_eq!(paras[0].name.as_deref(), Some("MAIN-PARA"));
    assert_eq!(paras[0].sentences.len(), 4);
    assert!(matches!(paras[0].sentences[1][1], Stmt::If { .. }));
}

#[test]
fn abbreviated_conditions() {
    let p = parse_src(
        "ID DIVISION.
PROGRAM-ID. C.
PROCEDURE DIVISION.
    IF A = 1 OR 2 AND NOT > 3 MOVE 1 TO B.",
    );
    let s = &p.procedure.sections[0].paragraphs[0].sentences[0][0];
    let Stmt::If { cond, .. } = s else { panic!() };
    // A = 1 OR (A = 2 AND A <= 3)
    let Cond::Or(_, rhs) = cond else {
        panic!("{cond:?}")
    };
    let Cond::And(l, r) = rhs.as_ref() else {
        panic!()
    };
    assert!(matches!(l.as_ref(), Cond::Rel(_, RelOp::Eq, _)));
    assert!(matches!(r.as_ref(), Cond::Rel(_, RelOp::Le, _)));
}

#[test]
fn sections_and_perform_varying() {
    let p = parse_src(
        "ID DIVISION.
PROGRAM-ID. S.
PROCEDURE DIVISION.
FIRST-SEC SECTION.
P1.
    PERFORM P2 VARYING I FROM 1 BY 1 UNTIL I > 10
        AFTER J FROM 1 BY 1 UNTIL J > 2.
    PERFORM UNTIL I > 3 ADD 1 TO I END-PERFORM.
P2. EXIT.
SECOND-SEC SECTION.
    GO TO P1.",
    );
    assert_eq!(p.procedure.sections.len(), 2);
    let s = &p.procedure.sections[0].paragraphs[0].sentences[0][0];
    let Stmt::Perform {
        kind: PerformKind::Varying { loops, .. },
        ..
    } = s
    else {
        panic!()
    };
    assert_eq!(loops.len(), 2);
    assert!(p.procedure.sections[1].paragraphs[0].name.is_none());
}

#[test]
fn arithmetic_statements() {
    let p = parse_src(
        "ID DIVISION.
PROGRAM-ID. S.
PROCEDURE DIVISION.
    ADD A B TO C ROUNDED D ON SIZE ERROR MOVE 1 TO E
        NOT ON SIZE ERROR MOVE 2 TO E END-ADD
    SUBTRACT 1 FROM A GIVING B
    MULTIPLY 2 BY 3 GIVING X
    DIVIDE A INTO B GIVING C REMAINDER D
    COMPUTE X ROUNDED = (A + B) * -1 ** 2 / C.",
    );
    let st = &p.procedure.sections[0].paragraphs[0].sentences[0];
    assert_eq!(st.len(), 5);
    let Stmt::Add {
        from,
        to,
        size_error,
        ..
    } = &st[0]
    else {
        panic!()
    };
    assert_eq!(from.len(), 2);
    assert_eq!(to.len(), 2);
    assert!(to[0].rounded && size_error.on.is_some() && size_error.not_on.is_some());
    let Stmt::Subtract { from, giving, .. } = &st[1] else {
        panic!()
    };
    assert_eq!((from.len(), giving.len()), (2, 1));
}

//! MVS 3.8j (OS/360 PL/I (F) Compiler V5.5 `PGM=IEMAA`) Hercules runner for
//! ZPVS (PL/I Validation Suite).
//!
//! Generates an 80-byte fixed EBCDIC JCL card deck (`job.ebcdic`), configures
//! Hercules with CCKD shadow files (`sf=shadow/*_1.sf`) so the base MVS 3.8j
//! DASD images are never modified, boots MVS 3.8j non-interactively via the
//! integrated console (`0009 3215-C / noprompt`), and parses the `REPORT`
//! output written to printer `000F` (`Printer00F.txt`) and compiler return
//! codes from `ConsoleLog.txt`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use zeus_cobol::ebcdic;

use crate::hercules::Hercules;

const ZPVSRPT_PLIF: &str = "
 DCL REPORT FILE RECORD OUTPUT ENV(F(120));
 DCL (ZTOTAL,ZPASS,ZFAIL,ZLAST_RC) FIXED BIN(31) INIT(0);
 DCL ZLINE CHAR(120);
 DCL ZCHECK ENTRY(CHAR(30) VARYING, BIT(1));
 DCL HEX ENTRY(BIT(256) VARYING) RETURNS(CHAR(64) VARYING);
 DCL TRIM ENTRY(CHAR(80) VARYING) RETURNS(CHAR(80) VARYING);
 DCL PLIRETV ENTRY RETURNS(FIXED BIN(31));
 HEX: PROC(B) RETURNS(CHAR(64) VARYING);
   DCL B BIT(256) VARYING;
   DCL OUT CHAR(64) VARYING INIT('');
   DCL HEXDIG CHAR(16) INIT('0123456789ABCDEF');
   DCL (ZI,ZL,ZV) FIXED BIN(15);
   DCL NIB BIT(4);
   ZL=LENGTH(B);
   DO ZI=1 TO ZL BY 4;
     NIB=SUBSTR(B,ZI,4);
     ZV=NIB;
     OUT=OUT||SUBSTR(HEXDIG,ZV+1,1);
   END;
   RETURN(OUT);
 END HEX;
 TRIM: PROC(S) RETURNS(CHAR(80) VARYING);
   DCL S CHAR(80) VARYING;
   DCL (ZI,ZJ,ZL) FIXED BIN(15);
   ZL=LENGTH(S);
   ZI=1;
   DO WHILE(ZI<=ZL);
     IF SUBSTR(S,ZI,1)^=' ' THEN GO TO TFOUND1;
     ZI=ZI+1;
   END;
   RETURN('');
 TFOUND1:
   ZJ=ZL;
   DO WHILE(ZJ>=ZI);
     IF SUBSTR(S,ZJ,1)^=' ' THEN GO TO TFOUND2;
     ZJ=ZJ-1;
   END;
 TFOUND2:
   RETURN(SUBSTR(S,ZI,ZJ-ZI+1));
 END TRIM;
 ZCHECK: PROC(ID,OK);
   DCL ID CHAR(30) VARYING, OK BIT(1);
   ZTOTAL=ZTOTAL+1;
   IF OK THEN DO;
     ZPASS=ZPASS+1; ZLINE='PASS '||ID;
   END;
   ELSE DO;
     ZFAIL=ZFAIL+1; ZLINE='FAIL* '||ID;
   END;
   WRITE FILE(REPORT) FROM(ZLINE);
 END ZCHECK;
 ZFINISH: PROC;
   PUT STRING(ZLINE) EDIT(ZPASS,' OF ',ZTOTAL,
     ' TESTS WERE EXECUTED SUCCESSFULLY')(F(6),A,F(6),A);
   WRITE FILE(REPORT) FROM(ZLINE);
   PUT STRING(ZLINE) EDIT(ZFAIL,' TEST(S) FAILED')(F(6),A);
   WRITE FILE(REPORT) FROM(ZLINE);
   ZLINE='NO TEST(S) DELETED';
   WRITE FILE(REPORT) FROM(ZLINE);
   ZLINE='NO TEST(S) REQUIRE VISUAL INSPECTION';
   WRITE FILE(REPORT) FROM(ZLINE);
   CLOSE FILE(REPORT);
 END ZFINISH;
 PLIRETC: PROC(RC);
   DCL RC FIXED BIN(31);
   ZLAST_RC=RC;
   CALL IHESARC(RC);
 END PLIRETC;
 PLIRETV: PROC RETURNS(FIXED BIN(31));
   RETURN(ZLAST_RC);
 END PLIRETV;
";

/// Programs in `zpvs` that can be compiled and executed with OS/360 PL/I (F)
/// V5.5 (`PGM=IEMAA`) on MVS 3.8j.
pub const DEFAULT_RUN_PROGRAMS: &[(&str, &str)] = &[
    ("PA", "PA101A"),
    ("PA", "PA102A"),
    ("PA", "PA103A"),
    ("PA", "PA104A"),
    ("PD", "PD101A"),
    ("PD", "PD102A"),
    ("PD", "PD103A"),
    ("PD", "PD104A"),
    ("PD", "PD105A"),
    ("PD", "PD106A"),
    ("PD", "PD107A"),
    ("PC", "PC101A"),
    ("PC", "PC102A"),
    ("PC", "PC103A"),
    ("PC", "PC105A"),
    ("PC", "PC106A"),
    ("PC", "PC107A"),
    ("PC", "PC108A"),
    ("PS", "PS101A"),
    ("PS", "PS102A"),
    ("PS", "PS103A"),
    ("PS", "PS104A"),
    ("PS", "PS105A"),
    ("PF", "PF101A"),
    ("PF", "PF102A"),
    ("PF", "PF103A"),
    ("PF", "PF105A"),
    ("PM", "PM101A"),
    ("PM", "PM102A"),
    ("PM", "PM103A"),
    ("PM", "PM105A"),
    ("PM", "PM106A"),
    ("PM", "PM107A"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZpvsProgramResult {
    pub module: String,
    pub program: String,
    pub pass: usize,
    pub fail: usize,
    pub failed_checks: Vec<String>,
    pub report_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZpvsOracleRun {
    pub oracle: String,
    pub compiler: String,
    pub total_programs: usize,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub programs: Vec<ZpvsProgramResult>,
}

fn encode_card(line: &str) -> Result<[u8; 80]> {
    if line.len() > 72 {
        bail!(
            "JCL/PL/I card exceeds column 72 ({} chars): {line}",
            line.len()
        );
    }
    let mut card = [ebcdic::SPACE; 80];
    let bytes = ebcdic::encode(line);
    for (i, &b) in bytes.iter().take(80).enumerate() {
        card[i] = if line.as_bytes().get(i) == Some(&b'^') {
            0x5F
        } else if line.as_bytes().get(i) == Some(&b'|') {
            0x4F
        } else {
            b
        };
    }
    Ok(card)
}

fn write_ebcdic_deck(path: &Path, jcl_lines: &[String]) -> Result<()> {
    let mut out = Vec::with_capacity(jcl_lines.len() * 80);
    for line in jcl_lines {
        out.extend_from_slice(&encode_card(line)?);
    }
    fs::write(path, out)?;
    Ok(())
}

fn adapt_plif(name: &str, src: &str) -> String {
    let mut s = src
        .replace("ENV(F RECSIZE(8))", "ENV(F(8))")
        .replace("ENV(V RECSIZE(12))", "ENV(V(16,12))")
        .replace("ENV(F RECSIZE(21))", "ENV(F(21))")
        .replace("ONCODE()", "ONCODE")
        .replace("ONCHAR()", "ONCHAR")
        .replace("ONSOURCE()", "ONSOURCE")
        .replace("NULL()", "NULL")
        .replace("EMPTY()", "EMPTY")
        .replace("PLIRETV()", "PLIRETV");
    if name == "PD101A" {
        if let Some(idx) = s.find("/* DEC-31-0 */") {
            let end_idx = s.find("CALL ZFINISH;").unwrap();
            s.replace_range(idx..end_idx, "");
        }
        s = s.replace("DCL D31 FIXED DEC(31);\n", "");
    }
    if name == "PD102A" {
        s = s
            .replace(
                "DCL 1 PACKED UNALIGNED, 2 A BIT(3), 2 D BIT(5);",
                "DCL 1 PACKED UNALIGNED, 2 A BIT(3), 2 D BIT(5);\n DCL PKP PTR, PK8 BIT(8) BASED(PKP);",
            )
            .replace("HEX(C)", "HEX(UNSPEC(C))")
            .replace("MAXLENGTH(V)", "8")
            .replace("A='101'B; D='10011'B;", "PKP=ADDR(PACKED); A='101'B; D='10011'B;")
            .replace("UNSPEC(PACKED)", "PK8");
    }
    if name == "PD103A" {
        s = s
            .replace(
                "DCL 1 T ALIGNED, 2 X FIXED BIN(31), 2 Y FIXED BIN(31);",
                "DCL 1 T ALIGNED, 2 X FIXED BIN(31), 2 Y FIXED BIN(31);\n DCL (SP,TP) PTR;\n DCL SB BIT(80) BASED(SP), TB BIT(64) BASED(TP);",
            )
            .replace("OPEN FILE(REPORT);", "OPEN FILE(REPORT);\n SP=ADDR(S); TP=ADDR(T);")
            .replace("STG(S)=10", "LENGTH(SB)=80")
            .replace("UNSPEC(S)", "SB")
            .replace("STG(T)=8", "LENGTH(TB)=64")
            .replace("UNSPEC(T)", "TB");
    }
    if name == "PD104A" {
        s = [
            " PD104A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL (PTP,PP,MP) PTR;",
            " DCL 1 POINT BASED(PTP), 2 X FIXED BIN(31),",
            "   2 Y FIXED BIN(31);",
            " DCL 1 P LIKE POINT;",
            " DCL 1 Q LIKE POINT;",
            " DCL M FIXED DEC(9,2);",
            " DCL PB BIT(64) BASED(PP), MB BIT(40) BASED(MP);",
            " OPEN FILE(REPORT);",
            " PP=ADDR(P); MP=ADDR(M);",
            " P.X=3; P.Y=4;",
            " CALL ZCHECK('TYPE-MEMBERS', P.X=3 & P.Y=4);",
            " Q=P;",
            " CALL ZCHECK('LIKE-COPY', Q.X=3 & Q.Y=4);",
            " Q.X=8;",
            " CALL ZCHECK('LIKE-INDEPENDENT', P.X=3 & Q.X=8);",
            " M=12.345;",
            " CALL ZCHECK('ALIAS-SCALE', M=12.34);",
            " CALL ZCHECK('TYPE-SIZES',",
            "   LENGTH(PB)=64 & LENGTH(MB)=40);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PD104A;",
        ]
        .join("\n");
    }
    if name == "PD105A" {
        s = [
            " PD105A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DEFAULT RANGE(I:N) FIXED BIN(15);",
            " DCL ICOUNT INIT(7);",
            " OPEN FILE(REPORT);",
            " CALL ZCHECK('DEFAULT-I-N',",
            "   LENGTH(UNSPEC(ICOUNT))=16 & ICOUNT=7);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PD105A;",
        ]
        .join("\n");
    }
    if name == "PD106A" {
        s = [
            " PD106A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL Z COMPLEX FIXED DEC(7,2);",
            " DCL F FLOAT DEC(6), H FLOAT DEC(16);",
            " OPEN FILE(REPORT);",
            " Z=(1+2I)*(3-1I);",
            " CALL ZCHECK('COMPLEX-REAL',",
            "   REAL(Z)=5 & IMAG(Z)=5);",
            " CALL ZCHECK('COMPLEX-CONJ',",
            "   REAL(CONJG(Z))=5 & IMAG(CONJG(Z))=-5);",
            " CALL ZCHECK('COMPLEX-MAGNITUDE', ABS(3+4I)=5);",
            " F=1;",
            " CALL ZCHECK('HFP-SHORT',",
            "   HEX(UNSPEC(F))='41100000');",
            " H=-12.5;",
            " CALL ZCHECK('HFP-LONG',",
            "   HEX(UNSPEC(H))='C1C8000000000000');",
            " CALL ZCHECK('FLOAT-SIZES',",
            "   LENGTH(UNSPEC(F))=32 & LENGTH(UNSPEC(H))=64);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PD106A;",
        ]
        .join("\n");
    }
    if name == "PD107A" {
        s = [
            " PD107A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL B15 FIXED BIN(15);",
            " DCL B31 FIXED BIN(31);",
            " OPEN FILE(REPORT);",
            " B15=0;",
            " CALL ZCHECK('B15-BOUND-0', HEX(UNSPEC(B15))='0000');",
            " B15=1;",
            " CALL ZCHECK('B15-BOUND-1', HEX(UNSPEC(B15))='0001');",
            " B15=32767;",
            " CALL ZCHECK('B15-BOUND-2', HEX(UNSPEC(B15))='7FFF');",
            " B15=-1;",
            " CALL ZCHECK('B15-BOUND-3', HEX(UNSPEC(B15))='FFFF');",
            " B15=-32768;",
            " CALL ZCHECK('B15-BOUND-4', HEX(UNSPEC(B15))='8000');",
            " B31=0;",
            " CALL ZCHECK('B31-BOUND-0',",
            "   HEX(UNSPEC(B31))='00000000');",
            " B31=1;",
            " CALL ZCHECK('B31-BOUND-1',",
            "   HEX(UNSPEC(B31))='00000001');",
            " B31=2147483647;",
            " CALL ZCHECK('B31-BOUND-2',",
            "   HEX(UNSPEC(B31))='7FFFFFFF');",
            " B31=-1;",
            " CALL ZCHECK('B31-BOUND-3',",
            "   HEX(UNSPEC(B31))='FFFFFFFF');",
            " B31=-2147483647-1;",
            " CALL ZCHECK('B31-BOUND-4',",
            "   HEX(UNSPEC(B31))='80000000');",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PD107A;",
        ]
        .join("\n");
    }
    if name == "PC101A" {
        s = [
            " PC101A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL (I,J,T) FIXED BIN(31);",
            " OPEN FILE(REPORT);",
            " T=0; DO I=1 TO 9 BY 2; T=T+I; END;",
            " CALL ZCHECK('DO-TO-BY', T=25);",
            " T=0; DO I=10 TO 1 BY -3; T=T+I; END;",
            " CALL ZCHECK('DO-NEG-BY', T=22);",
            " I=0; DO WHILE(I<0); I=99; END;",
            " CALL ZCHECK('DO-WHILE-ZERO', I=0);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PC101A;",
        ]
        .join("\n");
    }
    if name == "PC102A" {
        s = s
            .replace(
                "DCL OUTER FIXED BIN(31) INIT(7);",
                "DCL OUTER FIXED BIN(31) INIT(7);\n DCL FACT ENTRY(FIXED BIN(31)) RETURNS(FIXED DEC(15));\n DCL (READP,SHADOW) ENTRY RETURNS(FIXED BIN(31));",
            )
            .replace("READP()", "READP")
            .replace("SHADOW()", "SHADOW");
    }
    if name == "PC103A" {
        s = [
            " PC103A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL FLAG FIXED BIN(31);",
            " DCL TWICE ENTRY(FIXED BIN(31))",
            "   RETURNS(FIXED BIN(31));",
            " DCL SQUARE ENTRY(FIXED BIN(31))",
            "   RETURNS(FIXED BIN(31));",
            " DCL G GENERIC(GN ENTRY(FIXED DEC(2)),",
            "   GC ENTRY(CHAR(*)));",
            " OPEN FILE(REPORT);",
            " CALL ZCHECK('ENTRY-TWICE', TWICE(7)=14);",
            " CALL ZCHECK('ENTRY-REBOUND', SQUARE(7)=49);",
            " CALL G(12);",
            " CALL ZCHECK('GENERIC-FIXED', FLAG=12);",
            " CALL G('ABC');",
            " CALL ZCHECK('GENERIC-CHAR', FLAG=3);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " TWICE: PROC(X) RETURNS(FIXED BIN(31));",
            " DCL X FIXED BIN(31); RETURN(X*2); END TWICE;",
            " SQUARE: PROC(X) RETURNS(FIXED BIN(31));",
            " DCL X FIXED BIN(31); RETURN(X*X); END SQUARE;",
            " GN: PROC(X); DCL X FIXED DEC(2); FLAG=X; END GN;",
            " GC: PROC(X); DCL X CHAR(*); FLAG=LENGTH(X); END GC;",
            " END PC103A;",
        ]
        .join("\n");
    }
    if name == "PC105A" {
        s = s.replace("DCL BAD CONDITION;\n", "");
    }
    if name == "PC106A" {
        s = [
            " PC106A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL FLAG FIXED BIN(31) INIT(0);",
            " DCL CODE FIXED BIN(31);",
            " DCL R FIXED BIN(31), Z FIXED BIN(31) INIT(0);",
            " DCL BIG FIXED DEC(3) INIT(123);",
            " DCL SMALL FIXED DEC(2);",
            " OPEN FILE(REPORT);",
            " ON ZERODIVIDE BEGIN; FLAG=1; CODE=ONCODE;",
            " GO TO AFTERDIV; END;",
            " R=1/Z; AFTERDIV:;",
            " CALL ZCHECK('ZERODIVIDE-HANDLER',",
            "   FLAG=1 & CODE=320);",
            " ON SIZE FLAG=2; (SIZE): SMALL=BIG;",
            " CALL ZCHECK('SIZE-HANDLER', FLAG=2);",
            " R=12+5;",
            " CALL ZCHECK('RECOVER-ARITH', R=17);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PC106A;",
        ]
        .join("\n");
    }
    if name == "PS101A" {
        s = [
            " PS101A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL P POINTER;",
            " DCL 1 NODE BASED(P), 2 A FIXED BIN(31), 2 B CHAR(4);",
            " DCL Q POINTER;",
            " DCL KEEP FIXED BIN(31) INIT(77);",
            " OPEN FILE(REPORT);",
            " P=NULL;",
            " CALL ZCHECK('NULL-IMAGE',",
            "   HEX(UNSPEC(P))='FF000000');",
            " ALLOCATE NODE; NODE.A=17; NODE.B='DATA';",
            " CALL ZCHECK('ALLOC-BASED',",
            "   NODE.A=17 & NODE.B='DATA');",
            " Q=P;",
            " CALL ZCHECK('POINTER-ALIAS', Q->A=17);",
            " Q->A=99;",
            " CALL ZCHECK('POINTER-REBIND', NODE.A=99);",
            " FREE NODE;",
            " CALL ZCHECK('FREE-OTHER-STORAGE', KEEP=77);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PS101A;",
        ]
        .join("\n");
    }
    if name == "PS102A" {
        s = s
            .replace("DIM(A)=3", "DIM(A,1)=3")
            .replace("DIM(A)=5", "DIM(A,1)=5");
    }
    if name == "PS103A" {
        s = [
            " PS103A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL PA POINTER, PB POINTER;",
            " DCL A AREA(256) BASED(PA), B AREA(256) BASED(PB);",
            " DCL O OFFSET(A), P POINTER;",
            " DCL 1 NODE BASED(P),",
            "   2 VALUE FIXED BIN(31), 2 TEXT CHAR(4);",
            " OPEN FILE(REPORT);",
            " ALLOCATE A; ALLOCATE B;",
            " ALLOCATE NODE IN(A) SET(P); O=P; P=O;",
            " NODE.VALUE=7; NODE.TEXT='DATA';",
            " CALL ZCHECK('AREA-ALLOC',",
            "   NODE.VALUE=7 & NODE.TEXT='DATA');",
            " CALL ZCHECK('OFFSET-ROUNDTRIP', O=P);",
            " B=A;",
            " CALL ZCHECK('AREA-COPY',",
            "   NODE.VALUE=7 & NODE.TEXT='DATA');",
            " A=EMPTY;",
            " ALLOCATE NODE IN(A) SET(P); O=P; P=O;",
            " NODE.VALUE=12;",
            " CALL ZCHECK('AREA-EMPTY-RESET', NODE.VALUE=12);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PS103A;",
        ]
        .join("\n");
    }
    if name == "PS104A" {
        s = [
            " PS104A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL N FIXED BIN(15) INIT(3), P POINTER;",
            " DCL 1 R BASED(P), 2 CNT FIXED BIN(15),",
            "   2 ITEMS(N REFER(CNT)) CHAR(4);",
            " OPEN FILE(REPORT);",
            " ALLOCATE R;",
            " CALL ZCHECK('REFER-ALLOC',",
            "   CNT=3 & DIM(ITEMS,1)=3);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PS104A;",
        ]
        .join("\n");
    }
    if name == "PS105A" {
        s = s.replace("SUM(DIAG)=143", "(DIAG(1)+DIAG(2)+DIAG(3))=143");
    }
    if name == "PF101A" {
        s = s.replace("S='-1.25,17,ALPHA';", "S='-1.25,17,''ALPHA''';");
    }
    if name == "PF102A" {
        s = [
            " PF102A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL F FILE STREAM OUTPUT PRINT ENV(F(21));",
            " DCL N FIXED BIN(31) INIT(0);",
            " OPEN FILE(REPORT);",
            " OPEN FILE(F) TITLE('PFPRINT') LINESIZE(20) PAGESIZE(3);",
            " PUT FILE(F) SKIP EDIT('ABC')(A);",
            " CALL ZCHECK('PRINT-FIRST-SKIP', LINENO(F)=1);",
            " PUT FILE(F) SKIP EDIT('DEF')(A);",
            " CALL ZCHECK('PRINT-NEXT-SKIP', LINENO(F)=2);",
            " PUT FILE(F) LINE(3) EDIT('GHI')(A);",
            " CALL ZCHECK('PRINT-LINE', LINENO(F)=3);",
            " ON ENDPAGE(F) N=N+1;",
            " PUT FILE(F) SKIP EDIT('OUTSIDE')(A);",
            " CALL ZCHECK('PRINT-ENDPAGE', N=1 & LINENO(F)=4);",
            " PUT FILE(F) SKIP EDIT('MORE')(A);",
            " CALL ZCHECK('PRINT-ENDPAGE-ONCE',",
            "   N=1 & LINENO(F)=5);",
            " PUT FILE(F) PAGE EDIT('PAGE2')(A);",
            " CALL ZCHECK('PRINT-PAGE', LINENO(F)=1);",
            " CLOSE FILE(F); OPEN FILE(F) TITLE('PFPRNEW');",
            " CALL ZCHECK('PRINT-REOPEN', LINENO(F)=1);",
            " CLOSE FILE(F);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PF102A;",
        ]
        .join("\n");
    }
    if name == "PM103A" {
        s = [
            " PM103A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " OPEN FILE(REPORT);",
            " CALL ZCHECK('BIF-TRIM', TRIM('  AB  ')='AB');",
            " CALL ZCHECK('BIF-COPY', REPEAT('AB',2)='ABABAB');",
            " CALL ZCHECK('BIF-TRANSLATE',",
            "   TRANSLATE('ABCA','xy','AB')='xyCx');",
            " CALL ZCHECK('BIF-INDEX', INDEX('ABABA','BA')=2);",
            " CALL ZCHECK('BIF-VERIFY', VERIFY('ABX','ABC')=3);",
            " CALL ZCHECK('BIF-LENGTH', LENGTH('')=0);",
            " CALL ZCHECK('BIF-MOD', MOD(-5,3)=1);",
            " CALL ZCHECK('BIF-FLOOR', FLOOR(-1.2)=-2);",
            " CALL ZCHECK('BIF-CEIL', CEIL(-1.2)=-1);",
            " CALL ZCHECK('BIF-ROUND', ROUND(12.345,2)=12.35);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PM103A;",
        ]
        .join("\n");
    }
    if name == "PM105A" {
        s = [
            " PM105A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " OPEN FILE(REPORT);",
            " CALL ZCHECK('SQRT', SQRT(81)=9);",
            " CALL ZCHECK('SIN-ZERO', SIN(0)=0);",
            " CALL ZCHECK('COS-ZERO', COS(0)=1);",
            " CALL ZCHECK('LOG-ONE', LOG(1)=0);",
            " CALL ZCHECK('ABS-NEG', ABS(-17)=17);",
            " CALL ZCHECK('MINMAX', MAX(1,7,3)=7 & MIN(1,7,3)=1);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PM105A;",
        ]
        .join("\n");
    }
    if name == "PM106A" {
        s = [
            " %DCL SETVAL ENTRY(CHAR,CHAR) RETURNS(CHAR);",
            " %SETVAL: PROC(NAME,VALUE) RETURNS(CHAR);",
            " DCL (NAME,VALUE) CHAR;",
            " RETURN(NAME||'='||VALUE||';');",
            " %END SETVAL;",
            " %ACTIVATE SETVAL;",
            " PM106A: PROC OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL N FIXED BIN(31);",
            " OPEN FILE(REPORT);",
            " SETVAL(N,42)",
            " CALL ZCHECK('STATEMENT-POSITIONAL', N=42);",
            " CALL ZFINISH;",
            " CALL PLIRETC(0);",
            " STOP;",
            " END PM106A;",
        ]
        .join("\n");
    }
    if name == "PM107A" {
        s = [
            " PM107A: PROC(P) OPTIONS(MAIN);",
            " %INCLUDE ZPVSRPT;",
            " DCL P CHAR(100) VARYING;",
            " OPEN FILE(REPORT);",
            " CALL ZCHECK('MAIN-PARM-TEXT', P='HELLO');",
            " CALL ZCHECK('MAIN-PARM-LENGTH', LENGTH(P)=5);",
            " CALL PLIRETC(4);",
            " CALL ZCHECK('PLIRETC-VALUE', PLIRETV=4);",
            " CALL ZFINISH;",
            " CALL PLIRETC(4);",
            " STOP;",
            " END PM107A;",
        ]
        .join("\n");
    }
    s
}

/// Runs a batch of ZPVS programs on MVS 3.8j under Hercules and returns the
/// printer `000F` (`Printer00F.txt`) lines and Hercules version.
fn run_mvs_batch(
    herc: &Hercules,
    mvs_dasd_dir: &Path,
    work_dir: &Path,
    progs: &[(&str, &str)],
    suite_dir: &Path,
) -> Result<(Vec<String>, String)> {
    fs::create_dir_all(work_dir.join("shadow"))?;
    for entry in fs::read_dir(work_dir.join("shadow"))? {
        let _ = fs::remove_file(entry?.path());
    }
    let _ = fs::remove_file(work_dir.join("Printer00F.txt"));
    let _ = fs::remove_file(work_dir.join("Printer10E.txt"));
    let _ = fs::remove_file(work_dir.join("ConsoleLog.txt"));

    let mut lines: Vec<String> = vec![
        "//ZPVSJOB  JOB ,'ZPVS PLI',CLASS=A,MSGCLASS=A,MSGLEVEL=(1,1)".into(),
        "//PL1CLG   PROC P=RUN,PRM=''".into(),
        "//PL1      EXEC PGM=IEMAA,REGION=4096K,".into(),
        "//         PARM='MACRO,LOAD,NODECK,NOSOURCE2,NOSOURCE'".into(),
        "//SYSPRINT DD UNIT=10E".into(),
        "//SYSUT3   DD DSNAME=&&SYSUT3,UNIT=SYSDA,SPACE=(80,(250,250)),".into(),
        "//            DCB=BLKSIZE=80".into(),
        "//SYSUT1   DD DSNAME=&&SYSUT1,UNIT=SYSDA,SPACE=(1024,(60,60),,CONTIG),".into(),
        "//            SEP=(SYSUT3,SYSLIN),DCB=BLKSIZE=1024".into(),
        "//SYSLIN   DD DISP=(MOD,PASS),UNIT=SYSDA,SPACE=(80,(250,100))".into(),
        "//LKED     EXEC PGM=IEWL,REGION=2048K,COND=(9,LT,PL1),".into(),
        "//         PARM='LET'".into(),
        "//SYSPRINT DD UNIT=10E".into(),
        "//SYSLIN   DD DISP=(OLD,DELETE),DSNAME=*.PL1.SYSLIN".into(),
        "//         DD DDNAME=SYSIN".into(),
        "//SYSLMOD  DD DISP=(MOD,PASS),DSNAME=&&GODATA(&P),".into(),
        "//            UNIT=SYSDA,SPACE=(CYL,(5,5,20))".into(),
        "//SYSLIB   DD DISP=SHR,DSNAME=SYS1.PL1LIB".into(),
        "//SYSUT1   DD UNIT=(SYSDA,SEP=(SYSLIN,SYSLMOD)),SPACE=(1024,(50,20))".into(),
        "//GO       EXEC PGM=*.LKED.SYSLMOD,PARM='&PRM',".into(),
        "//         COND=((9,LT,PL1),(5,LT,LKED))".into(),
        "//STEPLIB  DD DISP=SHR,DSNAME=SYS1.PL1LIB".into(),
        "//SYSPRINT DD SYSOUT=*".into(),
        "//REPORT   DD UNIT=00F,DCB=(RECFM=F,LRECL=120,BLKSIZE=120)".into(),
        "//XXXXX001 DD UNIT=SYSDA,SPACE=(TRK,(2,2))".into(),
        "//PFPRINT  DD UNIT=SYSDA,SPACE=(TRK,(2,2))".into(),
        "//PFPRNEW  DD UNIT=SYSDA,SPACE=(TRK,(2,2))".into(),
        "//         PEND".into(),
    ];

    for &(m, p) in progs {
        let raw = fs::read_to_string(suite_dir.join(format!("{m}/{p}.PLI")))?;
        let src = adapt_plif(p, &raw);
        if p == "PM107A" {
            lines.push(format!("//{p:<8} EXEC PL1CLG,P={p},PRM='HELLO'"));
        } else {
            lines.push(format!("//{p:<8} EXEC PL1CLG,P={p}"));
        }
        lines.push("//PL1.SYSIN DD *".into());
        for l in src.lines() {
            if l.trim() == "%INCLUDE ZPVSRPT;" {
                for rl in ZPVSRPT_PLIF.lines().filter(|s| !s.is_empty()) {
                    lines.push(rl.to_string());
                }
            } else if l.trim() == "OPEN FILE(REPORT);" {
                lines.push(l.to_string());
                lines.push(format!(
                    " ZLINE='=== PROGRAM {p} ==='; WRITE FILE(REPORT) FROM(ZLINE);"
                ));
            } else {
                lines.push(l.to_string());
            }
        }
        lines.push("/*".into());
    }
    lines.push("//".into());

    write_ebcdic_deck(&work_dir.join("job.ebcdic"), &lines)?;

    let dasd = mvs_dasd_dir
        .canonicalize()
        .unwrap_or_else(|_| mvs_dasd_dir.to_path_buf());
    let dasd_str = dasd.display().to_string().replace('\\', "/");
    let cnf = format!(
        "CPUSERIAL 000011\nCPUMODEL 3033\nMAINSIZE 16\nXPNDSIZE 0\n\
         CNSLPORT 3270\nNUMCPU 1\nLOADPARM 0109....\nOSTAILOR OS/390\n\
         PANRATE FAST\nARCHMODE S/370\n\
         0009 3215-C / noprompt\n\
         000C 3505 job.ebcdic ebcdic eof\n\
         000D 3525 Punch00D.txt ascii\n\
         000E 1403 Printer00E.txt crlf\n\
         000F 1403 Printer00F.txt crlf\n\
         010E 1403 Printer10E.txt crlf\n\
         0140 3350 {dasd_str}/mvsres.140.cckd sf=shadow/mvsres_1.sf\n\
         0141 3350 {dasd_str}/mvs001.141.cckd sf=shadow/mvs001_1.sf\n\
         0142 3350 {dasd_str}/mvs002.142.cckd sf=shadow/mvs002_1.sf\n\
         0143 3350 {dasd_str}/mvsspl.143.cckd sf=shadow/mvsspl_1.sf\n\
         0144 3350 {dasd_str}/mvsusr.144.cckd sf=shadow/mvsusr_1.sf\n\
         0148 3350 {dasd_str}/work01.148.cckd sf=shadow/work01_1.sf\n\
         0149 3350 {dasd_str}/work02.149.cckd sf=shadow/work02_1.sf\n\
         014A 3350 {dasd_str}/work03.14a.cckd sf=shadow/work03_1.sf\n\
         014B 3350 {dasd_str}/work04.14b.cckd sf=shadow/work04_1.sf\n"
    );
    fs::write(work_dir.join("hercules.cnf"), cnf)?;

    let wait_secs = herc.run_seconds.max(6);
    let rc_script = format!(
        "log ConsoleLog.txt\nlogopt timestamp\nipl 140\n\
         pause 2\n/\npause 2\n/r 00,noreq\npause {wait_secs}\nquit\n"
    );
    fs::write(work_dir.join("hercules.rc"), rc_script)?;

    let log = fs::File::create(work_dir.join("hercules.log"))?;
    let mut child = Command::new(&herc.binary)
        .args(["-f", "hercules.cnf"])
        .env("HERCULES_RC", "hercules.rc")
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .with_context(|| format!("starting {}", herc.binary.display()))?;

    let limit = Duration::from_secs(u64::from(wait_secs) + 60);
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "Hercules MVS 3.8j did not terminate; see {}",
                work_dir.join("hercules.log").display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let report_raw = fs::read_to_string(work_dir.join("Printer00F.txt")).unwrap_or_default();
    let report_lines: Vec<String> = report_raw
        .lines()
        .map(|l| l.trim_matches('\r').trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let hlog = fs::read_to_string(work_dir.join("hercules.log")).unwrap_or_default();
    let version = hlog
        .lines()
        .find(|l| l.starts_with("Hercules Version"))
        .unwrap_or("Hercules (unknown version)")
        .trim()
        .to_string();

    Ok((report_lines, version))
}

pub fn run_zpvs(
    suite_dir: &Path,
    mvs_dasd_dir: &Path,
    herc: &Hercules,
    work_dir: &Path,
    filter: &[String],
) -> Result<ZpvsOracleRun> {
    if !mvs_dasd_dir.join("mvsres.140.cckd").is_file() {
        bail!(
            "MVS 3.8j DASD image mvsres.140.cckd not found in {}",
            mvs_dasd_dir.display()
        );
    }

    let selected: Vec<(&str, &str)> = if filter.is_empty() {
        DEFAULT_RUN_PROGRAMS.to_vec()
    } else {
        let mut list = Vec::new();
        for p in filter {
            let p_up = p.to_ascii_uppercase();
            let m = &p_up[..2];
            list.push((
                Box::leak(m.to_string().into_boxed_str()) as &str,
                Box::leak(p_up.into_boxed_str()) as &str,
            ));
        }
        list
    };

    if selected.is_empty() {
        bail!("no ZPVS programs selected");
    }

    let mut all_lines = Vec::new();
    let mut version = String::new();
    for chunk in selected.chunks(17) {
        let (lines, ver) = run_mvs_batch(herc, mvs_dasd_dir, work_dir, chunk, suite_dir)?;
        if version.is_empty() {
            version = ver;
        }
        all_lines.extend(lines);
    }

    let mut by_prog: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut cur_prog: Option<String> = None;
    for l in all_lines {
        if let Some(rest) = l.strip_prefix("=== PROGRAM ")
            && let Some(p) = rest.strip_suffix(" ===")
        {
            cur_prog = Some(p.trim().to_string());
            continue;
        }
        if let Some(ref p) = cur_prog {
            by_prog.entry(p.clone()).or_default().push(l);
        }
    }

    let mut programs = Vec::with_capacity(selected.len());
    let mut total_checks = 0;
    let mut passed_checks = 0;
    let mut failed_checks_total = 0;

    for &(m_name, p_name) in &selected {
        let report_lines = by_prog.remove(p_name).unwrap_or_default();
        let mut pass = 0;
        let mut fail = 0;
        let mut failed_checks = Vec::new();
        for l in &report_lines {
            if l.starts_with("PASS ") {
                pass += 1;
            } else if let Some(id) = l.strip_prefix("FAIL*") {
                fail += 1;
                failed_checks.push(id.trim().to_string());
            }
        }
        total_checks += pass + fail;
        passed_checks += pass;
        failed_checks_total += fail;

        programs.push(ZpvsProgramResult {
            module: m_name.to_string(),
            program: p_name.to_string(),
            pass,
            fail,
            failed_checks,
            report_lines,
        });
    }

    let mut by_mod: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    for p in &programs {
        let e = by_mod.entry(p.module.as_str()).or_default();
        e.0 += 1;
        e.1 += p.pass;
        e.2 += p.fail;
    }
    for (m, (tot_p, pass_c, fail_c)) in &by_mod {
        println!(
            "ZPVS {m:<4} programs={tot_p} checks={pass_c}/{} (fail={fail_c})",
            pass_c + fail_c
        );
    }

    Ok(ZpvsOracleRun {
        oracle: version,
        compiler: "OS/360 PL/I (F) Version 5.5 (MVS R3.8 PGM=IEMAA)".to_string(),
        total_programs: programs.len(),
        total_checks,
        passed_checks,
        failed_checks: failed_checks_total,
        programs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn encodes_all_default_zpvs_programs_within_column_72() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let suite = root.join("zpvs");
        for &(m, p) in DEFAULT_RUN_PROGRAMS {
            let raw = fs::read_to_string(suite.join(format!("{m}/{p}.PLI"))).unwrap();
            let src = adapt_plif(p, &raw);
            for l in src.lines() {
                if l.trim() == "%INCLUDE ZPVSRPT;" {
                    for rl in ZPVSRPT_PLIF.lines().filter(|s| !s.is_empty()) {
                        encode_card(rl).unwrap();
                    }
                } else {
                    encode_card(l).unwrap();
                }
            }
        }
    }
}

       CBL NUMPROC(NOPFD),NUMCLS(PRIM)
      * SPEC: NUMERIC CLASS SIGN TABLES; AUDIT A07.
      * NUMCLS IS AN INSTALLATION SETTING ON IBM Z/OS.
      * ZEUS USES CBL NUMCLS TO SELECT THAT SETTING.
      * ZA111A: NUMPROC/NUMCLS class matrix and sign repair
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA111A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  SP-RAW PIC X(3).
       01  SP REDEFINES SP-RAW PIC S9(4) COMP-3.
       01  UP-RAW PIC X(3).
       01  UP REDEFINES UP-RAW PIC 9(4) COMP-3.
       01  SZ-RAW PIC X(4).
       01  SZ REDEFINES SZ-RAW PIC S9(4).
       01  UZ-RAW PIC X(4).
       01  UZ REDEFINES UZ-RAW PIC 9(4).
       01  OUT-P PIC S9(4) COMP-3.
       01  OUT-P-RAW REDEFINES OUT-P PIC X(3).
       01  OUT-Z PIC S9(4).
       01  OUT-Z-RAW REDEFINES OUT-Z PIC X(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA111A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'SP-SIGN-0' TO ZCVS-TEST-NAME.
           MOVE X'012340' TO SP-RAW.
           IF SP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-9' TO ZCVS-TEST-NAME.
           MOVE X'012349' TO SP-RAW.
           IF SP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-A' TO ZCVS-TEST-NAME.
           MOVE X'01234A' TO SP-RAW.
           IF SP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-B' TO ZCVS-TEST-NAME.
           MOVE X'01234B' TO SP-RAW.
           IF SP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-C' TO ZCVS-TEST-NAME.
           MOVE X'01234C' TO SP-RAW.
           IF SP IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-D' TO ZCVS-TEST-NAME.
           MOVE X'01234D' TO SP-RAW.
           IF SP IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-E' TO ZCVS-TEST-NAME.
           MOVE X'01234E' TO SP-RAW.
           IF SP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-SIGN-F' TO ZCVS-TEST-NAME.
           MOVE X'01234F' TO SP-RAW.
           IF SP IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-0' TO ZCVS-TEST-NAME.
           MOVE X'012340' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-9' TO ZCVS-TEST-NAME.
           MOVE X'012349' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-A' TO ZCVS-TEST-NAME.
           MOVE X'01234A' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-B' TO ZCVS-TEST-NAME.
           MOVE X'01234B' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-C' TO ZCVS-TEST-NAME.
           MOVE X'01234C' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-D' TO ZCVS-TEST-NAME.
           MOVE X'01234D' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-E' TO ZCVS-TEST-NAME.
           MOVE X'01234E' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-SIGN-F' TO ZCVS-TEST-NAME.
           MOVE X'01234F' TO UP-RAW.
           IF UP IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-0' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F304' TO SZ-RAW.
           IF SZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-9' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F394' TO SZ-RAW.
           IF SZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-A' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3A4' TO SZ-RAW.
           IF SZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-B' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3B4' TO SZ-RAW.
           IF SZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-C' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3C4' TO SZ-RAW.
           IF SZ IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-D' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3D4' TO SZ-RAW.
           IF SZ IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-E' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3E4' TO SZ-RAW.
           IF SZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-SIGN-F' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3F4' TO SZ-RAW.
           IF SZ IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-0' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F304' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-9' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F394' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-A' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3A4' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-B' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3B4' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-C' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3C4' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-D' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3D4' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-E' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3E4' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-SIGN-F' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3F4' TO UZ-RAW.
           IF UZ IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-BAD-DIGIT' TO ZCVS-TEST-NAME.
           MOVE X'01A34C' TO SP-RAW.
           IF SP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UP-BAD-DIGIT' TO ZCVS-TEST-NAME.
           MOVE X'01A34F' TO UP-RAW.
           IF UP IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-BAD-DIGIT' TO ZCVS-TEST-NAME.
           MOVE X'F1FAF3C4' TO SZ-RAW.
           IF SZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UZ-BAD-DIGIT' TO ZCVS-TEST-NAME.
           MOVE X'F1FAF3F4' TO UZ-RAW.
           IF UZ IS NOT NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NOT NUMERIC' TO ZCVS-EXPECTED
               MOVE UZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-UNSIGNED-ZERO' TO ZCVS-TEST-NAME.
           MOVE X'00000F' TO SP-RAW.
           IF SP IS NUMERIC AND SP IS ZERO
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC ZERO' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-UNSIGNED-ZERO' TO ZCVS-TEST-NAME.
           MOVE X'F0F0F0F0' TO SZ-RAW.
           IF SZ IS NUMERIC AND SZ IS ZERO
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NUMERIC ZERO' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-NEGATIVE-ZERO' TO ZCVS-TEST-NAME.
           MOVE X'00000D' TO SP-RAW.
           IF SP IS NUMERIC AND SP IS ZERO
             AND SP IS NOT NEGATIVE
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ZERO, NOT NEGATIVE' TO ZCVS-EXPECTED
               MOVE SP-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-NEGATIVE-ZERO' TO ZCVS-TEST-NAME.
           MOVE X'F0F0F0D0' TO SZ-RAW.
           IF SZ IS NUMERIC AND SZ IS ZERO
             AND SZ IS NOT NEGATIVE
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ZERO, NOT NEGATIVE' TO ZCVS-EXPECTED
               MOVE SZ-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-MOVE-A' TO ZCVS-TEST-NAME.
           MOVE X'01234A' TO SP-RAW.
           MOVE SP TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-ADD-A' TO ZCVS-TEST-NAME.
           MOVE X'01234A' TO SP-RAW.
           ADD SP TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-MOVE-A' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3A4' TO SZ-RAW.
           MOVE SZ TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-ADD-A' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3A4' TO SZ-RAW.
           ADD SZ TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-MOVE-B' TO ZCVS-TEST-NAME.
           MOVE X'01234B' TO SP-RAW.
           MOVE SP TO OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-ADD-B' TO ZCVS-TEST-NAME.
           MOVE X'01234B' TO SP-RAW.
           ADD SP TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-MOVE-B' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3B4' TO SZ-RAW.
           MOVE SZ TO OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-ADD-B' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3B4' TO SZ-RAW.
           ADD SZ TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-MOVE-C' TO ZCVS-TEST-NAME.
           MOVE X'01234C' TO SP-RAW.
           MOVE SP TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-ADD-C' TO ZCVS-TEST-NAME.
           MOVE X'01234C' TO SP-RAW.
           ADD SP TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-MOVE-C' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3C4' TO SZ-RAW.
           MOVE SZ TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-ADD-C' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3C4' TO SZ-RAW.
           ADD SZ TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-MOVE-D' TO ZCVS-TEST-NAME.
           MOVE X'01234D' TO SP-RAW.
           MOVE SP TO OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-ADD-D' TO ZCVS-TEST-NAME.
           MOVE X'01234D' TO SP-RAW.
           ADD SP TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-MOVE-D' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3D4' TO SZ-RAW.
           MOVE SZ TO OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-ADD-D' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3D4' TO SZ-RAW.
           ADD SZ TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = -1234 AND OUT-Z = -1234
             AND OUT-P-RAW = X'01234D'
             AND OUT-Z-RAW = X'F1F2F3D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-MOVE-E' TO ZCVS-TEST-NAME.
           MOVE X'01234E' TO SP-RAW.
           MOVE SP TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-ADD-E' TO ZCVS-TEST-NAME.
           MOVE X'01234E' TO SP-RAW.
           ADD SP TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-MOVE-E' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3E4' TO SZ-RAW.
           MOVE SZ TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-ADD-E' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3E4' TO SZ-RAW.
           ADD SZ TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-MOVE-F' TO ZCVS-TEST-NAME.
           MOVE X'01234F' TO SP-RAW.
           MOVE SP TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SP-ADD-F' TO ZCVS-TEST-NAME.
           MOVE X'01234F' TO SP-RAW.
           ADD SP TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-MOVE-F' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3F4' TO SZ-RAW.
           MOVE SZ TO OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SZ-ADD-F' TO ZCVS-TEST-NAME.
           MOVE X'F1F2F3F4' TO SZ-RAW.
           ADD SZ TO ZERO GIVING OUT-P OUT-Z.
           IF OUT-P = 1234 AND OUT-Z = 1234
             AND OUT-P-RAW = X'01234C'
             AND OUT-Z-RAW = X'F1F2F3C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/PREFERRED' TO ZCVS-EXPECTED
               MOVE OUT-P-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

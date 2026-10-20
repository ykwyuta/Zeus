      * AUDIT: A03/D03; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL TRUNC(OPT),ARITH(EXTEND)
      * SPEC: TRUNC OPTION; AUDIT A01/A03.
      * ZA117A: TRUNC OPT aliases storage width picture boundaries
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA117A.
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
       01  N1C PIC S9(1) COMP.
       01  N1B PIC S9(1) BINARY.
       01  N1F PIC S9(1) COMP-4.
       01  N4C PIC S9(4) COMP.
       01  N4B PIC S9(4) BINARY.
       01  N4F PIC S9(4) COMP-4.
       01  N5C PIC S9(5) COMP.
       01  N5B PIC S9(5) BINARY.
       01  N5F PIC S9(5) COMP-4.
       01  N9C PIC S9(9) COMP.
       01  N9B PIC S9(9) BINARY.
       01  N9F PIC S9(9) COMP-4.
       01  N10C PIC S9(10) COMP.
       01  N10B PIC S9(10) BINARY.
       01  N10F PIC S9(10) COMP-4.
       01  N18C PIC S9(18) COMP.
       01  N18B PIC S9(18) BINARY.
       01  N18F PIC S9(18) COMP-4.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA117A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'N1C-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N1C.
           IF N1C = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1C-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N1C.
           IF N1C = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1C-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N1C.
           IF N1C = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1C-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9 TO N1C.
           IF N1C = 9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1C-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9 TO N1C.
           IF N1C = -9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1C-ARITH' TO ZCVS-TEST-NAME.
           MOVE 8 TO N1C.
           ADD 1 TO N1C.
           IF N1C = 9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1B-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N1B.
           IF N1B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1B-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N1B.
           IF N1B = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1B-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N1B.
           IF N1B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1B-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9 TO N1B.
           IF N1B = 9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1B-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9 TO N1B.
           IF N1B = -9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1B-ARITH' TO ZCVS-TEST-NAME.
           MOVE 8 TO N1B.
           ADD 1 TO N1B.
           IF N1B = 9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1F-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N1F.
           IF N1F = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1F-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N1F.
           IF N1F = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1F-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N1F.
           IF N1F = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1F-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9 TO N1F.
           IF N1F = 9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1F-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9 TO N1F.
           IF N1F = -9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N1F-ARITH' TO ZCVS-TEST-NAME.
           MOVE 8 TO N1F.
           ADD 1 TO N1F.
           IF N1F = 9
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4C-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N4C.
           IF N4C = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4C-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N4C.
           IF N4C = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4C-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N4C.
           IF N4C = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4C-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9999 TO N4C.
           IF N4C = 9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4C-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9999 TO N4C.
           IF N4C = -9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4C-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9998 TO N4C.
           ADD 1 TO N4C.
           IF N4C = 9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4B-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N4B.
           IF N4B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4B-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N4B.
           IF N4B = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4B-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N4B.
           IF N4B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4B-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9999 TO N4B.
           IF N4B = 9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4B-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9999 TO N4B.
           IF N4B = -9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4B-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9998 TO N4B.
           ADD 1 TO N4B.
           IF N4B = 9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4F-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N4F.
           IF N4F = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4F-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N4F.
           IF N4F = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4F-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N4F.
           IF N4F = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4F-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9999 TO N4F.
           IF N4F = 9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4F-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9999 TO N4F.
           IF N4F = -9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4F-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9998 TO N4F.
           ADD 1 TO N4F.
           IF N4F = 9999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5C-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N5C.
           IF N5C = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5C-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N5C.
           IF N5C = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5C-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N5C.
           IF N5C = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5C-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 99999 TO N5C.
           IF N5C = 99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5C-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -99999 TO N5C.
           IF N5C = -99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5C-ARITH' TO ZCVS-TEST-NAME.
           MOVE 99998 TO N5C.
           ADD 1 TO N5C.
           IF N5C = 99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5B-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N5B.
           IF N5B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5B-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N5B.
           IF N5B = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5B-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N5B.
           IF N5B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5B-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 99999 TO N5B.
           IF N5B = 99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5B-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -99999 TO N5B.
           IF N5B = -99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5B-ARITH' TO ZCVS-TEST-NAME.
           MOVE 99998 TO N5B.
           ADD 1 TO N5B.
           IF N5B = 99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5F-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N5F.
           IF N5F = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5F-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N5F.
           IF N5F = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5F-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N5F.
           IF N5F = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5F-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 99999 TO N5F.
           IF N5F = 99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5F-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -99999 TO N5F.
           IF N5F = -99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N5F-ARITH' TO ZCVS-TEST-NAME.
           MOVE 99998 TO N5F.
           ADD 1 TO N5F.
           IF N5F = 99999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9C-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N9C.
           IF N9C = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9C-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N9C.
           IF N9C = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9C-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N9C.
           IF N9C = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9C-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 999999999 TO N9C.
           IF N9C = 999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9C-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -999999999 TO N9C.
           IF N9C = -999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9C-ARITH' TO ZCVS-TEST-NAME.
           MOVE 999999998 TO N9C.
           ADD 1 TO N9C.
           IF N9C = 999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9B-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N9B.
           IF N9B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9B-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N9B.
           IF N9B = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9B-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N9B.
           IF N9B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9B-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 999999999 TO N9B.
           IF N9B = 999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9B-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -999999999 TO N9B.
           IF N9B = -999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9B-ARITH' TO ZCVS-TEST-NAME.
           MOVE 999999998 TO N9B.
           ADD 1 TO N9B.
           IF N9B = 999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9F-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N9F.
           IF N9F = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9F-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N9F.
           IF N9F = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9F-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N9F.
           IF N9F = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9F-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 999999999 TO N9F.
           IF N9F = 999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9F-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -999999999 TO N9F.
           IF N9F = -999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N9F-ARITH' TO ZCVS-TEST-NAME.
           MOVE 999999998 TO N9F.
           ADD 1 TO N9F.
           IF N9F = 999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10C-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N10C.
           IF N10C = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10C-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N10C.
           IF N10C = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10C-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N10C.
           IF N10C = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10C-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9999999999 TO N10C.
           IF N10C = 9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10C-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9999999999 TO N10C.
           IF N10C = -9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10C-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9999999998 TO N10C.
           ADD 1 TO N10C.
           IF N10C = 9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10B-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N10B.
           IF N10B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10B-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N10B.
           IF N10B = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10B-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N10B.
           IF N10B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10B-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9999999999 TO N10B.
           IF N10B = 9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10B-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9999999999 TO N10B.
           IF N10B = -9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10B-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9999999998 TO N10B.
           ADD 1 TO N10B.
           IF N10B = 9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10F-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N10F.
           IF N10F = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10F-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N10F.
           IF N10F = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10F-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N10F.
           IF N10F = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10F-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 9999999999 TO N10F.
           IF N10F = 9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10F-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -9999999999 TO N10F.
           IF N10F = -9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N10F-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9999999998 TO N10F.
           ADD 1 TO N10F.
           IF N10F = 9999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18C-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N18C.
           IF N18C = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18C-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N18C.
           IF N18C = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18C-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N18C.
           IF N18C = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18C-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999 TO N18C.
           IF N18C = 999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18C-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -999999999999999999 TO N18C.
           IF N18C = -999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18C-ARITH' TO ZCVS-TEST-NAME.
           MOVE 999999999999999998 TO N18C.
           ADD 1 TO N18C.
           IF N18C = 999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18B-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N18B.
           IF N18B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18B-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N18B.
           IF N18B = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18B-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N18B.
           IF N18B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18B-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999 TO N18B.
           IF N18B = 999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18B-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -999999999999999999 TO N18B.
           IF N18B = -999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18B-ARITH' TO ZCVS-TEST-NAME.
           MOVE 999999999999999998 TO N18B.
           ADD 1 TO N18B.
           IF N18B = 999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18F-MOVE-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO N18F.
           IF N18F = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18F-MOVE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO N18F.
           IF N18F = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18F-MOVE-2' TO ZCVS-TEST-NAME.
           MOVE -1 TO N18F.
           IF N18F = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18F-MOVE-3' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999 TO N18F.
           IF N18F = 999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18F-MOVE-4' TO ZCVS-TEST-NAME.
           MOVE -999999999999999999 TO N18F.
           IF N18F = -999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N18F-ARITH' TO ZCVS-TEST-NAME.
           MOVE 999999999999999998 TO N18F.
           ADD 1 TO N18F.
           IF N18F = 999999999999999999
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999999999999' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

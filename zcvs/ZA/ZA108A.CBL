      *================================================================*
      * ZA108A - INTRINSIC FUNCTIONS WITH COMP-1, COMP-2, AND COMP-3   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA108A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-FN-FIELDS.
           05  WS-C2-IN             COMP-2.
           05  WS-C2-OUT            COMP-2.
           05  WS-DEC               PIC S9(7)V9(4) COMP-3.
           05  WS-INT               PIC S9(9) COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA108A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: FUNCTION SQRT ON COMP-2
           MOVE 'FN-SQRT-C2' TO ZCVS-TEST-NAME.
           MOVE 144.0 TO WS-C2-IN.
           COMPUTE WS-DEC ROUNDED = FUNCTION SQRT(WS-C2-IN).
           IF WS-DEC = 12.0000
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.0000' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: FUNCTION SIN(0) AND COS(0)
           MOVE 'FN-SIN-COS-0' TO ZCVS-TEST-NAME.
           MOVE 0.0 TO WS-C2-IN.
           COMPUTE WS-DEC ROUNDED =
             FUNCTION SIN(WS-C2-IN) + FUNCTION COS(WS-C2-IN).
           IF WS-DEC = 1.0000
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1.0000' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: FUNCTION LOG10(1000) AND FACTORIAL(5)
           MOVE 'FN-LOG10-FACT' TO ZCVS-TEST-NAME.
           COMPUTE WS-DEC ROUNDED =
             FUNCTION LOG10(1000) + FUNCTION FACTORIAL(5).
           IF WS-DEC = 123.0000
               PERFORM ZCVS-PASS
           ELSE
               MOVE '123.0000' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: FUNCTION INTEGER, INTEGER-PART, MOD, REM
           MOVE 'FN-INT-MOD-REM' TO ZCVS-TEST-NAME.
           COMPUTE WS-INT =
             FUNCTION INTEGER(-3.2) + FUNCTION INTEGER-PART(-3.8)
             + FUNCTION MOD(-11 5) + FUNCTION REM(-11 5).
           IF WS-INT = -4 + -3 + 4 + -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-4' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: FUNCTION NUMVAL AND NUMVAL-C IN EBCDIC
           MOVE 'FN-NUMVAL-C' TO ZCVS-TEST-NAME.
           COMPUTE WS-DEC =
             FUNCTION NUMVAL(' -123.45 ')
             + FUNCTION NUMVAL-C(' $1,200.50CR ').
           IF WS-DEC = -1323.9500
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1323.9500' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: FUNCTION MEAN, MEDIAN, SUM, MAX, MIN
           MOVE 'FN-STATS' TO ZCVS-TEST-NAME.
           COMPUTE WS-DEC =
             FUNCTION MEAN(10 20 30 40 50)
             + FUNCTION MEDIAN(10 50 20 40 30)
             + FUNCTION MAX(10 50 20)
             - FUNCTION MIN(10 50 20).
           IF WS-DEC = 30 + 30 + 50 - 10
               PERFORM ZCVS-PASS
           ELSE
               MOVE '100' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

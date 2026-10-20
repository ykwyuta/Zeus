      * AUDIT: D07; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZD115A: Nested SYNC strides COMP-5 and PACKED contrast
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD115A.
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
       01  NESTED-TABLE.
           05  FIRST-BYTE PIC X.
           05  OUTER-ROW OCCURS 2.
               10  INNER-ROW OCCURS 2.
                   15  TAG PIC X.
                   15  NUM-ITEM PIC S9(9) COMP SYNC.
                   15  END-TAG PIC X.
               10  OUTER-END PIC X.
       01  RAW REDEFINES NESTED-TABLE PIC X(41).
       01  I PIC 9 COMP.
       01  J PIC 9 COMP.
       01  DOUBLE-TABLE.
           05  DOUBLE-ROW OCCURS 2.
               10  DOUBLE-TAG PIC X.
               10  DOUBLE-NUM PIC S9(18) COMP-5 SYNC.
               10  DOUBLE-END PIC X.
       01  DOUBLE-RAW REDEFINES DOUBLE-TABLE PIC X(48).
       01  PACKED-TABLE.
           05  PACKED-ROW OCCURS 2.
               10  PACKED-TAG PIC X.
               10  PACKED-NUM PIC S9(4) COMP-3 SYNC.
               10  PACKED-END PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD115A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'NESTED-LENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(NESTED-TABLE) = 41
             AND FUNCTION LENGTH(OUTER-ROW(1)) = 20
             AND FUNCTION LENGTH(INNER-ROW(1, 1)) = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '41/20/8' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NESTED-1-1' TO ZCVS-TEST-NAME.
           MOVE 257 TO NUM-ITEM(1, 1).
           MOVE 'A' TO TAG(1, 1).
           MOVE 'Z' TO END-TAG(1, 1).
           IF RAW(5:4) = X'00000101'
             AND RAW(2:1) = 'A'
             AND RAW(9:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000101' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NESTED-1-2' TO ZCVS-TEST-NAME.
           MOVE 258 TO NUM-ITEM(1, 2).
           MOVE 'A' TO TAG(1, 2).
           MOVE 'Z' TO END-TAG(1, 2).
           IF RAW(13:4) = X'00000102'
             AND RAW(10:1) = 'A'
             AND RAW(17:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000102' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NESTED-2-1' TO ZCVS-TEST-NAME.
           MOVE 513 TO NUM-ITEM(2, 1).
           MOVE 'A' TO TAG(2, 1).
           MOVE 'Z' TO END-TAG(2, 1).
           IF RAW(25:4) = X'00000201'
             AND RAW(22:1) = 'A'
             AND RAW(29:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000201' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NESTED-2-2' TO ZCVS-TEST-NAME.
           MOVE 514 TO NUM-ITEM(2, 2).
           MOVE 'A' TO TAG(2, 2).
           MOVE 'Z' TO END-TAG(2, 2).
           IF RAW(33:4) = X'00000202'
             AND RAW(30:1) = 'A'
             AND RAW(37:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000202' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'OUTER-TRAILER' TO ZCVS-TEST-NAME.
           MOVE 'X' TO OUTER-END(1).
           MOVE 'Y' TO OUTER-END(2).
           IF RAW(18:1) = 'X' AND RAW(38:1) = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DOUBLE-TABLE-LENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(DOUBLE-TABLE) = 48
             AND FUNCTION LENGTH(DOUBLE-ROW(1)) = 24
               PERFORM ZCVS-PASS
           ELSE
               MOVE '48/24' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DOUBLE-ROW-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO DOUBLE-NUM(1).
           MOVE 'A' TO DOUBLE-TAG(1).
           MOVE 'Z' TO DOUBLE-END(1).
           IF DOUBLE-RAW(9:8) = X'0000000000000001'
             AND DOUBLE-RAW(1:1) = 'A'
             AND DOUBLE-RAW(17:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DOUBLE-ROW-2' TO ZCVS-TEST-NAME.
           MOVE 2 TO DOUBLE-NUM(2).
           MOVE 'A' TO DOUBLE-TAG(2).
           MOVE 'Z' TO DOUBLE-END(2).
           IF DOUBLE-RAW(33:8) = X'0000000000000002'
             AND DOUBLE-RAW(25:1) = 'A'
             AND DOUBLE-RAW(41:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PACKED-NO-SLACK' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(PACKED-TABLE) = 10
             AND FUNCTION LENGTH(PACKED-ROW(1)) = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10/5' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * AUDIT: D07; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * SPEC: SYNCHRONIZED CLAUSE; AUDIT D07.
      * ZD114A: SYNC table stride and HFP field alignment
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD114A.
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
       01  SYNC-TABLE.
           05  ROW-ITEM OCCURS 3 TIMES.
               10  TAG PIC X.
               10  HALFWORD PIC S9(4) COMP SYNC.
               10  END-TAG PIC X.
       01  RAW REDEFINES SYNC-TABLE PIC X(18).
       01  FLOAT-REC.
           05  LEAD-BYTE PIC X.
           05  SHORT-FLOAT COMP-1 SYNC.
           05  LONG-FLOAT COMP-2 SYNC.
       01  FLOAT-RAW REDEFINES FLOAT-REC PIC X(16).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD114A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'SYNC-TABLE-LENGTH' TO ZCVS-TEST-NAME.
           IF LENGTH OF SYNC-TABLE = 18
               PERFORM ZCVS-PASS
           ELSE
               MOVE '18' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SYNC-ELEMENT-LENGTH' TO ZCVS-TEST-NAME.
           IF LENGTH OF ROW-ITEM(1) = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SYNC-ROW-1' TO ZCVS-TEST-NAME.
           MOVE 'A' TO TAG(1).
           MOVE 'Z' TO END-TAG(1).
           MOVE 273 TO HALFWORD(1).
           IF RAW(3:2) = X'0111'
             AND RAW(1:1) = 'A'
             AND RAW(5:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0111' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SYNC-ROW-2' TO ZCVS-TEST-NAME.
           MOVE 'A' TO TAG(2).
           MOVE 'Z' TO END-TAG(2).
           MOVE 546 TO HALFWORD(2).
           IF RAW(9:2) = X'0222'
             AND RAW(7:1) = 'A'
             AND RAW(11:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0222' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SYNC-ROW-3' TO ZCVS-TEST-NAME.
           MOVE 'A' TO TAG(3).
           MOVE 'Z' TO END-TAG(3).
           MOVE 819 TO HALFWORD(3).
           IF RAW(15:2) = X'0333'
             AND RAW(13:1) = 'A'
             AND RAW(17:1) = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0333' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SYNC-FLOAT-LENGTH' TO ZCVS-TEST-NAME.
           IF LENGTH OF FLOAT-REC = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '16' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SYNC-FLOAT-OFFSETS' TO ZCVS-TEST-NAME.
           MOVE 1 TO SHORT-FLOAT LONG-FLOAT.
           IF FLOAT-RAW(5:4) = X'41100000'
             AND FLOAT-RAW(9:8) = X'4110000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HFP ONE ALIGNED' TO ZCVS-EXPECTED
               MOVE FLOAT-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

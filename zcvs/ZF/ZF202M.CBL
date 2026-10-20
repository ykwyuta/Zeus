      * AUDIT: F09/F10/F11; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF202M: Forced tape reels partial blocks and empty volume traver
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF202M.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT TFILE ASSIGN TO TAPE202 FILE STATUS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  TFILE RECORDING MODE F
           RECORD CONTAINS 8 CHARACTERS
           BLOCK CONTAINS 16 CHARACTERS
           LABEL RECORDS STANDARD.
       01  TREC PIC X(8).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS PIC XX.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF202M' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'REEL-OPEN-OUT' TO ZCVS-TEST-NAME.
           OPEN OUTPUT TFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-WRITE-FIRST' TO ZCVS-TEST-NAME.
           MOVE 'FIRST001' TO TREC.
           WRITE TREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-CLOSE-REEL' TO ZCVS-TEST-NAME.
           CLOSE TFILE REEL WITH NO REWIND.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-WRITE-SECOND' TO ZCVS-TEST-NAME.
           MOVE 'SECOND02' TO TREC.
           WRITE TREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-CLOSE-UNIT' TO ZCVS-TEST-NAME.
           CLOSE TFILE UNIT FOR REMOVAL.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-EMPTY-REEL' TO ZCVS-TEST-NAME.
           CLOSE TFILE REEL.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-WRITE-FOURTH' TO ZCVS-TEST-NAME.
           MOVE 'FOURTH04' TO TREC.
           WRITE TREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-CLOSE-OUT' TO ZCVS-TEST-NAME.
           CLOSE TFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-OPEN-INPUT' TO ZCVS-TEST-NAME.
           OPEN INPUT TFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-AUTO-READ-0' TO ZCVS-TEST-NAME.
           READ TFILE.
           IF FS = '00' AND TREC = 'FIRST001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FIRST001' TO ZCVS-EXPECTED
               MOVE TREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-AUTO-READ-1' TO ZCVS-TEST-NAME.
           READ TFILE.
           IF FS = '00' AND TREC = 'SECOND02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SECOND02' TO ZCVS-EXPECTED
               MOVE TREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-AUTO-READ-2' TO ZCVS-TEST-NAME.
           READ TFILE.
           IF FS = '00' AND TREC = 'FOURTH04'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FOURTH04' TO ZCVS-EXPECTED
               MOVE TREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-AUTO-EOF' TO ZCVS-TEST-NAME.
           READ TFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-REOPEN' TO ZCVS-TEST-NAME.
           CLOSE TFILE.
           OPEN INPUT TFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-EXPLICIT-FIRST' TO ZCVS-TEST-NAME.
           READ TFILE.
           IF FS = '00' AND TREC = 'FIRST001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FIRST001' TO ZCVS-EXPECTED
               MOVE TREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-INPUT-SWITCH' TO ZCVS-TEST-NAME.
           CLOSE TFILE REEL.
           READ TFILE.
           IF FS = '00' AND TREC = 'SECOND02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SECOND02' TO ZCVS-EXPECTED
               MOVE TREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-INPUT-EMPTY' TO ZCVS-TEST-NAME.
           CLOSE TFILE UNIT FOR REMOVAL.
           READ TFILE.
           IF FS = '00' AND TREC = 'FOURTH04'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FOURTH04' TO ZCVS-EXPECTED
               MOVE TREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REEL-LAST-UNIT' TO ZCVS-TEST-NAME.
           CLOSE TFILE UNIT.
           READ TFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE TFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

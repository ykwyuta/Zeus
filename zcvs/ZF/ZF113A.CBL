      * AUDIT: F01; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF113A: QSAM illegal modes EOF sequencing and recovery
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF113A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT SFILE ASSIGN TO XXXXX001 FILE STATUS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  SFILE RECORDING MODE F.
       01  SREC PIC X(8).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS PIC XX.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF113A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'SEQ-CLOSED-READ' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '47' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '47' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-CLOSED-WRITE' TO ZCVS-TEST-NAME.
           WRITE SREC.
           IF FS = '48' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '48' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-CLOSED-CLOSE' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           IF FS = '42' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '42' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-OPEN-OUTPUT' TO ZCVS-TEST-NAME.
           OPEN OUTPUT SFILE.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-DOUBLE-OPEN' TO ZCVS-TEST-NAME.
           OPEN OUTPUT SFILE.
           IF FS = '41' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '41' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-WRITE' TO ZCVS-TEST-NAME.
           MOVE 'ORIGINAL' TO SREC.
           WRITE SREC.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-CLOSE-OUTPUT' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-OPEN-INPUT' TO ZCVS-TEST-NAME.
           OPEN INPUT SFILE.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-INPUT-WRITE' TO ZCVS-TEST-NAME.
           WRITE SREC.
           IF FS = '48' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '48' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-INPUT-REWRITE' TO ZCVS-TEST-NAME.
           REWRITE SREC.
           IF FS = '49' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '49' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-READ-AFTER-ERROR' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '00' AND SREC = 'ORIGINAL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-FIRST-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-SECOND-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '46' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '46' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-REOPEN-RECOVER' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           READ SFILE.
           IF FS = '00' AND SREC = 'ORIGINAL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-FINAL-CLOSE' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SEQ-CLOSE-TWICE' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           IF FS = '42' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '42' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

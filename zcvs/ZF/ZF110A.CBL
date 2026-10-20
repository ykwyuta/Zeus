      * AUDIT: F05; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF110A: RRDS sparse slots mutation sequencing and error recovery
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF110A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT RFILE ASSIGN TO XXXXX001
             ORGANIZATION RELATIVE ACCESS MODE DYNAMIC
             RELATIVE KEY RRN FILE STATUS FS EXT-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  RFILE.
       01  RREC PIC X(8).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  RRN PIC 9(4) COMP.
       01  FS PIC XX.
       01  EXT-FS.
           05  XRC PIC 9(4) COMP-5.
           05  XCOMP PIC 9(4) COMP-5.
           05  XREASON PIC 9(4) COMP-5.
       01  BRANCH PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF110A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'RR-OPEN-OUTPUT' TO ZCVS-TEST-NAME.
           OPEN OUTPUT RFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-WRITE-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO RRN.
           MOVE 'SLOT0001' TO RREC.
           WRITE RREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-WRITE-3' TO ZCVS-TEST-NAME.
           MOVE 3 TO RRN.
           MOVE 'SLOT0003' TO RREC.
           WRITE RREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-WRITE-5' TO ZCVS-TEST-NAME.
           MOVE 5 TO RRN.
           MOVE 'SLOT0005' TO RREC.
           WRITE RREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-OPEN-IO' TO ZCVS-TEST-NAME.
           CLOSE RFILE.
           OPEN I-O RFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-DUPLICATE' TO ZCVS-TEST-NAME.
           MOVE 3 TO RRN.
           WRITE RREC.
           IF FS = '22' AND XRC = 8 AND XREASON = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-HOLE' TO ZCVS-TEST-NAME.
           MOVE 2 TO RRN.
           READ RFILE.
           IF FS = '23' AND XRC = 8 AND XREASON = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-RECOVER' TO ZCVS-TEST-NAME.
           MOVE 3 TO RRN.
           READ RFILE.
           IF FS = '00' AND RREC = 'SLOT0003' AND XRC = 0
             AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-REWRITE' TO ZCVS-TEST-NAME.
           MOVE 'CHANGED3' TO RREC.
           REWRITE RREC.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-READ-CHANGED' TO ZCVS-TEST-NAME.
           READ RFILE.
           IF FS = '00' AND RREC = 'CHANGED3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-DELETE' TO ZCVS-TEST-NAME.
           DELETE RFILE RECORD.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-DELETED-MISSING' TO ZCVS-TEST-NAME.
           READ RFILE.
           IF FS = '23' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-REINSERT' TO ZCVS-TEST-NAME.
           MOVE 'NEW-SLOT' TO RREC.
           WRITE RREC.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-START-HOLE' TO ZCVS-TEST-NAME.
           MOVE 2 TO RRN.
           START RFILE KEY >= RRN.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-NEXT-SKIPS-HOLE' TO ZCVS-TEST-NAME.
           READ RFILE NEXT RECORD.
           IF FS = '00' AND RRN = 3 AND RREC = 'NEW-SLOT'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-NEXT-LAST' TO ZCVS-TEST-NAME.
           READ RFILE NEXT RECORD.
           IF FS = '00' AND RRN = 5 AND RREC = 'SLOT0005'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-EOF' TO ZCVS-TEST-NAME.
           READ RFILE NEXT RECORD.
           IF FS = '10' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-EOF-RECOVER' TO ZCVS-TEST-NAME.
           MOVE 1 TO RRN.
           READ RFILE.
           IF FS = '00' AND RREC = 'SLOT0001' AND XRC = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-DELETE-HOLE' TO ZCVS-TEST-NAME.
           MOVE 4 TO RRN.
           DELETE RFILE RECORD.
           IF FS = '23' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-INSERT-HOLE' TO ZCVS-TEST-NAME.
           MOVE 'SLOT0004' TO RREC.
           WRITE RREC.
           IF FS = '00' 
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RR-READ-HOLE-FILLED' TO ZCVS-TEST-NAME.
           READ RFILE.
           IF FS = '00' AND RREC = 'SLOT0004'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE RFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

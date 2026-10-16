      *================================================================*
      * ZF101A - QSAM RECORDING MODE F AND V, VARYING DEPENDING ON     *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF101A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT FIX-FILE ASSIGN TO XXXXX001
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT VAR-FILE ASSIGN TO XXXXX002
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  FIX-FILE
           RECORDING MODE F
           BLOCK CONTAINS 2 RECORDS
           RECORD CONTAINS 10 CHARACTERS.
       01  FIX-REC                  PIC X(10).
       FD  VAR-FILE
           RECORDING MODE V
           RECORD IS VARYING IN SIZE FROM 4 TO 20 CHARACTERS
           DEPENDING ON WS-VAR-LEN.
       01  VAR-REC                  PIC X(20).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-FILE-WORK.
           05  WS-FS                PIC X(2).
           05  WS-VAR-LEN           PIC 9(4) COMP VALUE 0.
           05  WS-EOF               PIC X(1) VALUE 'N'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: RECORDING MODE F BLOCKED WRITE AND READ
           MOVE 'QSAM-MODE-F' TO ZCVS-TEST-NAME.
           OPEN OUTPUT FIX-FILE.
           MOVE 'FIXED-0001' TO FIX-REC.
           WRITE FIX-REC.
           MOVE 'FIXED-0002' TO FIX-REC.
           WRITE FIX-REC.
           CLOSE FIX-FILE.
           OPEN INPUT FIX-FILE.
           READ FIX-FILE.
           IF WS-FS = '00' AND FIX-REC = 'FIXED-0001'
               READ FIX-FILE
               IF WS-FS = '00' AND FIX-REC = 'FIXED-0002'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '00/FIXED-0002' TO ZCVS-EXPECTED
                   MOVE FIX-REC TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '00/FIXED-0001' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE FIX-FILE.

      * TEST 02: RECORDING MODE V WITH VARYING DEPENDING ON LENGTHS
           MOVE 'QSAM-MODE-V-LEN' TO ZCVS-TEST-NAME.
           OPEN OUTPUT VAR-FILE.
           MOVE 6 TO WS-VAR-LEN.
           MOVE 'SHORT6..............' TO VAR-REC.
           WRITE VAR-REC.
           MOVE 14 TO WS-VAR-LEN.
           MOVE 'LONGER-REC-014......' TO VAR-REC.
           WRITE VAR-REC.
           CLOSE VAR-FILE.

           OPEN INPUT VAR-FILE.
           MOVE 0 TO WS-VAR-LEN.
           READ VAR-FILE.
           IF WS-FS = '00'
             AND WS-VAR-LEN = 6
             AND VAR-REC(1:6) = 'SHORT6'
               READ VAR-FILE
               IF WS-FS = '00'
                 AND WS-VAR-LEN = 14
                 AND VAR-REC(1:14) = 'LONGER-REC-014'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '14/LONGER-REC-014' TO ZCVS-EXPECTED
                   MOVE VAR-REC(1:14) TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '6/SHORT6' TO ZCVS-EXPECTED
               MOVE VAR-REC(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: AT END SETS FILE STATUS '10'
           MOVE 'QSAM-MODE-V-EOF' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-EOF.
           READ VAR-FILE
               AT END
                   MOVE 'Y' TO WS-EOF
           END-READ.
           IF WS-EOF = 'Y' AND WS-FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/10' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE VAR-FILE.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

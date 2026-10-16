      *================================================================*
      * ZF201M - PHYSICAL TAPE (AWSTAPE), NO REWIND, REVERSED, MODE U  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF201M.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT TAPE-F1 ASSIGN TO TAPE1
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT TAPE-U ASSIGN TO TAPE2
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
       I-O-CONTROL.
           MULTIPLE FILE TAPE CONTAINS TAPE-F1 POSITION 1.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  TAPE-F1
           RECORDING MODE F
           BLOCK CONTAINS 2 RECORDS
           RECORD CONTAINS 8 CHARACTERS
           LABEL RECORDS ARE STANDARD.
       01  TAPE-F1-REC              PIC X(8).
       FD  TAPE-U
           RECORDING MODE U
           RECORD IS VARYING IN SIZE FROM 4 TO 12 CHARACTERS
           DEPENDING ON WS-ULEN
           LABEL RECORDS ARE STANDARD.
       01  TAPE-U-REC               PIC X(12).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-TAPE-WORK.
           05  WS-FS                PIC X(2).
           05  WS-ULEN              PIC 9(4) COMP VALUE 0.
           05  WS-R1                PIC X(8).
           05  WS-R2                PIC X(8).
           05  WS-R3                PIC X(8).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF201M' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: WRITE STANDARD-LABEL TAPE AND READ BACK IN REVERSE
           MOVE 'TAPE-OPEN-REVERSED' TO ZCVS-TEST-NAME.
           OPEN OUTPUT TAPE-F1.
           MOVE 'T-REC-01' TO TAPE-F1-REC.
           WRITE TAPE-F1-REC.
           MOVE 'T-REC-02' TO TAPE-F1-REC.
           WRITE TAPE-F1-REC.
           MOVE 'T-REC-03' TO TAPE-F1-REC.
           WRITE TAPE-F1-REC.
           CLOSE TAPE-F1 WITH NO REWIND.

           OPEN INPUT TAPE-F1 REVERSED.
           READ TAPE-F1 INTO WS-R1.
           READ TAPE-F1 INTO WS-R2.
           READ TAPE-F1 INTO WS-R3.
           CLOSE TAPE-F1.
           IF WS-R1 = 'T-REC-03'
             AND WS-R2 = 'T-REC-02'
             AND WS-R3 = 'T-REC-01'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'T-REC-03/02/01' TO ZCVS-EXPECTED
               MOVE WS-R1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: RECORDING MODE U ON TAPE WRITES AND READS BLOCKS
           MOVE 'TAPE-MODE-U-RW' TO ZCVS-TEST-NAME.
           OPEN OUTPUT TAPE-U.
           MOVE 6 TO WS-ULEN.
           MOVE 'UBLK06      ' TO TAPE-U-REC.
           WRITE TAPE-U-REC.
           MOVE 12 TO WS-ULEN.
           MOVE 'UBLK12-LONGR' TO TAPE-U-REC.
           WRITE TAPE-U-REC.
           CLOSE TAPE-U.

           OPEN INPUT TAPE-U.
           MOVE 0 TO WS-ULEN.
           READ TAPE-U.
           IF WS-FS = '00'
             AND WS-ULEN = 6
             AND TAPE-U-REC(1:6) = 'UBLK06'
               READ TAPE-U
               IF WS-FS = '00'
                 AND WS-ULEN = 12
                 AND TAPE-U-REC(1:12) = 'UBLK12-LONGR'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '12/UBLK12-LONGR' TO ZCVS-EXPECTED
                   MOVE TAPE-U-REC TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '6/UBLK06' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE TAPE-U.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

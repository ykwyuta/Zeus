      *================================================================*
      * ZF102A - QSAM RECORDING MODE S (SPANNED) AND MODE U STATUS 39  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF102A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT SPAN-FILE ASSIGN TO XXXXX001
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT BADU-FILE ASSIGN TO XXXXX002
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  SPAN-FILE
           RECORDING MODE S
           BLOCK CONTAINS 16 CHARACTERS
           RECORD IS VARYING IN SIZE FROM 4 TO 24 CHARACTERS
           DEPENDING ON WS-SPAN-LEN.
       01  SPAN-REC                 PIC X(24).
       FD  BADU-FILE
           RECORDING MODE U
           RECORD CONTAINS 10 CHARACTERS.
       01  BADU-REC                 PIC X(10).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-WORK.
           05  WS-FS                PIC X(2).
           05  WS-SPAN-LEN          PIC 9(4) COMP VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: RECORDING MODE S SPANS RECORD ACROSS 16-BYTE BLOCKS
           MOVE 'QSAM-MODE-S-SPAN' TO ZCVS-TEST-NAME.
           OPEN OUTPUT SPAN-FILE.
           MOVE 20 TO WS-SPAN-LEN.
           MOVE 'SPANNED-RECORD-12345....' TO SPAN-REC.
           WRITE SPAN-REC.
           MOVE 8 TO WS-SPAN-LEN.
           MOVE 'TAIL-008................' TO SPAN-REC.
           WRITE SPAN-REC.
           CLOSE SPAN-FILE.

           OPEN INPUT SPAN-FILE.
           MOVE 0 TO WS-SPAN-LEN.
           MOVE SPACES TO SPAN-REC.
           READ SPAN-FILE.
           IF WS-FS = '00'
             AND WS-SPAN-LEN = 20
             AND SPAN-REC(1:20) = 'SPANNED-RECORD-12345'
               READ SPAN-FILE
               IF WS-FS = '00'
                 AND WS-SPAN-LEN = 8
                 AND SPAN-REC(1:8) = 'TAIL-008'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '8/TAIL-008' TO ZCVS-EXPECTED
                   MOVE SPAN-REC(1:8) TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '20/SPANNED-RECORD' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE SPAN-FILE.

      * TEST 02: RECORDING MODE U ON NON-TAPE DISK RETURNS FS 39
           MOVE 'QSAM-MODE-U-FS39' TO ZCVS-TEST-NAME.
           OPEN OUTPUT BADU-FILE.
           IF WS-FS = '39'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '39' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

      *================================================================*
      * ZF105A - I-O-CONTROL SAME RECORD AREA AND APPLY WRITE-ONLY     *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF105A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT FILE-A ASSIGN TO XXXXX001
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT FILE-B ASSIGN TO XXXXX002
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
       I-O-CONTROL.
           SAME RECORD AREA FOR FILE-A FILE-B.
           APPLY WRITE-ONLY ON FILE-B.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  FILE-A
           RECORDING MODE F
           RECORD CONTAINS 8 CHARACTERS.
       01  REC-A                    PIC X(8).
       FD  FILE-B
           RECORDING MODE V
           BLOCK CONTAINS 32 CHARACTERS
           RECORD IS VARYING IN SIZE FROM 4 TO 8 CHARACTERS.
       01  REC-B                    PIC X(8).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-FS                    PIC X(2).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF105A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: SAME RECORD AREA SHARES STORAGE BETWEEN REC-A AND REC-B
           MOVE 'IOCTL-SAME-REC' TO ZCVS-TEST-NAME.
           MOVE 'SHARED01' TO REC-A.
           IF REC-B = 'SHARED01'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SHARED01' TO ZCVS-EXPECTED
               MOVE REC-B TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: READ INTO FILE-A POPULATES REC-B FOR WRITE
           MOVE 'IOCTL-COPY-THRU' TO ZCVS-TEST-NAME.
           OPEN OUTPUT FILE-A.
           MOVE 'PIPE-REC' TO REC-A.
           WRITE REC-A.
           CLOSE FILE-A.

           OPEN INPUT FILE-A.
           OPEN OUTPUT FILE-B.
           READ FILE-A.
           WRITE REC-B.
           CLOSE FILE-A.
           CLOSE FILE-B.

           MOVE SPACES TO REC-A.
           OPEN INPUT FILE-B.
           READ FILE-B.
           CLOSE FILE-B.
           IF WS-FS = '00' AND REC-B = 'PIPE-REC'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'PIPE-REC' TO ZCVS-EXPECTED
               MOVE REC-B TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

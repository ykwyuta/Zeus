      *================================================================*
      * ZF104A - VSAM KSDS & RRDS EXTENDED FILE STATUS (2-PARAMETER)   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF104A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT KSDS-FILE ASSIGN TO XXXXX001
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS KS-KEY
               FILE STATUS IS WS-FS WS-VSAM-EXT.
           SELECT RRDS-FILE ASSIGN TO XXXXX002
               ORGANIZATION IS RELATIVE
               ACCESS MODE IS DYNAMIC
               RELATIVE KEY IS WS-RRN
               FILE STATUS IS WS-FS WS-VSAM-EXT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  KSDS-FILE.
       01  KSDS-REC.
           05  KS-KEY               PIC X(4).
           05  KS-DATA              PIC X(8).
       FD  RRDS-FILE.
       01  RRDS-REC                 PIC X(8).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-VSAM-WORK.
           05  WS-FS                PIC X(2).
           05  WS-VSAM-EXT.
               10  WS-VSAM-RC       PIC 9(4) COMP-5.
               10  WS-VSAM-COMP     PIC 9(4) COMP-5.
               10  WS-VSAM-FDBK     PIC 9(4) COMP-5.
           05  WS-RRN               PIC 9(4) COMP VALUE 1.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF104A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: KSDS WRITE AND DUPLICATE KEY (FS='22', RC=8, FDBK=8)
           MOVE 'VSAM-KSDS-DUP-22' TO ZCVS-TEST-NAME.
           OPEN OUTPUT KSDS-FILE.
           CLOSE KSDS-FILE.
           OPEN I-O KSDS-FILE.
           MOVE 'K001' TO KS-KEY.
           MOVE 'FIRST   ' TO KS-DATA.
           WRITE KSDS-REC.
           WRITE KSDS-REC.
           IF WS-FS = '22'
             AND WS-VSAM-RC = 8
             AND WS-VSAM-COMP = 0
             AND WS-VSAM-FDBK = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22/8/0/8' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: KSDS RECORD NOT FOUND (FS='23', RC=8, FDBK=16)
           MOVE 'VSAM-KSDS-NF-23' TO ZCVS-TEST-NAME.
           MOVE 'K999' TO KS-KEY.
           READ KSDS-FILE KEY IS KS-KEY.
           IF WS-FS = '23'
             AND WS-VSAM-RC = 8
             AND WS-VSAM-COMP = 0
             AND WS-VSAM-FDBK = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/8/0/16' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: KSDS READ ON CLOSED FILE CLEARS EXT STATUS (FS='47')
           MOVE 'VSAM-KSDS-CL-47' TO ZCVS-TEST-NAME.
           CLOSE KSDS-FILE.
           READ KSDS-FILE.
           IF WS-FS = '47'
             AND WS-VSAM-RC = 0
             AND WS-VSAM-COMP = 0
             AND WS-VSAM-FDBK = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '47/0/0/0' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: RRDS DUPLICATE RELATIVE RECORD (FS='22', RC=8, FDBK=8)
           MOVE 'VSAM-RRDS-DUP-22' TO ZCVS-TEST-NAME.
           OPEN OUTPUT RRDS-FILE.
           CLOSE RRDS-FILE.
           OPEN I-O RRDS-FILE.
           MOVE 1 TO WS-RRN.
           MOVE 'SLOT-001' TO RRDS-REC.
           WRITE RRDS-REC.
           WRITE RRDS-REC.
           IF WS-FS = '22'
             AND WS-VSAM-RC = 8
             AND WS-VSAM-COMP = 0
             AND WS-VSAM-FDBK = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22/8/0/8' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE RRDS-FILE.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * ZF107A: KSDS unique alternate key lifecycle and recovery
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF107A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT KFILE ASSIGN TO XXXXX001
             ORGANIZATION INDEXED ACCESS MODE DYNAMIC
             RECORD KEY PK
             ALTERNATE RECORD KEY AK
             FILE STATUS FS EXT-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  KFILE.
       01  KREC.
           05  PK PIC XX.
           05  AK PIC XX.
           05  PAYLOAD PIC X(4).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS PIC XX.
       01  EXT-FS.
           05  XRC PIC 9(4) COMP-5.
           05  XCOMP PIC 9(4) COMP-5.
           05  XREASON PIC 9(4) COMP-5.
       01  BRANCH PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF107A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'KS-OPEN-OUT' TO ZCVS-TEST-NAME.
           OPEN OUTPUT KFILE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-CREATE-CLOSE' TO ZCVS-TEST-NAME.
           CLOSE KFILE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-OPEN-IO' TO ZCVS-TEST-NAME.
           OPEN I-O KFILE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-INSERT-10' TO ZCVS-TEST-NAME.
           MOVE '10BBONE1' TO KREC.
           WRITE KREC.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-INSERT-20' TO ZCVS-TEST-NAME.
           MOVE '20AATWO2' TO KREC.
           WRITE KREC.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-INSERT-30' TO ZCVS-TEST-NAME.
           MOVE '30CCTHR3' TO KREC.
           WRITE KREC.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-DUP-PRIMARY' TO ZCVS-TEST-NAME.
           MOVE '10ZZBAD1' TO KREC.
           MOVE 'N' TO BRANCH.
           WRITE KREC
             INVALID KEY MOVE 'Y' TO BRANCH
             NOT INVALID KEY MOVE 'X' TO BRANCH
           END-WRITE.
           IF FS = '22' AND XRC = 8
             AND XCOMP = 0 AND XREASON = 8
             AND BRANCH = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22/8/0/8' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-AFTER-DUP' TO ZCVS-TEST-NAME.
           MOVE '10' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND AK = 'BB' AND PAYLOAD = 'ONE1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-DUP-UNIQUE' TO ZCVS-TEST-NAME.
           MOVE '40AABAD2' TO KREC.
           WRITE KREC.
           IF FS = '22' AND XRC = 8
             AND XCOMP = 2 AND XREASON = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22/8/2/8' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-AFTER-DUP' TO ZCVS-TEST-NAME.
           MOVE '20' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND AK = 'AA' AND PAYLOAD = 'TWO2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-NOT-FOUND' TO ZCVS-TEST-NAME.
           MOVE '99' TO PK.
           READ KFILE KEY PK.
           IF FS = '23' AND XRC = 8
             AND XCOMP = 0 AND XREASON = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/8/0/16' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-READ-EXACT' TO ZCVS-TEST-NAME.
           MOVE 'CC' TO AK.
           READ KFILE KEY AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '30' AND PAYLOAD = 'THR3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-START-EXACT' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO AK.
           START KFILE KEY = AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-NEXT-FIRST' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '20' AND AK = 'AA'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-NEXT-SECOND' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '10' AND AK = 'BB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-NEXT-THIRD' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '30' AND AK = 'CC'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-END' TO ZCVS-TEST-NAME.
           MOVE 'N' TO BRANCH.
           READ KFILE NEXT RECORD
             AT END MOVE 'Y' TO BRANCH
           END-READ.
           IF FS = '10' AND XRC = 8
             AND XCOMP = 2 AND XREASON = 4
             AND BRANCH = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10/8/2/4' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-RECOVER-AFTER-EOF' TO ZCVS-TEST-NAME.
           MOVE 'BB' TO AK.
           READ KFILE KEY AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '10' AND PAYLOAD = 'ONE1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-REWRITE-MOVE' TO ZCVS-TEST-NAME.
           MOVE 'DD' TO AK.
           MOVE 'NEW1' TO PAYLOAD.
           REWRITE KREC.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-OLD-REMOVED' TO ZCVS-TEST-NAME.
           MOVE 'BB' TO AK.
           READ KFILE KEY AK.
           IF FS = '23' AND XRC = 8
             AND XCOMP = 2 AND XREASON = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/8/2/16' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-NEW-FOUND' TO ZCVS-TEST-NAME.
           MOVE 'DD' TO AK.
           READ KFILE KEY AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '10' AND PAYLOAD = 'NEW1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-REWRITE-CONFLICT' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO AK.
           REWRITE KREC.
           IF FS = '22' AND XRC = 8
             AND XCOMP = 2 AND XREASON = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22/8/2/8' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-REWRITE-ATOMIC' TO ZCVS-TEST-NAME.
           MOVE 'DD' TO AK.
           READ KFILE KEY AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '10' AND PAYLOAD = 'NEW1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-DELETE' TO ZCVS-TEST-NAME.
           DELETE KFILE RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-DELETE-REMOVED' TO ZCVS-TEST-NAME.
           MOVE 'DD' TO AK.
           READ KFILE KEY AK.
           IF FS = '23' AND XRC = 8
             AND XCOMP = 2 AND XREASON = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/8/2/16' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-DELETE-REMOVED' TO ZCVS-TEST-NAME.
           MOVE '10' TO PK.
           READ KFILE KEY PK.
           IF FS = '23' AND XRC = 8
             AND XCOMP = 0 AND XREASON = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/8/0/16' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-DELETE-RECOVER' TO ZCVS-TEST-NAME.
           MOVE '20' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND AK = 'AA' AND PAYLOAD = 'TWO2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'KS-CLOSE-IO' TO ZCVS-TEST-NAME.
           CLOSE KFILE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AK-REOPEN-PERSIST' TO ZCVS-TEST-NAME.
           OPEN INPUT KFILE.
           MOVE 'CC' TO AK.
           READ KFILE KEY AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '30' AND PAYLOAD = 'THR3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE KFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

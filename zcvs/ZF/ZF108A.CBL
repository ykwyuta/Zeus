      * ZF108A: Duplicate alternate keys status and insertion order
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF108A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT KFILE ASSIGN TO XXXXX001
             ORGANIZATION INDEXED ACCESS MODE DYNAMIC
             RECORD KEY PK
             ALTERNATE RECORD KEY AK WITH DUPLICATES
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
           MOVE 'ZF108A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'DUP-OPEN' TO ZCVS-TEST-NAME.
           OPEN OUTPUT KFILE.
           CLOSE KFILE.
           OPEN I-O KFILE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-INSERT-30' TO ZCVS-TEST-NAME.
           MOVE '30AAONE1' TO KREC.
           MOVE 'N' TO BRANCH.
           WRITE KREC
             INVALID KEY MOVE 'I' TO BRANCH
             NOT INVALID KEY MOVE 'S' TO BRANCH
           END-WRITE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND BRANCH = 'S'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-INSERT-10' TO ZCVS-TEST-NAME.
           MOVE '10AATWO2' TO KREC.
           MOVE 'N' TO BRANCH.
           WRITE KREC
             INVALID KEY MOVE 'I' TO BRANCH
             NOT INVALID KEY MOVE 'S' TO BRANCH
           END-WRITE.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND BRANCH = 'S'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-INSERT-20' TO ZCVS-TEST-NAME.
           MOVE '20BBTHR3' TO KREC.
           MOVE 'N' TO BRANCH.
           WRITE KREC
             INVALID KEY MOVE 'I' TO BRANCH
             NOT INVALID KEY MOVE 'S' TO BRANCH
           END-WRITE.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND BRANCH = 'S'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-PRIMARY-REJECT' TO ZCVS-TEST-NAME.
           MOVE '30ZZBAD1' TO KREC.
           WRITE KREC.
           IF FS = '22' AND XRC = 8
             AND XCOMP = 0 AND XREASON = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22/8/0/8' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-RANDOM-FIRST' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO AK.
           READ KFILE KEY AK.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '30' AND PAYLOAD = 'ONE1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-START' TO ZCVS-TEST-NAME.
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

           MOVE 'DUP-NEXT-FIRST' TO ZCVS-TEST-NAME.
           MOVE 'N' TO BRANCH.
           READ KFILE NEXT RECORD
             AT END MOVE 'E' TO BRANCH
             NOT AT END MOVE 'S' TO BRANCH
           END-READ.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '30' AND PAYLOAD = 'ONE1' AND BRANCH = 'S'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-NEXT-SECOND' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '10' AND PAYLOAD = 'TWO2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-NEXT-THIRD' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '20' AND AK = 'BB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-READ-PRIMARY' TO ZCVS-TEST-NAME.
           MOVE '30' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PAYLOAD = 'ONE1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-REWRITE-SAME' TO ZCVS-TEST-NAME.
           MOVE 'UPD1' TO PAYLOAD.
           REWRITE KREC.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-ORDER-UNCHANGED' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO AK.
           READ KFILE KEY AK.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '30' AND PAYLOAD = 'UPD1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-DELETE-FIRST' TO ZCVS-TEST-NAME.
           DELETE KFILE RECORD.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-REMAINING-FIRST' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO AK.
           READ KFILE KEY AK.
           IF FS = '00' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '10' AND PAYLOAD = 'TWO2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-REWRITE-INTO' TO ZCVS-TEST-NAME.
           MOVE 'BB' TO AK.
           MOVE 'N' TO BRANCH.
           REWRITE KREC
             INVALID KEY MOVE 'I' TO BRANCH
             NOT INVALID KEY MOVE 'S' TO BRANCH
           END-REWRITE.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND BRANCH = 'S'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-OLD-KEY-GONE' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO AK.
           READ KFILE KEY AK.
           IF FS = '23' AND XRC = 8
             AND XCOMP = 2 AND XREASON = 16
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/8/2/16' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-NEW-ORDER' TO ZCVS-TEST-NAME.
           MOVE 'BB' TO AK.
           READ KFILE KEY AK.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '20' AND PAYLOAD = 'THR3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DUP-REOPEN-ORDER' TO ZCVS-TEST-NAME.
           CLOSE KFILE.
           OPEN INPUT KFILE.
           MOVE 'BB' TO AK.
           READ KFILE KEY AK.
           IF FS = '02' AND XRC = 0
             AND XCOMP = 0 AND XREASON = 0
             AND PK = '20' AND PAYLOAD = 'THR3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/0/0/0' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE KFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * AUDIT: F06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF112A: Two alternate indexes atomic rejection update and delete
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF112A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT KFILE ASSIGN TO XXXXX001
             ORGANIZATION INDEXED ACCESS MODE DYNAMIC
             RECORD KEY PK
             ALTERNATE RECORD KEY DEPT WITH DUPLICATES
             ALTERNATE RECORD KEY EMAIL-KEY
             FILE STATUS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  KFILE.
       01  KREC.
           05  PK PIC XX.
           05  DEPT PIC XX.
           05  EMAIL-KEY PIC XX.
           05  PAYLOAD PIC XX.
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS PIC XX.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF112A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'AIX-OPEN' TO ZCVS-TEST-NAME.
           OPEN OUTPUT KFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-WRITE-03' TO ZCVS-TEST-NAME.
           MOVE '03AAE3C3' TO KREC.
           WRITE KREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-WRITE-01' TO ZCVS-TEST-NAME.
           MOVE '01AAE1A1' TO KREC.
           WRITE KREC.
           IF FS = '02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-WRITE-02' TO ZCVS-TEST-NAME.
           MOVE '02BBE2B2' TO KREC.
           WRITE KREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-REOPEN' TO ZCVS-TEST-NAME.
           CLOSE KFILE.
           OPEN I-O KFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-PRIMARY-01' TO ZCVS-TEST-NAME.
           MOVE '01' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND PK = '01'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-PRIMARY-02' TO ZCVS-TEST-NAME.
           MOVE '02' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND PK = '02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-PRIMARY-03' TO ZCVS-TEST-NAME.
           MOVE '03' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND PK = '03'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-UNIQUE-E1' TO ZCVS-TEST-NAME.
           MOVE 'E1' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '00' AND PK = '01'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '01' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-UNIQUE-E2' TO ZCVS-TEST-NAME.
           MOVE 'E2' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '00' AND PK = '02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-UNIQUE-E3' TO ZCVS-TEST-NAME.
           MOVE 'E3' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '00' AND PK = '03'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '03' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-DUP-READ' TO ZCVS-TEST-NAME.
           MOVE 'AA' TO DEPT.
           READ KFILE KEY DEPT.
           IF FS = '02' AND PK = '03'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/03' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-DUP-LAST' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = '01'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/01' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-REWRITE-CONFLICT' TO ZCVS-TEST-NAME.
           MOVE '02' TO PK.
           READ KFILE KEY PK.
           MOVE 'AA' TO DEPT.
           MOVE 'E1' TO EMAIL-KEY.
           MOVE 'XX' TO PAYLOAD.
           REWRITE KREC.
           IF FS = '22'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '22' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-FAIL-ATOMIC' TO ZCVS-TEST-NAME.
           MOVE '02' TO PK.
           READ KFILE KEY PK.
           IF FS = '00' AND KREC = '02BBE2B2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02BBE2B2' TO ZCVS-EXPECTED
               MOVE KREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-FAIL-OLD-INDEX' TO ZCVS-TEST-NAME.
           MOVE 'E2' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '00' AND PK = '02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-CHANGE-BOTH' TO ZCVS-TEST-NAME.
           MOVE 'CC' TO DEPT.
           MOVE 'E4' TO EMAIL-KEY.
           REWRITE KREC.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-OLD-UNIQUE-GONE' TO ZCVS-TEST-NAME.
           MOVE 'E2' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '23'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-OLD-DEPT-GONE' TO ZCVS-TEST-NAME.
           MOVE 'BB' TO DEPT.
           READ KFILE KEY DEPT.
           IF FS = '23'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-NEW-UNIQUE' TO ZCVS-TEST-NAME.
           MOVE 'E4' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '00' AND KREC = '02CCE4B2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02CCE4B2' TO ZCVS-EXPECTED
               MOVE KREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-NEW-DEPT' TO ZCVS-TEST-NAME.
           MOVE 'CC' TO DEPT.
           START KFILE KEY = DEPT.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = '02'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-DELETE' TO ZCVS-TEST-NAME.
           DELETE KFILE RECORD.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-DEL-PK' TO ZCVS-TEST-NAME.
           MOVE '02' TO PK.
           READ KFILE KEY PK.
           IF FS = '23'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-DEL-DEPT' TO ZCVS-TEST-NAME.
           MOVE 'CC' TO DEPT.
           READ KFILE KEY DEPT.
           IF FS = '23'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-DEL-EMAIL-KEY' TO ZCVS-TEST-NAME.
           MOVE 'E4' TO EMAIL-KEY.
           READ KFILE KEY EMAIL-KEY.
           IF FS = '23'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'AIX-PERSIST-OTHERS' TO ZCVS-TEST-NAME.
           CLOSE KFILE.
           OPEN INPUT KFILE.
           MOVE 'AA' TO DEPT.
           READ KFILE KEY DEPT.
           IF FS = '02' AND PK = '03'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02/03' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE KFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

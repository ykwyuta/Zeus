      * AUDIT: F05/F06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF111A: KSDS full and partial START relation and gap boundaries
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF111A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT KFILE ASSIGN TO XXXXX001
             ORGANIZATION INDEXED ACCESS MODE DYNAMIC
             RECORD KEY PK FILE STATUS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  KFILE.
       01  KREC.
           05  PK.
               10  PREFIX-KEY PIC XX.
               10  SUFFIX-KEY PIC XX.
           05  PAYLOAD PIC XX.
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS PIC XX.
       01  FLAG PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF111A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'START-FIXTURE' TO ZCVS-TEST-NAME.
           OPEN OUTPUT KFILE.
           MOVE 'A100' TO PK.
           MOVE 'OK' TO PAYLOAD.
           WRITE KREC.
           MOVE 'A200' TO PK.
           MOVE 'OK' TO PAYLOAD.
           WRITE KREC.
           MOVE 'B100' TO PK.
           MOVE 'OK' TO PAYLOAD.
           WRITE KREC.
           MOVE 'D100' TO PK.
           MOVE 'OK' TO PAYLOAD.
           WRITE KREC.
           CLOSE KFILE.
           OPEN INPUT KFILE.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-0' TO ZCVS-TEST-NAME.
           MOVE 'A100' TO PK.
           START KFILE KEY = PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-0' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-1' TO ZCVS-TEST-NAME.
           MOVE 'A100' TO PK.
           START KFILE KEY > PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-1' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A200'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A200' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-2' TO ZCVS-TEST-NAME.
           MOVE 'A100' TO PK.
           START KFILE KEY >= PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-2' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-3' TO ZCVS-TEST-NAME.
           MOVE 'A150' TO PK.
           START KFILE KEY >= PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-3' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A200'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A200' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-4' TO ZCVS-TEST-NAME.
           MOVE 'A150' TO PK.
           START KFILE KEY > PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-4' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A200'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A200' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-5' TO ZCVS-TEST-NAME.
           MOVE '    ' TO PK.
           START KFILE KEY >= PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-5' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-6' TO ZCVS-TEST-NAME.
           MOVE 'B100' TO PK.
           START KFILE KEY > PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-6' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'D100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-FULL-7' TO ZCVS-TEST-NAME.
           MOVE 'D100' TO PK.
           START KFILE KEY = PK.
           IF FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-NEXT-7' TO ZCVS-TEST-NAME.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'D100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-PREFIX-0' TO ZCVS-TEST-NAME.
           MOVE 'A1' TO PREFIX-KEY.
           START KFILE KEY = PREFIX-KEY.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-PREFIX-1' TO ZCVS-TEST-NAME.
           MOVE 'A1' TO PREFIX-KEY.
           START KFILE KEY >= PREFIX-KEY.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-PREFIX-2' TO ZCVS-TEST-NAME.
           MOVE 'A1' TO PREFIX-KEY.
           START KFILE KEY > PREFIX-KEY.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A200'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A200' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-PREFIX-3' TO ZCVS-TEST-NAME.
           MOVE 'A3' TO PREFIX-KEY.
           START KFILE KEY >= PREFIX-KEY.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'B100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'B100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-PREFIX-4' TO ZCVS-TEST-NAME.
           MOVE 'D1' TO PREFIX-KEY.
           START KFILE KEY = PREFIX-KEY.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'D100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-INVALID-0' TO ZCVS-TEST-NAME.
           MOVE 'A150' TO PK.
           MOVE 'N' TO FLAG.
           START KFILE KEY = PK
             INVALID KEY MOVE 'Y' TO FLAG END-START.
           IF FS = '23' AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/Y' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-RECOVER-0' TO ZCVS-TEST-NAME.
           MOVE 'A100' TO PK.
           START KFILE KEY = PK.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-INVALID-1' TO ZCVS-TEST-NAME.
           MOVE 'D100' TO PK.
           MOVE 'N' TO FLAG.
           START KFILE KEY > PK
             INVALID KEY MOVE 'Y' TO FLAG END-START.
           IF FS = '23' AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/Y' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-RECOVER-1' TO ZCVS-TEST-NAME.
           MOVE 'A100' TO PK.
           START KFILE KEY = PK.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-INVALID-2' TO ZCVS-TEST-NAME.
           MOVE 'Z999' TO PK.
           MOVE 'N' TO FLAG.
           START KFILE KEY >= PK
             INVALID KEY MOVE 'Y' TO FLAG END-START.
           IF FS = '23' AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23/Y' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'START-RECOVER-2' TO ZCVS-TEST-NAME.
           MOVE 'A100' TO PK.
           START KFILE KEY = PK.
           READ KFILE NEXT RECORD.
           IF FS = '00' AND PK = 'A100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A100' TO ZCVS-EXPECTED
               MOVE PK TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE KFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

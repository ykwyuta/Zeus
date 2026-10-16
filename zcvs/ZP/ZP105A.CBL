      *================================================================*
      * ZP105A - POINTER, PROCEDURE-POINTER, AND FUNCTION-POINTER      *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP105A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-PTR-FIELDS.
           05  WS-DATA-PTR          USAGE POINTER VALUE NULL.
           05  WS-PROC-PTR          USAGE PROCEDURE-POINTER VALUE NULL.
           05  WS-FUNC-PTR          USAGE FUNCTION-POINTER VALUE NULL.
           05  WS-TARGET-NAME       PIC X(8) VALUE 'ZPS05A  '.
           05  WS-BUF-A             PIC X(8) VALUE 'ORIGIN-A'.
           05  WS-BUF-B             PIC X(8) VALUE 'ORIGIN-B'.
           05  WS-RET-VAL           PIC 9(4) COMP VALUE 0.
       LINKAGE SECTION.
       01  LK-VIEW                  PIC X(8).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP105A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: POINTER(4B), PROCEDURE-POINTER(8B), FUNC-POINTER(4B)
           MOVE 'PTR-SIZES-NULL' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-DATA-PTR = 4
             AND LENGTH OF WS-PROC-PTR = 8
             AND LENGTH OF WS-FUNC-PTR = 4
             AND WS-DATA-PTR = NULL
             AND WS-PROC-PTR = NULL
             AND WS-FUNC-PTR = NULL
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4/8/4/NULL' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: SET ADDRESS OF LINKAGE ITEM VIA POINTER
           MOVE 'PTR-ADDR-LINKAGE' TO ZCVS-TEST-NAME.
           SET WS-DATA-PTR TO ADDRESS OF WS-BUF-A.
           SET ADDRESS OF LK-VIEW TO WS-DATA-PTR.
           IF LK-VIEW = 'ORIGIN-A'
               SET ADDRESS OF LK-VIEW TO ADDRESS OF WS-BUF-B
               MOVE 'MOD-BY-P' TO LK-VIEW
               IF WS-BUF-B = 'MOD-BY-P'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE 'MOD-BY-P' TO ZCVS-EXPECTED
                   MOVE WS-BUF-B TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'ORIGIN-A' TO ZCVS-EXPECTED
               MOVE LK-VIEW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: SET PROCEDURE-POINTER TO ENTRY AND INDIRECT CALL
           MOVE 'PROCPTR-CALL' TO ZCVS-TEST-NAME.
           SET WS-PROC-PTR TO ENTRY WS-TARGET-NAME.
           CALL WS-PROC-PTR RETURNING WS-RET-VAL.
           IF WS-PROC-PTR NOT = NULL AND WS-RET-VAL = 10
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: COPY PROCEDURE-POINTER TO FUNCTION-POINTER AND CALL
           MOVE 'FUNCPTR-CALL' TO ZCVS-TEST-NAME.
           SET WS-FUNC-PTR TO WS-PROC-PTR.
           SET WS-PROC-PTR TO NULL.
           CALL WS-FUNC-PTR RETURNING WS-RET-VAL.
           IF WS-PROC-PTR = NULL
             AND WS-FUNC-PTR NOT = NULL
             AND WS-RET-VAL = 20
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NULL/20' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

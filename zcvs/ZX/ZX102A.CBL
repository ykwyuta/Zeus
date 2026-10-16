      *================================================================*
      * ZX102A - SET CONDITION TO FALSE & INITIALIZE TO VALUE/DEFAULT  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX102A.
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
       01  WS-COND-GROUP.
           05  WS-STATUS-CODE       PIC 9(1) VALUE 9.
               88  ST-ACTIVE        VALUE 1 THRU 3
                                    WHEN SET TO FALSE IS 0.
           05  WS-FLAGS             PIC X(1) VALUE 'Y' OCCURS 2 TIMES.
               88  FLG-ON           VALUE 'Y' FALSE 'N'.
       01  WS-INIT-REC.
           05  WS-IR-A              PIC X(1) VALUE 'A'.
           05  WS-IR-B              PIC X(1).
           05  FILLER               PIC X(1) VALUE 'F'.
           05  WS-IR-N              PIC 9(1) VALUE 7.
           05  WS-IR-M              PIC 9(1).
       01  WS-INIT-RAW REDEFINES WS-INIT-REC PIC X(5).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: SET 88 CONDITION TO TRUE AND TO FALSE
           MOVE 'COND-SET-TRUE-FALSE' TO ZCVS-TEST-NAME.
           SET ST-ACTIVE TO TRUE.
           IF WS-STATUS-CODE = 1 AND ST-ACTIVE
               SET ST-ACTIVE TO FALSE
               IF WS-STATUS-CODE = 0 AND NOT ST-ACTIVE
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '0/FALSE' TO ZCVS-EXPECTED
                   MOVE 'STILL-TRUE' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '1/TRUE' TO ZCVS-EXPECTED
               MOVE 'NOT-TRUE' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: SUBSCRIPTED SET CONDITION TO FALSE AND TRUE
           MOVE 'COND-SUBSCRIPT-SET' TO ZCVS-TEST-NAME.
           SET FLG-ON(1) FLG-ON(2) TO FALSE.
           IF WS-FLAGS(1) = 'N' AND WS-FLAGS(2) = 'N'
               SET FLG-ON(2) TO TRUE
               IF WS-FLAGS(1) = 'N' AND WS-FLAGS(2) = 'Y'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE 'NY' TO ZCVS-EXPECTED
                   MOVE 'MISMATCH' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'NN' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: INITIALIZE WITH FILLER ALL TO VALUE AND REPLACING
           MOVE 'INIT-VALUE-PRECED' TO ZCVS-TEST-NAME.
           MOVE 'ZZZ99' TO WS-INIT-RAW.
           INITIALIZE WS-INIT-REC WITH FILLER ALL TO VALUE
               THEN REPLACING ALPHANUMERIC BY 'B'
               THEN TO DEFAULT.
           IF WS-INIT-RAW = 'ABF70'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABF70' TO ZCVS-EXPECTED
               MOVE WS-INIT-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

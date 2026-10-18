      *================================================================*
      * ZP103A - PROCEDURE DIVISION RETURNING AND RETURN-CODE          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP103A.
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
       01  WS-RET-FIELDS.
           05  WS-A                 PIC S9(9) COMP-5 VALUE 12.
           05  WS-B                 PIC S9(9) COMP-5 VALUE 34.
           05  WS-ANS               PIC S9(9) COMP-5 VALUE 0.
           05  WS-STR-ANS           PIC X(8) VALUE SPACES.
           05  WS-EXC-FLAG          PIC X(1) VALUE 'N'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP103A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: CALL RETURNING SETS RESULT AND PRESERVES RETURN-CODE
           MOVE 'RET-NUM-RC-PRES' TO ZCVS-TEST-NAME.
           MOVE 7 TO RETURN-CODE.
           CALL 'ZPS03A' USING WS-A WS-B RETURNING WS-ANS.
           IF WS-ANS = 1234 AND RETURN-CODE = 7
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234/RC=7' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: CALL MISSING WITH RETURNING ON EXCEPTION PRESERVES ANS
           MOVE 'RET-ON-EXCEPTION' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-EXC-FLAG.
           CALL 'NOMODULE' RETURNING WS-ANS
               ON EXCEPTION
                   MOVE 'Y' TO WS-EXC-FLAG
           END-CALL.
           IF WS-EXC-FLAG = 'Y'
             AND WS-ANS = 1234
             AND RETURN-CODE = 7
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/1234/RC=7' TO ZCVS-EXPECTED
               MOVE WS-EXC-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: SUBPROGRAM RETURNING CHARACTER ITEM
           MOVE 'RET-CHAR-ITEM' TO ZCVS-TEST-NAME.
           CALL 'ZPS03B' RETURNING WS-STR-ANS.
           IF WS-STR-ANS = 'RET-OK! '
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'RET-OK! ' TO ZCVS-EXPECTED
               MOVE WS-STR-ANS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 0 TO RETURN-CODE.
           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.
       END PROGRAM ZP103A.

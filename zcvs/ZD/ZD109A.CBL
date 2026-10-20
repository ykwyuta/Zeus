      * SPEC: ALPHANUMERIC COMPARISON; AUDIT D02.
      * ZD109A: Native EBCDIC collation and padding
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD109A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  LO PIC X VALUE 'a'.
       01  HI PIC X VALUE 'A'.
       01  DIG PIC X VALUE '0'.
       01  WORD-A PIC XX VALUE 'A'.
       01  WORD-B PIC XX VALUE 'AA'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD109A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'COLL-LOWER-UPPER' TO ZCVS-TEST-NAME.
           IF LO < HI
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'a < A' TO ZCVS-EXPECTED
               MOVE LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COLL-UPPER-DIGIT' TO ZCVS-TEST-NAME.
           IF HI < DIG
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A < 0' TO ZCVS-EXPECTED
               MOVE HI TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COLL-SPACE' TO ZCVS-TEST-NAME.
           IF SPACE < LO
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPACE < a' TO ZCVS-EXPECTED
               MOVE LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COLL-LOW-HIGH' TO ZCVS-TEST-NAME.
           IF LOW-VALUES < LO AND HIGH-VALUES > DIG
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00 < a < FF' TO ZCVS-EXPECTED
               MOVE DIG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COLL-PAD-GROUP' TO ZCVS-TEST-NAME.
           IF WORD-A < WORD-B
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A SPACE < AA' TO ZCVS-EXPECTED
               MOVE WORD-A TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COLL-RAW-BYTES' TO ZCVS-TEST-NAME.
           IF LO = X'81' AND HI = X'C1' AND DIG = X'F0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '81/C1/F0' TO ZCVS-EXPECTED
               MOVE LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

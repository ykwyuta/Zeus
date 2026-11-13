       CBL NUMCHECK(ZON,PAC,BIN,ABD)
      * AUDIT: A07; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZA130A: NUMCHECK(ZON,PAC,BIN,ABD). Valid zoned, packed and
      * binary senders behave as under NONUMCHECK in ADD, COMPUTE, a
      * comparison and a receiver that is also a sender. After the
      * report is closed, an ADD whose zoned sender holds 'A12' must
      * end the run unit (a terminating message under ABD); if it
      * returns, the step ends with RC 0, which the manifest rejects.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA130A.
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
       01  Z PIC 9(3) VALUE 12.
       01  ZX REDEFINES Z PIC X(3).
       01  P PIC S9(3) COMP-3 VALUE 5.
       01  B PIC S9(4) BINARY VALUE 30.
       01  R PIC S9(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA130A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'NC-VALID-ADD' TO ZCVS-TEST-NAME.
           ADD Z P B GIVING R.
           IF R = 47
               PERFORM ZCVS-PASS
           ELSE
               MOVE '47' TO ZCVS-EXPECTED
               MOVE R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NC-VALID-COMPUTE' TO ZCVS-TEST-NAME.
           COMPUTE R = Z * P - B.
           IF R = 30
               PERFORM ZCVS-PASS
           ELSE
               MOVE '30' TO ZCVS-EXPECTED
               MOVE R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NC-VALID-COMPARE' TO ZCVS-TEST-NAME.
           IF Z > P AND B > Z
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Z > P, B > Z' TO ZCVS-EXPECTED
               MOVE 'NOT' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NC-SEND-RECEIVE' TO ZCVS-TEST-NAME.
           ADD Z TO Z.
           IF Z = 24
               PERFORM ZCVS-PASS
           ELSE
               MOVE '24' TO ZCVS-EXPECTED
               MOVE Z TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
      * THE REPORT IS CLOSED. THE ZONED SENDER IS NOT NUMERIC; UNDER
      * NUMCHECK(...,ABD) THE ADD MUST NOT COMPLETE.
           MOVE 'A12' TO ZX.
           ADD Z TO R.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

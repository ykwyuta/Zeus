      * ZS109A: special registers and compiler defaults that the suite
      * and NIST CCVS85 did not use (Enterprise COBOL Language
      * Reference and Programming Guide): SHIFT-OUT X0E and SHIFT-IN
      * X0F; the initial SORT-RETURN, SORT-CORE-SIZE, SORT-FILE-SIZE
      * and SORT-MODE-SIZE zero, SORT-CONTROL IGZSRTCD and SORT-
      * MESSAGE SYSOUT; under the default ZWB a signed zoned item
      * compared with an alphanumeric literal compares as its unsigned
      * value; FORMATTED-CURRENT-DATE returns a valid value in the
      * given format.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS109A.
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
       01  WS-C PIC X VALUE SPACE.
       01  WS-S PIC S9 VALUE -1.
       01  WS-P PIC S9 VALUE +1.
       01  WS-N PIC S9(9).
       01  WS-NE PIC -9(9).
       01  WS-F PIC X(19).
       01  WS-SG.
           05  WS-SO PIC X.
           05  WS-SI PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZS109A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'SHIFT-OUT-IN' TO ZCVS-TEST-NAME.
           MOVE SHIFT-OUT TO WS-SO.
           MOVE SHIFT-IN TO WS-SI.
           MOVE WS-SG TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '0E0F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0E0F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SORT-RETURN-INITIAL' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = SORT-RETURN.
           MOVE WS-N TO WS-NE.
           IF WS-N = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SORT-CONTROL-INITIAL' TO ZCVS-TEST-NAME.
           IF SORT-CONTROL = 'IGZSRTCD'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'IGZSRTCD' TO ZCVS-EXPECTED
               MOVE SORT-CONTROL TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SORT-CORE-SIZE-INIT' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = SORT-CORE-SIZE.
           MOVE WS-N TO WS-NE.
           IF WS-N = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SORT-FILE-SIZE-INIT' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = SORT-FILE-SIZE.
           MOVE WS-N TO WS-NE.
           IF WS-N = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SORT-MODE-SIZE-INIT' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = SORT-MODE-SIZE.
           MOVE WS-N TO WS-NE.
           IF WS-N = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SORT-MESSAGE-INITIAL' TO ZCVS-TEST-NAME.
           IF SORT-MESSAGE = 'SYSOUT'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SYSOUT' TO ZCVS-EXPECTED
               MOVE SORT-MESSAGE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ZWB-DEFAULT-NEGATIVE' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-C.
           IF WS-S = '1'
               MOVE 'Y' TO WS-C
           END-IF.
           IF WS-C = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ZWB-DEFAULT-POSITIVE' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-C.
           IF WS-P = '1'
               MOVE 'Y' TO WS-C
           END-IF.
           IF WS-C = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FMT-CURRENT-DATE' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-CURRENT-DATE(
               'YYYY-MM-DDThh:mm:ss') TO WS-F.
           COMPUTE WS-N = FUNCTION TEST-FORMATTED-DATETIME(
               'YYYY-MM-DDThh:mm:ss', WS-F).
           MOVE WS-N TO WS-NE.
           IF WS-N = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

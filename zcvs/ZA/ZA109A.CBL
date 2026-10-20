      * ZA109A: Arithmetic decision boundaries and receiver safety
      * SPEC: SIZE ERROR/ROUNDED/DIVIDE; AUDIT A06.
      * ERROR BRANCHES MUST PRESERVE THE ORIGINAL RECEIVER.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA109A.
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
       01  WS-S PIC S9(3) COMP-3.
       01  WS-Q PIC S9(3) COMP-3.
       01  WS-R PIC S9(3) COMP-3.
       01  WS-D PIC S9(3)V99 COMP-3.
       01  WS-F PIC X.
       01  WS-Z PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA109A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ADD-UPPER' TO ZCVS-TEST-NAME.
           MOVE 998 TO WS-S.
           MOVE SPACE TO WS-F.
           ADD 1 TO WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-ADD.
           IF WS-S = 999 AND WS-F = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999/N' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ADD-OVER' TO ZCVS-TEST-NAME.
           MOVE 999 TO WS-S.
           MOVE SPACE TO WS-F.
           ADD 1 TO WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-ADD.
           IF WS-S = 999 AND WS-F = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999/Y' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SUB-LOWER' TO ZCVS-TEST-NAME.
           MOVE -998 TO WS-S.
           MOVE SPACE TO WS-F.
           SUBTRACT 1 FROM WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-SUBTRACT.
           IF WS-S = -999 AND WS-F = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999/N' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SUB-OVER' TO ZCVS-TEST-NAME.
           MOVE -999 TO WS-S.
           MOVE SPACE TO WS-F.
           SUBTRACT 1 FROM WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-SUBTRACT.
           IF WS-S = -999 AND WS-F = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-999/Y' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'MUL-EXACT' TO ZCVS-TEST-NAME.
           MOVE 333 TO WS-S.
           MOVE SPACE TO WS-F.
           MULTIPLY 3 BY WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-MULTIPLY.
           IF WS-S = 999 AND WS-F = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999/N' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'MUL-OVER' TO ZCVS-TEST-NAME.
           MOVE 500 TO WS-S.
           MOVE SPACE TO WS-F.
           MULTIPLY 2 BY WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-MULTIPLY.
           IF WS-S = 500 AND WS-F = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '500/Y' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIV-EXACT' TO ZCVS-TEST-NAME.
           MOVE 999 TO WS-S.
           MOVE SPACE TO WS-F.
           DIVIDE 3 INTO WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-DIVIDE.
           IF WS-S = 333 AND WS-F = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '333/N' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIV-ZERO' TO ZCVS-TEST-NAME.
           MOVE 123 TO WS-S.
           MOVE SPACE TO WS-F.
           DIVIDE WS-Z INTO WS-S
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-DIVIDE.
           IF WS-S = 123 AND WS-F = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '123/Y' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMP-OVER' TO ZCVS-TEST-NAME.
           MOVE 123 TO WS-S.
           MOVE SPACE TO WS-F.
           COMPUTE WS-S = 1000
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-COMPUTE.
           IF WS-S = 123 AND WS-F = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '123/Y' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMP-ZERO' TO ZCVS-TEST-NAME.
           MOVE 123 TO WS-S.
           MOVE SPACE TO WS-F.
           COMPUTE WS-S = 1 / WS-Z
             ON SIZE ERROR MOVE 'Y' TO WS-F
             NOT ON SIZE ERROR MOVE 'N' TO WS-F
           END-COMPUTE.
           IF WS-S = 123 AND WS-F = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '123/Y' TO ZCVS-EXPECTED
               MOVE WS-F TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-0' TO ZCVS-TEST-NAME.
           COMPUTE WS-D ROUNDED = 12.344.
           IF WS-D = 12.34
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.34' TO ZCVS-EXPECTED
               MOVE WS-D TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-1' TO ZCVS-TEST-NAME.
           COMPUTE WS-D ROUNDED = 12.345.
           IF WS-D = 12.35
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.35' TO ZCVS-EXPECTED
               MOVE WS-D TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-2' TO ZCVS-TEST-NAME.
           COMPUTE WS-D ROUNDED = 12.346.
           IF WS-D = 12.35
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.35' TO ZCVS-EXPECTED
               MOVE WS-D TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-3' TO ZCVS-TEST-NAME.
           COMPUTE WS-D ROUNDED = -12.344.
           IF WS-D = -12.34
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.34' TO ZCVS-EXPECTED
               MOVE WS-D TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-4' TO ZCVS-TEST-NAME.
           COMPUTE WS-D ROUNDED = -12.345.
           IF WS-D = -12.35
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.35' TO ZCVS-EXPECTED
               MOVE WS-D TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-5' TO ZCVS-TEST-NAME.
           COMPUTE WS-D ROUNDED = -12.346.
           IF WS-D = -12.35
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.35' TO ZCVS-EXPECTED
               MOVE WS-D TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REM-POS' TO ZCVS-TEST-NAME.
           DIVIDE 17 BY 5 GIVING WS-Q
             REMAINDER WS-R.
           IF WS-Q = 3 AND WS-R = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3/2' TO ZCVS-EXPECTED
               MOVE WS-R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REM-NEG' TO ZCVS-TEST-NAME.
           DIVIDE -17 BY 5 GIVING WS-Q
             REMAINDER WS-R.
           IF WS-Q = -3 AND WS-R = -2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-3/-2' TO ZCVS-EXPECTED
               MOVE WS-R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REM-NEG-DIV' TO ZCVS-TEST-NAME.
           DIVIDE 17 BY -5 GIVING WS-Q
             REMAINDER WS-R.
           IF WS-Q = -3 AND WS-R = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-3/2' TO ZCVS-EXPECTED
               MOVE WS-R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

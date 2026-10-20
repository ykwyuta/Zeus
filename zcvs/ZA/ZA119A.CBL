      * AUDIT: A06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * SPEC: SIZE ERROR AND DIVIDE REMAINDER; AUDIT A06.
      * ZA119A: Multiple receivers independent overflow and remainder
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA119A.
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
       01  SMALL-NUM PIC S99 COMP-3.
       01  LARGE-NUM PIC S9(4) COMP-3.
       01  QUOTIENT-NUM PIC S99 COMP-3.
       01  REMAINDER-NUM PIC S9 COMP-3.
       01  FLAG PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA119A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ADD-SMALL-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           ADD 1 TO SMALL-NUM LARGE-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ADD-WIDE-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           ADD 1 TO LARGE-NUM SMALL-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SUB-SMALL-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           SUBTRACT -1 FROM SMALL-NUM LARGE-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SUB-WIDE-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           SUBTRACT -1 FROM LARGE-NUM SMALL-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'MUL-SMALL-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           MULTIPLY 10 BY 10 GIVING SMALL-NUM LARGE-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-MULTIPLY.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'MUL-WIDE-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           MULTIPLY 10 BY 10 GIVING LARGE-NUM SMALL-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-MULTIPLY.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIV-SMALL-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           DIVIDE 100 BY 1 GIVING SMALL-NUM LARGE-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-DIVIDE.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIV-WIDE-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           DIVIDE 100 BY 1 GIVING LARGE-NUM SMALL-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-DIVIDE.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMPUTE-SMALL-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           COMPUTE SMALL-NUM LARGE-NUM = 100
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-COMPUTE.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMPUTE-WIDE-FIRST' TO ZCVS-TEST-NAME.
           MOVE 99 TO SMALL-NUM LARGE-NUM.
           MOVE SPACE TO FLAG.
           COMPUTE LARGE-NUM SMALL-NUM = 100
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-COMPUTE.
           IF SMALL-NUM = 99 AND LARGE-NUM = 100
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99/100/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'REM-ONLY-OVERFLOW' TO ZCVS-TEST-NAME.
           MOVE 55 TO QUOTIENT-NUM.
           MOVE 7 TO REMAINDER-NUM.
           MOVE 'N' TO FLAG.
           DIVIDE 199 BY 100 GIVING QUOTIENT-NUM
             REMAINDER REMAINDER-NUM
             ON SIZE ERROR MOVE 'Y' TO FLAG
           END-DIVIDE.
           IF QUOTIENT-NUM = 1 AND REMAINDER-NUM = 7
             AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1/7/Y' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

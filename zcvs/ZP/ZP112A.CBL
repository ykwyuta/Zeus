      * AUDIT: P02; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL DYNAM
      * ZP112A: Recursive PERFORM continuation and local frame isolation
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP112A.
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
       01  ARG-N PIC S9(9) COMP.
       01  RESULT-N PIC S9(9) COMP.
       01  CALLS-N PIC 9(4) COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP112A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'RECURSIVE-DEPTH-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 0 AND RESULT-N = 0
             AND CALLS-N = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0/1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 1 AND RESULT-N = 1
             AND CALLS-N = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1/3' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-2' TO ZCVS-TEST-NAME.
           MOVE 2 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 2 AND RESULT-N = 3
             AND CALLS-N = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3/6' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-3' TO ZCVS-TEST-NAME.
           MOVE 3 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 3 AND RESULT-N = 6
             AND CALLS-N = 10
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6/10' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-4' TO ZCVS-TEST-NAME.
           MOVE 4 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 4 AND RESULT-N = 10
             AND CALLS-N = 15
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10/15' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-5' TO ZCVS-TEST-NAME.
           MOVE 5 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 5 AND RESULT-N = 15
             AND CALLS-N = 21
               PERFORM ZCVS-PASS
           ELSE
               MOVE '15/21' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-6' TO ZCVS-TEST-NAME.
           MOVE 6 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 6 AND RESULT-N = 21
             AND CALLS-N = 28
               PERFORM ZCVS-PASS
           ELSE
               MOVE '21/28' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-7' TO ZCVS-TEST-NAME.
           MOVE 7 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 7 AND RESULT-N = 28
             AND CALLS-N = 36
               PERFORM ZCVS-PASS
           ELSE
               MOVE '28/36' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-DEPTH-8' TO ZCVS-TEST-NAME.
           MOVE 8 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 8 AND RESULT-N = 36
             AND CALLS-N = 45
               PERFORM ZCVS-PASS
           ELSE
               MOVE '36/45' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RECURSIVE-CANCEL' TO ZCVS-TEST-NAME.
           CANCEL 'ZPS12A'.
           MOVE 3 TO ARG-N.
           CALL 'ZPS12A' USING ARG-N RESULT-N CALLS-N.
           IF ARG-N = 3 AND RESULT-N = 6 AND CALLS-N = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6/4' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

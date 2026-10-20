      * AUDIT: P04/P06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL DYNAM
      * ZP111A: Alternating ENTRY arities reference value and pointer ca
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP111A.
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
       01  ARG-A PIC S9(9) COMP-5.
       01  ARG-B PIC S9(9) COMP-5.
       01  RESULT-NUM PIC S9(9) COMP-5.
       01  ENTRY-PTR USAGE PROCEDURE-POINTER.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP111A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ENTRY-MAIN-0' TO ZCVS-TEST-NAME.
           MOVE 1 TO ARG-A.
           MOVE -2 TO ARG-B.
           CALL 'ZPS11A' USING ARG-A ARG-B RESULT-NUM.
           IF RESULT-NUM = 99
             AND ARG-A = 2 AND ARG-B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-SHORT-0' TO ZCVS-TEST-NAME.
           CALL 'ZPS11B' USING ARG-A RESULT-NUM.
           IF RESULT-NUM = 202
             AND ARG-A = 12 AND ARG-B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-VALUE-0' TO ZCVS-TEST-NAME.
           CALL 'ZPS11C' USING BY VALUE ARG-B
             BY REFERENCE RESULT-NUM.
           IF RESULT-NUM = 300
             AND ARG-A = 12 AND ARG-B = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-MAIN-1' TO ZCVS-TEST-NAME.
           MOVE 11 TO ARG-A.
           MOVE -3 TO ARG-B.
           CALL 'ZPS11A' USING ARG-A ARG-B RESULT-NUM.
           IF RESULT-NUM = 108
             AND ARG-A = 12 AND ARG-B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-SHORT-1' TO ZCVS-TEST-NAME.
           CALL 'ZPS11B' USING ARG-A RESULT-NUM.
           IF RESULT-NUM = 212
             AND ARG-A = 22 AND ARG-B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-VALUE-1' TO ZCVS-TEST-NAME.
           CALL 'ZPS11C' USING BY VALUE ARG-B
             BY REFERENCE RESULT-NUM.
           IF RESULT-NUM = 299
             AND ARG-A = 22 AND ARG-B = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-MAIN-2' TO ZCVS-TEST-NAME.
           MOVE 21 TO ARG-A.
           MOVE -4 TO ARG-B.
           CALL 'ZPS11A' USING ARG-A ARG-B RESULT-NUM.
           IF RESULT-NUM = 117
             AND ARG-A = 22 AND ARG-B = -2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-SHORT-2' TO ZCVS-TEST-NAME.
           CALL 'ZPS11B' USING ARG-A RESULT-NUM.
           IF RESULT-NUM = 222
             AND ARG-A = 32 AND ARG-B = -2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-VALUE-2' TO ZCVS-TEST-NAME.
           CALL 'ZPS11C' USING BY VALUE ARG-B
             BY REFERENCE RESULT-NUM.
           IF RESULT-NUM = 298
             AND ARG-A = 32 AND ARG-B = -2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ENTRY-POINTER' TO ZCVS-TEST-NAME.
           SET ENTRY-PTR TO ENTRY 'ZPS11C'.
           CALL ENTRY-PTR USING BY VALUE ARG-B
             BY REFERENCE RESULT-NUM.
           IF RESULT-NUM = 298 AND ARG-B = -2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

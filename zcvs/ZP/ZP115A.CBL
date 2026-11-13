      * AUDIT: P05; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZP115A: >>CALLINTERFACE STATIC and DYNAMIC under NODYNAM. A
      * CANCEL of a program called statically takes no action, so its
      * WORKING-STORAGE keeps counting; after >>CALLINTERFACE DYNAMIC
      * the CALL is dynamic and CANCEL puts the program back in its
      * initial state. The directive does not affect CANCEL itself.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP115A.
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
       01  ANSWER-NUM PIC 99.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP115A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'STATIC-CANCEL-NOOP' TO ZCVS-TEST-NAME.
       >>CALLINTERFACE STATIC
           CALL 'ZPS15A' USING ANSWER-NUM.
           CANCEL 'ZPS15A'.
           CALL 'ZPS15A' USING ANSWER-NUM.
           IF ANSWER-NUM = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02' TO ZCVS-EXPECTED
               MOVE ANSWER-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DYNAMIC-CANCEL-RESET' TO ZCVS-TEST-NAME.
       >>CALLINTERFACE DYNAMIC
           CALL 'ZPS15B' USING ANSWER-NUM.
           CALL 'ZPS15B' USING ANSWER-NUM.
           CANCEL 'ZPS15B'.
           CALL 'ZPS15B' USING ANSWER-NUM.
           IF ANSWER-NUM = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '01' TO ZCVS-EXPECTED
               MOVE ANSWER-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIRECTIVE-RESET' TO ZCVS-TEST-NAME.
       >>CALLINTERFACE
           CALL 'ZPS15A' USING ANSWER-NUM.
           CANCEL 'ZPS15A'.
           CALL 'ZPS15A' USING ANSWER-NUM.
           IF ANSWER-NUM = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '04' TO ZCVS-EXPECTED
               MOVE ANSWER-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

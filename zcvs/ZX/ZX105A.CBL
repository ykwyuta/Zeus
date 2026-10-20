      * ZX105A: Nested EXIT/CYCLE targets and TEST AFTER ordering
      * SPEC: EXIT PERFORM/CYCLE AND PERFORM; AUDIT X01.
      * OUTER LOOP MUST CONTINUE AFTER INNER EXIT OR CYCLE.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX105A.
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
       01  WS-I PIC 99 COMP.
       01  WS-J PIC 99 COMP.
       01  WS-N PIC 999 COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX105A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'CYCLE-NEST-INNER' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-N.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
             PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > 3
               IF WS-J = 2
                 EXIT PERFORM CYCLE
               END-IF
               ADD 1 TO WS-N
             END-PERFORM
           END-PERFORM.
           IF WS-N = 6 AND WS-I = 4 AND WS-J = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6/4/4' TO ZCVS-EXPECTED
               MOVE WS-N TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXIT-NEST-INNER' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-N.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
             PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > 3
               IF WS-J = 2
                 EXIT PERFORM
               END-IF
               ADD 1 TO WS-N
             END-PERFORM
           END-PERFORM.
           IF WS-N = 3 AND WS-I = 4 AND WS-J = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3/4/2' TO ZCVS-EXPECTED
               MOVE WS-N TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CYCLE-TEST-AFTER' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-N.
           PERFORM WITH TEST AFTER
             VARYING WS-I FROM 1 BY 1 UNTIL WS-I >= 3
               ADD WS-I TO WS-N
               EXIT PERFORM CYCLE
               ADD 100 TO WS-N
           END-PERFORM.
           IF WS-N = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6' TO ZCVS-EXPECTED
               MOVE WS-N TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXIT-FIRST-ITER' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-N.
           PERFORM UNTIL EXIT
             ADD 1 TO WS-N
             EXIT PERFORM
             ADD 100 TO WS-N
           END-PERFORM.
           IF WS-N = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE WS-N TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

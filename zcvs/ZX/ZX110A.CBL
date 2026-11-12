      * ZX110A: PERFORM VARYING AFTER and SET TO FALSE
      * SPEC: PERFORM FORMAT 4 VARYING/AFTER; SET CONDITION-NAME TO
      * FALSE; AUDIT X01, X02. AFTER RESETS TO FROM AFTER THE OUTER
      * IDENTIFIER IS AUGMENTED; FALSE USES THE WHEN SET TO FALSE VALUE.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX110A.
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
       01  WS-BUF PIC X(8).
       01  WS-D PIC 9.
       01  WS-P PIC 99 COMP.
       01  WS-FLAG-A PIC X VALUE 'N'.
           88  A-YES VALUE 'Y' FALSE 'N'.
           88  A-NO VALUE 'N'.
       01  WS-FLAG-B PIC X VALUE 'Q'.
           88  B-YES VALUE 'Y' FALSE 'X'.
       01  WS-GRADE PIC X VALUE 'M'.
           88  G-PASS VALUE 'A' THRU 'C' FALSE 'Z'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX110A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'VARYING-AFTER-FINAL' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-N.
           PERFORM COUNT-ONE
               VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 2
               AFTER WS-J FROM 1 BY 1 UNTIL WS-J > 3.
           IF WS-N = 6 AND WS-I = 3 AND WS-J = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N6 I3 J1' TO ZCVS-EXPECTED
               MOVE WS-N TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VARYING-AFTER-DEPEND' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-N.
           PERFORM COUNT-ONE
               VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               AFTER WS-J FROM WS-I BY 1 UNTIL WS-J > 3.
           IF WS-N = 6 AND WS-I = 4 AND WS-J = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N6 I4 J4' TO ZCVS-EXPECTED
               MOVE WS-N TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VARYING-AFTER-ORDER' TO ZCVS-TEST-NAME.
           MOVE SPACES TO WS-BUF.
           MOVE 1 TO WS-P.
           PERFORM RECORD-PAIR
               VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 2
               AFTER WS-J FROM 2 BY -1 UNTIL WS-J < 1.
           IF WS-BUF = '12112221'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12112221' TO ZCVS-EXPECTED
               MOVE WS-BUF TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SET-FALSE-MULTIPLE' TO ZCVS-TEST-NAME.
           MOVE 'Y' TO WS-FLAG-A WS-FLAG-B.
           SET A-YES B-YES TO FALSE.
           IF WS-FLAG-A = 'N' AND WS-FLAG-B = 'X'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N X' TO ZCVS-EXPECTED
               MOVE WS-FLAG-A TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SET-FALSE-SIBLING' TO ZCVS-TEST-NAME.
           MOVE 'Y' TO WS-FLAG-A.
           SET A-YES TO FALSE.
           IF A-NO AND NOT A-YES
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A-NO TRUE' TO ZCVS-EXPECTED
               MOVE WS-FLAG-A TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SET-TRUE-RANGE-FIRST' TO ZCVS-TEST-NAME.
           SET G-PASS TO TRUE.
           IF WS-GRADE = 'A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A' TO ZCVS-EXPECTED
               MOVE WS-GRADE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SET-FALSE-RANGE' TO ZCVS-TEST-NAME.
           SET G-PASS TO FALSE.
           IF WS-GRADE = 'Z' AND NOT G-PASS
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Z' TO ZCVS-EXPECTED
               MOVE WS-GRADE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
       COUNT-ONE.
           ADD 1 TO WS-N.
       RECORD-PAIR.
           MOVE WS-I TO WS-D.
           MOVE WS-D TO WS-BUF(WS-P:1).
           ADD 1 TO WS-P.
           MOVE WS-J TO WS-D.
           MOVE WS-D TO WS-BUF(WS-P:1).
           ADD 1 TO WS-P.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

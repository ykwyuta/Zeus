      * AUDIT: A09; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZA128A: FUNCTION CONTENT-OF returns the content of its argument,
      * of the same size, with a type that follows the argument:
      * alphanumeric, national or numeric. An alphanumeric result can
      * be reference-modified.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA128A.
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
       01  A PIC X(5) VALUE 'AB CD'.
       01  N PIC S9(3)V9 VALUE -12.5.
       01  W PIC N(2) USAGE NATIONAL VALUE N'XY'.
       01  R PIC S9(5)V99.
       01  T PIC X(3).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA128A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'CONTENT-OF-ALNUM' TO ZCVS-TEST-NAME.
           IF FUNCTION CONTENT-OF(A) = 'AB CD'
             AND FUNCTION LENGTH(FUNCTION CONTENT-OF(A)) = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB CD/5' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CONTENT-OF-NUMERIC' TO ZCVS-TEST-NAME.
           COMPUTE R = FUNCTION CONTENT-OF(N) * 2.
           IF R = -25
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-25' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CONTENT-OF-NATIONAL' TO ZCVS-TEST-NAME.
           IF FUNCTION CONTENT-OF(W) = N'XY'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NXY' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CONTENT-OF-REFMOD' TO ZCVS-TEST-NAME.
           MOVE FUNCTION CONTENT-OF(A)(2:3) TO T.
           IF T = 'B C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'B C' TO ZCVS-EXPECTED
               MOVE T TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

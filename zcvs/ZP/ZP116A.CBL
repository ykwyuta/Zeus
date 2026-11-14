      * AUDIT: P07; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL QUALIFY(EXTEND)
      * ZP116A: QUALIFY(EXTEND) with the Programming Guide example. C OF
      * A has a complete set of qualifiers only for the 02 C, and the
      * unqualified A only for the 01 A, so both resolve; C OF B and
      * A OF B name the 03 items under standard rules.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP116A.
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
       01  A.
           02  B.
               03  C PIC X.
               03  A PIC X.
           02  C PIC X.
       01  SAVED PIC X(3).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP116A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'EXTEND-RECORD-A' TO ZCVS-TEST-NAME.
           MOVE ALL '*' TO A.
           MOVE A TO SAVED.
           IF SAVED = '***'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '***' TO ZCVS-EXPECTED
               MOVE SAVED TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXTEND-C-OF-A' TO ZCVS-TEST-NAME.
           MOVE '1' TO C OF A.
           MOVE A TO SAVED.
           IF SAVED = '**1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '**1' TO ZCVS-EXPECTED
               MOVE SAVED TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'STANDARD-OF-B' TO ZCVS-TEST-NAME.
           MOVE '2' TO C OF B.
           MOVE '3' TO A OF B.
           MOVE A TO SAVED.
           IF SAVED = '231'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '231' TO ZCVS-EXPECTED
               MOVE SAVED TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

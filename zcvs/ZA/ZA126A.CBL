       CBL INTDATE(LILIAN),NOZWB
      * ZA126A: the INTDATE(LILIAN) and NOZWB compiler options
      * (Enterprise COBOL Programming Guide). Under INTDATE(LILIAN)
      * integer day 1 is 15 October 1582 (ANSI day 1, 1 January 1601,
      * is Lilian day 6654) for INTEGER-OF-DATE, INTEGER-OF-DAY, DATE-
      * OF-INTEGER and DAY-OF-INTEGER. Under NOZWB a signed zoned item
      * compared with an alphanumeric operand keeps its sign: -1 is
      * the byte XD1 (J) and +1 XC1 (A).
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA126A.
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
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA126A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'LILIAN-DAY-ONE' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = FUNCTION INTEGER-OF-DATE(15821015).
           MOVE WS-N TO WS-NE.
           IF WS-N = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LILIAN-ANSI-DAY-ONE' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = FUNCTION INTEGER-OF-DATE(16010101).
           MOVE WS-N TO WS-NE.
           IF WS-N = 6654
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6654' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LILIAN-DATE-OF-INT' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = FUNCTION DATE-OF-INTEGER(1).
           MOVE WS-N TO WS-NE.
           IF WS-N = 15821015
               PERFORM ZCVS-PASS
           ELSE
               MOVE '15821015' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LILIAN-DAY-OF-INT' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = FUNCTION DAY-OF-INTEGER(6654).
           MOVE WS-N TO WS-NE.
           IF WS-N = 1601001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1601001' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LILIAN-INT-OF-DAY' TO ZCVS-TEST-NAME.
           COMPUTE WS-N = FUNCTION INTEGER-OF-DAY(1582288).
           MOVE WS-N TO WS-NE.
           IF WS-N = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE WS-NE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NOZWB-UNSIGNED-NE' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-C.
           IF WS-S = '1'
               MOVE 'Y' TO WS-C
           END-IF.
           IF WS-C = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NOZWB-SIGN-BYTE' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-C.
           IF WS-S = 'J'
               MOVE 'Y' TO WS-C
           END-IF.
           IF WS-C = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NOZWB-POSITIVE-SIGN' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-C.
           IF WS-P = 'A'
               MOVE 'Y' TO WS-C
           END-IF.
           IF WS-C = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

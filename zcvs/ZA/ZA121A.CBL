      * AUDIT: A09; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * SPEC: INTRINSIC FUNCTIONS; AUDIT A09.
      * ABSOLUTE ERROR BOUND 1E-9; ALL INPUTS IN DEFINED DOMAIN.
      * ZA121A: Nontrivial trigonometric finance and power functions
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA121A.
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
       01  RESULT-NUM COMP-2.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA121A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'LOG-ONE' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION LOG(1).
           IF FUNCTION ABS(RESULT-NUM - (0))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LOG-E' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION LOG(2.718281828459045).
           IF FUNCTION ABS(RESULT-NUM - (1))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TAN-ZERO' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION TAN(0).
           IF FUNCTION ABS(RESULT-NUM - (0))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ASIN-HALF' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION ASIN(0.5).
           IF FUNCTION ABS(RESULT-NUM - (0.523598775598299))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0.523598775598299' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ACOS-HALF' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION ACOS(0.5).
           IF FUNCTION ABS(RESULT-NUM - (1.047197551196598))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1.047197551196598' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ATAN-ONE' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION ATAN(1).
           IF FUNCTION ABS(RESULT-NUM - (0.785398163397448))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0.785398163397448' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIN-PI-HALF' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION SIN(1.570796326794897).
           IF FUNCTION ABS(RESULT-NUM - (1))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COS-PI' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION COS(3.141592653589793).
           IF FUNCTION ABS(RESULT-NUM - (-1))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ANNUITY-TWO' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION ANNUITY(0.1, 2).
           IF FUNCTION ABS(RESULT-NUM - (0.576190476190476))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0.576190476190476' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PRESENT-VALUE' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = FUNCTION PRESENT-VALUE(0.1, 100, 100).
           IF FUNCTION ABS(RESULT-NUM - (173.553719008264463))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '173.553719008264463' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'POWER-NEGEXP' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = 2 ** -3.
           IF FUNCTION ABS(RESULT-NUM - (0.125))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0.125' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'POWER-ZEROEXP' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = 7 ** 0.
           IF FUNCTION ABS(RESULT-NUM - (1))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'POWER-NEGBASE' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = (-3) ** 3.
           IF FUNCTION ABS(RESULT-NUM - (-27))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-27' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'POWER-FRACTION' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM = 4 ** 0.5.
           IF FUNCTION ABS(RESULT-NUM - (2))
               < 0.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

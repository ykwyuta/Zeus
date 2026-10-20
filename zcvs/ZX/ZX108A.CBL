      * NUMVAL RETURNS FLOAT; ROUND TO TWO DECIMAL PLACES.
      * AUDIT: X05; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZX108A: National TRIM result lengths and NUMVAL diagnostics
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX108A.
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
       01  NAT-IN PIC N(8) USAGE NATIONAL.
       01  NAT-OUT PIC N(8) USAGE NATIONAL.
       01  RESULT-NUM PIC S9(5)V99 COMP-3.
       01  POSITION-NUM PIC 99 COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX108A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'NTRIM-BOTH' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(N'  AB  ') TO NAT-OUT.
           IF NAT-OUT = N'AB'
             AND FUNCTION LENGTH(FUNCTION TRIM(N'  AB  ')) = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NTRIM-LEADING' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(N'  AB  ' LEADING) TO NAT-OUT.
           IF NAT-OUT = N'AB  '
             AND FUNCTION LENGTH(FUNCTION TRIM(N'  AB  ' LEADING)) = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NTRIM-TRAILING' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(N'  AB  ' TRAILING) TO NAT-OUT.
           IF NAT-OUT = N'  AB'
             AND FUNCTION LENGTH(FUNCTION TRIM(N'  AB  ' TRAILING)) = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NTRIM-EMPTY' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(N'    ') TO NAT-OUT.
           IF NAT-OUT = N' '
             AND FUNCTION LENGTH(FUNCTION TRIM(N'    ')) = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NTRIM-INTERNAL' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(N' A B ') TO NAT-OUT.
           IF NAT-OUT = N'A B'
             AND FUNCTION LENGTH(FUNCTION TRIM(N' A B ')) = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-TEST-0' TO ZCVS-TEST-NAME.
           COMPUTE POSITION-NUM =
             FUNCTION TEST-NUMVAL(N'123.45').
           IF POSITION-NUM = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE POSITION-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-TEST-1' TO ZCVS-TEST-NAME.
           COMPUTE POSITION-NUM =
             FUNCTION TEST-NUMVAL(N'-12.50').
           IF POSITION-NUM = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE POSITION-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-TEST-2' TO ZCVS-TEST-NAME.
           COMPUTE POSITION-NUM =
             FUNCTION TEST-NUMVAL(N'12X.50').
           IF POSITION-NUM = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE POSITION-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-TEST-3' TO ZCVS-TEST-NAME.
           COMPUTE POSITION-NUM =
             FUNCTION TEST-NUMVAL(N'1.2.3').
           IF POSITION-NUM = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE POSITION-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-TEST-4' TO ZCVS-TEST-NAME.
           COMPUTE POSITION-NUM =
             FUNCTION TEST-NUMVAL(N' ').
           IF POSITION-NUM = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE POSITION-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-TEST-5' TO ZCVS-TEST-NAME.
           COMPUTE POSITION-NUM =
             FUNCTION TEST-NUMVAL(N'12-').
           IF POSITION-NUM = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE POSITION-NUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-VALUE-0' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM ROUNDED = FUNCTION NUMVAL(N'123.45').
           IF RESULT-NUM = 123.45
               PERFORM ZCVS-PASS
           ELSE
               MOVE '123.45' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-VALUE-1' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM ROUNDED = FUNCTION NUMVAL(N'-12.50').
           IF RESULT-NUM = -12.5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.5' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N-NUMVAL-VALUE-2' TO ZCVS-TEST-NAME.
           COMPUTE RESULT-NUM ROUNDED = FUNCTION NUMVAL(N'12-').
           IF RESULT-NUM = -12
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

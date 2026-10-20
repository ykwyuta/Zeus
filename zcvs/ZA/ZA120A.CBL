      * AUDIT: A08; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * SPEC: EXTERNAL FLOATING POINT; AUDIT A08.
      * ZA120A: External floating point display and national storage
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA120A.
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
       01  EXT-DISPLAY PIC +99.99E+99.
       01  EXT-NATIONAL PIC +99.99E+99 USAGE NATIONAL.
       01  BACK-NUM PIC S9(8)V9(8) COMP-3.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA120A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'EXT-0-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE 0 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '+00.00E+00'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+00.00E+00' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-0-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'+00.00E+00'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+00.00E+00' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-0-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-1-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE 1.25 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '+12.50E-01'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+12.50E-01' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-1-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'+12.50E-01'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+12.50E-01' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-1-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = 1.25
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1.25' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-2-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE -1.25 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '-12.50E-01'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.50E-01' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-2-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'-12.50E-01'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.50E-01' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-2-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = -1.25
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1.25' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-3-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE 1234 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '+12.34E+02'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+12.34E+02' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-3-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'+12.34E+02'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+12.34E+02' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-3-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = 1234
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1234' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-4-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE -1234 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '-12.34E+02'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.34E+02' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-4-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'-12.34E+02'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.34E+02' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-4-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = -1234
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-5-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE 0.00125 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '+12.50E-04'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+12.50E-04' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-5-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'+12.50E-04'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+12.50E-04' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-5-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = 0.00125
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0.00125' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-6-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE 1000000 TO EXT-DISPLAY EXT-NATIONAL.
           IF EXT-DISPLAY = '+10.00E+05'
             AND EXT-DISPLAY IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+10.00E+05' TO ZCVS-EXPECTED
               MOVE EXT-DISPLAY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-6-NATIONAL' TO ZCVS-TEST-NAME.
           IF EXT-NATIONAL = N'+10.00E+05'
             AND EXT-NATIONAL IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+10.00E+05' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'EXT-6-BACK' TO ZCVS-TEST-NAME.
           MOVE EXT-DISPLAY TO BACK-NUM.
           IF BACK-NUM = 1000000
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1000000' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

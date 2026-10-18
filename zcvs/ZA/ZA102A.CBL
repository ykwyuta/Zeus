       CBL TRUNC(OPT)
      *================================================================*
      * ZA102A - CBL TRUNC(OPT) BINARY ARITHMETIC AND STORAGE          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA102A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-OPT-FIELDS.
           05  WS-HW-A              PIC S9(4) COMP.
           05  WS-HW-B              PIC S9(4) COMP.
           05  WS-HW-RES            PIC S9(4) COMP.
           05  WS-HW-RES-RAW REDEFINES WS-HW-RES PIC X(2).
           05  WS-FW-A              PIC S9(9) COMP.
           05  WS-FW-B              PIC S9(9) COMP.
           05  WS-FW-RES            PIC S9(9) COMP.
           05  WS-FW-RES-RAW REDEFINES WS-FW-RES PIC X(4).
           05  WS-DW-RES            PIC S9(18) COMP.
           05  WS-DW-RES-RAW REDEFINES WS-DW-RES PIC X(8).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: HALFWORD ADD WITHIN PIC RANGE (4500 + 3200 = 7700)
           MOVE 'TOPT-HW-ADD' TO ZCVS-TEST-NAME.
           MOVE 4500 TO WS-HW-A.
           MOVE 3200 TO WS-HW-B.
           COMPUTE WS-HW-RES = WS-HW-A + WS-HW-B.
           MOVE WS-HW-RES-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-HW-RES = 7700 AND ZCVS-HEX-OUT(1:4) = '1E14'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1E14' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: HALFWORD SUBTRACT NEGATIVE (1200 - 5400 = -4200 = EF98)
           MOVE 'TOPT-HW-SUB' TO ZCVS-TEST-NAME.
           MOVE 1200 TO WS-HW-A.
           MOVE 5400 TO WS-HW-B.
           SUBTRACT WS-HW-B FROM WS-HW-A GIVING WS-HW-RES.
           MOVE WS-HW-RES-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-HW-RES = -4200 AND ZCVS-HEX-OUT(1:4) = 'EF98'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'EF98' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: FULLWORD MULTIPLY (12345 * -6789 = FB012863)
           MOVE 'TOPT-FW-MUL' TO ZCVS-TEST-NAME.
           MOVE 12345 TO WS-FW-A.
           MOVE -6789 TO WS-FW-B.
           MULTIPLY WS-FW-A BY WS-FW-B GIVING WS-FW-RES.
           MOVE WS-FW-RES-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-FW-RES = -83810205
             AND ZCVS-HEX-OUT(1:8) = 'FB012863'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FB012863' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: FULLWORD DIVIDE WITH REMAINDER
           MOVE 'TOPT-FW-DIV' TO ZCVS-TEST-NAME.
           MOVE 98765432 TO WS-FW-A.
           DIVIDE WS-FW-A BY 10000 GIVING WS-HW-RES REMAINDER WS-HW-B.
           IF WS-HW-RES = 9876 AND WS-HW-B = 5432
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9876/5432' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: DOUBLEWORD COMPUTE
           MOVE 'TOPT-DW-COMP' TO ZCVS-TEST-NAME.
           MOVE 500000000 TO WS-FW-A.
           MOVE 400000000 TO WS-FW-B.
           COMPUTE WS-DW-RES = WS-FW-A * WS-FW-B.
           IF WS-DW-RES = 200000000000000000
               PERFORM ZCVS-PASS
           ELSE
               MOVE '200000000000000000' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

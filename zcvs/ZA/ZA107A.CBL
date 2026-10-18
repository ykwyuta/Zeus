      *================================================================*
      * ZA107A - SYSTEM/390 HFP (COMP-1 / COMP-2) BYTE REPRESENTATION  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA107A.
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
       01  WS-HFP-FIELDS.
           05  WS-C1                COMP-1.
           05  WS-C1-RAW REDEFINES WS-C1 PIC X(4).
           05  WS-C2                COMP-2.
           05  WS-C2-RAW REDEFINES WS-C2 PIC X(8).
           05  WS-C2-B              COMP-2.
           05  WS-DEC-OUT           PIC S9(5)V9(5) COMP-3.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA107A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: COMP-1 ZERO -> 00000000
           MOVE 'HFP-C1-ZERO' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-C1.
           MOVE WS-C1-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '00000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: COMP-1 +1.0 -> 41100000 (16^1 * 1/16)
           MOVE 'HFP-C1-ONE' TO ZCVS-TEST-NAME.
           MOVE 1.0 TO WS-C1.
           MOVE WS-C1-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '41100000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '41100000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: COMP-1 -1.0 -> C1100000
           MOVE 'HFP-C1-NEG-ONE' TO ZCVS-TEST-NAME.
           MOVE -1.0 TO WS-C1.
           MOVE WS-C1-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'C1100000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1100000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: COMP-1 +100.0 -> 42640000 (16^2 * 100/256)
           MOVE 'HFP-C1-100' TO ZCVS-TEST-NAME.
           MOVE 100.0 TO WS-C1.
           MOVE WS-C1-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '42640000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '42640000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: COMP-1 -0.5 -> C0800000 (16^0 * 8/16)
           MOVE 'HFP-C1-NEG-HALF' TO ZCVS-TEST-NAME.
           MOVE -0.5 TO WS-C1.
           MOVE WS-C1-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'C0800000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0800000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: COMP-2 +1.0 -> 4110000000000000 (8 BYTES)
           MOVE 'HFP-C2-ONE' TO ZCVS-TEST-NAME.
           MOVE 1.0 TO WS-C2.
           MOVE WS-C2-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '4110000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4110000000000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: COMP-2 -0.15625 -> C028000000000000
           MOVE 'HFP-C2-FRAC' TO ZCVS-TEST-NAME.
           MOVE -0.15625 TO WS-C2.
           MOVE WS-C2-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = 'C028000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C028000000000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: HFP ARITHMETIC AND CONVERSION TO COMP-3
           MOVE 'HFP-C2-ARITH' TO ZCVS-TEST-NAME.
           MOVE 15.5 TO WS-C2.
           MOVE 2.25 TO WS-C2-B.
           COMPUTE WS-C2 = WS-C2 * WS-C2-B - 4.875.
           MOVE WS-C2 TO WS-DEC-OUT.
           IF WS-DEC-OUT = 30.00000
               PERFORM ZCVS-PASS
           ELSE
               MOVE '30.00000' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

      *================================================================*
      * ZD103A - BIG-ENDIAN BINARY (COMP / COMP-4 / BINARY / COMP-5)   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD103A.
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
       01  WS-BIN-GROUP.
           05  WS-HW-S              PIC S9(4) COMP.
           05  WS-HW-S-RAW REDEFINES WS-HW-S PIC X(2).
           05  WS-HW-U              PIC 9(4) COMP.
           05  WS-HW-U-RAW REDEFINES WS-HW-U PIC X(2).
           05  WS-FW-S              PIC S9(9) BINARY.
           05  WS-FW-S-RAW REDEFINES WS-FW-S PIC X(4).
           05  WS-FW-U              PIC 9(9) COMP-4.
           05  WS-FW-U-RAW REDEFINES WS-FW-U PIC X(4).
           05  WS-DW-S              PIC S9(18) COMP.
           05  WS-DW-S-RAW REDEFINES WS-DW-S PIC X(8).
           05  WS-C5-HW-S           PIC S9(4) COMP-5.
           05  WS-C5-HW-S-RAW REDEFINES WS-C5-HW-S PIC X(2).
           05  WS-C5-HW-U           PIC 9(4) COMP-5.
           05  WS-C5-HW-U-RAW REDEFINES WS-C5-HW-U PIC X(2).
           05  WS-C5-FW-S           PIC S9(9) COMP-5.
           05  WS-C5-FW-S-RAW REDEFINES WS-C5-FW-S PIC X(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD103A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: HALFWORD PIC S9(4) COMP +1 -> 0001
           MOVE 'HW-POS-1' TO ZCVS-TEST-NAME.
           MOVE +1 TO WS-HW-S.
           MOVE WS-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '0001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: HALFWORD PIC S9(4) COMP -1 -> FFFF
           MOVE 'HW-NEG-1' TO ZCVS-TEST-NAME.
           MOVE -1 TO WS-HW-S.
           MOVE WS-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'FFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: HALFWORD PIC S9(4) COMP +256 -> 0100 (BIG-ENDIAN)
           MOVE 'HW-POS-256' TO ZCVS-TEST-NAME.
           MOVE +256 TO WS-HW-S.
           MOVE WS-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '0100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: HALFWORD PIC S9(4) COMP -256 -> FF00
           MOVE 'HW-NEG-256' TO ZCVS-TEST-NAME.
           MOVE -256 TO WS-HW-S.
           MOVE WS-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'FF00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FF00' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: UNSIGNED HALFWORD PIC 9(4) COMP 9999 -> 270F
           MOVE 'HW-U-9999' TO ZCVS-TEST-NAME.
           MOVE 9999 TO WS-HW-U.
           MOVE WS-HW-U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '270F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '270F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: FULLWORD PIC S9(9) BINARY +1 -> 00000001
           MOVE 'FW-POS-1' TO ZCVS-TEST-NAME.
           MOVE +1 TO WS-FW-S.
           MOVE WS-FW-S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '00000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: FULLWORD PIC S9(9) BINARY -1 -> FFFFFFFF
           MOVE 'FW-NEG-1' TO ZCVS-TEST-NAME.
           MOVE -1 TO WS-FW-S.
           MOVE WS-FW-S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'FFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: FULLWORD PIC S9(9) BINARY +123456789 -> 075BCD15
           MOVE 'FW-POS-123456789' TO ZCVS-TEST-NAME.
           MOVE +123456789 TO WS-FW-S.
           MOVE WS-FW-S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '075BCD15'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '075BCD15' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 09: FULLWORD PIC S9(9) BINARY -123456789 -> F8A432EB
           MOVE 'FW-NEG-123456789' TO ZCVS-TEST-NAME.
           MOVE -123456789 TO WS-FW-S.
           MOVE WS-FW-S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F8A432EB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F8A432EB' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 10: UNSIGNED FULLWORD PIC 9(9) COMP-4 999999999 -> 3B9AC9FF
           MOVE 'FW-U-999999999' TO ZCVS-TEST-NAME.
           MOVE 999999999 TO WS-FW-U.
           MOVE WS-FW-U-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '3B9AC9FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3B9AC9FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 11: DOUBLEWORD PIC S9(18) COMP +1 -> 0000000000000001
           MOVE 'DW-POS-1' TO ZCVS-TEST-NAME.
           MOVE +1 TO WS-DW-S.
           MOVE WS-DW-S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0000000000000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 12: DOUBLEWORD PIC S9(18) COMP -1 -> FFFFFFFFFFFFFFFF
           MOVE 'DW-NEG-1' TO ZCVS-TEST-NAME.
           MOVE -1 TO WS-DW-S.
           MOVE WS-DW-S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = 'FFFFFFFFFFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFFFFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 13: DOUBLEWORD PIC S9(18) COMP +1000000000000
           MOVE 'DW-POS-1E12' TO ZCVS-TEST-NAME.
           MOVE +1000000000000 TO WS-DW-S.
           MOVE WS-DW-S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '000000E8D4A51000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000E8D4A51000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 14: COMP-5 SIGNED HALFWORD MAX +32767 -> 7FFF
           MOVE 'C5-HW-S-32767' TO ZCVS-TEST-NAME.
           MOVE +32767 TO WS-C5-HW-S.
           MOVE WS-C5-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '7FFF' AND WS-C5-HW-S = 32767
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7FFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 15: COMP-5 SIGNED HALFWORD MIN -32768 -> 8000
           MOVE 'C5-HW-S-N32768' TO ZCVS-TEST-NAME.
           MOVE -32768 TO WS-C5-HW-S.
           MOVE WS-C5-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '8000' AND WS-C5-HW-S = -32768
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 16: COMP-5 UNSIGNED HALFWORD MAX 65535 -> FFFF
           MOVE 'C5-HW-U-65535' TO ZCVS-TEST-NAME.
           MOVE 65535 TO WS-C5-HW-U.
           MOVE WS-C5-HW-U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'FFFF' AND WS-C5-HW-U = 65535
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

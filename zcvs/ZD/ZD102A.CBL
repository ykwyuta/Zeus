      *================================================================*
      * ZD102A - ZONED DECIMAL (USAGE DISPLAY) AND SIGN CLAUSES        *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD102A.
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
       01  WS-ZONED-GROUP.
           05  WS-UZ4               PIC 9(4).
           05  WS-UZ4-RAW REDEFINES WS-UZ4 PIC X(4).
           05  WS-SZ4-TR            PIC S9(4).
           05  WS-SZ4-TR-RAW REDEFINES WS-SZ4-TR PIC X(4).
           05  WS-SZ4-LD            PIC S9(4) SIGN IS LEADING.
           05  WS-SZ4-LD-RAW REDEFINES WS-SZ4-LD PIC X(4).
           05  WS-SZ4-TRS           PIC S9(4)
                                    SIGN IS TRAILING SEPARATE.
           05  WS-SZ4-TRS-RAW REDEFINES WS-SZ4-TRS PIC X(5).
           05  WS-SZ4-LDS           PIC S9(4)
                                    SIGN IS LEADING SEPARATE.
           05  WS-SZ4-LDS-RAW REDEFINES WS-SZ4-LDS PIC X(5).
           05  WS-SZ1               PIC S9(1).
           05  WS-SZ1-RAW REDEFINES WS-SZ1 PIC X(1).
           05  WS-CHECK-COMP3       PIC S9(4) COMP-3.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: UNSIGNED ZONED PIC 9(4) 1234 -> F1F2F3F4
           MOVE 'UZ4-1234-HEX' TO ZCVS-TEST-NAME.
           MOVE 1234 TO WS-UZ4.
           MOVE WS-UZ4-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F1F2F3F4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3F4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: TRAILING OVERPUNCH +1234 -> F1F2F3C4 ('123D')
           MOVE 'SZ4-TR-POS' TO ZCVS-TEST-NAME.
           MOVE +1234 TO WS-SZ4-TR.
           MOVE WS-SZ4-TR-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F1F2F3C4'
             AND WS-SZ4-TR-RAW = '123D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3C4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: TRAILING OVERPUNCH -1234 -> F1F2F3D4 ('123M')
           MOVE 'SZ4-TR-NEG' TO ZCVS-TEST-NAME.
           MOVE -1234 TO WS-SZ4-TR.
           MOVE WS-SZ4-TR-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F1F2F3D4'
             AND WS-SZ4-TR-RAW = '123M'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3D4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: TRAILING OVERPUNCH +1230 -> F1F2F3C0
           MOVE 'SZ4-TR-POS0' TO ZCVS-TEST-NAME.
           MOVE +1230 TO WS-SZ4-TR.
           MOVE WS-SZ4-TR-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F1F2F3C0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3C0' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: TRAILING OVERPUNCH -1230 -> F1F2F3D0
           MOVE 'SZ4-TR-NEG0' TO ZCVS-TEST-NAME.
           MOVE -1230 TO WS-SZ4-TR.
           MOVE WS-SZ4-TR-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F1F2F3D0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3D0' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: OVERPUNCH +1 ('A' = C1) AND +9 ('I' = C9)
           MOVE 'SZ1-POS-1-9' TO ZCVS-TEST-NAME.
           MOVE +1 TO WS-SZ1.
           MOVE WS-SZ1-RAW TO ZCVS-HEX-IN(1:1).
           MOVE +9 TO WS-SZ1.
           MOVE WS-SZ1-RAW TO ZCVS-HEX-IN(2:1).
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'C1C9'
             AND ZCVS-HEX-IN(1:2) = 'AI'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1C9' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: OVERPUNCH -1 ('J' = D1) AND -9 ('R' = D9)
           MOVE 'SZ1-NEG-1-9' TO ZCVS-TEST-NAME.
           MOVE -1 TO WS-SZ1.
           MOVE WS-SZ1-RAW TO ZCVS-HEX-IN(1:1).
           MOVE -9 TO WS-SZ1.
           MOVE WS-SZ1-RAW TO ZCVS-HEX-IN(2:1).
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'D1D9'
             AND ZCVS-HEX-IN(1:2) = 'JR'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D1D9' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: SIGN IS LEADING POSITIVE +1234 -> C1F2F3F4 ('A234')
           MOVE 'SZ4-LD-POS' TO ZCVS-TEST-NAME.
           MOVE +1234 TO WS-SZ4-LD.
           MOVE WS-SZ4-LD-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'C1F2F3F4'
             AND WS-SZ4-LD-RAW = 'A234'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1F2F3F4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 09: SIGN IS LEADING NEGATIVE -1234 -> D1F2F3F4 ('J234')
           MOVE 'SZ4-LD-NEG' TO ZCVS-TEST-NAME.
           MOVE -1234 TO WS-SZ4-LD.
           MOVE WS-SZ4-LD-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'D1F2F3F4'
             AND WS-SZ4-LD-RAW = 'J234'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D1F2F3F4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 10: SIGN IS TRAILING SEPARATE +1234 -> F1F2F3F44E ('1234+')
           MOVE 'SZ4-TRS-POS' TO ZCVS-TEST-NAME.
           MOVE +1234 TO WS-SZ4-TRS.
           MOVE WS-SZ4-TRS-RAW TO ZCVS-HEX-IN.
           MOVE 5 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:10) = 'F1F2F3F44E'
             AND WS-SZ4-TRS-RAW = '1234+'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3F44E' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:10) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 11: SIGN IS TRAILING SEPARATE -1234 -> F1F2F3F460 ('1234-')
           MOVE 'SZ4-TRS-NEG' TO ZCVS-TEST-NAME.
           MOVE -1234 TO WS-SZ4-TRS.
           MOVE WS-SZ4-TRS-RAW TO ZCVS-HEX-IN.
           MOVE 5 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:10) = 'F1F2F3F460'
             AND WS-SZ4-TRS-RAW = '1234-'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F2F3F460' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:10) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 12: SIGN IS LEADING SEPARATE +1234 -> 4EF1F2F3F4 ('+1234')
           MOVE 'SZ4-LDS-POS' TO ZCVS-TEST-NAME.
           MOVE +1234 TO WS-SZ4-LDS.
           MOVE WS-SZ4-LDS-RAW TO ZCVS-HEX-IN.
           MOVE 5 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:10) = '4EF1F2F3F4'
             AND WS-SZ4-LDS-RAW = '+1234'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF1F2F3F4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:10) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 13: SIGN IS LEADING SEPARATE -1234 -> 60F1F2F3F4 ('-1234')
           MOVE 'SZ4-LDS-NEG' TO ZCVS-TEST-NAME.
           MOVE -1234 TO WS-SZ4-LDS.
           MOVE WS-SZ4-LDS-RAW TO ZCVS-HEX-IN.
           MOVE 5 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:10) = '60F1F2F3F4'
             AND WS-SZ4-LDS-RAW = '-1234'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F1F2F3F4' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:10) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 14: ROUND-TRIP FROM TRAILING OVERPUNCH TO COMP-3
           MOVE 'RT-TR-TO-COMP3' TO ZCVS-TEST-NAME.
           MOVE '123M' TO WS-SZ4-TR-RAW.
           MOVE WS-SZ4-TR TO WS-CHECK-COMP3.
           IF WS-CHECK-COMP3 = -1234
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 15: ROUND-TRIP FROM LEADING OVERPUNCH TO COMP-3
           MOVE 'RT-LD-TO-COMP3' TO ZCVS-TEST-NAME.
           MOVE 'J234' TO WS-SZ4-LD-RAW.
           MOVE WS-SZ4-LD TO WS-CHECK-COMP3.
           IF WS-CHECK-COMP3 = -1234
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 16: ROUND-TRIP FROM SEPARATE SIGN TO COMP-3
           MOVE 'RT-SEP-TO-COMP3' TO ZCVS-TEST-NAME.
           MOVE '1234-' TO WS-SZ4-TRS-RAW.
           MOVE WS-SZ4-TRS TO WS-CHECK-COMP3.
           IF WS-CHECK-COMP3 = -1234
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1234' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

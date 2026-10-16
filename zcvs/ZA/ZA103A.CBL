       CBL TRUNC(BIN)
      *================================================================*
      * ZA103A - CBL TRUNC(BIN) FULL BINARY CAPACITY ON COMP FIELDS    *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA103A.
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
       01  WS-TBIN-FIELDS.
           05  WS-HW-S              PIC S9(4) COMP.
           05  WS-HW-S-RAW REDEFINES WS-HW-S PIC X(2).
           05  WS-HW-U              PIC 9(4) COMP.
           05  WS-HW-U-RAW REDEFINES WS-HW-U PIC X(2).
           05  WS-FW-S              PIC S9(9) COMP.
           05  WS-FW-S-RAW REDEFINES WS-FW-S PIC X(4).
           05  WS-B2-S              PIC S9(2) COMP.
           05  WS-B2-S-RAW REDEFINES WS-B2-S PIC X(2).
           05  WS-DISP-OUT          PIC -9(5).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA103A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: PIC S9(4) COMP HOLDS +32767 (7FFF) UNDER TRUNC(BIN)
           MOVE 'TBIN-HW-32767' TO ZCVS-TEST-NAME.
           MOVE 32767 TO WS-HW-S.
           MOVE WS-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-HW-S = 32767 AND ZCVS-HEX-OUT(1:4) = '7FFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7FFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: PIC S9(4) COMP HOLDS -32768 (8000) UNDER TRUNC(BIN)
           MOVE 'TBIN-HW-N32768' TO ZCVS-TEST-NAME.
           MOVE -32768 TO WS-HW-S.
           MOVE WS-HW-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-HW-S = -32768 AND ZCVS-HEX-OUT(1:4) = '8000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: PIC 9(4) COMP HOLDS 65535 (FFFF) UNDER TRUNC(BIN)
           MOVE 'TBIN-HW-U-65535' TO ZCVS-TEST-NAME.
           MOVE 65535 TO WS-HW-U.
           MOVE WS-HW-U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-HW-U = 65535 AND ZCVS-HEX-OUT(1:4) = 'FFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: PIC S9(2) COMP HOLDS 12345 (3039) UNDER TRUNC(BIN)
           MOVE 'TBIN-S2-12345' TO ZCVS-TEST-NAME.
           MOVE 12345 TO WS-B2-S.
           MOVE WS-B2-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-B2-S = 12345 AND ZCVS-HEX-OUT(1:4) = '3039'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3039' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: PIC S9(9) COMP HOLDS +2147483647 (7FFFFFFF)
           MOVE 'TBIN-FW-MAX' TO ZCVS-TEST-NAME.
           MOVE 2147483647 TO WS-FW-S.
           MOVE WS-FW-S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-FW-S = 2147483647
             AND ZCVS-HEX-OUT(1:8) = '7FFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7FFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: ARITHMETIC EXCEEDING 9999 IN PIC S9(4) COMP
           MOVE 'TBIN-HW-ARITH' TO ZCVS-TEST-NAME.
           MOVE 9000 TO WS-HW-S.
           ADD 6000 TO WS-HW-S.
           MOVE WS-HW-S TO WS-DISP-OUT.
           IF WS-HW-S = 15000 AND WS-DISP-OUT = ' 15000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' 15000' TO ZCVS-EXPECTED
               MOVE WS-DISP-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

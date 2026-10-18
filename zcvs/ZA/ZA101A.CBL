       CBL TRUNC(STD)
      *================================================================*
      * ZA101A - CBL TRUNC(STD) DECIMAL PICTURE TRUNCATION             *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA101A.
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
       01  WS-TRUNC-FIELDS.
           05  WS-B2-U              PIC 9(2) COMP.
           05  WS-B2-U-RAW REDEFINES WS-B2-U PIC X(2).
           05  WS-B2-S              PIC S9(2) COMP.
           05  WS-B2-S-RAW REDEFINES WS-B2-S PIC X(2).
           05  WS-B4-S              PIC S9(4) COMP.
           05  WS-B4-S-RAW REDEFINES WS-B4-S PIC X(2).
           05  WS-B9-S              PIC S9(9) COMP.
           05  WS-B9-S-RAW REDEFINES WS-B9-S PIC X(4).
           05  WS-C5-4              PIC S9(4) COMP-5.
           05  WS-C5-4-RAW REDEFINES WS-C5-4 PIC X(2).
           05  WS-SZ-FLAG           PIC X(1).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: PIC 9(2) COMP MOVE 123 TRUNCATES TO 23 (0017)
           MOVE 'TSTD-U2-MOVE' TO ZCVS-TEST-NAME.
           MOVE 123 TO WS-B2-U.
           MOVE WS-B2-U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-B2-U = 23 AND ZCVS-HEX-OUT(1:4) = '0017'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0017' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: PIC S9(2) COMP MOVE -123 TRUNCATES TO -23 (FFE9)
           MOVE 'TSTD-S2-MOVE' TO ZCVS-TEST-NAME.
           MOVE -123 TO WS-B2-S.
           MOVE WS-B2-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-B2-S = -23 AND ZCVS-HEX-OUT(1:4) = 'FFE9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFE9' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: PIC S9(4) COMP MOVE 12345 TRUNCATES TO 2345 (0929)
           MOVE 'TSTD-S4-MOVE' TO ZCVS-TEST-NAME.
           MOVE 12345 TO WS-B4-S.
           MOVE WS-B4-S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-B4-S = 2345 AND ZCVS-HEX-OUT(1:4) = '0929'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0929' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: PIC S9(4) COMP ADD WITHOUT SIZE ERROR TRUNCATES AT 10^4
           MOVE 'TSTD-S4-ADD' TO ZCVS-TEST-NAME.
           MOVE 9990 TO WS-B4-S.
           ADD 25 TO WS-B4-S.
           IF WS-B4-S = 15
               PERFORM ZCVS-PASS
           ELSE
               MOVE '15' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: PIC 9(2) COMP ADD WITH ON SIZE ERROR FIRES AT 100
           MOVE 'TSTD-S2-SZERR' TO ZCVS-TEST-NAME.
           MOVE 99 TO WS-B2-U.
           MOVE 'N' TO WS-SZ-FLAG.
           ADD 1 TO WS-B2-U
               ON SIZE ERROR
                   MOVE 'Y' TO WS-SZ-FLAG
           END-ADD.
           IF WS-SZ-FLAG = 'Y' AND WS-B2-U = 99
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/99' TO ZCVS-EXPECTED
               MOVE WS-SZ-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: PIC S9(9) COMP MOVE 1234567890 -> 234567890 (0DFB38D2)
           MOVE 'TSTD-S9-MOVE' TO ZCVS-TEST-NAME.
           MOVE 1234567890 TO WS-B9-S.
           MOVE WS-B9-S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-B9-S = 234567890
             AND ZCVS-HEX-OUT(1:8) = '0DFB38D2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0DFB38D2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: COMP-5 NOT TRUNCATED BY TRUNC(STD) -> 12345 (3039)
           MOVE 'TSTD-COMP5-EXEMPT' TO ZCVS-TEST-NAME.
           MOVE 12345 TO WS-C5-4.
           MOVE WS-C5-4-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-C5-4 = 12345 AND ZCVS-HEX-OUT(1:4) = '3039'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3039' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

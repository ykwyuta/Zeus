       CBL NSYMBOL(DBCS)
      *================================================================*
      * ZN102B - CBL NSYMBOL(DBCS): PIC N AND N'...' AS DBCS DISPLAY-1 *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN102B.
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
       01  WS-DBCS-N-FIELDS.
           05  WS-N-DBCS            PIC N(2).
           05  WS-N-RAW REDEFINES WS-N-DBCS PIC X(4).
           05  WS-G-DBCS            PIC G(2) DISPLAY-1.
           05  WS-G-RAW REDEFINES WS-G-DBCS PIC X(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN102B' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: UNDER NSYMBOL(DBCS), N'日' ENCODES AS CCSID 300 X'4562'
           MOVE 'NSDBCS-LIT-HEX' TO ZCVS-TEST-NAME.
           MOVE N'日本' TO WS-N-DBCS.
           MOVE G'日本' TO WS-G-DBCS.
           MOVE WS-N-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-N-RAW = WS-G-RAW
             AND ZCVS-HEX-OUT(1:4) = '4562'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4562' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: UNDER NSYMBOL(DBCS), PADDING USES DBCS SPACE X'4040'
           MOVE 'NSDBCS-PAD-4040' TO ZCVS-TEST-NAME.
           MOVE N'日' TO WS-N-DBCS.
           MOVE WS-N-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '45624040'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '45624040' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: INITIALIZE REPLACING DBCS BY SPACE SETS X'40404040'
           MOVE 'NSDBCS-INIT-DBCS' TO ZCVS-TEST-NAME.
           INITIALIZE WS-N-DBCS REPLACING DBCS DATA BY SPACE.
           MOVE WS-N-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '40404040'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '40404040' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

      *================================================================*
      * ZD107A - FIGURATIVE CONSTANTS AND INITIALIZE IN EBCDIC         *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD107A.
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
       01  WS-FIG-BUF               PIC X(4).
       01  WS-INIT-GROUP.
           05  WS-IG-ALPHA          PIC A(4) VALUE 'ABCD'.
           05  WS-IG-ALNUM          PIC X(4) VALUE 'WXYZ'.
           05  WS-IG-DISP           PIC 9(4) VALUE 9876.
           05  WS-IG-PACK           PIC S9(5) COMP-3 VALUE -12345.
           05  WS-IG-PACK-RAW REDEFINES WS-IG-PACK PIC X(3).
           05  WS-IG-BIN            PIC S9(4) COMP VALUE -999.
           05  WS-IG-BIN-RAW REDEFINES WS-IG-BIN PIC X(2).
           05  FILLER               PIC X(2) VALUE '##'.
       01  WS-INIT-RAW REDEFINES WS-INIT-GROUP PIC X(19).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD107A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: SPACES IN EBCDIC -> 40404040
           MOVE 'FIG-SPACES-HEX' TO ZCVS-TEST-NAME.
           MOVE SPACES TO WS-FIG-BUF.
           MOVE WS-FIG-BUF TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '40404040'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '40404040' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: ZEROS IN PIC X(4) -> F0F0F0F0
           MOVE 'FIG-ZEROS-HEX' TO ZCVS-TEST-NAME.
           MOVE ZEROS TO WS-FIG-BUF.
           MOVE WS-FIG-BUF TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'F0F0F0F0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F0' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: HIGH-VALUES -> FFFFFFFF
           MOVE 'FIG-HIGH-HEX' TO ZCVS-TEST-NAME.
           MOVE HIGH-VALUES TO WS-FIG-BUF.
           MOVE WS-FIG-BUF TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'FFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: LOW-VALUES -> 00000000
           MOVE 'FIG-LOW-HEX' TO ZCVS-TEST-NAME.
           MOVE LOW-VALUES TO WS-FIG-BUF.
           MOVE WS-FIG-BUF TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '00000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: QUOTES IN EBCDIC -> 7F7F7F7F
           MOVE 'FIG-QUOTES-HEX' TO ZCVS-TEST-NAME.
           MOVE QUOTES TO WS-FIG-BUF.
           MOVE WS-FIG-BUF TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '7F7F7F7F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7F7F7F7F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: ALL 'AB' -> C1C2C1C2
           MOVE 'FIG-ALL-AB-HEX' TO ZCVS-TEST-NAME.
           MOVE ALL 'AB' TO WS-FIG-BUF.
           MOVE WS-FIG-BUF TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'C1C2C1C2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1C2C1C2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: INITIALIZE SETS DISPLAY, COMP-3 (00000C), COMP (0000)
           MOVE 'INIT-DEFAULT' TO ZCVS-TEST-NAME.
           INITIALIZE WS-INIT-GROUP.
           MOVE WS-IG-PACK-RAW TO ZCVS-HEX-IN(1:3).
           MOVE WS-IG-BIN-RAW TO ZCVS-HEX-IN(4:2).
           MOVE 5 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-IG-ALPHA = SPACES
             AND WS-IG-ALNUM = SPACES
             AND WS-IG-DISP = 0
             AND ZCVS-HEX-OUT(1:10) = '00000C0000'
             AND WS-INIT-RAW(18:2) = '##'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000C0000/##' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:10) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: INITIALIZE REPLACING ALPHANUMERIC AND NUMERIC
           MOVE 'INIT-REPLACING' TO ZCVS-TEST-NAME.
           INITIALIZE WS-INIT-GROUP
             REPLACING ALPHANUMERIC DATA BY 'ZZZZ'
                       NUMERIC DATA BY 42.
           IF WS-IG-ALNUM = 'ZZZZ'
             AND WS-IG-DISP = 42
             AND WS-IG-PACK = 42
             AND WS-IG-BIN = 42
             AND WS-INIT-RAW(18:2) = '##'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ZZZZ/42/##' TO ZCVS-EXPECTED
               MOVE WS-IG-ALNUM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

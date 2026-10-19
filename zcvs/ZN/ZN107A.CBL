       CBL NSYMBOL(NATIONAL)
      * SPEC: REFERENCE MODIFICATION/USAGE NATIONAL; AUDIT N07.
      * NATIONAL POSITIONS ARE UTF-16 UNITS; RAW BYTES ARE SEPARATE.
      * ZN107A: National character boundaries and byte observations
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN107A.
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
       01  WS-N PIC N(4) USAGE NATIONAL.
       01  WS-RAW REDEFINES WS-N PIC X(8).
       01  WS-SHORT PIC N(2) USAGE NATIONAL.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN107A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'NAT-PAD' TO ZCVS-TEST-NAME.
           MOVE N'AB' TO WS-N.
           MOVE WS-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0041004200200020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0041004200200020' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-TRUNC' TO ZCVS-TEST-NAME.
           MOVE N'ABCDE' TO WS-N.
           MOVE WS-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0041004200430044'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0041004200430044' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-LOW' TO ZCVS-TEST-NAME.
           MOVE NX'0000000000000000' TO WS-N.
           MOVE WS-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0000000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-FIRST' TO ZCVS-TEST-NAME.
           MOVE N'ABCD' TO WS-N.
           IF WS-N(1:1) = N'A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-LAST' TO ZCVS-TEST-NAME.
           IF WS-N(4:1) = N'D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-TAIL' TO ZCVS-TEST-NAME.
           MOVE WS-N(3:) TO WS-SHORT.
           IF WS-SHORT = N'CD'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'CD' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-REPLACE' TO ZCVS-TEST-NAME.
           MOVE N'XY' TO WS-N(2:2).
           IF WS-N = N'AXYD'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AXYD' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-INIT' TO ZCVS-TEST-NAME.
           INITIALIZE WS-N.
           IF WS-N = NX'0020002000200020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NATIONAL SPACES' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-SURR-UNITS' TO ZCVS-TEST-NAME.
           MOVE NX'D869DEB200410042' TO WS-N.
           IF WS-N(1:2) = NX'D869DEB2'
             AND WS-N(3:1) = N'A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'PAIR THEN A' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-CHAR-BYTE-LEN' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(WS-N) = 4
             AND FUNCTION BYTE-LENGTH(WS-N) = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4/8' TO ZCVS-EXPECTED
               MOVE WS-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

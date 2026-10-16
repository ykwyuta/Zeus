       CBL NSYMBOL(NATIONAL)
      *================================================================*
      * ZN101A - USAGE NATIONAL (UTF-16BE), GROUP-USAGE NATIONAL       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN101A.
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
       01  WS-NAT-FIELDS.
           05  WS-N4                PIC N(4) USAGE NATIONAL.
           05  WS-N4-RAW REDEFINES WS-N4 PIC X(8).
           05  WS-N2                PIC N(2) USAGE NATIONAL.
           05  WS-N2-RAW REDEFINES WS-N2 PIC X(4).
           05  WS-NGROUP            GROUP-USAGE NATIONAL.
               10  WS-NG-CODE       PIC N(2).
               10  WS-NG-NUM        PIC 9(2).
           05  WS-NGROUP-RAW REDEFINES WS-NGROUP PIC X(8).
           05  WS-CHAR-LEN          PIC 9(4) COMP.
           05  WS-BYTE-LEN          PIC 9(4) COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: N'AB' IN PIC N(4) PADS WITH UTF-16BE SPACE (0020)
           MOVE 'NAT-UTF16-PAD' TO ZCVS-TEST-NAME.
           MOVE N'AB' TO WS-N4.
           MOVE WS-N4-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0041004200200020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0041004200200020' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: NX'30423044' LITERAL (HIRAGANA A, I IN UTF-16BE)
           MOVE 'NAT-NX-LITERAL' TO ZCVS-TEST-NAME.
           MOVE NX'30423044' TO WS-N2.
           MOVE WS-N2-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '30423044'
             AND WS-N2 = N'あい'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '30423044' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: MOVE SPACES TO PIC N(4) -> 0020002000200020
           MOVE 'NAT-SPACES-HEX' TO ZCVS-TEST-NAME.
           MOVE SPACES TO WS-N4.
           MOVE WS-N4-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0020002000200020'
             AND WS-N4 = SPACES
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0020002000200020' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: FUNCTION LENGTH (4 CHARS) VS BYTE-LENGTH (8 BYTES)
           MOVE 'NAT-LEN-BYTELEN' TO ZCVS-TEST-NAME.
           COMPUTE WS-CHAR-LEN = FUNCTION LENGTH(WS-N4).
           COMPUTE WS-BYTE-LEN = FUNCTION BYTE-LENGTH(WS-N4).
           IF WS-CHAR-LEN = 4 AND WS-BYTE-LEN = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4/8' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: CHARACTER REFERENCE MODIFICATION WS-N4(2:2)
           MOVE 'NAT-REFMOD-CHAR' TO ZCVS-TEST-NAME.
           MOVE N'WXYZ' TO WS-N4.
           MOVE WS-N4(2:2) TO WS-N2.
           IF WS-N2 = N'XY'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'XY' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: GROUP-USAGE NATIONAL STORES ALL ITEMS IN UTF-16BE
           MOVE 'NAT-GROUP-USAGE' TO ZCVS-TEST-NAME.
           MOVE N'AB12' TO WS-NGROUP.
           MOVE WS-NGROUP-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-NG-CODE = N'AB'
             AND WS-NG-NUM = 12
             AND ZCVS-HEX-OUT(1:16) = '0041004200310032'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0041004200310032' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: MOVE ALPHANUMERIC 'CD' TO PIC N(4) CONVERTS TO UTF-16BE
           MOVE 'NAT-FROM-ALNUM' TO ZCVS-TEST-NAME.
           MOVE 'CD' TO WS-N4.
           MOVE WS-N4-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '0043004400200020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0043004400200020' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

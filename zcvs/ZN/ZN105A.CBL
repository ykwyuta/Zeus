       CBL NSYMBOL(NATIONAL)
      *================================================================*
      * ZN105A - UTF-16BE SURROGATES, COMPARISONS, NATIONAL INITIALIZE *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN105A.
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
       01  WS-UTF16-FIELDS.
           05  WS-SURR              PIC N(2) USAGE NATIONAL.
           05  WS-SURR-RAW REDEFINES WS-SURR PIC X(4).
           05  WS-NA                PIC N(3) USAGE NATIONAL.
           05  WS-NB                PIC N(3) USAGE NATIONAL.
           05  WS-MIX-INIT.
               10  WS-MI-X          PIC X(2) VALUE 'AB'.
               10  WS-MI-N          PIC N(2) USAGE NATIONAL
                                    VALUE N'CD'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN105A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: UTF-16BE SURROGATE PAIR (U+2A6B2 = D869 DEB2)
           MOVE 'UTF16-SURROGATE' TO ZCVS-TEST-NAME.
           MOVE NX'D869DEB2' TO WS-SURR.
           MOVE WS-SURR-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'D869DEB2'
             AND FUNCTION LENGTH(WS-SURR) = 2
             AND FUNCTION BYTE-LENGTH(WS-SURR) = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D869DEB2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: NATIONAL RELATIONAL COMPARISON PADS WITH U+0020
           MOVE 'UTF16-CMP-PAD' TO ZCVS-TEST-NAME.
           MOVE N'AB' TO WS-NA.
           MOVE NX'004100420020' TO WS-NB.
           IF WS-NA = WS-NB AND WS-NA > N'AA' AND WS-NA < N'AC'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'EQUAL/ORD' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: INITIALIZE REPLACING NATIONAL DATA DOES NOT TOUCH PIC X
           MOVE 'UTF16-INIT-REPL' TO ZCVS-TEST-NAME.
           INITIALIZE WS-MIX-INIT
             REPLACING NATIONAL DATA BY N'ZZ'.
           IF WS-MI-X = 'AB' AND WS-MI-N = N'ZZ'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB/ZZ' TO ZCVS-EXPECTED
               MOVE WS-MI-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: INITIALIZE DEFAULT SETS NATIONAL AND PIC X SPACES
           MOVE 'UTF16-INIT-DEF' TO ZCVS-TEST-NAME.
           INITIALIZE WS-MIX-INIT.
           IF WS-MI-X = SPACES AND WS-MI-N = SPACES
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPACES' TO ZCVS-EXPECTED
               MOVE 'NON-SPACE' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

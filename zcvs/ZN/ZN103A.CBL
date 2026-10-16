      *================================================================*
      * ZN103A - DBCS (PIC G DISPLAY-1), EGCS, AND SO/SI FRAMING       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN103A.
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
       01  WS-DBCS-REC.
           05  WS-SBCS-TAG          PIC X(1) VALUE 'A'.
           05  WS-G2                PIC G(2) DISPLAY-1.
           05  WS-G2-RAW REDEFINES WS-G2 PIC X(4).
           05  WS-NAT2              PIC N(2) USAGE NATIONAL.
           05  WS-NAT2-RAW REDEFINES WS-NAT2 PIC X(4).
       01  WS-MIXED-6               PIC X(6).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN103A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: G'日' IN PIC G(2) -> 45624040 (NO SO/SI IN PURE DBCS)
           MOVE 'DBCS-G-NO-SOSI' TO ZCVS-TEST-NAME.
           MOVE G'日' TO WS-G2.
           MOVE WS-G2-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '45624040'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '45624040' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: MIXED LITERAL 'A日B' HAS SO (0E) AND SI (0F)
           MOVE 'MIXED-SOSI-HEX' TO ZCVS-TEST-NAME.
           MOVE 'A日B' TO WS-MIXED-6.
           MOVE WS-MIXED-6 TO ZCVS-HEX-IN.
           MOVE 6 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:12) = 'C10E45620FC2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C10E45620FC2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:12) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: MOVE PIC G(2) TO PIC N(2) NATIONAL CONVERTS TO UTF-16BE
           MOVE 'DBCS-TO-NAT' TO ZCVS-TEST-NAME.
           MOVE G'日' TO WS-G2.
           MOVE WS-G2 TO WS-NAT2.
           MOVE WS-NAT2-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '65E53000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '65E53000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: INITIALIZE REPLACING DBCS AND EGCS BY SPACE
           MOVE 'DBCS-EGCS-INIT' TO ZCVS-TEST-NAME.
           MOVE X'45624562' TO WS-G2-RAW.
           INITIALIZE WS-DBCS-REC REPLACING DBCS DATA BY SPACE.
           IF WS-G2-RAW = X'40404040' AND WS-SBCS-TAG = 'A'
               MOVE X'45624562' TO WS-G2-RAW
               INITIALIZE WS-DBCS-REC REPLACING EGCS DATA BY SPACE
               IF WS-G2-RAW = X'40404040' AND WS-SBCS-TAG = 'A'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE 'EGCS-40404040' TO ZCVS-EXPECTED
                   MOVE 'FAIL-EGCS' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'DBCS-40404040' TO ZCVS-EXPECTED
               MOVE 'FAIL-DBCS' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

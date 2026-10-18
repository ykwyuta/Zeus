      *================================================================*
      * ZN104A - FUNCTION NATIONAL-OF AND FUNCTION DISPLAY-OF          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN104A.
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
       01  WS-CONV-FIELDS.
           05  WS-N2                PIC N(2) USAGE NATIONAL.
           05  WS-N2-RAW REDEFINES WS-N2 PIC X(4).
           05  WS-N3                PIC N(3) USAGE NATIONAL.
           05  WS-N3-RAW REDEFINES WS-N3 PIC X(6).
           05  WS-G2                PIC G(2) DISPLAY-1.
           05  WS-X4                PIC X(4).
           05  WS-X6                PIC X(6).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN104A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: NATIONAL-OF('日本') IN CCSID 939 -> UTF-16BE 65E5672C
           MOVE 'NATOF-MIXED-939' TO ZCVS-TEST-NAME.
           MOVE FUNCTION NATIONAL-OF('日本') TO WS-N2.
           MOVE WS-N2-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '65E5672C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '65E5672C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: NATIONAL-OF(PIC G) CONVERTS DBCS TO UTF-16BE
           MOVE 'NATOF-DBCS-G' TO ZCVS-TEST-NAME.
           MOVE G'日本' TO WS-G2.
           MOVE FUNCTION NATIONAL-OF(WS-G2) TO WS-N2.
           MOVE WS-N2-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '65E5672C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '65E5672C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: DISPLAY-OF(N'日') IN CCSID 939 -> 0E45620F
           MOVE 'DISPOF-939-SOSI' TO ZCVS-TEST-NAME.
           MOVE N'日' TO WS-N2.
           MOVE FUNCTION DISPLAY-OF(WS-N2(1:1)) TO WS-X4.
           MOVE WS-X4 TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '0E45620F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0E45620F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: EXPLICIT CCSID 37 IN DISPLAY-OF / NATIONAL-OF
           MOVE 'DISPOF-CCSID-37' TO ZCVS-TEST-NAME.
           MOVE N'ABC' TO WS-N3.
           MOVE FUNCTION DISPLAY-OF(WS-N3, 37) TO WS-X4.
           MOVE WS-X4 TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'C1C2C340'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1C2C340' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: ROUND-TRIP DISPLAY-OF -> NATIONAL-OF WITH CCSID 1399
           MOVE 'RT-CCSID-1399' TO ZCVS-TEST-NAME.
           MOVE N'東京' TO WS-N2.
           MOVE FUNCTION DISPLAY-OF(WS-N2, 1399) TO WS-X6.
           MOVE FUNCTION NATIONAL-OF(WS-X6, 1399) TO WS-N3.
           IF WS-N3(1:2) = N'東京'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TOKYO' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

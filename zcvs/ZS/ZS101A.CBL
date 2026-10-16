      *================================================================*
      * ZS101A - IBM SPECIAL REGISTERS AND DATE/WINDOWING FUNCTIONS    *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS101A.
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
       01  WS-SR-FIELDS.
           05  WS-WC-REG            PIC X(16).
           05  WS-WC-FUNC           PIC X(21).
           05  WS-CUR-DATE          PIC X(21).
           05  WS-CUR-YEAR          PIC 9(4).
           05  WS-WINDOW            PIC S9(5) COMP.
           05  WS-INT-DATE          PIC 9(8) COMP.
           05  WS-GREG-OUT          PIC 9(8).
           05  WS-JUL-OUT           PIC 9(7).
           05  WS-YYYY-OUT          PIC 9(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZS101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: TALLY AND RETURN-CODE SPECIAL REGISTERS
           MOVE 'SR-TALLY-RC' TO ZCVS-TEST-NAME.
           MOVE 12345 TO TALLY.
           MOVE 8 TO RETURN-CODE.
           IF TALLY = 12345 AND RETURN-CODE = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345/8' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           MOVE 0 TO RETURN-CODE.

      * TEST 02: WHEN-COMPILED REGISTER (MM/DD/YYHH.MM.SS) VS FUNCTION
           MOVE 'SR-WHEN-COMPILED' TO ZCVS-TEST-NAME.
           MOVE WHEN-COMPILED TO WS-WC-REG.
           MOVE FUNCTION WHEN-COMPILED TO WS-WC-FUNC.
           IF WS-WC-REG(3:1) = '/'
             AND WS-WC-REG(6:1) = '/'
             AND WS-WC-REG(11:1) = '.'
             AND WS-WC-FUNC(1:4) IS NUMERIC
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'MM/DD/YYHH.MM.SS' TO ZCVS-EXPECTED
               MOVE WS-WC-REG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: INTEGER-OF-DATE / DATE-OF-INTEGER / DAY-OF-INTEGER
           MOVE 'FN-INT-DATE-DAY' TO ZCVS-TEST-NAME.
           COMPUTE WS-INT-DATE = FUNCTION INTEGER-OF-DATE(20261016).
           COMPUTE WS-GREG-OUT = FUNCTION DATE-OF-INTEGER(WS-INT-DATE).
           COMPUTE WS-JUL-OUT = FUNCTION DAY-OF-INTEGER(WS-INT-DATE).
           IF WS-GREG-OUT = 20261016 AND WS-JUL-OUT = 2026289
               PERFORM ZCVS-PASS
           ELSE
               MOVE '20261016/2026289' TO ZCVS-EXPECTED
               MOVE WS-GREG-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: YEAR-TO-YYYY, DATE-TO-YYYYMMDD, DAY-TO-YYYYDDD
           MOVE 'FN-Y2K-WINDOW' TO ZCVS-TEST-NAME.
           MOVE FUNCTION CURRENT-DATE TO WS-CUR-DATE.
           MOVE WS-CUR-DATE(1:4) TO WS-CUR-YEAR.
           COMPUTE WS-WINDOW = 2050 - WS-CUR-YEAR.
           COMPUTE WS-YYYY-OUT = FUNCTION YEAR-TO-YYYY(26 WS-WINDOW).
           COMPUTE WS-GREG-OUT =
             FUNCTION DATE-TO-YYYYMMDD(981016 WS-WINDOW).
           COMPUTE WS-JUL-OUT =
             FUNCTION DAY-TO-YYYYDDD(98289 WS-WINDOW).
           IF WS-YYYY-OUT = 2026
             AND WS-GREG-OUT = 19981016
             AND WS-JUL-OUT = 1998289
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2026/19981016' TO ZCVS-EXPECTED
               MOVE WS-GREG-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 0 TO RETURN-CODE.
           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

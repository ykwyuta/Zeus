      *================================================================*
      * ZA105A - ON SIZE ERROR, NOT ON SIZE ERROR, AND ROUNDED         *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA105A.
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
       01  WS-ARITH-FIELDS.
           05  WS-P3                PIC S9(3) COMP-3.
           05  WS-DEC2              PIC S9(3)V99 COMP-3.
           05  WS-DEC2-T            PIC S9(3)V99 COMP-3.
           05  WS-QUOT              PIC S9(3) COMP-3.
           05  WS-REM               PIC S9(3) COMP-3.
           05  WS-ZERO              PIC S9(3) COMP-3 VALUE 0.
           05  WS-FLAG              PIC X(1).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA105A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: POSITIVE ROUNDED HALF-UP (12.345 -> 12.35)
           MOVE 'RND-POS-HALFUP' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-DEC2.
           MOVE 0 TO WS-DEC2-T.
           ADD 12.345 TO WS-DEC2 ROUNDED.
           ADD 12.345 TO WS-DEC2-T.
           IF WS-DEC2 = 12.35 AND WS-DEC2-T = 12.34
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.35/12.34' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: NEGATIVE ROUNDED AWAY FROM ZERO (-12.345 -> -12.35)
           MOVE 'RND-NEG-HALFUP' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-DEC2.
           MOVE 0 TO WS-DEC2-T.
           ADD -12.345 TO WS-DEC2 ROUNDED.
           ADD -12.345 TO WS-DEC2-T.
           IF WS-DEC2 = -12.35 AND WS-DEC2-T = -12.34
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-12.35/-12.34' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: ADD OVERFLOW TRIGGERS SIZE ERROR & PRESERVES TARGET
           MOVE 'SZERR-ADD-OVF' TO ZCVS-TEST-NAME.
           MOVE 950 TO WS-P3.
           MOVE 'N' TO WS-FLAG.
           ADD 100 TO WS-P3
               ON SIZE ERROR
                   MOVE 'Y' TO WS-FLAG
               NOT ON SIZE ERROR
                   MOVE 'X' TO WS-FLAG
           END-ADD.
           IF WS-FLAG = 'Y' AND WS-P3 = 950
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/950' TO ZCVS-EXPECTED
               MOVE WS-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: ADD WITHOUT OVERFLOW TRIGGERS NOT ON SIZE ERROR
           MOVE 'NOSZERR-ADD-OK' TO ZCVS-TEST-NAME.
           MOVE 800 TO WS-P3.
           MOVE 'N' TO WS-FLAG.
           ADD 150 TO WS-P3
               ON SIZE ERROR
                   MOVE 'E' TO WS-FLAG
               NOT ON SIZE ERROR
                   MOVE 'K' TO WS-FLAG
           END-ADD.
           IF WS-FLAG = 'K' AND WS-P3 = 950
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'K/950' TO ZCVS-EXPECTED
               MOVE WS-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: DIVIDE BY ZERO TRIGGERS ON SIZE ERROR
           MOVE 'SZERR-DIV-ZERO' TO ZCVS-TEST-NAME.
           MOVE 123 TO WS-QUOT.
           MOVE 'N' TO WS-FLAG.
           DIVIDE 100 BY WS-ZERO GIVING WS-QUOT
               ON SIZE ERROR
                   MOVE 'Z' TO WS-FLAG
           END-DIVIDE.
           IF WS-FLAG = 'Z' AND WS-QUOT = 123
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Z/123' TO ZCVS-EXPECTED
               MOVE WS-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: ROUNDED CARRY CAUSING OVERFLOW (999.995 -> 1000.00)
           MOVE 'SZERR-RND-CARRY' TO ZCVS-TEST-NAME.
           MOVE 999.99 TO WS-DEC2.
           MOVE 'N' TO WS-FLAG.
           COMPUTE WS-DEC2 ROUNDED = WS-DEC2 + 0.005
               ON SIZE ERROR
                   MOVE 'C' TO WS-FLAG
           END-COMPUTE.
           IF WS-FLAG = 'C' AND WS-DEC2 = 999.99
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C/999.99' TO ZCVS-EXPECTED
               MOVE WS-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

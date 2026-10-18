      *================================================================*
      * ZA106A - IS NUMERIC CLASS TEST AND SIGN CONDITIONS             *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA106A.
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
       01  WS-NUMTEST-FIELDS.
           05  WS-SP5-RAW           PIC X(3).
           05  WS-SP5 REDEFINES WS-SP5-RAW PIC S9(5) COMP-3.
           05  WS-UP5-RAW           PIC X(3).
           05  WS-UP5 REDEFINES WS-UP5-RAW PIC 9(5) COMP-3.
           05  WS-SZ4-RAW           PIC X(4).
           05  WS-SZ4 REDEFINES WS-SZ4-RAW PIC S9(4).
           05  WS-UZ4-RAW           PIC X(4).
           05  WS-UZ4 REDEFINES WS-UZ4-RAW PIC 9(4).
           05  WS-SEP-RAW           PIC X(5).
           05  WS-SEP REDEFINES WS-SEP-RAW
                                    PIC S9(4) SIGN TRAILING SEPARATE.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA106A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: SIGNED COMP-3 WITH C/D/F SIGN NIBBLES IS NUMERIC
           MOVE 'NUM-SP5-VALID' TO ZCVS-TEST-NAME.
           MOVE X'12345C' TO WS-SP5-RAW.
           IF WS-SP5 IS NUMERIC
               MOVE X'12345D' TO WS-SP5-RAW
               IF WS-SP5 IS NUMERIC
                   MOVE X'12345F' TO WS-SP5-RAW
                   IF WS-SP5 IS NUMERIC
                       PERFORM ZCVS-PASS
                   ELSE
                       MOVE 'NUMERIC-F' TO ZCVS-EXPECTED
                       MOVE 'NOT-NUM' TO ZCVS-ACTUAL
                       PERFORM ZCVS-FAIL
                   END-IF
               ELSE
                   MOVE 'NUMERIC-D' TO ZCVS-EXPECTED
                   MOVE 'NOT-NUM' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'NUMERIC-C' TO ZCVS-EXPECTED
               MOVE 'NOT-NUM' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: SIGNED COMP-3 WITH INVALID SIGN OR DIGIT NIBBLE
           MOVE 'NUM-SP5-INVALID' TO ZCVS-TEST-NAME.
           MOVE X'123450' TO WS-SP5-RAW.
           IF WS-SP5 IS NOT NUMERIC
               MOVE X'12A45C' TO WS-SP5-RAW
               IF WS-SP5 IS NOT NUMERIC
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE 'NOT-NUM-DIG' TO ZCVS-EXPECTED
                   MOVE 'NUMERIC' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'NOT-NUM-SGN' TO ZCVS-EXPECTED
               MOVE 'NUMERIC' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: UNSIGNED COMP-3 REQUIRES F SIGN NIBBLE
           MOVE 'NUM-UP5-SIGN' TO ZCVS-TEST-NAME.
           MOVE X'12345F' TO WS-UP5-RAW.
           IF WS-UP5 IS NUMERIC
               MOVE X'12345C' TO WS-UP5-RAW
               IF WS-UP5 IS NOT NUMERIC
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE 'NOT-NUM-C' TO ZCVS-EXPECTED
                   MOVE 'NUMERIC' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'NUMERIC-F' TO ZCVS-EXPECTED
               MOVE 'NOT-NUM' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: SIGNED ZONED DECIMAL VALID OVERPUNCH VS SPACES
           MOVE 'NUM-SZ4-CHECK' TO ZCVS-TEST-NAME.
           MOVE '123D' TO WS-SZ4-RAW.
           IF WS-SZ4 IS NUMERIC
               MOVE '123M' TO WS-SZ4-RAW
               IF WS-SZ4 IS NUMERIC
                   MOVE SPACES TO WS-SZ4-RAW
                   IF WS-SZ4 IS NOT NUMERIC
                       PERFORM ZCVS-PASS
                   ELSE
                       MOVE 'SPACES-NOT-NUM' TO ZCVS-EXPECTED
                       MOVE 'NUMERIC' TO ZCVS-ACTUAL
                       PERFORM ZCVS-FAIL
                   END-IF
               ELSE
                   MOVE '123M-NUMERIC' TO ZCVS-EXPECTED
                   MOVE 'NOT-NUM' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '123D-NUMERIC' TO ZCVS-EXPECTED
               MOVE 'NOT-NUM' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: UNSIGNED ZONED DECIMAL REJECTS SIGNED OVERPUNCH
           MOVE 'NUM-UZ4-CHECK' TO ZCVS-TEST-NAME.
           MOVE '1234' TO WS-UZ4-RAW.
           IF WS-UZ4 IS NUMERIC
               MOVE '123D' TO WS-UZ4-RAW
               IF WS-UZ4 IS NOT NUMERIC
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '123D-NOT-NUM' TO ZCVS-EXPECTED
                   MOVE 'NUMERIC' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '1234-NUMERIC' TO ZCVS-EXPECTED
               MOVE 'NOT-NUM' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: SEPARATE SIGN ZONED DECIMAL NUMERIC CHECK
           MOVE 'NUM-SEP-CHECK' TO ZCVS-TEST-NAME.
           MOVE '1234-' TO WS-SEP-RAW.
           IF WS-SEP IS NUMERIC
               MOVE '1234 ' TO WS-SEP-RAW
               IF WS-SEP IS NOT NUMERIC
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE 'SPACE-NOT-NUM' TO ZCVS-EXPECTED
                   MOVE 'NUMERIC' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '1234--NUMERIC' TO ZCVS-EXPECTED
               MOVE 'NOT-NUM' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: IS POSITIVE / IS NEGATIVE / IS ZERO CONDITIONS
           MOVE 'SIGN-COND-CHECK' TO ZCVS-TEST-NAME.
           MOVE +42 TO WS-SP5.
           IF WS-SP5 IS POSITIVE
               MOVE -42 TO WS-SP5
               IF WS-SP5 IS NEGATIVE
                   MOVE 0 TO WS-SP5
                   IF WS-SP5 IS ZERO
                       PERFORM ZCVS-PASS
                   ELSE
                       MOVE 'ZERO' TO ZCVS-EXPECTED
                       MOVE 'NONZERO' TO ZCVS-ACTUAL
                       PERFORM ZCVS-FAIL
                   END-IF
               ELSE
                   MOVE 'NEGATIVE' TO ZCVS-EXPECTED
                   MOVE 'NOT-NEG' TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE 'POSITIVE' TO ZCVS-EXPECTED
               MOVE 'NOT-POS' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

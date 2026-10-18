      *================================================================*
      * ZD101A - PACKED-DECIMAL (COMP-3) INTERNAL BYTE REPRESENTATION  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD101A.
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
       01  WS-PACK-GROUP.
           05  WS-UP5               PIC 9(5) COMP-3.
           05  WS-UP5-RAW REDEFINES WS-UP5 PIC X(3).
           05  WS-SP5               PIC S9(5) COMP-3.
           05  WS-SP5-RAW REDEFINES WS-SP5 PIC X(3).
           05  WS-SP4               PIC S9(4) COMP-3.
           05  WS-SP4-RAW REDEFINES WS-SP4 PIC X(3).
           05  WS-DEC               PIC S9(3)V99 COMP-3.
           05  WS-DEC-RAW REDEFINES WS-DEC PIC X(3).
           05  WS-SP18              PIC S9(18) COMP-3.
           05  WS-SP18-RAW REDEFINES WS-SP18 PIC X(10).
           05  WS-SRC-RAW           PIC X(3).
           05  WS-SRC-P5 REDEFINES WS-SRC-RAW PIC S9(5) COMP-3.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: UNSIGNED PIC 9(5) COMP-3 -> SIGN NIBBLE F
           MOVE 'UP5-12345-HEX' TO ZCVS-TEST-NAME.
           MOVE 12345 TO WS-UP5.
           MOVE WS-UP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: UNSIGNED PIC 9(5) COMP-3 ZERO -> 00000F
           MOVE 'UP5-ZERO-HEX' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-UP5.
           MOVE WS-UP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '00000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: UNSIGNED PIC 9(5) COMP-3 MAX -> 99999F
           MOVE 'UP5-MAX-HEX' TO ZCVS-TEST-NAME.
           MOVE 99999 TO WS-UP5.
           MOVE WS-UP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '99999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: SIGNED PIC S9(5) COMP-3 POSITIVE -> SIGN NIBBLE C
           MOVE 'SP5-POS-HEX' TO ZCVS-TEST-NAME.
           MOVE +12345 TO WS-SP5.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: SIGNED PIC S9(5) COMP-3 NEGATIVE -> SIGN NIBBLE D
           MOVE 'SP5-NEG-HEX' TO ZCVS-TEST-NAME.
           MOVE -12345 TO WS-SP5.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: SIGNED PIC S9(5) COMP-3 ZERO -> 00000C
           MOVE 'SP5-ZERO-HEX' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-SP5.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '00000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: SIGNED PIC S9(5) COMP-3 MAX -> 99999C
           MOVE 'SP5-MAX-HEX' TO ZCVS-TEST-NAME.
           MOVE +99999 TO WS-SP5.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '99999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: SIGNED PIC S9(5) COMP-3 MIN -> 99999D
           MOVE 'SP5-MIN-HEX' TO ZCVS-TEST-NAME.
           MOVE -99999 TO WS-SP5.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '99999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 09: EVEN DIGITS PIC S9(4) COMP-3 POSITIVE -> 01234C
           MOVE 'SP4-EVEN-POS' TO ZCVS-TEST-NAME.
           MOVE +1234 TO WS-SP4.
           MOVE WS-SP4-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '01234C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '01234C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 10: EVEN DIGITS PIC S9(4) COMP-3 NEGATIVE -> 01234D
           MOVE 'SP4-EVEN-NEG' TO ZCVS-TEST-NAME.
           MOVE -1234 TO WS-SP4.
           MOVE WS-SP4-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '01234D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '01234D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 11: IMPLIED DECIMAL PIC S9(3)V99 COMP-3 POSITIVE
           MOVE 'DEC-POS-HEX' TO ZCVS-TEST-NAME.
           MOVE +123.45 TO WS-DEC.
           MOVE WS-DEC-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 12: IMPLIED DECIMAL PIC S9(3)V99 COMP-3 NEGATIVE
           MOVE 'DEC-NEG-HEX' TO ZCVS-TEST-NAME.
           MOVE -123.45 TO WS-DEC.
           MOVE WS-DEC-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 13: 18-DIGIT PACKED DECIMAL PIC S9(18) COMP-3 (10 BYTES)
           MOVE 'SP18-POS-HEX' TO ZCVS-TEST-NAME.
           MOVE +123456789012345678 TO WS-SP18.
           MOVE WS-SP18-RAW TO ZCVS-HEX-IN.
           MOVE 10 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:20) = '0123456789012345678C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0123456789012345678C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 14: SIGN NORMALIZATION FROM NIBBLE A -> C
           MOVE 'NORM-SIGN-A' TO ZCVS-TEST-NAME.
           MOVE X'12345A' TO WS-SRC-RAW.
           COMPUTE WS-SP5 = WS-SRC-P5 + 0.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 15: SIGN NORMALIZATION FROM NIBBLE E -> C
           MOVE 'NORM-SIGN-E' TO ZCVS-TEST-NAME.
           MOVE X'12345E' TO WS-SRC-RAW.
           COMPUTE WS-SP5 = WS-SRC-P5 + 0.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 16: SIGN NORMALIZATION FROM NIBBLE F -> C
           MOVE 'NORM-SIGN-F' TO ZCVS-TEST-NAME.
           MOVE X'12345F' TO WS-SRC-RAW.
           COMPUTE WS-SP5 = WS-SRC-P5 + 0.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 17: SIGN NORMALIZATION FROM NIBBLE B -> D
           MOVE 'NORM-SIGN-B' TO ZCVS-TEST-NAME.
           MOVE X'12345B' TO WS-SRC-RAW.
           COMPUTE WS-SP5 = WS-SRC-P5 + 0.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '12345D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12345D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 18: NEGATIVE ZERO X'00000D' NORMALIZATION -> 00000C
           MOVE 'NORM-NEG-ZERO' TO ZCVS-TEST-NAME.
           MOVE X'00000D' TO WS-SRC-RAW.
           COMPUTE WS-SP5 = WS-SRC-P5 + 0.
           MOVE WS-SP5-RAW TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '00000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

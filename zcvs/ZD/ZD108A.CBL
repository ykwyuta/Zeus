       CBL ARITH(EXTEND)
      * SPEC: COMPUTATIONAL ITEMS; AUDIT SCENARIOS.md D04.
      * EXTEND ALLOWS 19/20-DIGIT BINARY CAPACITY LITERALS.
      * HEX EXPECTATIONS ARE TWO'S COMPLEMENT, BIG ENDIAN.
      * ZD108A: COMP-5 signed/unsigned 2/4/8 byte limits
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD108A.
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
       01  B2S PIC S9(4) COMP-5.
       01  B2S-RAW REDEFINES B2S PIC X(2).
       01  B2U PIC 9(4) COMP-5.
       01  B2U-RAW REDEFINES B2U PIC X(2).
       01  B4S PIC S9(9) COMP-5.
       01  B4S-RAW REDEFINES B4S PIC X(4).
       01  B4U PIC 9(9) COMP-5.
       01  B4U-RAW REDEFINES B4U PIC X(4).
       01  B8S PIC S9(18) COMP-5.
       01  B8S-RAW REDEFINES B8S PIC X(8).
       01  B8U PIC 9(18) COMP-5.
       01  B8U-RAW REDEFINES B8U PIC X(8).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD108A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'B2S-BOUND-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = 0
             AND ZCVS-HEX-OUT(1:4) = '0000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = 1
             AND ZCVS-HEX-OUT(1:4) = '0001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-2' TO ZCVS-TEST-NAME.
           MOVE 255 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = 255
             AND ZCVS-HEX-OUT(1:4) = '00FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-3' TO ZCVS-TEST-NAME.
           MOVE 256 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = 256
             AND ZCVS-HEX-OUT(1:4) = '0100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-4' TO ZCVS-TEST-NAME.
           MOVE 32767 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = 32767
             AND ZCVS-HEX-OUT(1:4) = '7FFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7FFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-5' TO ZCVS-TEST-NAME.
           MOVE -1 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = -1
             AND ZCVS-HEX-OUT(1:4) = 'FFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-6' TO ZCVS-TEST-NAME.
           MOVE -256 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = -256
             AND ZCVS-HEX-OUT(1:4) = 'FF00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FF00' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2S-BOUND-7' TO ZCVS-TEST-NAME.
           MOVE -32768 TO B2S.
           MOVE B2S-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2S = -32768
             AND ZCVS-HEX-OUT(1:4) = '8000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2U-BOUND-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO B2U.
           MOVE B2U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2U = 0
             AND ZCVS-HEX-OUT(1:4) = '0000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2U-BOUND-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO B2U.
           MOVE B2U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2U = 1
             AND ZCVS-HEX-OUT(1:4) = '0001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2U-BOUND-2' TO ZCVS-TEST-NAME.
           MOVE 255 TO B2U.
           MOVE B2U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2U = 255
             AND ZCVS-HEX-OUT(1:4) = '00FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2U-BOUND-3' TO ZCVS-TEST-NAME.
           MOVE 256 TO B2U.
           MOVE B2U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2U = 256
             AND ZCVS-HEX-OUT(1:4) = '0100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B2U-BOUND-4' TO ZCVS-TEST-NAME.
           MOVE 65535 TO B2U.
           MOVE B2U-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B2U = 65535
             AND ZCVS-HEX-OUT(1:4) = 'FFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = 0
             AND ZCVS-HEX-OUT(1:8) = '00000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = 1
             AND ZCVS-HEX-OUT(1:8) = '00000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-2' TO ZCVS-TEST-NAME.
           MOVE 255 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = 255
             AND ZCVS-HEX-OUT(1:8) = '000000FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-3' TO ZCVS-TEST-NAME.
           MOVE 256 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = 256
             AND ZCVS-HEX-OUT(1:8) = '00000100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-4' TO ZCVS-TEST-NAME.
           MOVE 2147483647 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = 2147483647
             AND ZCVS-HEX-OUT(1:8) = '7FFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7FFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-5' TO ZCVS-TEST-NAME.
           MOVE -1 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = -1
             AND ZCVS-HEX-OUT(1:8) = 'FFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-6' TO ZCVS-TEST-NAME.
           MOVE -256 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = -256
             AND ZCVS-HEX-OUT(1:8) = 'FFFFFF00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFF00' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4S-BOUND-7' TO ZCVS-TEST-NAME.
           MOVE -2147483648 TO B4S.
           MOVE B4S-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4S = -2147483648
             AND ZCVS-HEX-OUT(1:8) = '80000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '80000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4U-BOUND-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO B4U.
           MOVE B4U-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4U = 0
             AND ZCVS-HEX-OUT(1:8) = '00000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4U-BOUND-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO B4U.
           MOVE B4U-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4U = 1
             AND ZCVS-HEX-OUT(1:8) = '00000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4U-BOUND-2' TO ZCVS-TEST-NAME.
           MOVE 255 TO B4U.
           MOVE B4U-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4U = 255
             AND ZCVS-HEX-OUT(1:8) = '000000FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4U-BOUND-3' TO ZCVS-TEST-NAME.
           MOVE 256 TO B4U.
           MOVE B4U-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4U = 256
             AND ZCVS-HEX-OUT(1:8) = '00000100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B4U-BOUND-4' TO ZCVS-TEST-NAME.
           MOVE 4294967295 TO B4U.
           MOVE B4U-RAW TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B4U = 4294967295
             AND ZCVS-HEX-OUT(1:8) = 'FFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = 0
             AND ZCVS-HEX-OUT(1:16) = '0000000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = 1
             AND ZCVS-HEX-OUT(1:16) = '0000000000000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-2' TO ZCVS-TEST-NAME.
           MOVE 255 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = 255
             AND ZCVS-HEX-OUT(1:16) = '00000000000000FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-3' TO ZCVS-TEST-NAME.
           MOVE 256 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = 256
             AND ZCVS-HEX-OUT(1:16) = '0000000000000100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-4' TO ZCVS-TEST-NAME.
           MOVE 9223372036854775807 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = 9223372036854775807
             AND ZCVS-HEX-OUT(1:16) = '7FFFFFFFFFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7FFFFFFFFFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-5' TO ZCVS-TEST-NAME.
           MOVE -1 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = -1
             AND ZCVS-HEX-OUT(1:16) = 'FFFFFFFFFFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFFFFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-6' TO ZCVS-TEST-NAME.
           MOVE -256 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = -256
             AND ZCVS-HEX-OUT(1:16) = 'FFFFFFFFFFFFFF00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFFFFFFFF00' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8S-BOUND-7' TO ZCVS-TEST-NAME.
           MOVE -9223372036854775808 TO B8S.
           MOVE B8S-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8S = -9223372036854775808
             AND ZCVS-HEX-OUT(1:16) = '8000000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8000000000000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8U-BOUND-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO B8U.
           MOVE B8U-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8U = 0
             AND ZCVS-HEX-OUT(1:16) = '0000000000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8U-BOUND-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO B8U.
           MOVE B8U-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8U = 1
             AND ZCVS-HEX-OUT(1:16) = '0000000000000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8U-BOUND-2' TO ZCVS-TEST-NAME.
           MOVE 255 TO B8U.
           MOVE B8U-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8U = 255
             AND ZCVS-HEX-OUT(1:16) = '00000000000000FF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000FF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8U-BOUND-3' TO ZCVS-TEST-NAME.
           MOVE 256 TO B8U.
           MOVE B8U-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8U = 256
             AND ZCVS-HEX-OUT(1:16) = '0000000000000100'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000100' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'B8U-BOUND-4' TO ZCVS-TEST-NAME.
           MOVE 18446744073709551615 TO B8U.
           MOVE B8U-RAW TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF B8U = 18446744073709551615
             AND ZCVS-HEX-OUT(1:16) = 'FFFFFFFFFFFFFFFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FFFFFFFFFFFFFFFF' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * AUDIT: A02/D04; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL TRUNC(BIN),ARITH(EXTEND)
      * SPEC: TRUNC BIN AND SIZE ERROR; AUDIT A02.
      * ZA118A: TRUNC BIN physical arithmetic overflow boundaries
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA118A.
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
       01  FLAG PIC X.
       01  N2S PIC S9(4) BINARY.
       01  N2U PIC 9(4) BINARY.
       01  N4S PIC S9(9) BINARY.
       01  N4U PIC 9(9) BINARY.
       01  N8S PIC S9(18) BINARY.
       01  N8U PIC 9(18) BINARY.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA118A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'N2S-MAX-OK' TO ZCVS-TEST-NAME.
           MOVE 32766 TO N2S.
           MOVE SPACE TO FLAG.
           ADD 1 TO N2S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N2S = 32767 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N2S-MAX-OVER' TO ZCVS-TEST-NAME.
           MOVE 32767 TO N2S.
           MOVE SPACE TO FLAG.
           ADD 1 TO N2S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N2S = 32767 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N2S-MIN-OK' TO ZCVS-TEST-NAME.
           MOVE -32767 TO N2S.
           MOVE SPACE TO FLAG.
           SUBTRACT 1 FROM N2S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF N2S = -32768 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N2S-MIN-OVER' TO ZCVS-TEST-NAME.
           MOVE -32768 TO N2S.
           MOVE SPACE TO FLAG.
           SUBTRACT 1 FROM N2S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF N2S = -32768 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N2U-MAX-OK' TO ZCVS-TEST-NAME.
           MOVE 65534 TO N2U.
           MOVE SPACE TO FLAG.
           ADD 1 TO N2U
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N2U = 65535 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N2U-MAX-OVER' TO ZCVS-TEST-NAME.
           MOVE 65535 TO N2U.
           MOVE SPACE TO FLAG.
           ADD 1 TO N2U
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N2U = 65535 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4S-MAX-OK' TO ZCVS-TEST-NAME.
           MOVE 2147483646 TO N4S.
           MOVE SPACE TO FLAG.
           ADD 1 TO N4S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N4S = 2147483647 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4S-MAX-OVER' TO ZCVS-TEST-NAME.
           MOVE 2147483647 TO N4S.
           MOVE SPACE TO FLAG.
           ADD 1 TO N4S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N4S = 2147483647 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4S-MIN-OK' TO ZCVS-TEST-NAME.
           MOVE -2147483647 TO N4S.
           MOVE SPACE TO FLAG.
           SUBTRACT 1 FROM N4S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF N4S = -2147483648 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4S-MIN-OVER' TO ZCVS-TEST-NAME.
           MOVE -2147483648 TO N4S.
           MOVE SPACE TO FLAG.
           SUBTRACT 1 FROM N4S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF N4S = -2147483648 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4U-MAX-OK' TO ZCVS-TEST-NAME.
           MOVE 4294967294 TO N4U.
           MOVE SPACE TO FLAG.
           ADD 1 TO N4U
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N4U = 4294967295 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N4U-MAX-OVER' TO ZCVS-TEST-NAME.
           MOVE 4294967295 TO N4U.
           MOVE SPACE TO FLAG.
           ADD 1 TO N4U
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N4U = 4294967295 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N8S-MAX-OK' TO ZCVS-TEST-NAME.
           MOVE 9223372036854775806 TO N8S.
           MOVE SPACE TO FLAG.
           ADD 1 TO N8S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N8S = 9223372036854775807 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N8S-MAX-OVER' TO ZCVS-TEST-NAME.
           MOVE 9223372036854775807 TO N8S.
           MOVE SPACE TO FLAG.
           ADD 1 TO N8S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N8S = 9223372036854775807 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N8S-MIN-OK' TO ZCVS-TEST-NAME.
           MOVE -9223372036854775807 TO N8S.
           MOVE SPACE TO FLAG.
           SUBTRACT 1 FROM N8S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF N8S = -9223372036854775808 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N8S-MIN-OVER' TO ZCVS-TEST-NAME.
           MOVE -9223372036854775808 TO N8S.
           MOVE SPACE TO FLAG.
           SUBTRACT 1 FROM N8S
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-SUBTRACT.
           IF N8S = -9223372036854775808 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N8U-MAX-OK' TO ZCVS-TEST-NAME.
           MOVE 18446744073709551614 TO N8U.
           MOVE SPACE TO FLAG.
           ADD 1 TO N8U
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N8U = 18446744073709551615 AND FLAG = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'N8U-MAX-OVER' TO ZCVS-TEST-NAME.
           MOVE 18446744073709551615 TO N8U.
           MOVE SPACE TO FLAG.
           ADD 1 TO N8U
             ON SIZE ERROR MOVE 'Y' TO FLAG
             NOT ON SIZE ERROR MOVE 'N' TO FLAG
           END-ADD.
           IF N8U = 18446744073709551615 AND FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/KEEP OR STORE' TO ZCVS-EXPECTED
               MOVE FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

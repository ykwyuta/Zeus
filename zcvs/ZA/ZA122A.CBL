      * ZA122A: ROUNDED carry into SIZE ERROR and separate signs
      * SPEC: ROUNDED, SIZE ERROR, SIGN CLAUSE; AUDIT A06, A07.
      * ROUNDING MAY ADD A HIGH-ORDER DIGIT: WITH ON SIZE ERROR THE
      * RECEIVER IS UNCHANGED, WITHOUT IT THE HIGH-ORDER DIGIT IS LOST.
      * SEPARATE SIGNS ARE X60 AND X4E; EMBEDDED SIGNS USE THE ZONE.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA122A.
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
       01  WS-R PIC 99.
       01  WS-RP PIC 99 COMP-3.
       01  WS-RS PIC S99.
       01  WS-SZ PIC X.
       01  WS-LS-G.
           05  WS-LS PIC S99 SIGN LEADING SEPARATE.
       01  WS-TS-G.
           05  WS-TS PIC S99 SIGN TRAILING SEPARATE.
       01  WS-LO-G.
           05  WS-LO PIC S99 SIGN LEADING.
       01  WS-TO-G.
           05  WS-TO PIC S99.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA122A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ROUND-CARRY-DISPLAY' TO ZCVS-TEST-NAME.
           MOVE 11 TO WS-R. MOVE 'N' TO WS-SZ.
           COMPUTE WS-R ROUNDED = 99.5
               ON SIZE ERROR MOVE 'Y' TO WS-SZ
           END-COMPUTE.
           IF WS-R = 11 AND WS-SZ = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '11 Y' TO ZCVS-EXPECTED
               MOVE WS-R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-CARRY-PACKED' TO ZCVS-TEST-NAME.
           MOVE 11 TO WS-RP. MOVE 'N' TO WS-SZ.
           COMPUTE WS-RP ROUNDED = 99.5
               ON SIZE ERROR MOVE 'Y' TO WS-SZ
           END-COMPUTE.
           IF WS-RP = 11 AND WS-SZ = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '11 Y' TO ZCVS-EXPECTED
               MOVE WS-RP TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-NO-CARRY' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-SZ.
           COMPUTE WS-R ROUNDED = 98.5
               ON SIZE ERROR MOVE 'Y' TO WS-SZ
           END-COMPUTE.
           IF WS-R = 99 AND WS-SZ = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99 N' TO ZCVS-EXPECTED
               MOVE WS-R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-NEGATIVE-HALF' TO ZCVS-TEST-NAME.
           COMPUTE WS-RS ROUNDED = -98.5.
           IF WS-RS = -99
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-99' TO ZCVS-EXPECTED
               MOVE WS-RS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ROUND-CARRY-TRUNCATE' TO ZCVS-TEST-NAME.
           MOVE 11 TO WS-R.
           COMPUTE WS-R ROUNDED = 99.5.
           IF WS-R = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-R TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIGN-LEAD-SEP-HEX' TO ZCVS-TEST-NAME.
           MOVE -12 TO WS-LS.
           MOVE WS-LS-G TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '60F1F2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F1F2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIGN-TRAIL-SEP-HEX' TO ZCVS-TEST-NAME.
           MOVE 12 TO WS-TS.
           MOVE WS-TS-G TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = 'F1F24E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1F24E' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIGN-LEAD-OVER-HEX' TO ZCVS-TEST-NAME.
           MOVE -12 TO WS-LO.
           MOVE WS-LO-G TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'D1F2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D1F2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIGN-TRAIL-OVER-HEX' TO ZCVS-TEST-NAME.
           MOVE -12 TO WS-TO.
           MOVE WS-TO-G TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'F1D2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F1D2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIGN-SEP-ARITH' TO ZCVS-TEST-NAME.
           MOVE -12 TO WS-LS.
           ADD 5 TO WS-LS.
           MOVE WS-LS-G TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '60F0F7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F7' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SIGN-SEP-MOVE' TO ZCVS-TEST-NAME.
           MOVE -7 TO WS-LS.
           MOVE WS-LS TO WS-TS.
           MOVE WS-TS-G TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = 'F0F760'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F760' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

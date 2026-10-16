      *================================================================*
      * ZP101A - LOCAL-STORAGE VS WORKING-STORAGE AND CANCEL           *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP101A.
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
       01  WS-CALL-RESULTS.
           05  WS-OUT-WS            PIC 9(4) COMP.
           05  WS-OUT-LS            PIC 9(4) COMP.
           05  WS-OUT-GRP           PIC X(6).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: FIRST CALL TO ZPS01A (WS=11, LS=101, GRP='LS0505')
           MOVE 'LS-FIRST-CALL' TO ZCVS-TEST-NAME.
           CALL 'ZPS01A' USING WS-OUT-WS WS-OUT-LS WS-OUT-GRP.
           IF WS-OUT-WS = 11
             AND WS-OUT-LS = 101
             AND WS-OUT-GRP = 'LS0505'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '11/101/LS0505' TO ZCVS-EXPECTED
               MOVE WS-OUT-GRP TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: SECOND CALL TO ZPS01A (WS=12 PERSISTS, LS=101 RESETS)
           MOVE 'LS-SECOND-CALL' TO ZCVS-TEST-NAME.
           CALL 'ZPS01A' USING WS-OUT-WS WS-OUT-LS WS-OUT-GRP.
           IF WS-OUT-WS = 12
             AND WS-OUT-LS = 101
             AND WS-OUT-GRP = 'LS0505'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12/101/LS0505' TO ZCVS-EXPECTED
               MOVE WS-OUT-GRP TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: CANCEL 'ZPS01A' RESETS WORKING-STORAGE TO INITIAL VALUE
           MOVE 'LS-AFTER-CANCEL' TO ZCVS-TEST-NAME.
           CANCEL 'ZPS01A'.
           CALL 'ZPS01A' USING WS-OUT-WS WS-OUT-LS WS-OUT-GRP.
           IF WS-OUT-WS = 11
             AND WS-OUT-LS = 101
             AND WS-OUT-GRP = 'LS0505'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '11/101/LS0505' TO ZCVS-EXPECTED
               MOVE WS-OUT-GRP TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

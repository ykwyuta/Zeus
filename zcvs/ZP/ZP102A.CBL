      *================================================================*
      * ZP102A - RECURSIVE PROGRAMS AND ACTIVATION STACK FRAMES        *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP102A.
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
       01  WS-REC-ARGS.
           05  WS-N                 PIC S9(9) COMP.
           05  WS-RES               PIC S9(9) COMP.
           05  WS-CALLS             PIC 9(4) COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: RECURSIVE 5! = 120 (5 RECURSIVE ACTIVATIONS)
           MOVE 'REC-FACT-5' TO ZCVS-TEST-NAME.
           MOVE 5 TO WS-N.
           MOVE 0 TO WS-RES.
           MOVE 0 TO WS-CALLS.
           CALL 'ZPS02A' USING WS-N WS-RES WS-CALLS.
           IF WS-RES = 120 AND WS-CALLS = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '120/5' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: RECURSIVE 6! = 720 (CUMULATIVE CALLS = 5 + 6 = 11)
           MOVE 'REC-FACT-6' TO ZCVS-TEST-NAME.
           MOVE 6 TO WS-N.
           MOVE 0 TO WS-RES.
           CALL 'ZPS02A' USING WS-N WS-RES WS-CALLS.
           IF WS-RES = 720 AND WS-CALLS = 11
               PERFORM ZCVS-PASS
           ELSE
               MOVE '720/11' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: RECURSIVE FIBONACCI FIB(7) = 13
           MOVE 'REC-FIB-7' TO ZCVS-TEST-NAME.
           MOVE 7 TO WS-N.
           MOVE 0 TO WS-RES.
           CALL 'ZPS02B' USING WS-N WS-RES.
           IF WS-RES = 13
               PERFORM ZCVS-PASS
           ELSE
               MOVE '13' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.
       END PROGRAM ZP102A.

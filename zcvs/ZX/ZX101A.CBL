      *================================================================*
      * ZX101A - EXIT PERFORM, EXIT PERFORM CYCLE, UNTIL EXIT, EXIT    *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX101A.
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
       01  WS-FLOW-FIELDS.
           05  WS-I                 PIC 9(2) COMP.
           05  WS-J                 PIC 9(2) COMP.
           05  WS-ACC               PIC 9(4) COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX101A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: EXIT PERFORM EXITS INNERMOST INLINE LOOP ONLY
           MOVE 'FLOW-EXIT-PERFORM' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-ACC.
           PERFORM 3 TIMES
               PERFORM 5 TIMES
                   ADD 1 TO WS-ACC
                   EXIT PERFORM
                   ADD 100 TO WS-ACC
               END-PERFORM
           END-PERFORM.
           IF WS-ACC = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: EXIT PERFORM CYCLE INCREMENTS VARYING AND RETESTS
           MOVE 'FLOW-EXIT-CYCLE' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-ACC.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4
               ADD WS-I TO WS-ACC
               IF WS-I >= 2
                   EXIT PERFORM CYCLE
               END-IF
               ADD 10 TO WS-ACC
           END-PERFORM.
           IF WS-ACC = 20 AND WS-I = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '20/5' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: PERFORM UNTIL EXIT TERMINATED BY EXIT PERFORM
           MOVE 'FLOW-UNTIL-EXIT' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-ACC.
           PERFORM UNTIL EXIT
               ADD 1 TO WS-ACC
               IF WS-ACC = 4
                   EXIT PERFORM
               END-IF
           END-PERFORM.
           IF WS-ACC = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: EXIT PARAGRAPH AND EXIT SECTION
           MOVE 'FLOW-EXIT-PAR-SEC' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-ACC.
           PERFORM SUB-PAR.
           PERFORM SUB-SEC-SECTION.
           IF WS-ACC = 15
               PERFORM ZCVS-PASS
           ELSE
               MOVE '15' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

       SUB-PAR.
           ADD 5 TO WS-ACC.
           EXIT PARAGRAPH.
           ADD 100 TO WS-ACC.

       SUB-SEC-SECTION SECTION.
       SUB-SEC-P1.
           ADD 10 TO WS-ACC.
           EXIT SECTION.
       SUB-SEC-P2.
           ADD 200 TO WS-ACC.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

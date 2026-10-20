      * AUDIT: D06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * SPEC: SIGN CLAUSE AND DISPLAY ITEMS; AUDIT D06.
      * ZD112A: Zoned signs all overpunch digits and separate signs
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD112A.
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
       01  N-LO PIC S9(4) SIGN LEADING.
       01  R-LO REDEFINES N-LO PIC X(4).
       01  N-TO PIC S9(4) SIGN TRAILING.
       01  R-TO REDEFINES N-TO PIC X(4).
       01  N-LS PIC S9(4) SIGN LEADING SEPARATE.
       01  R-LS REDEFINES N-LS PIC X(5).
       01  N-TS PIC S9(4) SIGN TRAILING SEPARATE.
       01  R-TS REDEFINES N-TS PIC X(5).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD112A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'LO-0-POS' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-LO.
           IF N-LO = 0 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F0' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-0-POS' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-TO.
           IF N-TO = 0 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C0' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-0-POS' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-LS.
           IF N-LS = 0 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F0' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-0-POS' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-TS.
           IF N-TS = 0 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F04E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F04E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-0-NEG' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-LO.
           IF N-LO = 0 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F0' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-0-NEG' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-TO.
           IF N-TO = 0 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C0' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-0-NEG' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-LS.
           IF N-LS = 0 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F0'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F0' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-0-NEG' TO ZCVS-TEST-NAME.
           MOVE 0 TO N-TS.
           IF N-TS = 0 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F04E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F04E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-1-POS' TO ZCVS-TEST-NAME.
           MOVE 1 TO N-LO.
           IF N-LO = 1 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F1' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-1-POS' TO ZCVS-TEST-NAME.
           MOVE 1 TO N-TO.
           IF N-TO = 1 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C1' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-1-POS' TO ZCVS-TEST-NAME.
           MOVE 1 TO N-LS.
           IF N-LS = 1 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F1' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-1-POS' TO ZCVS-TEST-NAME.
           MOVE 1 TO N-TS.
           IF N-TS = 1 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F14E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F14E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-1-NEG' TO ZCVS-TEST-NAME.
           MOVE -1 TO N-LO.
           IF N-LO = -1 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F1' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-1-NEG' TO ZCVS-TEST-NAME.
           MOVE -1 TO N-TO.
           IF N-TO = -1 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D1' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-1-NEG' TO ZCVS-TEST-NAME.
           MOVE -1 TO N-LS.
           IF N-LS = -1 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F1' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-1-NEG' TO ZCVS-TEST-NAME.
           MOVE -1 TO N-TS.
           IF N-TS = -1 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F160'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F160' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-2-POS' TO ZCVS-TEST-NAME.
           MOVE 2 TO N-LO.
           IF N-LO = 2 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F2' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-2-POS' TO ZCVS-TEST-NAME.
           MOVE 2 TO N-TO.
           IF N-TO = 2 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C2' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-2-POS' TO ZCVS-TEST-NAME.
           MOVE 2 TO N-LS.
           IF N-LS = 2 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F2' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-2-POS' TO ZCVS-TEST-NAME.
           MOVE 2 TO N-TS.
           IF N-TS = 2 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F24E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F24E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-2-NEG' TO ZCVS-TEST-NAME.
           MOVE -2 TO N-LO.
           IF N-LO = -2 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F2' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-2-NEG' TO ZCVS-TEST-NAME.
           MOVE -2 TO N-TO.
           IF N-TO = -2 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D2' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-2-NEG' TO ZCVS-TEST-NAME.
           MOVE -2 TO N-LS.
           IF N-LS = -2 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F2' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-2-NEG' TO ZCVS-TEST-NAME.
           MOVE -2 TO N-TS.
           IF N-TS = -2 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F260'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F260' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-3-POS' TO ZCVS-TEST-NAME.
           MOVE 3 TO N-LO.
           IF N-LO = 3 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F3' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-3-POS' TO ZCVS-TEST-NAME.
           MOVE 3 TO N-TO.
           IF N-TO = 3 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C3' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-3-POS' TO ZCVS-TEST-NAME.
           MOVE 3 TO N-LS.
           IF N-LS = 3 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F3' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-3-POS' TO ZCVS-TEST-NAME.
           MOVE 3 TO N-TS.
           IF N-TS = 3 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F34E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F34E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-3-NEG' TO ZCVS-TEST-NAME.
           MOVE -3 TO N-LO.
           IF N-LO = -3 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F3' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-3-NEG' TO ZCVS-TEST-NAME.
           MOVE -3 TO N-TO.
           IF N-TO = -3 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D3' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-3-NEG' TO ZCVS-TEST-NAME.
           MOVE -3 TO N-LS.
           IF N-LS = -3 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F3'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F3' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-3-NEG' TO ZCVS-TEST-NAME.
           MOVE -3 TO N-TS.
           IF N-TS = -3 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F360'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F360' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-4-POS' TO ZCVS-TEST-NAME.
           MOVE 4 TO N-LO.
           IF N-LO = 4 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F4' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-4-POS' TO ZCVS-TEST-NAME.
           MOVE 4 TO N-TO.
           IF N-TO = 4 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C4' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-4-POS' TO ZCVS-TEST-NAME.
           MOVE 4 TO N-LS.
           IF N-LS = 4 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F4' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-4-POS' TO ZCVS-TEST-NAME.
           MOVE 4 TO N-TS.
           IF N-TS = 4 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F44E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F44E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-4-NEG' TO ZCVS-TEST-NAME.
           MOVE -4 TO N-LO.
           IF N-LO = -4 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F4' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-4-NEG' TO ZCVS-TEST-NAME.
           MOVE -4 TO N-TO.
           IF N-TO = -4 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D4' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-4-NEG' TO ZCVS-TEST-NAME.
           MOVE -4 TO N-LS.
           IF N-LS = -4 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F4' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-4-NEG' TO ZCVS-TEST-NAME.
           MOVE -4 TO N-TS.
           IF N-TS = -4 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F460'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F460' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-5-POS' TO ZCVS-TEST-NAME.
           MOVE 5 TO N-LO.
           IF N-LO = 5 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F5'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F5' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-5-POS' TO ZCVS-TEST-NAME.
           MOVE 5 TO N-TO.
           IF N-TO = 5 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C5'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C5' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-5-POS' TO ZCVS-TEST-NAME.
           MOVE 5 TO N-LS.
           IF N-LS = 5 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F5'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F5' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-5-POS' TO ZCVS-TEST-NAME.
           MOVE 5 TO N-TS.
           IF N-TS = 5 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F54E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F54E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-5-NEG' TO ZCVS-TEST-NAME.
           MOVE -5 TO N-LO.
           IF N-LO = -5 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F5'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F5' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-5-NEG' TO ZCVS-TEST-NAME.
           MOVE -5 TO N-TO.
           IF N-TO = -5 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D5'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D5' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-5-NEG' TO ZCVS-TEST-NAME.
           MOVE -5 TO N-LS.
           IF N-LS = -5 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F5'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F5' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-5-NEG' TO ZCVS-TEST-NAME.
           MOVE -5 TO N-TS.
           IF N-TS = -5 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F560'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F560' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-6-POS' TO ZCVS-TEST-NAME.
           MOVE 6 TO N-LO.
           IF N-LO = 6 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F6'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F6' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-6-POS' TO ZCVS-TEST-NAME.
           MOVE 6 TO N-TO.
           IF N-TO = 6 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C6'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C6' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-6-POS' TO ZCVS-TEST-NAME.
           MOVE 6 TO N-LS.
           IF N-LS = 6 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F6'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F6' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-6-POS' TO ZCVS-TEST-NAME.
           MOVE 6 TO N-TS.
           IF N-TS = 6 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F64E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F64E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-6-NEG' TO ZCVS-TEST-NAME.
           MOVE -6 TO N-LO.
           IF N-LO = -6 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F6'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F6' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-6-NEG' TO ZCVS-TEST-NAME.
           MOVE -6 TO N-TO.
           IF N-TO = -6 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D6'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D6' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-6-NEG' TO ZCVS-TEST-NAME.
           MOVE -6 TO N-LS.
           IF N-LS = -6 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F6'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F6' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-6-NEG' TO ZCVS-TEST-NAME.
           MOVE -6 TO N-TS.
           IF N-TS = -6 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F660'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F660' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-7-POS' TO ZCVS-TEST-NAME.
           MOVE 7 TO N-LO.
           IF N-LO = 7 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F7' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-7-POS' TO ZCVS-TEST-NAME.
           MOVE 7 TO N-TO.
           IF N-TO = 7 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C7' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-7-POS' TO ZCVS-TEST-NAME.
           MOVE 7 TO N-LS.
           IF N-LS = 7 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F7' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-7-POS' TO ZCVS-TEST-NAME.
           MOVE 7 TO N-TS.
           IF N-TS = 7 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F74E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F74E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-7-NEG' TO ZCVS-TEST-NAME.
           MOVE -7 TO N-LO.
           IF N-LO = -7 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F7' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-7-NEG' TO ZCVS-TEST-NAME.
           MOVE -7 TO N-TO.
           IF N-TO = -7 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D7' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-7-NEG' TO ZCVS-TEST-NAME.
           MOVE -7 TO N-LS.
           IF N-LS = -7 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F7'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F7' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-7-NEG' TO ZCVS-TEST-NAME.
           MOVE -7 TO N-TS.
           IF N-TS = -7 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F760'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F760' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-8-POS' TO ZCVS-TEST-NAME.
           MOVE 8 TO N-LO.
           IF N-LO = 8 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F8'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F8' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-8-POS' TO ZCVS-TEST-NAME.
           MOVE 8 TO N-TO.
           IF N-TO = 8 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C8'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C8' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-8-POS' TO ZCVS-TEST-NAME.
           MOVE 8 TO N-LS.
           IF N-LS = 8 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F8'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F8' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-8-POS' TO ZCVS-TEST-NAME.
           MOVE 8 TO N-TS.
           IF N-TS = 8 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F84E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F84E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-8-NEG' TO ZCVS-TEST-NAME.
           MOVE -8 TO N-LO.
           IF N-LO = -8 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F8'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F8' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-8-NEG' TO ZCVS-TEST-NAME.
           MOVE -8 TO N-TO.
           IF N-TO = -8 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D8'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D8' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-8-NEG' TO ZCVS-TEST-NAME.
           MOVE -8 TO N-LS.
           IF N-LS = -8 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F8'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F8' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-8-NEG' TO ZCVS-TEST-NAME.
           MOVE -8 TO N-TS.
           IF N-TS = -8 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F860'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F860' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-9-POS' TO ZCVS-TEST-NAME.
           MOVE 9 TO N-LO.
           IF N-LO = 9 AND N-LO IS NUMERIC
             AND R-LO = X'C0F0F0F9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C0F0F0F9' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-9-POS' TO ZCVS-TEST-NAME.
           MOVE 9 TO N-TO.
           IF N-TO = 9 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0C9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0C9' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-9-POS' TO ZCVS-TEST-NAME.
           MOVE 9 TO N-LS.
           IF N-LS = 9 AND N-LS IS NUMERIC
             AND R-LS = X'4EF0F0F0F9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4EF0F0F0F9' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-9-POS' TO ZCVS-TEST-NAME.
           MOVE 9 TO N-TS.
           IF N-TS = 9 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F94E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F94E' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LO-9-NEG' TO ZCVS-TEST-NAME.
           MOVE -9 TO N-LO.
           IF N-LO = -9 AND N-LO IS NUMERIC
             AND R-LO = X'D0F0F0F9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'D0F0F0F9' TO ZCVS-EXPECTED
               MOVE R-LO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TO-9-NEG' TO ZCVS-TEST-NAME.
           MOVE -9 TO N-TO.
           IF N-TO = -9 AND N-TO IS NUMERIC
             AND R-TO = X'F0F0F0D9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0D9' TO ZCVS-EXPECTED
               MOVE R-TO TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-9-NEG' TO ZCVS-TEST-NAME.
           MOVE -9 TO N-LS.
           IF N-LS = -9 AND N-LS IS NUMERIC
             AND R-LS = X'60F0F0F0F9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60F0F0F0F9' TO ZCVS-EXPECTED
               MOVE R-LS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TS-9-NEG' TO ZCVS-TEST-NAME.
           MOVE -9 TO N-TS.
           IF N-TS = -9 AND N-TS IS NUMERIC
             AND R-TS = X'F0F0F0F960'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F0F0F0F960' TO ZCVS-EXPECTED
               MOVE R-TS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

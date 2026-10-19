      * SPEC: TRIM/TEST-NUMVAL(-C)/ABS FUNCTIONS; AUDIT X05.
      * ZX107A: TRIM lengths NUMVAL diagnostics ABS signs
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX107A.
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
       01  TEXT-IN PIC X(8) VALUE '  AB  CD'.
       01  TEXT-OUT PIC X(12).
       01  NUM-OUT PIC S9(5)V99 COMP-3.
       01  POS-OUT PIC 99 COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX107A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'TRIM-BOTH' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM('  AB  ') TO TEXT-OUT.
           IF TEXT-OUT = 'AB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TRIMMED TEXT' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRIM-LEADING' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM('  AB  ' LEADING) TO TEXT-OUT.
           IF TEXT-OUT = 'AB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TRIMMED TEXT' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRIM-TRAILING' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM('  AB  ' TRAILING) TO TEXT-OUT.
           IF TEXT-OUT = '  AB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TRIMMED TEXT' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRIM-INTERNAL' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(TEXT-IN) TO TEXT-OUT.
           IF TEXT-OUT = 'AB  CD'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TRIMMED TEXT' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRIM-LEAD-LENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(
               FUNCTION TRIM('  AB  ' LEADING)) = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRIM-BOTH-LENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(FUNCTION TRIM('  AB  ')) = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRIM-EMPTY-LENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(FUNCTION TRIM('    ')) = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE TEXT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NUMVAL-VALID' TO ZCVS-TEST-NAME.
           COMPUTE POS-OUT = FUNCTION TEST-NUMVAL(' -12.50 ').
           IF POS-OUT = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE POS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NUMVAL-BAD-CHAR' TO ZCVS-TEST-NAME.
           COMPUTE POS-OUT = FUNCTION TEST-NUMVAL('12X4').
           IF POS-OUT = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE POS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NUMVAL-TWO-DOTS' TO ZCVS-TEST-NAME.
           COMPUTE POS-OUT = FUNCTION TEST-NUMVAL('1.2.3').
           IF POS-OUT = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE POS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NUMVAL-EMPTY' TO ZCVS-TEST-NAME.
           COMPUTE POS-OUT = FUNCTION TEST-NUMVAL('    ').
           IF POS-OUT = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '5' TO ZCVS-EXPECTED
               MOVE POS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NUMVAL-C-VALID' TO ZCVS-TEST-NAME.
           COMPUTE POS-OUT = FUNCTION TEST-NUMVAL-C('$1,234.50').
           IF POS-OUT = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE POS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NUMVAL-C-BAD' TO ZCVS-TEST-NAME.
           COMPUTE POS-OUT = FUNCTION TEST-NUMVAL-C('$12X4').
           IF POS-OUT = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE POS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ABS-0' TO ZCVS-TEST-NAME.
           COMPUTE NUM-OUT = FUNCTION ABS(-12.5).
           IF NUM-OUT = 12.5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.5' TO ZCVS-EXPECTED
               MOVE NUM-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ABS-1' TO ZCVS-TEST-NAME.
           COMPUTE NUM-OUT = FUNCTION ABS(0).
           IF NUM-OUT = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE NUM-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ABS-2' TO ZCVS-TEST-NAME.
           COMPUTE NUM-OUT = FUNCTION ABS(12.5).
           IF NUM-OUT = 12.5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12.5' TO ZCVS-EXPECTED
               MOVE NUM-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

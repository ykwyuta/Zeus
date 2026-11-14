      * AUDIT: A09; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZA127A: FUNCTION UUID4 returns a 36-character alphanumeric
      * version 4 UUID: hexadecimal groups of 8-4-4-4-12 characters
      * separated by hyphens, the version digit 4 in position 15, the
      * variant (8, 9, a or b) in position 20, and a new value at each
      * reference.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA127A.
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
       01  U1 PIC X(36).
       01  U2 PIC X(36).
       01  HEX-COUNT PIC 99.
       01  HYPHENS PIC 99.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA127A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.
           MOVE FUNCTION UUID4 TO U1.
           MOVE FUNCTION UUID4 TO U2.

           MOVE 'UUID4-LENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION LENGTH(FUNCTION UUID4) = 36
               PERFORM ZCVS-PASS
           ELSE
               MOVE '36' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UUID4-HYPHENS' TO ZCVS-TEST-NAME.
           MOVE 0 TO HYPHENS.
           INSPECT U1 TALLYING HYPHENS FOR ALL '-'.
           IF HYPHENS = 4 AND U1(9:1) = '-' AND U1(14:1) = '-'
             AND U1(19:1) = '-' AND U1(24:1) = '-'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8-4-4-4-12' TO ZCVS-EXPECTED
               MOVE U1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UUID4-HEX-DIGITS' TO ZCVS-TEST-NAME.
           MOVE 0 TO HEX-COUNT.
           INSPECT U1 TALLYING HEX-COUNT FOR ALL '0' '1' '2' '3' '4'
             '5' '6' '7' '8' '9' 'a' 'b' 'c' 'd' 'e' 'f'.
           IF HEX-COUNT = 32
               PERFORM ZCVS-PASS
           ELSE
               MOVE '32' TO ZCVS-EXPECTED
               MOVE U1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UUID4-VERSION' TO ZCVS-TEST-NAME.
           IF U1(15:1) = '4' AND U2(15:1) = '4'
             AND (U1(20:1) = '8' OR '9' OR 'a' OR 'b')
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4 / 8,9,a,b' TO ZCVS-EXPECTED
               MOVE U1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UUID4-DIFFERENT' TO ZCVS-TEST-NAME.
           IF U1 NOT = U2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TWO VALUES' TO ZCVS-EXPECTED
               MOVE U1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

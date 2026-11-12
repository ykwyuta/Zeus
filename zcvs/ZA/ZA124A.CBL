      * Z A 1 2 4 A   -   N U M V A L   W I T H   A N   I N V A L I D
      * A R G U M E N T   E N D S   T H E   R U N   U N I T .   T H E
      * R E P O R T   I S   C L O S E D   F I R S T ;   T H E   S T E
      * P   M U S T   T H E N   E N D   A B N O R M A L L Y   ( M A N
      * I F E S T   E X P E C T _ E X I T _ C O D E ) .   T O   B E
      * C O N F I R M E D   O N   Z / O S .
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA124A.
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
       01  WS-P PIC S9(4).
       01  WS-N PIC S9(4)V99.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA124A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'TEST-NUMVAL-INVALID' TO ZCVS-TEST-NAME.
           COMPUTE WS-P = FUNCTION TEST-NUMVAL('12X').
           IF WS-P = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE WS-P TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TEST-NUMVAL-VALID' TO ZCVS-TEST-NAME.
           COMPUTE WS-P = FUNCTION TEST-NUMVAL(' -1.5 ').
           IF WS-P = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-P TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
      * THE REPORT IS CLOSED. NUMVAL MUST NOT RETURN; IF IT DOES THE
      * STEP ENDS WITH RC 0, WHICH THE MANIFEST REJECTS.
           COMPUTE WS-N = FUNCTION NUMVAL('12X').
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * Z N 1 1 4 A   -   M O V E   F R O M   A   N A T I O N A L   I
      * T E M   T O   A N   A L P H A N U M E R I C   I T E M .   W H
      * E T H E R   T H E   M O V E   I S   V A L I D   A N D   W H A
      * T   I T   S T O R E S   I S   N O T   C O N F I R M E D   F R
      * O M   T H E   D O C U M E N T A T I O N ;   E X P E C T E D
      * V A L U E S   A R E   T H E   C U R R E N T   Z E U S   B E H
      * A V I O R .
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN114A.
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
       01  WS-N PIC N(3) USAGE NATIONAL VALUE N'ABC'.
       01  WS-X6-G.
           05  WS-X6 PIC X(6).
       01  WS-X2-G.
           05  WS-X2 PIC X(2).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN114A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'NAT-TO-ALNUM-SAME' TO ZCVS-TEST-NAME.
           MOVE WS-N TO WS-X6.
           MOVE WS-X6-G TO ZCVS-HEX-IN.
           MOVE 6 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:12) = 'C1C2C3404040'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1C2C3404040' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:12) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'NAT-TO-ALNUM-SHORT' TO ZCVS-TEST-NAME.
           MOVE WS-N TO WS-X2.
           MOVE WS-X2-G TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = 'C1C2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1C2' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

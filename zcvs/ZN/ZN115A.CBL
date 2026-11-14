      * AUDIT: N06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL CODEPAGE(1140)
      * ZN115A: the CODEPAGE compiler option. Under CODEPAGE(1140) the
      * alphanumeric byte X'9F' is the euro sign (U+20AC), where CCSID
      * 037 has the currency sign (U+00A4). NATIONAL-OF and DISPLAY-OF
      * convert with the CODEPAGE CCSID unless a CCSID argument names
      * another code page.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN115A.
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
       01  N1 PIC N(1) USAGE NATIONAL.
       01  X1 PIC X(1).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN115A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'CP1140-EURO' TO ZCVS-TEST-NAME.
           MOVE FUNCTION NATIONAL-OF(X'9F') TO N1.
           IF N1 = NX'20AC'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NX20AC' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CP1140-DISPLAY-OF' TO ZCVS-TEST-NAME.
           MOVE FUNCTION DISPLAY-OF(NX'20AC') TO X1.
           IF X1 = X'9F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'X9F' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CP1140-LETTERS' TO ZCVS-TEST-NAME.
           MOVE FUNCTION NATIONAL-OF('A') TO N1.
           IF N1 = NX'0041'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NX0041' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'CCSID-ARGUMENT-037' TO ZCVS-TEST-NAME.
           MOVE FUNCTION NATIONAL-OF(X'9F' 37) TO N1.
           IF N1 = NX'00A4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NX00A4' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

       CBL RENT,NODYNAM,LIST,MAP,SOURCE,XREF,FLAG(I)
      *================================================================*
      * ZX104A - CBL OPTIONS, EJECT/SKIP/TITLE, COMPUTER CLAUSES       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX104A.
           TITLE 'ZCVS ZX104A COMPILER DIRECTIVES'.
           EJECT
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SOURCE-COMPUTER. IBM-Z.
       OBJECT-COMPUTER. IBM-Z
           MEMORY SIZE 65536 WORDS
           SEGMENT-LIMIT IS 49.
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
           SKIP1
           COPY ZCVSRPT.
           SKIP2.
       01  WS-DIR-CHECK             PIC 9(4) COMP VALUE 42.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX104A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: LISTING DIRECTIVES DO NOT BREAK IF / NEXT SENTENCE
           MOVE 'DIR-SCOPE-CHECK' TO ZCVS-TEST-NAME.
           IF WS-DIR-CHECK = 42
               NEXT SENTENCE
           END-IF
           SKIP3
           MOVE 999 TO WS-DIR-CHECK.
           IF WS-DIR-CHECK = 42
               PERFORM ZCVS-PASS
           ELSE
               MOVE '42' TO ZCVS-EXPECTED
               MOVE '999' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

      *================================================================*
      * ZX103A - COPY REPLACING, COPY OF COPYALT, NESTED COPY, REPLACE *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX103A.
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
           COPY ZCVSCOPY REPLACING ==:PFX:== BY ==PRI==.
           COPY ZCVSCOPY OF COPYALT REPLACING ==:PFX:== BY ==ALT==.
       REPLACE LEADING ==OLD-== BY ==NEW-==
               TRAILING ==-OLD== BY ==-NEW==.
       01  OLD-ITEM                 PIC X(4) VALUE 'LEAD'.
       01  ITEM-OLD                 PIC X(4) VALUE 'TRAL'.
       REPLACE OFF.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX103A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: PRIMARY COPYBOOK WITH :PFX: REPLACING AND NESTED COPY
           MOVE 'COPY-PRI-NESTED' TO ZCVS-TEST-NAME.
           IF PRI-LIB-NAME = 'PRIMARY '
             AND PRI-COPY-VAL = 1234
             AND ZCVS-NESTED-TAG = 'NESTED'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'PRIMARY/1234/NESTED' TO ZCVS-EXPECTED
               MOVE PRI-LIB-NAME TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: ALTERNATE LIBRARY COPY OF COPYALT
           MOVE 'COPY-OF-COPYALT' TO ZCVS-TEST-NAME.
           IF ALT-LIB-NAME = 'COPYALT ' AND ALT-COPY-VAL = 5678
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'COPYALT/5678' TO ZCVS-EXPECTED
               MOVE ALT-LIB-NAME TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: REPLACE LEADING AND TRAILING DIRECTIVES
           MOVE 'REPLACE-LEAD-TRAIL' TO ZCVS-TEST-NAME.
           IF NEW-ITEM = 'LEAD' AND ITEM-NEW = 'TRAL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'LEAD/TRAL' TO ZCVS-EXPECTED
               MOVE NEW-ITEM TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

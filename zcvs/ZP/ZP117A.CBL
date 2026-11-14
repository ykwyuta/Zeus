      * AUDIT: P06; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZP117A: ALLOCATE and FREE. ALLOCATE of a LINKAGE record sets its
      * address (and the RETURNING pointer); INITIALIZED initializes it
      * as INITIALIZE WITH FILLER ALL TO VALUE THEN TO DEFAULT, so the
      * VALUE clauses, FILLER included, are applied. ALLOCATE n
      * CHARACTERS INITIALIZED gives binary zeros; a size of zero gives
      * NULL. FREE releases the storage and sets the pointer to NULL.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP117A.
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
       01  P1 POINTER.
       01  P2 POINTER.
       01  P3 POINTER.
       01  SAVED PIC X(9).
       LINKAGE SECTION.
       01  REC.
           05  R-NUM PIC 9(3) VALUE 123.
           05  R-TXT PIC X(4) VALUE 'ABCD'.
           05  FILLER PIC X(2) VALUE 'ZZ'.
       01  BUF PIC X(8).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP117A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ALLOC-INITIALIZED' TO ZCVS-TEST-NAME.
           ALLOCATE REC INITIALIZED RETURNING P1.
           MOVE REC TO SAVED.
           IF SAVED = '123ABCDZZ'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '123ABCDZZ' TO ZCVS-EXPECTED
               MOVE SAVED TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ALLOC-RETURNING' TO ZCVS-TEST-NAME.
           IF P1 = ADDRESS OF REC AND P1 NOT = NULL
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ADDRESS OF REC' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ALLOC-CHARS-ZEROS' TO ZCVS-TEST-NAME.
           ALLOCATE 8 CHARACTERS INITIALIZED RETURNING P2.
           SET ADDRESS OF BUF TO P2.
           IF BUF = LOW-VALUES
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'LOW-VALUES' TO ZCVS-EXPECTED
               MOVE BUF TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ALLOC-DISTINCT' TO ZCVS-TEST-NAME.
           ALLOCATE 7.5 CHARACTERS RETURNING P3.
           IF P3 NOT = P2 AND P3 NOT = P1 AND P3 NOT = NULL
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NEW STORAGE' TO ZCVS-EXPECTED
               MOVE 'SAME' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FREE-SETS-NULL' TO ZCVS-TEST-NAME.
           FREE P2 P3.
           IF P2 = NULL AND P3 = NULL
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NULL' TO ZCVS-EXPECTED
               MOVE 'NOT NULL' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ALLOC-ZERO-NULL' TO ZCVS-TEST-NAME.
           ALLOCATE 0 CHARACTERS RETURNING P2.
           IF P2 = NULL
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NULL' TO ZCVS-EXPECTED
               MOVE 'NOT NULL' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           FREE P1.
           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

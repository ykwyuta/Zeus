      * AUDIT: F01; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF115A: ORGANIZATION LINE SEQUENTIAL in the z/OS UNIX file
      * system. WRITE removes the trailing blanks and adds the record
      * delimiter (the EBCDIC newline X'15'); READ fills the rest of a
      * longer record area with spaces; a shorter record area leaves
      * the rest of the record for the next READ; a X'15' written as
      * data ends a record; OPEN EXTEND adds records after the last.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF115A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT LS-OUT ASSIGN TO LSFILE
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS FS1.
           SELECT LS-SHORT ASSIGN TO LSFILE
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS FS2.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  LS-OUT
           RECORD CONTAINS 10 CHARACTERS.
       01  LS-REC PIC X(10).
       FD  LS-SHORT
           RECORD CONTAINS 4 CHARACTERS.
       01  SHORT-REC PIC X(4).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS1 PIC XX.
       01  FS2 PIC XX.
       01  R1 PIC X(10).
       01  R2 PIC X(10).
       01  R3 PIC X(10).
       01  R4 PIC X(10).
       01  S1 PIC X(4).
       01  S2 PIC X(4).
       01  S3 PIC X(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF115A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.
           OPEN OUTPUT LS-OUT.
           MOVE 'ABC' TO LS-REC.
           WRITE LS-REC.
           MOVE 'ABCDEFGHIJ' TO LS-REC.
           WRITE LS-REC.
           MOVE SPACES TO LS-REC.
           MOVE 'XY' TO LS-REC(1:2).
           MOVE X'15' TO LS-REC(3:1).
           MOVE 'Z' TO LS-REC(4:1).
           WRITE LS-REC.
           CLOSE LS-OUT.
           OPEN EXTEND LS-OUT.
           MOVE 'LAST' TO LS-REC.
           WRITE LS-REC.
           CLOSE LS-OUT.

           MOVE 'LS-READ-PADDED' TO ZCVS-TEST-NAME.
           OPEN INPUT LS-OUT.
           READ LS-OUT INTO R1.
           READ LS-OUT INTO R2.
           IF R1 = 'ABC' AND R2 = 'ABCDEFGHIJ' AND FS1 = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABC/ABCDEFGHIJ' TO ZCVS-EXPECTED
               MOVE R1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-NEWLINE-DATA' TO ZCVS-TEST-NAME.
           READ LS-OUT INTO R3.
           READ LS-OUT INTO R4.
           IF R3 = 'XY' AND R4 = 'Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'XY/Z' TO ZCVS-EXPECTED
               MOVE R3 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'LS-EXTEND-EOF' TO ZCVS-TEST-NAME.
           READ LS-OUT INTO R1.
           READ LS-OUT AT END CONTINUE END-READ.
           IF R1 = 'LAST' AND FS1 = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'LAST/10' TO ZCVS-EXPECTED
               MOVE FS1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE LS-OUT.

           MOVE 'LS-SHORT-AREA' TO ZCVS-TEST-NAME.
           OPEN INPUT LS-SHORT.
           READ LS-SHORT INTO S1.
           READ LS-SHORT INTO S1.
           READ LS-SHORT INTO S2.
           READ LS-SHORT INTO S3.
           IF S1 = 'ABCD' AND S2 = 'EFGH' AND S3 = 'IJ'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABCD/EFGH/IJ' TO ZCVS-EXPECTED
               MOVE S2 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           CLOSE LS-SHORT.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * AUDIT: F01; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF114A: QSAM empty full partial fixed blocks and EXTEND
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF114A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT SFILE ASSIGN TO XXXXX001 FILE STATUS FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  SFILE RECORDING MODE F
           RECORD CONTAINS 8 CHARACTERS
           BLOCK CONTAINS 2 RECORDS.
       01  SREC.
           05  REC-NUM PIC 9(4).
           05  REC-TEXT PIC X(4).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS PIC XX.
       01  I PIC 99 COMP.
       01  N PIC 99 COMP.
       01  BAD PIC 99 COMP.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF114A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'BLOCK-0-PAYLOAD' TO ZCVS-TEST-NAME.
           OPEN OUTPUT SFILE.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 0
               MOVE I TO REC-NUM
               MOVE 'DATA' TO REC-TEXT
               WRITE SREC
           END-PERFORM.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO N BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 0
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
               ADD 1 TO N
           END-PERFORM.
           IF BAD = 0 AND N = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-0-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-0-EXTEND' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN EXTEND SFILE.
           MOVE 9999 TO REC-NUM.
           MOVE 'TAIL' TO REC-TEXT.
           WRITE SREC.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 0
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
           END-PERFORM.
           READ SFILE.
           IF FS = '00' AND SREC = '9999TAIL' AND BAD = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999TAIL' TO ZCVS-EXPECTED
               MOVE SREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-0-FINAL-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-1-PAYLOAD' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN OUTPUT SFILE.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 1
               MOVE I TO REC-NUM
               MOVE 'DATA' TO REC-TEXT
               WRITE SREC
           END-PERFORM.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO N BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 1
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
               ADD 1 TO N
           END-PERFORM.
           IF BAD = 0 AND N = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-1-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-1-EXTEND' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN EXTEND SFILE.
           MOVE 9999 TO REC-NUM.
           MOVE 'TAIL' TO REC-TEXT.
           WRITE SREC.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 1
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
           END-PERFORM.
           READ SFILE.
           IF FS = '00' AND SREC = '9999TAIL' AND BAD = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999TAIL' TO ZCVS-EXPECTED
               MOVE SREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-1-FINAL-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-2-PAYLOAD' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN OUTPUT SFILE.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2
               MOVE I TO REC-NUM
               MOVE 'DATA' TO REC-TEXT
               WRITE SREC
           END-PERFORM.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO N BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
               ADD 1 TO N
           END-PERFORM.
           IF BAD = 0 AND N = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-2-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-2-EXTEND' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN EXTEND SFILE.
           MOVE 9999 TO REC-NUM.
           MOVE 'TAIL' TO REC-TEXT.
           WRITE SREC.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
           END-PERFORM.
           READ SFILE.
           IF FS = '00' AND SREC = '9999TAIL' AND BAD = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999TAIL' TO ZCVS-EXPECTED
               MOVE SREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-2-FINAL-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-3-PAYLOAD' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN OUTPUT SFILE.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
               MOVE I TO REC-NUM
               MOVE 'DATA' TO REC-TEXT
               WRITE SREC
           END-PERFORM.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO N BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
               ADD 1 TO N
           END-PERFORM.
           IF BAD = 0 AND N = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-3-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-3-EXTEND' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN EXTEND SFILE.
           MOVE 9999 TO REC-NUM.
           MOVE 'TAIL' TO REC-TEXT.
           WRITE SREC.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
           END-PERFORM.
           READ SFILE.
           IF FS = '00' AND SREC = '9999TAIL' AND BAD = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999TAIL' TO ZCVS-EXPECTED
               MOVE SREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-3-FINAL-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-4-PAYLOAD' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN OUTPUT SFILE.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4
               MOVE I TO REC-NUM
               MOVE 'DATA' TO REC-TEXT
               WRITE SREC
           END-PERFORM.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO N BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
               ADD 1 TO N
           END-PERFORM.
           IF BAD = 0 AND N = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-4-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-4-EXTEND' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN EXTEND SFILE.
           MOVE 9999 TO REC-NUM.
           MOVE 'TAIL' TO REC-TEXT.
           WRITE SREC.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
           END-PERFORM.
           READ SFILE.
           IF FS = '00' AND SREC = '9999TAIL' AND BAD = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999TAIL' TO ZCVS-EXPECTED
               MOVE SREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-4-FINAL-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-5-PAYLOAD' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN OUTPUT SFILE.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
               MOVE I TO REC-NUM
               MOVE 'DATA' TO REC-TEXT
               WRITE SREC
           END-PERFORM.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO N BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
               ADD 1 TO N
           END-PERFORM.
           IF BAD = 0 AND N = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-5-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-5-EXTEND' TO ZCVS-TEST-NAME.
           CLOSE SFILE.
           OPEN EXTEND SFILE.
           MOVE 9999 TO REC-NUM.
           MOVE 'TAIL' TO REC-TEXT.
           WRITE SREC.
           CLOSE SFILE.
           OPEN INPUT SFILE.
           MOVE 0 TO BAD.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
               READ SFILE
               IF FS NOT = '00' OR REC-NUM NOT = I
                   OR REC-TEXT NOT = 'DATA'
                   ADD 1 TO BAD
               END-IF
           END-PERFORM.
           READ SFILE.
           IF FS = '00' AND SREC = '9999TAIL' AND BAD = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999TAIL' TO ZCVS-EXPECTED
               MOVE SREC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'BLOCK-5-FINAL-EOF' TO ZCVS-TEST-NAME.
           READ SFILE.
           IF FS = '10'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE SFILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

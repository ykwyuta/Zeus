      * ZF106A: Variable records min/max/reuse and EOF branches
      * SPEC: RECORD VARYING/READ/FILE STATUS; AUDIT F01.
      * MAXIMUM THEN SHORTER RECORD DETECTS STALE LENGTH STATE.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF106A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT VAR-FILE ASSIGN TO XXXXX001
             ORGANIZATION SEQUENTIAL
             FILE STATUS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  VAR-FILE
           RECORDING MODE V
           RECORD VARYING FROM 4 TO 20 CHARACTERS
             DEPENDING ON WS-LEN.
       01  VAR-REC PIC X(20).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-LEN PIC 9(4) COMP.
       01  WS-FS PIC XX.
       01  WS-END PIC X.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF106A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'VAR-OPEN-OUT' TO ZCVS-TEST-NAME.
           OPEN OUTPUT VAR-FILE.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-WRITE-4' TO ZCVS-TEST-NAME.
           MOVE 4 TO WS-LEN.
           MOVE ALL 'A' TO VAR-REC.
           WRITE VAR-REC.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-WRITE-20' TO ZCVS-TEST-NAME.
           MOVE 20 TO WS-LEN.
           MOVE ALL 'A' TO VAR-REC.
           WRITE VAR-REC.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-WRITE-5' TO ZCVS-TEST-NAME.
           MOVE 5 TO WS-LEN.
           MOVE ALL 'A' TO VAR-REC.
           WRITE VAR-REC.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-CLOSE-OUT' TO ZCVS-TEST-NAME.
           CLOSE VAR-FILE.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-OPEN-IN' TO ZCVS-TEST-NAME.
           OPEN INPUT VAR-FILE.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-READ-4' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-LEN.
           MOVE ALL 'Z' TO VAR-REC.
           READ VAR-FILE.
           IF WS-FS = '00' AND WS-LEN = 4
             AND VAR-REC(1:4) = ALL 'A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/LENGTH 4/A' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-READ-20' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-LEN.
           MOVE ALL 'Z' TO VAR-REC.
           READ VAR-FILE.
           IF WS-FS = '00' AND WS-LEN = 20
             AND VAR-REC(1:20) = ALL 'A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/LENGTH 20/A' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-READ-5' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-LEN.
           MOVE ALL 'Z' TO VAR-REC.
           READ VAR-FILE.
           IF WS-FS = '00' AND WS-LEN = 5
             AND VAR-REC(1:5) = ALL 'A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/LENGTH 5/A' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-EOF' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-END.
           READ VAR-FILE
             AT END MOVE 'Y' TO WS-END
             NOT AT END MOVE 'X' TO WS-END
           END-READ.
           IF WS-FS = '10' AND WS-END = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10/Y' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-CLOSE-IN' TO ZCVS-TEST-NAME.
           CLOSE VAR-FILE.
           IF WS-FS = '00'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VAR-REOPEN' TO ZCVS-TEST-NAME.
           OPEN INPUT VAR-FILE.
           READ VAR-FILE.
           IF WS-FS = '00' AND WS-LEN = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/LENGTH 4' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
           CLOSE VAR-FILE.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

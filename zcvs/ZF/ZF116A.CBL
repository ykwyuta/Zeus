      * AUDIT: F01; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZF116A: a print file (WRITE AFTER ADVANCING) is written with
      * ASA control characters. With the ADV option (the default) the
      * record is one byte longer than the FD record and the first
      * byte is the control character: blank, 0 and - space 1, 2 and 3
      * lines, + suppresses spacing, 1 skips to channel 1 (C01 and
      * PAGE). The data set is read back through a second FD of 81
      * bytes that is not a print file.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF116A.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           C01 IS TOP-OF-FORM.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT PRT-OUT ASSIGN TO PRTFILE FILE STATUS FS1.
           SELECT PRT-IN ASSIGN TO PRTFILE FILE STATUS FS2.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       FD  PRT-OUT
           RECORDING MODE F
           RECORD CONTAINS 80 CHARACTERS.
       01  PRT-REC PIC X(80).
       FD  PRT-IN
           RECORDING MODE F
           RECORD CONTAINS 81 CHARACTERS.
       01  IN-REC.
           05 IN-CC PIC X.
           05 IN-DATA PIC X(80).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  FS1 PIC XX.
       01  FS2 PIC XX.
       01  CODES PIC X(7) VALUE SPACES.
       01  DATAS PIC X(7) VALUE SPACES.
       01  K PIC 9.
       01  N PIC 9 VALUE 0.
       01  EOF-SEEN PIC X VALUE 'N'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF116A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.
           OPEN OUTPUT PRT-OUT.
           MOVE 'A' TO PRT-REC.
           WRITE PRT-REC AFTER ADVANCING 1 LINE.
           MOVE 'B' TO PRT-REC.
           WRITE PRT-REC AFTER ADVANCING 2 LINES.
           MOVE 'C' TO PRT-REC.
           WRITE PRT-REC AFTER ADVANCING 3 LINES.
           MOVE 'D' TO PRT-REC.
           WRITE PRT-REC AFTER ADVANCING 0 LINES.
           MOVE 'E' TO PRT-REC.
           WRITE PRT-REC AFTER ADVANCING TOP-OF-FORM.
           MOVE 'F' TO PRT-REC.
           WRITE PRT-REC AFTER ADVANCING PAGE.
           MOVE 'G' TO PRT-REC.
           WRITE PRT-REC.
           CLOSE PRT-OUT.
           OPEN INPUT PRT-IN.
           PERFORM VARYING K FROM 1 BY 1 UNTIL K > 7
               READ PRT-IN
                   AT END MOVE 'Y' TO EOF-SEEN
                   NOT AT END
                       ADD 1 TO N
                       MOVE IN-CC TO CODES(K:1)
                       MOVE IN-DATA(1:1) TO DATAS(K:1)
               END-READ
           END-PERFORM.
           READ PRT-IN AT END MOVE 'Y' TO EOF-SEEN.
           CLOSE PRT-IN.

           MOVE 'PRT-C01-SKIP' TO ZCVS-TEST-NAME.
           IF CODES(5:1) = '1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE CODES(5:1) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PRT-PAGE-SKIP' TO ZCVS-TEST-NAME.
           IF CODES(6:1) = '1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE CODES(6:1) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PRT-ASA-SPACING' TO ZCVS-TEST-NAME.
           IF CODES(1:3) = ' 0-'
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' 0-' TO ZCVS-EXPECTED
               MOVE CODES(1:3) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PRT-ASA-OVERPRINT' TO ZCVS-TEST-NAME.
           IF CODES(4:1) = '+'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '+' TO ZCVS-EXPECTED
               MOVE CODES(4:1) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PRT-NO-ADVANCING' TO ZCVS-TEST-NAME.
           IF CODES(7:1) = ' '
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' ' TO ZCVS-EXPECTED
               MOVE CODES(7:1) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PRT-ADV-LENGTH' TO ZCVS-TEST-NAME.
           IF DATAS = 'ABCDEFG' AND N = 7 AND EOF-SEEN = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABCDEFG' TO ZCVS-EXPECTED
               MOVE DATAS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.
           COPY ZCVSPROC.

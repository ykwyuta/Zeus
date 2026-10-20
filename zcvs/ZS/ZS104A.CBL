       CBL TRUNC(STD),DBCS,NOSEQ,NOD
      *================================================================*
      * ZS104A - IDIOMS OF PRODUCTION SOURCES (PROPOSAL 0022)          *
      *   BATCH COMPILATION WITH ITS OWN CBL OPTIONS PER PROGRAM,      *
      *   COPY REPLACING OF A LITERAL WRITTEN BEFORE A HYPHEN, REPLACE *
      *   OF :TAG: BEFORE A HYPHEN, FILE NAMES ENDED BY ELSE / END-IF, *
      *   INSPECT TALLYING OF A FUNCTION RESULT                        *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS104A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT WORK-FILE ASSIGN TO XXXXX001
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  WORK-FILE
           RECORDING MODE F
           RECORD CONTAINS 10 CHARACTERS.
       01  WORK-REC                 PIC X(10).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
           COPY ZCVSLIT REPLACING 'X' BY WS-LIT.
           REPLACE ==:TAG:== BY ==WS-TAG==.
       01  :TAG:-AREA.
           05  :TAG:-VALUE          PIC X(4)  VALUE 'TAG1'.
           REPLACE OFF.
       01  WS-FIELDS.
           05  WS-FS                PIC XX    VALUE SPACES.
           05  WS-FIRST             PIC X     VALUE 'Y'.
           05  WS-STEPS             PIC 9(4)  VALUE 0.
           05  WS-TEXT              PIC X(8)  VALUE 'ABC'.
           05  WS-TRAIL             PIC 9(4)  VALUE 0.
           05  WS-BIN               PIC 9(4)  COMP.
           05  WS-OUT               PIC 9(5)  VALUE 0.
           05  WS-NUM               PIC 9(5)  VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZS104A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: COPY REPLACING 'X' BY WS-LIT MAKES 'X'-KEY WS-LIT-KEY
           MOVE 'COPY-LITERAL-WORD' TO ZCVS-TEST-NAME.
           IF WS-LIT-KEY = 'LIT1' AND WS-LIT-CNT = 7
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'LIT1/0007' TO ZCVS-EXPECTED
               MOVE WS-LIT-KEY TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: REPLACE ==:TAG:== MAKES :TAG:-VALUE WS-TAG-VALUE
           MOVE 'REPLACE-TAG-WORD' TO ZCVS-TEST-NAME.
           IF WS-TAG-VALUE = 'TAG1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TAG1' TO ZCVS-EXPECTED
               MOVE WS-TAG-VALUE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: OPEN AND CLOSE FILE NAME LISTS END AT ELSE
           MOVE 'OPEN-CLOSE-ELSE' TO ZCVS-TEST-NAME.
           IF WS-FIRST = 'Y'
               OPEN OUTPUT WORK-FILE;
           ELSE
               ADD 100 TO WS-STEPS
           END-IF.
           IF WS-FS = '00'
               WRITE WORK-REC FROM 'ZS104A'
           END-IF.
           IF WS-FIRST = 'Y'
               CLOSE WORK-FILE
           ELSE
               ADD 100 TO WS-STEPS
           END-IF.
           IF WS-FS = '00' AND WS-STEPS = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00/0000' TO ZCVS-EXPECTED
               MOVE WS-FS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: INSPECT TALLYING OF FUNCTION REVERSE COUNTS TRAILING
      *          SPACES OF WS-TEXT
           MOVE 'INSPECT-FUNCTION' TO ZCVS-TEST-NAME.
           INSPECT FUNCTION REVERSE(WS-TEXT)
               TALLYING WS-TRAIL FOR LEADING SPACES.
           IF WS-TRAIL = 5 AND WS-TEXT = 'ABC'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0005' TO ZCVS-EXPECTED
               MOVE WS-TRAIL TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: EACH PROGRAM OF A BATCH COMPILATION HAS ITS OWN CBL
      *          OPTIONS: TRUNC(STD) HERE, TRUNC(BIN) IN ZS104S
           MOVE 'BATCH-OPTIONS' TO ZCVS-TEST-NAME.
           COMPUTE WS-BIN = 12000 + 345.
           MOVE WS-BIN TO WS-NUM.
           CALL 'ZS104S' USING WS-OUT.
           IF WS-NUM = 2345 AND WS-OUT = 12345
               PERFORM ZCVS-PASS
           ELSE
               MOVE '02345/12345' TO ZCVS-EXPECTED
               MOVE WS-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 0 TO RETURN-CODE.
           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.
       END PROGRAM ZS104A.
       CBL TRUNC(BIN)
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS104S.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-BIN                   PIC 9(4)  COMP.
       LINKAGE SECTION.
       01  LS-OUT                   PIC 9(5).
       PROCEDURE DIVISION USING LS-OUT.
           COMPUTE WS-BIN = 12000 + 345.
           MOVE WS-BIN TO LS-OUT.
           GOBACK.
       END PROGRAM ZS104S.

      *================================================================*
      * ZF103A - CODE-SET CLAUSE (ASCII TRANSLATION & RDW PRESERVATION)*
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZF103A.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           ALPHABET ASCII-CS IS STANDARD-1.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
           SELECT CS-FILE ASSIGN TO XXXXX001
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT RAW-FILE ASSIGN TO XXXXX001
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       FD  CS-FILE
           RECORDING MODE F
           RECORD CONTAINS 9 CHARACTERS
           CODE-SET IS ASCII-CS.
       01  CS-REC.
           05  CS-TXT               PIC X(4).
           05  CS-NUM               PIC S9(4) SIGN TRAILING SEPARATE.
       FD  RAW-FILE
           RECORDING MODE F
           RECORD CONTAINS 9 CHARACTERS.
       01  RAW-REC                  PIC X(9).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-FS                    PIC X(2).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZF103A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: WRITE VIA CODE-SET STANDARD-1 STORES ASCII BYTES
           MOVE 'CODESET-ASCII-HEX' TO ZCVS-TEST-NAME.
           OPEN OUTPUT CS-FILE.
           MOVE 'ABCD' TO CS-TXT.
           MOVE -1234 TO CS-NUM.
           WRITE CS-REC.
           CLOSE CS-FILE.

           OPEN INPUT RAW-FILE.
           READ RAW-FILE.
           CLOSE RAW-FILE.
           MOVE RAW-REC TO ZCVS-HEX-IN.
           MOVE 9 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:18) = '41424344313233342D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '41424344313233342D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:18) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: READ VIA CODE-SET STANDARD-1 TRANSLATES BACK TO EBCDIC
           MOVE 'CODESET-EBCDIC-RT' TO ZCVS-TEST-NAME.
           OPEN INPUT CS-FILE.
           MOVE SPACES TO CS-TXT.
           MOVE 0 TO CS-NUM.
           READ CS-FILE.
           CLOSE CS-FILE.
           IF WS-FS = '00'
             AND CS-TXT = 'ABCD'
             AND CS-NUM = -1234
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABCD/-1234' TO ZCVS-EXPECTED
               MOVE CS-TXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

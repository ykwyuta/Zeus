      *================================================================*
      * ZP106A - NESTED PROGRAMS, GLOBAL, COMMON, AND EXTERNAL         *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP106A.
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
       01  WS-GLOBAL-INFO GLOBAL.
           05  WS-GLB-TAG           PIC X(4) VALUE 'INIT'.
           05  WS-GLB-COUNT         PIC 9(4) COMP VALUE 100.
       01  EX-SHARED-BLOCK EXTERNAL.
           05  EX-CODE              PIC X(4).
           05  EX-AMOUNT            PIC S9(7) COMP-3.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP106A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: NESTED PROGRAM ACCESSING GLOBAL AND COMMON SIBLING
           MOVE 'NEST-GLOBAL-COMMON' TO ZCVS-TEST-NAME.
           CALL 'ZPNEST1'.
           IF WS-GLB-TAG = 'CMN2' AND WS-GLB-COUNT = 135
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'CMN2/135' TO ZCVS-EXPECTED
               MOVE WS-GLB-TAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: EXTERNAL DATA SHARED WITH SEPARATE SUBPROGRAM ZPS06A
           MOVE 'EXTERNAL-SHARED' TO ZCVS-TEST-NAME.
           MOVE 'MAIN' TO EX-CODE.
           MOVE 1000 TO EX-AMOUNT.
           CALL 'ZPS06A'.
           IF EX-CODE = 'SUB6' AND EX-AMOUNT = 3500
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SUB6/3500' TO ZCVS-EXPECTED
               MOVE EX-CODE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPNEST1.
       PROCEDURE DIVISION.
       N1-MAIN.
           ADD 10 TO WS-GLB-COUNT.
           CALL 'ZPNEST2'.
           GOBACK.
       END PROGRAM ZPNEST1.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPNEST2 IS COMMON.
       PROCEDURE DIVISION.
       N2-MAIN.
           MOVE 'CMN2' TO WS-GLB-TAG.
           ADD 25 TO WS-GLB-COUNT.
           GOBACK.
       END PROGRAM ZPNEST2.
       END PROGRAM ZP106A.

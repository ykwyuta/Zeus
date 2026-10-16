      *================================================================*
      * ZPS02A - RECURSIVE FACTORIAL SUBPROGRAM                        *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS02A IS RECURSIVE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-TOTAL-CALLS           PIC 9(4) COMP VALUE 0.
       LOCAL-STORAGE SECTION.
       01  LS-SAVED-N               PIC S9(9) COMP VALUE 0.
       01  LS-SUB-N                 PIC S9(9) COMP VALUE 0.
       01  LS-SUB-RES               PIC S9(9) COMP VALUE 0.
       LINKAGE SECTION.
       01  LK-N                     PIC S9(9) COMP.
       01  LK-RES                   PIC S9(9) COMP.
       01  LK-CALLS                 PIC 9(4) COMP.
       PROCEDURE DIVISION USING LK-N LK-RES LK-CALLS.
       MAIN-REC.
           ADD 1 TO WS-TOTAL-CALLS.
           MOVE LK-N TO LS-SAVED-N.
           IF LS-SAVED-N <= 1
               MOVE 1 TO LK-RES
           ELSE
               COMPUTE LS-SUB-N = LS-SAVED-N - 1
               CALL 'ZPS02A' USING LS-SUB-N LS-SUB-RES LK-CALLS
               COMPUTE LK-RES = LS-SAVED-N * LS-SUB-RES
           END-IF.
           MOVE WS-TOTAL-CALLS TO LK-CALLS.
           GOBACK.
       END PROGRAM ZPS02A.

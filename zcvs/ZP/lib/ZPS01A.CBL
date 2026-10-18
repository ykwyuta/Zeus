      *================================================================*
      * ZPS01A - SUBPROGRAM WITH WORKING-STORAGE AND LOCAL-STORAGE     *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS01A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-STATIC-COUNT          PIC 9(4) COMP VALUE 10.
       LOCAL-STORAGE SECTION.
       01  LS-AUTO-COUNT            PIC 9(4) COMP VALUE 100.
       01  LS-GROUP.
           05  LS-TAG               PIC X(2) VALUE 'LS'.
           05  LS-TBL               PIC 9(2) VALUE 5 OCCURS 2 TIMES.
       LINKAGE SECTION.
       01  LK-OUT-WS                PIC 9(4) COMP.
       01  LK-OUT-LS                PIC 9(4) COMP.
       01  LK-OUT-GRP               PIC X(6).
       PROCEDURE DIVISION USING LK-OUT-WS LK-OUT-LS LK-OUT-GRP.
       MAIN-SUB.
           ADD 1 TO WS-STATIC-COUNT.
           ADD 1 TO LS-AUTO-COUNT.
           MOVE WS-STATIC-COUNT TO LK-OUT-WS.
           MOVE LS-AUTO-COUNT TO LK-OUT-LS.
           MOVE LS-GROUP TO LK-OUT-GRP.
           ADD 9 TO LS-TBL(1) LS-TBL(2).
           MOVE 'XX' TO LS-TAG.
           GOBACK.
       END PROGRAM ZPS01A.

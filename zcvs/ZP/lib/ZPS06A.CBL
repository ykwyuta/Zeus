      *================================================================*
      * ZPS06A - SUBPROGRAM READING AND MODIFYING EXTERNAL STORAGE     *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS06A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  EX-SHARED-BLOCK EXTERNAL.
           05  EX-CODE              PIC X(4).
           05  EX-AMOUNT            PIC S9(7) COMP-3.
       PROCEDURE DIVISION.
       MAIN-EXT.
           MOVE 'SUB6' TO EX-CODE.
           ADD 2500 TO EX-AMOUNT.
           GOBACK.
       END PROGRAM ZPS06A.

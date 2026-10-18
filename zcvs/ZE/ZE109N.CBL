      *================================================================*
      * ZE109N - NEGATIVE TEST: EXTERNAL IN LOCAL-STORAGE SECTION      *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE109N.
       DATA DIVISION.
       LOCAL-STORAGE SECTION.
       01  LS-EXT                   PIC X(4) EXTERNAL.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE109N.

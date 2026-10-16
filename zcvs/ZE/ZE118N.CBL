      *================================================================*
      * ZE118N - NEGATIVE TEST: RECORDING MODE U WITH BLOCK CONTAINS   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE118N.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-UNBLK ASSIGN TO OUTFILE.
       DATA DIVISION.
       FILE SECTION.
       FD  F-UNBLK
           RECORDING MODE U
           BLOCK CONTAINS 80 CHARACTERS.
       01  F-REC                    PIC X(80).
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE118N.

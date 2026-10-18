      *================================================================*
      * ZE117N - NEGATIVE TEST: RECORDING MODE S WITHOUT BLOCK CONTAINS*
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE117N.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-SPAN ASSIGN TO OUTFILE.
       DATA DIVISION.
       FILE SECTION.
       FD  F-SPAN
           RECORDING MODE S.
       01  F-REC                    PIC X(20).
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE117N.

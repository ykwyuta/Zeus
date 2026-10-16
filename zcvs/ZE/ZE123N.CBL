      *================================================================*
      * ZE123N - NEGATIVE TEST: USER TAPE LABEL RECORDS CLAUSE         *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE123N.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-LBL ASSIGN TO OUTFILE.
       DATA DIVISION.
       FILE SECTION.
       FD  F-LBL
           LABEL RECORDS ARE USER-LBL.
       01  F-REC                    PIC X(10).
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE123N.

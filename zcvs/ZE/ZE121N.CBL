      *================================================================*
      * ZE121N - NEGATIVE TEST: MULTI-CHAR PADDING CHARACTER LITERAL   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE121N.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-PAD ASSIGN TO OUTFILE
               PADDING CHARACTER IS 'AB'.
       DATA DIVISION.
       FILE SECTION.
       FD  F-PAD.
       01  F-REC                    PIC X(10).
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE121N.

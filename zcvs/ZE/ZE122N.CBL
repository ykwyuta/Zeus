      *================================================================*
      * ZE122N - NEGATIVE TEST: UNSUPPORTED RERUN CHECKPOINT CLAUSE    *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE122N.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-SEQ ASSIGN TO OUTFILE.
       I-O-CONTROL.
           RERUN ON F-SEQ EVERY 100 RECORDS.
       DATA DIVISION.
       FILE SECTION.
       FD  F-SEQ.
       01  F-REC                    PIC X(10).
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE122N.

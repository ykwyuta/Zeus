      *================================================================*
      * ZE119N - NEGATIVE TEST: CODE-SET WITH OVERPUNCH SIGNED NUMERIC *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE119N.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           ALPHABET ASCII-CS IS STANDARD-1.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-CS ASSIGN TO OUTFILE.
       DATA DIVISION.
       FILE SECTION.
       FD  F-CS
           CODE-SET IS ASCII-CS.
       01  F-REC                    PIC S9(4).
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE119N.

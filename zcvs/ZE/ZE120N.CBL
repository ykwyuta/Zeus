      *================================================================*
      * ZE120N - NEGATIVE TEST: 2-PARAM FILE STATUS ON SEQUENTIAL FILE *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE120N.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F-SEQ ASSIGN TO OUTFILE
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS WS-FS WS-EXT.
       DATA DIVISION.
       FILE SECTION.
       FD  F-SEQ.
       01  F-REC                    PIC X(10).
       WORKING-STORAGE SECTION.
       01  WS-FS                    PIC X(2).
       01  WS-EXT.
           05  WS-RC                PIC 9(4) COMP-5.
           05  WS-COMP              PIC 9(4) COMP-5.
           05  WS-FDBK              PIC 9(4) COMP-5.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE120N.

      *================================================================*
      * ZE107N - NEGATIVE TEST: INITIALIZE RENAMES ITEM TO VALUE       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE107N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-REC.
           05  WS-A                 PIC X(2) VALUE 'AB'.
           05  WS-B                 PIC X(2) VALUE 'CD'.
       66  WS-ALIAS RENAMES WS-A THRU WS-B.
       PROCEDURE DIVISION.
           INITIALIZE WS-ALIAS ALL TO VALUE.
           STOP RUN.
       END PROGRAM ZE107N.

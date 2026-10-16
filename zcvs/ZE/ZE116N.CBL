      *================================================================*
      * ZE116N - NEGATIVE TEST: PROCEDURE-POINTER NON-NULL VALUE       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE116N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-PP                    USAGE PROCEDURE-POINTER VALUE 1.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE116N.

      *================================================================*
      * ZE103N - NEGATIVE TEST: SET TO FALSE WITHOUT FALSE CLAUSE      *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE103N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-FLAG                  PIC 9(1).
           88  FLG-ACTIVE           VALUE 1.
       PROCEDURE DIVISION.
           SET FLG-ACTIVE TO FALSE.
           STOP RUN.
       END PROGRAM ZE103N.

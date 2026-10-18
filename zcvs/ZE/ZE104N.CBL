      *================================================================*
      * ZE104N - NEGATIVE TEST: NON-LITERAL IN WHEN SET TO FALSE       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE104N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-OTHER                 PIC 9(1) VALUE 0.
       01  WS-FLAG                  PIC 9(1).
           88  FLG-ACTIVE           VALUE 1
                                    WHEN SET TO FALSE WS-OTHER.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE104N.

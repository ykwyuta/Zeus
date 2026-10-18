      *================================================================*
      * ZE115N - NEGATIVE TEST: BINARY ITEM IN GROUP-USAGE NATIONAL    *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE115N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-NGROUP                GROUP-USAGE NATIONAL.
           05  WS-BAD-BIN           PIC 9(4) BINARY.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE115N.

      *================================================================*
      * ZE108N - NEGATIVE TEST: INITIALIZE POINTER RECEIVER TO VALUE   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE108N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-PTR                   USAGE POINTER.
       PROCEDURE DIVISION.
           INITIALIZE WS-PTR ALL TO VALUE.
           STOP RUN.
       END PROGRAM ZE108N.

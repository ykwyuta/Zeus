      *================================================================*
      * ZE106N - NEGATIVE TEST: DUPLICATE CATEGORY IN INITIALIZE       *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE106N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-A                     PIC X(4).
       PROCEDURE DIVISION.
           INITIALIZE WS-A
             REPLACING ALPHANUMERIC DATA BY 'AAAA'
                       ALPHANUMERIC DATA BY 'BBBB'.
           STOP RUN.
       END PROGRAM ZE106N.

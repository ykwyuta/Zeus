      *================================================================*
      * ZE114N - NEGATIVE TEST: SIGNED NATIONAL WITHOUT SIGN SEPARATE  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE114N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-BAD-NAT               PIC S9(3) NATIONAL.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE114N.

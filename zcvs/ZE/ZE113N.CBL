      *================================================================*
      * ZE113N - NEGATIVE TEST: RETURNING COMBINED WITH ENTRY          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE113N.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-RESULT                PIC 9(4) COMP.
       PROCEDURE DIVISION RETURNING LK-RESULT.
           ENTRY 'ALTENT'.
           GOBACK.
       END PROGRAM ZE113N.

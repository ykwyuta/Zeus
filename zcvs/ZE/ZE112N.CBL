      *================================================================*
      * ZE112N - NEGATIVE TEST: RETURNING ITEM NOT IN LINKAGE SECTION  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE112N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-RESULT                PIC 9(4) COMP.
       PROCEDURE DIVISION RETURNING WS-RESULT.
           MOVE 1 TO WS-RESULT.
           GOBACK.
       END PROGRAM ZE112N.

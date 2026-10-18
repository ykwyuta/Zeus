      *================================================================*
      * ZPS05A - TARGET SUBPROGRAM FOR PROCEDURE/FUNCTION-POINTER      *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS05A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-CALL-SEQ              PIC 9(4) COMP VALUE 0.
       LINKAGE SECTION.
       01  LK-RET                   PIC 9(4) COMP.
       PROCEDURE DIVISION RETURNING LK-RET.
       MAIN-FP.
           ADD 10 TO WS-CALL-SEQ.
           MOVE WS-CALL-SEQ TO LK-RET.
           GOBACK.
       END PROGRAM ZPS05A.

      *================================================================*
      * ZPS03A - SUBPROGRAM WITH PROCEDURE DIVISION RETURNING          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS03A.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-A                     PIC S9(9) COMP-5.
       01  LK-B                     PIC S9(9) COMP-5.
       01  LK-RESULT                PIC S9(9) COMP-5.
       PROCEDURE DIVISION USING LK-A LK-B RETURNING LK-RESULT.
       MAIN-RET.
           COMPUTE LK-RESULT = LK-A * 100 + LK-B.
           MOVE 99 TO RETURN-CODE.
           GOBACK.
       END PROGRAM ZPS03A.

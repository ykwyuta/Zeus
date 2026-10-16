      *================================================================*
      * ZE110N - NEGATIVE TEST: ALTER PROHIBITED IN RECURSIVE PROGRAM  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE110N IS RECURSIVE.
       PROCEDURE DIVISION.
       P-MAIN.
           ALTER P-JUMP TO PROCEED TO P-DONE.
       P-JUMP.
           GO TO P-DONE.
       P-DONE.
           GOBACK.
       END PROGRAM ZE110N.

      *================================================================*
      * ZE111N - NEGATIVE TEST: OPERANDLESS GO TO IN RECURSIVE PROGRAM *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE111N IS RECURSIVE.
       PROCEDURE DIVISION.
       P-MAIN.
           GO TO.
       P-DONE.
           GOBACK.
       END PROGRAM ZE111N.

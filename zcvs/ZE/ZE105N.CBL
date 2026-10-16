      *================================================================*
      * ZE105N - NEGATIVE TEST: PERFORM TEST AFTER UNTIL EXIT          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE105N.
       PROCEDURE DIVISION.
           PERFORM WITH TEST AFTER UNTIL EXIT
               CONTINUE
           END-PERFORM.
           STOP RUN.
       END PROGRAM ZE105N.

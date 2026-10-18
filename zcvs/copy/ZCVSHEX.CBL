      *================================================================*
      * ZCVSHEX - CONVERT ZCVS-HEX-IN(1:ZCVS-HEX-LEN) TO ZCVS-HEX-OUT  *
      *================================================================*
       ZCVS-TO-HEX SECTION.
       ZCVS-TO-HEX-P.
           MOVE SPACES TO ZCVS-HEX-OUT.
           PERFORM VARYING ZCVS-HEX-IDX FROM 1 BY 1
             UNTIL ZCVS-HEX-IDX > ZCVS-HEX-LEN
               MOVE LOW-VALUES TO ZCVS-HEX-PAD
               MOVE ZCVS-HEX-IN(ZCVS-HEX-IDX:1) TO ZCVS-HEX-CH
               DIVIDE ZCVS-HEX-WORD BY 16
                 GIVING ZCVS-HEX-HI REMAINDER ZCVS-HEX-LO
               ADD 1 TO ZCVS-HEX-HI
               ADD 1 TO ZCVS-HEX-LO
               COMPUTE ZCVS-HEX-OUT-POS =
                 (ZCVS-HEX-IDX - 1) * 2 + 1
               MOVE ZCVS-HEX-TABLE(ZCVS-HEX-HI:1)
                 TO ZCVS-HEX-OUT(ZCVS-HEX-OUT-POS:1)
               ADD 1 TO ZCVS-HEX-OUT-POS
               MOVE ZCVS-HEX-TABLE(ZCVS-HEX-LO:1)
                 TO ZCVS-HEX-OUT(ZCVS-HEX-OUT-POS:1)
           END-PERFORM.
       ZCVS-TO-HEX-EXIT.
           EXIT.

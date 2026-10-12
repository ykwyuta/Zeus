       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZSF.
      * Sends a Read Partition Query and receives the reply.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-QUERY  PIC X(5) VALUE X'000501FF02'.
       01 WS-IN     PIC X(200).
       01 WS-LEN    PIC S9(4) COMP VALUE 200.
       01 WS-LINE.
          05 FILLER PIC X(10) VALUE 'SF BYTES: '.
          05 WS-N   PIC 999.
          05 FILLER PIC X(6) VALUE ' AID: '.
          05 WS-AID PIC X(2).
       PROCEDURE DIVISION.
           EXEC CICS SEND STRFIELD FROM(WS-QUERY) LENGTH(5) END-EXEC
           EXEC CICS RECEIVE INTO(WS-IN) LENGTH(WS-LEN) END-EXEC
           MOVE WS-LEN TO WS-N
           IF EIBAID = X'88'
              MOVE 'SF' TO WS-AID
           ELSE
              MOVE '??' TO WS-AID
           END-IF
           EXEC CICS SEND TEXT FROM(WS-LINE) LENGTH(21) ERASE END-EXEC
           EXEC CICS RETURN END-EXEC.

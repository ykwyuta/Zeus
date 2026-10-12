       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZLONG.
      * A long task: it holds a resource (ENQ), computes for a while and
      * waits (DELAY). Other terminals' tasks run meanwhile; another
      * ZLONG finds the resource busy.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-RESP   PIC S9(8) COMP.
       01 WS-I      PIC S9(9) COMP.
       01 WS-SUM    PIC S9(18) COMP VALUE 0.
       01 WS-LIMIT  PIC S9(9) COMP VALUE 8000000.
       01 WS-LINE.
          05 FILLER PIC X(12) VALUE 'ZLONG: DONE '.
          05 WS-D   PIC 9(15).
       PROCEDURE DIVISION.
           EXEC CICS ENQ RESOURCE('ZLONGRES') LENGTH(8) NOSUSPEND
                RESP(WS-RESP)
           END-EXEC
           IF WS-RESP = DFHRESP(ENQBUSY)
              EXEC CICS SEND TEXT FROM('ZLONG: BUSY') LENGTH(11) ERASE
              END-EXEC
              EXEC CICS RETURN END-EXEC
           END-IF
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-LIMIT
              ADD WS-I TO WS-SUM
           END-PERFORM
           EXEC CICS DELAY FOR SECONDS(2) END-EXEC
           MOVE WS-SUM TO WS-D
           EXEC CICS SEND TEXT FROM(WS-LINE) LENGTH(27) ERASE END-EXEC
           EXEC CICS DEQ RESOURCE('ZLONGRES') LENGTH(8) END-EXEC
           EXEC CICS RETURN END-EXEC.

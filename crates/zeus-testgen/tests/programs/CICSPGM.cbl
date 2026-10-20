       IDENTIFICATION DIVISION.
       PROGRAM-ID. CICSPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-RESP               PIC S9(8) COMP.
       01  WS-LEN                PIC S9(4) COMP VALUE 8.
       01  WS-KEY                PIC X(8).
       01  WS-COUNT              PIC 9(3) VALUE 0.
       01  WS-INPUT.
           05  IN-CMD            PIC X(4).
           05  IN-ACCT           PIC X(8).
       01  WS-ACCT.
           05  AC-ID             PIC X(8).
           05  AC-STATUS         PIC X.
           05  AC-BAL            PIC S9(7)V99 COMP-3.
       LINKAGE SECTION.
       01  DFHCOMMAREA.
           05  CA-STEP           PIC X.
       PROCEDURE DIVISION.
       MAIN-PARA.
           IF EIBCALEN = 0
               MOVE LOW-VALUES TO WS-INPUT
               EXEC CICS RETURN TRANSID('ACCT') END-EXEC
           END-IF
           EXEC CICS RECEIVE INTO(WS-INPUT) LENGTH(WS-LEN)
               RESP(WS-RESP) END-EXEC
           IF WS-RESP NOT = DFHRESP(NORMAL)
               DISPLAY 'RECEIVE FAILED'
               EXEC CICS RETURN END-EXEC
           END-IF
           EVALUATE IN-CMD
               WHEN 'SHOW'
                   PERFORM SHOW-PARA
               WHEN 'LIST'
                   PERFORM LIST-PARA
               WHEN OTHER
                   DISPLAY 'BAD COMMAND'
           END-EVALUATE
           EXEC CICS RETURN TRANSID('ACCT') COMMAREA(CA-STEP)
           END-EXEC
           GOBACK.
       SHOW-PARA.
           EXEC CICS READ FILE('ACCTDAT') INTO(WS-ACCT)
               RIDFLD(IN-ACCT) RESP(WS-RESP) END-EXEC
           EVALUATE WS-RESP
               WHEN DFHRESP(NORMAL)
                   IF AC-STATUS = 'C'
                       DISPLAY 'CLOSED ' AC-ID
                   ELSE
                       IF AC-BAL > 1000
                           DISPLAY 'RICH ' AC-ID
                       ELSE
                           DISPLAY 'ACCOUNT ' AC-ID
                       END-IF
                   END-IF
               WHEN DFHRESP(NOTFND)
                   DISPLAY 'NOT FOUND'
               WHEN OTHER
                   DISPLAY 'READ ERROR'
           END-EVALUATE.
       LIST-PARA.
           EXEC CICS HANDLE CONDITION ENDFILE(LIST-END) END-EXEC
           MOVE LOW-VALUES TO WS-KEY
           EXEC CICS STARTBR FILE('ACCTDAT') RIDFLD(WS-KEY) END-EXEC
           PERFORM UNTIL WS-COUNT > 5
               EXEC CICS READNEXT FILE('ACCTDAT') INTO(WS-ACCT)
                   RIDFLD(WS-KEY) END-EXEC
               ADD 1 TO WS-COUNT
           END-PERFORM
           DISPLAY 'LIST FULL'.
       LIST-END.
           DISPLAY 'LISTED ' WS-COUNT.

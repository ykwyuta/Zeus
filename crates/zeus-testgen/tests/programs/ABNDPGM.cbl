       IDENTIFICATION DIVISION.
       PROGRAM-ID. ABNDPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-RESP               PIC S9(8) COMP.
       01  WS-CODE               PIC X(4).
       01  WS-KEY                PIC X(4).
       01  WS-REC                PIC X(20).
       01  WS-AMT                COMP-2.
       LINKAGE SECTION.
       01  DFHCOMMAREA.
           05  CA-AMT-LEN        PIC S9(4) COMP.
           05  CA-AMT            PIC X(8).
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC CICS HANDLE ABEND LABEL(ABEND-PARA) END-EXEC
           IF CA-AMT(1:CA-AMT-LEN) IS NUMERIC
               COMPUTE WS-AMT = FUNCTION NUMVAL(CA-AMT(1:CA-AMT-LEN))
               IF WS-AMT > 5
                   DISPLAY 'BIG AMOUNT'
               END-IF
           END-IF
           EXEC CICS READ FILE('ACCT') INTO(WS-REC) RIDFLD(WS-KEY)
               RESP(WS-RESP) END-EXEC
           IF WS-RESP NOT = DFHRESP(NORMAL)
               EXEC CICS ABEND ABCODE('HRAC') END-EXEC
           END-IF
           EXEC CICS RETURN END-EXEC.
       ABEND-PARA.
           EXEC CICS ASSIGN ABCODE(WS-CODE) END-EXEC
           EVALUATE WS-CODE
               WHEN 'HRAC'
                   DISPLAY 'READ FAILED'
               WHEN 'AD2Z'
                   DISPLAY 'DEADLOCK'
               WHEN OTHER
                   DISPLAY 'OTHER ABEND'
           END-EVALUATE
           EXEC CICS RETURN END-EXEC.

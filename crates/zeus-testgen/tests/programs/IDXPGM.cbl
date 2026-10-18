       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDXPGM.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT ACCT-FILE ASSIGN TO ACCTDD
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS AC-ID
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  ACCT-FILE.
       01  ACCT-REC.
           05  AC-ID             PIC X(4).
           05  AC-STATUS         PIC X.
               88  AC-ACTIVE     VALUE 'A'.
           05  AC-BAL            PIC S9(5) COMP-3.
       WORKING-STORAGE SECTION.
       01  WS-FS                 PIC XX.
       01  WS-LISTED             PIC 9(3) VALUE 0.
       LINKAGE SECTION.
       01  LK-REQ.
           05  LK-FUNC           PIC X.
           05  LK-ID             PIC X(4).
           05  LK-AMT            PIC 9(3).
           05  LK-MSG            PIC X(12).
       PROCEDURE DIVISION USING LK-REQ.
       MAIN-PARA.
           OPEN I-O ACCT-FILE
           EVALUATE LK-FUNC
               WHEN 'P'
                   PERFORM POST-PARA
               WHEN 'N'
                   PERFORM NEW-PARA
               WHEN 'L'
                   PERFORM LIST-PARA
               WHEN OTHER
                   MOVE 'BAD FUNC' TO LK-MSG
           END-EVALUATE
           CLOSE ACCT-FILE
           GOBACK.
       POST-PARA.
           MOVE LK-ID TO AC-ID
           READ ACCT-FILE
               INVALID KEY
                   MOVE 'NOT FOUND' TO LK-MSG
               NOT INVALID KEY
                   IF NOT AC-ACTIVE
                       MOVE 'CLOSED' TO LK-MSG
                   ELSE
                       IF AC-BAL < LK-AMT
                           MOVE 'NO FUNDS' TO LK-MSG
                       ELSE
                           SUBTRACT LK-AMT FROM AC-BAL
                           REWRITE ACCT-REC
                           MOVE 'POSTED' TO LK-MSG
                       END-IF
                   END-IF
           END-READ.
       NEW-PARA.
           MOVE LK-ID TO AC-ID
           MOVE 'A' TO AC-STATUS
           MOVE LK-AMT TO AC-BAL
           WRITE ACCT-REC
           IF WS-FS = '22'
               MOVE 'DUPLICATE' TO LK-MSG
           ELSE
               MOVE 'ADDED' TO LK-MSG
           END-IF.
       LIST-PARA.
           MOVE LK-ID TO AC-ID
           START ACCT-FILE KEY >= AC-ID
               INVALID KEY
                   MOVE 'NONE' TO LK-MSG
           END-START
           IF WS-FS = '00'
               PERFORM UNTIL WS-FS NOT = '00'
                   READ ACCT-FILE NEXT
                   IF WS-FS = '00'
                       ADD 1 TO WS-LISTED
                   END-IF
               END-PERFORM
               IF WS-LISTED > 1
                   MOVE 'MANY' TO LK-MSG
               END-IF
           END-IF.

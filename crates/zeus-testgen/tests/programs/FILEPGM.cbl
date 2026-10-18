       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILEPGM.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT TRAN-FILE ASSIGN TO TRANIN
               FILE STATUS IS WS-FS.
           SELECT RPT-FILE ASSIGN TO RPTOUT.
       DATA DIVISION.
       FILE SECTION.
       FD  TRAN-FILE RECORDING MODE F.
       01  TRAN-REC.
           05  TR-TYPE           PIC X.
               88  TR-CREDIT     VALUE 'C'.
               88  TR-DEBIT      VALUE 'D'.
           05  TR-ACCT           PIC X(6).
           05  TR-AMOUNT         PIC S9(5)V99 COMP-3.
           05  FILLER            PIC X(9).
       FD  RPT-FILE RECORDING MODE F.
       01  RPT-REC               PIC X(20).
       WORKING-STORAGE SECTION.
       01  WS-FS                 PIC XX.
       01  WS-EOF                PIC X VALUE 'N'.
           88  END-OF-FILE       VALUE 'Y'.
       01  WS-COUNT              PIC 9(3) VALUE 0.
       01  WS-BAL                PIC S9(7)V99 VALUE 0.
       01  WS-LINE.
           05  WL-ACCT           PIC X(6).
           05  FILLER            PIC X VALUE SPACE.
           05  WL-TEXT           PIC X(13).
       PROCEDURE DIVISION.
       MAIN-PARA.
           OPEN INPUT TRAN-FILE OUTPUT RPT-FILE
           READ TRAN-FILE
               AT END SET END-OF-FILE TO TRUE
           END-READ
           PERFORM UNTIL END-OF-FILE
               ADD 1 TO WS-COUNT
               EVALUATE TRUE
                   WHEN TR-CREDIT
                       ADD TR-AMOUNT TO WS-BAL
                   WHEN TR-DEBIT
                       IF TR-AMOUNT > 500
                           MOVE TR-ACCT TO WL-ACCT
                           MOVE 'LARGE DEBIT' TO WL-TEXT
                           WRITE RPT-REC FROM WS-LINE
                       END-IF
                       SUBTRACT TR-AMOUNT FROM WS-BAL
                   WHEN OTHER
                       MOVE TR-ACCT TO WL-ACCT
                       MOVE 'BAD TYPE' TO WL-TEXT
                       WRITE RPT-REC FROM WS-LINE
               END-EVALUATE
               READ TRAN-FILE
               IF WS-FS = '10'
                   SET END-OF-FILE TO TRUE
               END-IF
           END-PERFORM
           IF WS-COUNT > 2
               DISPLAY 'MANY ' WS-COUNT
           END-IF
           IF WS-BAL < 0
               DISPLAY 'OVERDRAWN'
               MOVE 4 TO RETURN-CODE
           END-IF
           CLOSE TRAN-FILE RPT-FILE
           GOBACK.

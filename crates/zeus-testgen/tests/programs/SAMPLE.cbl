       IDENTIFICATION DIVISION.
       PROGRAM-ID. SAMPLE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-I                  PIC 9(3).
       01  WS-TOTAL              PIC 9(5).
       01  WS-OPT                PIC X(4).
       LINKAGE SECTION.
       01  LK-REQ.
           05  LK-CODE           PIC X(2).
               88  LK-ADD        VALUE 'AD'.
               88  LK-DEL        VALUE 'DE'.
           05  LK-AMOUNT         PIC S9(5)V99 COMP-3.
           05  LK-COUNT          PIC 9(2).
           05  LK-USER           PIC X(8).
           05  LK-RESULT         PIC X(10).
       PROCEDURE DIVISION USING LK-REQ.
       MAIN-PARA.
           EVALUATE TRUE
               WHEN LK-ADD
                   PERFORM ADD-PARA
               WHEN LK-DEL
                   IF LK-USER = 'ADMIN001'
                       MOVE 'DELETED' TO LK-RESULT
                   ELSE
                       MOVE 'DENIED' TO LK-RESULT
                   END-IF
               WHEN OTHER
                   MOVE 'BAD CODE' TO LK-RESULT
           END-EVALUATE
           ACCEPT WS-OPT
           IF WS-OPT = 'LOOP'
               MOVE 0 TO WS-TOTAL
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > LK-COUNT
                   ADD WS-I TO WS-TOTAL
               END-PERFORM
               IF WS-TOTAL > 20
                   DISPLAY 'BIG ' WS-TOTAL
               END-IF
           END-IF
           GOBACK.
       ADD-PARA.
           IF LK-AMOUNT > 1000.50
               COMPUTE WS-TOTAL = LK-AMOUNT * 100
                   ON SIZE ERROR MOVE 'OVERFLOW' TO LK-RESULT
                   NOT ON SIZE ERROR MOVE 'ADDED BIG' TO LK-RESULT
               END-COMPUTE
           ELSE
               IF LK-AMOUNT < 0
                   MOVE 'NEGATIVE' TO LK-RESULT
               ELSE
                   MOVE 'ADDED' TO LK-RESULT
               END-IF
           END-IF.

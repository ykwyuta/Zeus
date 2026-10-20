       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQLPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           EXEC SQL INCLUDE SQLCA END-EXEC.
       01  HV-ID                 PIC X(8).
       01  HV-NAME               PIC X(20).
       01  HV-LIMIT              PIC S9(7)V99 COMP-3.
       01  WS-ROWS               PIC 9(3) VALUE 0.
           EXEC SQL DECLARE C1 CURSOR FOR
               SELECT NAME FROM CUST WHERE ID > :HV-ID
           END-EXEC.
       LINKAGE SECTION.
       01  LK-ID                 PIC X(8).
       PROCEDURE DIVISION USING LK-ID.
       MAIN-PARA.
           MOVE LK-ID TO HV-ID
           EXEC SQL SELECT NAME, CREDIT_LIMIT INTO :HV-NAME, :HV-LIMIT
               FROM CUST WHERE ID = :HV-ID END-EXEC
           EVALUATE SQLCODE
               WHEN 0
                   IF HV-LIMIT > 5000
                       DISPLAY 'GOLD ' HV-NAME
                   ELSE
                       DISPLAY 'CUSTOMER ' HV-NAME
                   END-IF
               WHEN 100
                   DISPLAY 'NO CUSTOMER'
               WHEN OTHER
                   DISPLAY 'SQL ERROR'
                   GOBACK
           END-EVALUATE
           EXEC SQL WHENEVER SQLERROR GO TO SQL-FAIL END-EXEC
           EXEC SQL OPEN C1 END-EXEC
           PERFORM UNTIL SQLCODE NOT = 0
               EXEC SQL FETCH C1 INTO :HV-NAME END-EXEC
               IF SQLCODE = 0
                   ADD 1 TO WS-ROWS
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C1 END-EXEC
           IF WS-ROWS > 1
               DISPLAY 'MANY ' WS-ROWS
           END-IF
           GOBACK.
       SQL-FAIL.
           DISPLAY 'CURSOR FAILED'
           GOBACK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAGPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-EOF                PIC X VALUE 'N'.
       01  WS-COUNT              PIC 9(3) VALUE 0.
       01  WS-STATE              PIC X VALUE SPACE.
           88  ST-ERROR          VALUE 'E'.
           88  ST-OK             VALUE 'O'.
       01  WS-MAX                PIC 9(2) VALUE 3.
       01  WS-I                  PIC 9(2).
       LINKAGE SECTION.
       01  LK-REC.
           05  LK-CODE           PIC X(2).
           05  LK-LIMIT          PIC 9(2).
       PROCEDURE DIVISION USING LK-REC.
       MAIN-PARA.
           PERFORM UNTIL WS-EOF = 'Y'
               IF WS-EOF = 'N'
                   ADD 1 TO WS-COUNT
                   IF WS-COUNT > LK-LIMIT
                       MOVE 'Y' TO WS-EOF
                   END-IF
               END-IF
           END-PERFORM
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-MAX
               IF WS-I = 5
                   DISPLAY 'FIVE'
               END-IF
           END-PERFORM
           PERFORM CHECK-PARA THRU CHECK-EXIT
           IF ST-ERROR
               DISPLAY 'ERROR ' LK-CODE
           ELSE
               DISPLAY 'OK ' WS-COUNT
           END-IF
           GOBACK.
       CHECK-PARA.
           SET ST-OK TO TRUE
           IF LK-CODE = 'XX'
               SET ST-ERROR TO TRUE
               GO TO CHECK-EXIT
           END-IF
           IF LK-CODE = SPACES
               DISPLAY 'NO CODE'
           END-IF.
       CHECK-EXIT.
           EXIT.

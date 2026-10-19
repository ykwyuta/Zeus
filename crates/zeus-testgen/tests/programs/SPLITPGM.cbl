       IDENTIFICATION DIVISION.
       PROGRAM-ID. SPLITPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-KIND               PIC X(4).
       01  WS-CODE               PIC X(6).
       01  WS-AMOUNT-X           PIC X(10).
       01  WS-AMOUNT             PIC 9(7)V99.
       01  WS-FIELDS             PIC 9(2) VALUE 0.
       01  WS-OUT                PIC X(30).
       01  WS-HEAD               PIC X(10).
       01  WS-TAIL               PIC X(10).
       LINKAGE SECTION.
       01  LK-MSG                PIC X(30).
       PROCEDURE DIVISION USING LK-MSG.
       MAIN-PARA.
           MOVE SPACES TO WS-KIND WS-CODE WS-AMOUNT-X
           UNSTRING LK-MSG DELIMITED BY ','
               INTO WS-KIND WS-CODE WS-AMOUNT-X
               TALLYING IN WS-FIELDS
           END-UNSTRING
           IF WS-KIND = 'AUTH'
               IF WS-CODE = 'X12345'
                   DISPLAY 'KNOWN CODE'
               END-IF
               COMPUTE WS-AMOUNT = FUNCTION NUMVAL(WS-AMOUNT-X)
               IF WS-AMOUNT > 500
                   DISPLAY 'LARGE ' WS-AMOUNT
               ELSE
                   DISPLAY 'SMALL ' WS-AMOUNT
               END-IF
           ELSE
               DISPLAY 'OTHER KIND'
           END-IF
           MOVE SPACES TO WS-OUT
           STRING WS-CODE DELIMITED BY SPACE
                  '-' DELIMITED BY SIZE
                  WS-KIND DELIMITED BY SIZE
                  INTO WS-OUT
           END-STRING
           IF WS-OUT(1:1) = '-'
               DISPLAY 'NO CODE'
           END-IF
           MOVE SPACES TO WS-HEAD WS-TAIL
           UNSTRING LK-MSG DELIMITED BY '::'
               INTO WS-HEAD WS-TAIL
           END-UNSTRING
           IF WS-TAIL = 'END'
               DISPLAY 'DOUBLE COLON'
           END-IF
           GOBACK.

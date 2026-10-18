       IDENTIFICATION DIVISION.
       PROGRAM-ID. ENVPGM.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IN-FILE ASSIGN TO INDD
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  IN-FILE.
       01  IN-REC                PIC X(10).
       WORKING-STORAGE SECTION.
       01  WS-FS                 PIC XX.
       01  WS-CHECK.
           05  WS-RESULT         PIC XX.
           05  WS-SCORE          PIC 9(3).
       PROCEDURE DIVISION.
       MAIN-PARA.
           OPEN INPUT IN-FILE
           IF WS-FS NOT = '00'
               DISPLAY 'OPEN FAILED ' WS-FS
               MOVE 8 TO RETURN-CODE
               GOBACK
           END-IF
           READ IN-FILE
           IF WS-FS = '30'
               DISPLAY 'READ ERROR'
           END-IF
           MOVE SPACES TO WS-RESULT
           MOVE 0 TO WS-SCORE
           CALL 'CHKSUB' USING WS-CHECK
           IF WS-RESULT = 'OK'
               IF WS-SCORE > 500
                   DISPLAY 'HIGH'
               ELSE
                   DISPLAY 'CHECKED'
               END-IF
           ELSE
               DISPLAY 'NOT CHECKED'
           END-IF
           CLOSE IN-FILE
           GOBACK.

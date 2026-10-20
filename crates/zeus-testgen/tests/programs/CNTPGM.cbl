       IDENTIFICATION DIVISION.
       PROGRAM-ID. CNTPGM.
      * Checks a path by the slashes counted in a loop, as services
      * check the parts of a URI.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-I                  PIC 9(3).
       01  WS-SLASHES            PIC 9(3) VALUE 0.
       01  WS-ONE                PIC 9 VALUE 1.
       01  WS-THREE              PIC 9 VALUE 3.
       LINKAGE SECTION.
       01  PATH-IN               PIC X(12).
       PROCEDURE DIVISION USING PATH-IN.
           PERFORM VARYING WS-I FROM 1 BY 1
                   UNTIL WS-I > 12 OR WS-SLASHES = WS-THREE
               IF PATH-IN(WS-I:1) = '/'
                   ADD WS-ONE TO WS-SLASHES
               END-IF
           END-PERFORM
           IF WS-SLASHES NOT = WS-THREE
               DISPLAY 'BAD PATH'
           ELSE
               DISPLAY 'GOOD PATH'
           END-IF
           GOBACK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZALT.
      * The alternate screen size and the terminal's attributes.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H      PIC S9(4) COMP.
       01 WS-W      PIC S9(4) COMP.
       01 WS-AH     PIC S9(4) COMP.
       01 WS-AW     PIC S9(4) COMP.
       01 WS-COLOR  PIC X.
       01 WS-LINE.
          05 FILLER PIC X(5) VALUE 'SIZE '.
          05 WS-DH  PIC 999.
          05 FILLER PIC X VALUE 'X'.
          05 WS-DW  PIC 999.
          05 FILLER PIC X(5) VALUE ' ALT '.
          05 WS-DAH PIC 999.
          05 FILLER PIC X VALUE 'X'.
          05 WS-DAW PIC 999.
          05 FILLER PIC X(7) VALUE ' COLOR '.
          05 WS-C   PIC X(3).
       PROCEDURE DIVISION.
           EXEC CICS SEND TEXT FROM(WS-LINE) LENGTH(1) ERASE ALTERNATE
           END-EXEC
           EXEC CICS ASSIGN SCRNHT(WS-H) SCRNWD(WS-W)
                ALTSCRNHT(WS-AH) ALTSCRNWD(WS-AW) COLOR(WS-COLOR)
           END-EXEC
           MOVE WS-H TO WS-DH
           MOVE WS-W TO WS-DW
           MOVE WS-AH TO WS-DAH
           MOVE WS-AW TO WS-DAW
           IF WS-COLOR = HIGH-VALUE
              MOVE 'YES' TO WS-C
           ELSE
              MOVE 'NO' TO WS-C
           END-IF
           EXEC CICS SEND TEXT FROM(WS-LINE) LENGTH(34) END-EXEC
           EXEC CICS RETURN END-EXEC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZONED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 U PIC 9(4) VALUE 0.
       01 T PIC S9(4) VALUE 0.
       01 L PIC S9(4) SIGN LEADING VALUE 0.
       01 LS PIC S9(4) SIGN LEADING SEPARATE VALUE 0.
       01 TS PIC S9(4) SIGN TRAILING SEPARATE VALUE 0.
       01 P PIC S9(4) COMP-3 VALUE 0.
       01 B PIC S9(4) COMP VALUE 0.
       01 I PIC S9(4) VALUE 0.
       01 BIG PIC 9(18) VALUE 0.
       01 TEXT-SOURCE PIC X(6) VALUE 'ABCDEF'.
       01 SHORT-TEXT PIC X(3) JUSTIFIED RIGHT.
       01 LONG-TEXT PIC X(8) JUSTIFIED RIGHT.
       PROCEDURE DIVISION.
           MOVE -123 TO T
           MOVE T TO L LS TS P B U
           PERFORM VARYING I FROM -2 BY 1 UNTIL I > 2
             ADD I TO T
             SUBTRACT I FROM L
             COMPUTE LS = LS + I * 2
             COMPUTE TS = -LS
           END-PERFORM
           MOVE 999999999999999999 TO BIG
           SUBTRACT 1 FROM BIG
           MOVE TEXT-SOURCE TO SHORT-TEXT LONG-TEXT
           IF T NOT = -123 OR L NOT = -123 OR LS NOT = -123
             DISPLAY 'BAD NEGATIVE'
           END-IF
           IF TS NOT = 123 OR U NOT = 123 OR P NOT = -123
             DISPLAY 'BAD MOVE'
           END-IF
           IF B NOT = -123 OR I NOT = 3
             DISPLAY 'BAD LOOP'
           END-IF
           DISPLAY U ':' T ':' L ':' LS ':' TS ':' BIG
           DISPLAY '[' SHORT-TEXT '][' LONG-TEXT ']'
           MOVE 'XY' TO LONG-TEXT
           MOVE 'LONGER' TO SHORT-TEXT
           DISPLAY '[' SHORT-TEXT '][' LONG-TEXT ']'
           MOVE 9999 TO T
           ADD 2 TO T
           DISPLAY 'TRUNC=' T
           ADD 9999 TO T ON SIZE ERROR DISPLAY 'SIZE ERROR'
           END-ADD
           DISPLAY 'KEPT=' T
           STOP RUN.

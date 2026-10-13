       IDENTIFICATION DIVISION.
       PROGRAM-ID. ARITH.
      * Binary and packed decimal arithmetic, assignments and
      * comparisons that the machine code does itself, and the cases it
      * leaves to the interpreter (size errors, truncation, JUSTIFIED).
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I     PIC S9(9) COMP VALUE 0.
       01 J     PIC S9(4) COMP VALUE 0.
       01 U     PIC 9(4) COMP VALUE 0.
       01 H     PIC S9(3) COMP VALUE 0.
       01 L     PIC S9(18) COMP VALUE 0.
       01 P     PIC S9(7) COMP-3 VALUE 0.
       01 Q     PIC 9(5) COMP-3 VALUE 0.
       01 R     PIC S9(4) COMP-3 VALUE 0.
       01 T     PIC X(8) VALUE 'START'.
       01 V     PIC X(4).
       01 W     PIC X(12).
       01 JR    PIC X(6) JUSTIFIED RIGHT.
       01 N     PIC S9(9) COMP VALUE 0.
       PROCEDURE DIVISION.
           PERFORM VARYING I FROM -5 BY 1 UNTIL I > 5
             COMPUTE P = P + I * 1000
             COMPUTE J = I * 3 - 7
             SUBTRACT 3 FROM R
             IF I < 0 AND NOT J = -22
               ADD 1 TO N
             END-IF
           END-PERFORM
           DISPLAY 'P=' P ' J=' J ' R=' R ' N=' N
      *    unsigned items take the absolute value
           COMPUTE U = 0 - 1234
           COMPUTE Q = -54321
           DISPLAY 'U=' U ' Q=' Q
      *    more digits than the picture: truncated (size error)
           MOVE 999 TO H
           ADD 5 TO H
           COMPUTE R = 98765
           DISPLAY 'H=' H ' R=' R
           ADD 1 TO H ON SIZE ERROR DISPLAY 'NO SIZE ERROR'
           END-ADD
      *    beyond 64 bits: left to the interpreter
           MOVE 999999999999999999 TO L
           COMPUTE L = L * 10 + 9
           DISPLAY 'L=' L
           MOVE 'AB' TO V
           MOVE T TO W
           MOVE 'XY' TO JR
           MOVE W TO V
           DISPLAY 'V=' V ' W=' W ' JR=' JR
           IF P > 0 OR U = 1234
             DISPLAY 'OK'
           END-IF
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TABLE-NATIVE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I PIC S9(4) COMP VALUE 1.
       01 J PIC S9(4) VALUE 1.
       01 TOTAL PIC S9(6) COMP-3 VALUE 0.
       01 NUMBERS.
          02 N PIC S9(4) OCCURS 3 TIMES.
       01 MATRIX.
          02 ROW-ITEM OCCURS 2 TIMES.
             03 CELL PIC S9(4) COMP-3 OCCURS 3 TIMES.
       01 WORDS.
          02 WORD-ITEM PIC X(6) OCCURS 2 TIMES.
       01 OUT-TEXT PIC X(6) VALUE SPACES.
       01 DIGITS PIC 9(6) VALUE 123456.
       01 ALIAS-AREA.
          02 ALIAS-N PIC 9(2) OCCURS 2 TIMES.
          02 AFTER-TABLE PIC 9(2) VALUE 7.
       PROCEDURE DIVISION.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
             COMPUTE N(I) = I * -10
             ADD N(I) TO TOTAL
           END-PERFORM
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2
             PERFORM VARYING J FROM 1 BY 1 UNTIL J > 3
               COMPUTE CELL(I, J) = I * 10 + J
               ADD CELL(I, J) TO TOTAL
             END-PERFORM
           END-PERFORM
           MOVE 2 TO I
           MOVE N(I + 1) TO CELL(I, I)
           IF CELL(2, 2) NOT = -30 OR N(I - 1) NOT = -10
             DISPLAY 'BAD TABLE'
           END-IF
           DISPLAY 'TOTAL=' TOTAL
           MOVE -2 TO I
           ADD 1 TO N(I)
           IF N(1) NOT = -9 DISPLAY 'BAD CLAMP' END-IF
           MOVE 'ABCDEF' TO WORD-ITEM(1)
           MOVE 'UVWXYZ' TO WORD-ITEM(2)
           MOVE WORD-ITEM(1)(2:3) TO OUT-TEXT
           IF OUT-TEXT NOT = 'BCD' DISPLAY 'BAD SLICE' END-IF
           MOVE WORD-ITEM(2)(4:) TO OUT-TEXT(3:3)
           DISPLAY '[' OUT-TEXT ']'
           MOVE DIGITS(2:3) TO OUT-TEXT
           IF OUT-TEXT NOT = '234' DISPLAY 'BAD DIGITS' END-IF
           MOVE WORD-ITEM(1)(1:4) TO WORD-ITEM(1)(2:4)
           DISPLAY '[' WORD-ITEM(1) ']'
      *    Above the declared OCCURS bound but still in storage:
      *    preserve the interpreter's alias behavior via fallback.
           MOVE 3 TO I
           ADD 1 TO ALIAS-N(I)
           IF AFTER-TABLE NOT = 8 DISPLAY 'BAD ALIAS' END-IF
           DISPLAY 'DONE'
           STOP RUN.

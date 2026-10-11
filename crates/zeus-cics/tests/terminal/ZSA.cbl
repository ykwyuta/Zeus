       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZSA.
      * A 3270 data stream of its own: SBA, SF / SFE, SA (character
      * colour, highlighting and character set), RA and IC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STREAM.
      *    SBA row 2 column 1, SFE protected / red, 'RED FIELD'
          05 FILLER PIC X(3) VALUE X'11C150'.
          05 FILLER PIC X(6) VALUE X'2902C06042F2'.
          05 FILLER PIC X(9) VALUE 'RED FIELD'.
      *    SA colour yellow, 'YEL', SA reverse, 'REV', SA reset, ' PLAIN'
          05 FILLER PIC X(3) VALUE X'2842F6'.
          05 FILLER PIC X(3) VALUE 'YEL'.
          05 FILLER PIC X(3) VALUE X'2841F2'.
          05 FILLER PIC X(3) VALUE 'REV'.
          05 FILLER PIC X(3) VALUE X'280000'.
          05 FILLER PIC X(6) VALUE ' PLAIN'.
      *    SBA row 4 column 1, SFE unprotected with character set X'F1',
      *    RA of '*' to column 11, IC
          05 FILLER PIC X(3) VALUE X'11C3F0'.
          05 FILLER PIC X(6) VALUE X'2902C04043F1'.
          05 FILLER PIC X(4) VALUE X'3CC37A5C'.
          05 FILLER PIC X(1) VALUE X'13'.
       01 WS-CTL    PIC X VALUE X'C3'.
       PROCEDURE DIVISION.
           EXEC CICS SEND FROM(WS-STREAM) LENGTH(53) CTLCHAR(WS-CTL)
                ERASE END-EXEC
           EXEC CICS RETURN END-EXEC.

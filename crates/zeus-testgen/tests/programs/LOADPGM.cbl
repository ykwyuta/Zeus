       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOADPGM.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IN-FILE ASSIGN TO INFILE.
       DATA DIVISION.
       FILE SECTION.
       FD  IN-FILE.
       01  IN-REC                PIC X(40).
       WORKING-STORAGE SECTION.
       01  WS-EOF                PIC X VALUE 'N'.
       01  TXT-ID                PIC X(9).
       01  TXT-QTY               PIC X(9).
       01  TXT-PRICE             PIC X(9).
       01  WS-ID                 PIC 9(9).
       01  WS-QTY                PIC 9(9).
       01  WS-PRICE              PIC 9(9).
       01  START-K               PIC 9(5).
       01  END-K                 PIC 9(5).
       LINKAGE SECTION.
       01  PARM-AREA.
           05  PARM-LEN          PIC 9(4) BINARY.
           05  PARM              PIC X(20).
       PROCEDURE DIVISION USING PARM-AREA.
       MAIN-PARA.
           UNSTRING PARM(1:PARM-LEN) DELIMITED BY ','
               INTO START-K END-K
           END-UNSTRING
           IF END-K < START-K
               DISPLAY 'BAD RANGE'
               GOBACK
           END-IF
           IF START-K = ZERO
               DISPLAY 'NO START'
               GOBACK
           END-IF
           OPEN INPUT IN-FILE
           PERFORM UNTIL WS-EOF = 'Y'
               READ IN-FILE
                   AT END MOVE 'Y' TO WS-EOF
                   NOT AT END PERFORM LOAD-ONE
               END-READ
           END-PERFORM
           CLOSE IN-FILE
           GOBACK.
       LOAD-ONE.
           UNSTRING IN-REC DELIMITED BY ','
               INTO TXT-ID TXT-QTY TXT-PRICE
           END-UNSTRING
           COMPUTE WS-QTY = FUNCTION NUMVAL(TXT-QTY)
           COMPUTE WS-ID = FUNCTION NUMVAL(TXT-ID)
           COMPUTE WS-PRICE = FUNCTION NUMVAL(TXT-PRICE)
           IF WS-QTY > 100
               DISPLAY 'BULK ' WS-ID
           ELSE
               DISPLAY 'SMALL ' WS-ID
           END-IF.

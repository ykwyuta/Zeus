       IDENTIFICATION DIVISION.
       PROGRAM-ID. NAT1.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N1      PIC N(6) VALUE N'東京'.
       01 WS-N2      PIC N(6) USAGE NATIONAL.
       01 WS-HEX     PIC N(2) VALUE NX'00410042'.
       01 WS-X       PIC X(20).
       01 WS-D1      PIC G(4) USAGE DISPLAY-1 VALUE G'大阪'.
       01 WS-IN      PIC N(10).
       PROCEDURE DIVISION.
           DISPLAY WS-N1 '|'
           DISPLAY WS-HEX
           MOVE 'ABC' TO WS-N2
           DISPLAY WS-N2 '|'
           MOVE WS-N1 TO WS-X
           DISPLAY WS-X '|'
           MOVE WS-D1 TO WS-N2
           DISPLAY WS-N2 '|'
           DISPLAY FUNCTION LENGTH(WS-N1)
           DISPLAY FUNCTION BYTE-LENGTH(WS-N1)
           MOVE FUNCTION NATIONAL-OF('日本') TO WS-N2
           DISPLAY FUNCTION DISPLAY-OF(WS-N2) '|'
           DISPLAY FUNCTION DISPLAY-OF(WS-N2, 939) '|'
           IF WS-N1 = N'東京'
               DISPLAY 'EQUAL'
           END-IF
           IF WS-N2 > N'あ'
               DISPLAY 'GREATER'
           END-IF
           MOVE SPACES TO WS-N2
           IF WS-N2 = SPACES
               DISPLAY 'SPACES'
           END-IF
           DISPLAY WS-N1(2:1)
           ACCEPT WS-IN
           DISPLAY 'IN=' WS-IN '|'
           STOP RUN.

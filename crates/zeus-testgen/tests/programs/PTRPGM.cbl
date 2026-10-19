       IDENTIFICATION DIVISION.
       PROGRAM-ID. PTRPGM.
      * An exit that gets a list of pointers, a record passed as one
      * PIC X item and moved into a structured record, and a blocked
      * variable-length report file.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT RPT-FILE ASSIGN TO RPTDD.
       DATA DIVISION.
       FILE SECTION.
       FD  RPT-FILE
           BLOCK CONTAINS 0
           RECORDING MODE V
           RECORD VARYING DEPENDING ON RPT-LEN.
       01  RPT-REC.
           05  RPT-CHAR          PIC X OCCURS 1 TO 40
                                 DEPENDING ON RPT-LEN.
       WORKING-STORAGE SECTION.
       01  RPT-LEN               PIC 9(4) BINARY.
       01  WS-ORDER.
           05  WS-KIND           PIC X.
           05  WS-AMOUNT         PIC 9(5).
           05  WS-QTY            PIC 9(3).
       01  WS-LINE               PIC X(12).
       LINKAGE SECTION.
       01  PARMS.
           05  P-REC             POINTER.
           05  P-FLAGS           POINTER.
       01  REC-AREA.
           05  REC-TYPE          PIC X.
               88  REC-DATA      VALUE 'D'.
           05  REC-LEN           PIC 9(4) BINARY.
       01  FLAG-AREA.
           05  FLAG-EOF          PIC X.
               88  AT-EOF        VALUE 'Y'.
       01  ORDER-IN              PIC X(9).
       PROCEDURE DIVISION USING PARMS ORDER-IN.
           SET ADDRESS OF REC-AREA TO P-REC
           SET ADDRESS OF FLAG-AREA TO P-FLAGS
           IF AT-EOF
               DISPLAY 'EOF'
               GOBACK
           END-IF
           IF REC-DATA AND REC-LEN > 100
               DISPLAY 'LONG DATA'
           END-IF
           MOVE ORDER-IN TO WS-ORDER
           IF WS-KIND = 'B' AND WS-AMOUNT * WS-QTY > 50000
               MOVE 'BIG ORDER' TO WS-LINE
           ELSE
               MOVE 'ORDER' TO WS-LINE
           END-IF
           OPEN OUTPUT RPT-FILE
           MOVE 12 TO RPT-LEN
           WRITE RPT-REC FROM WS-LINE
           CLOSE RPT-FILE
           GOBACK.

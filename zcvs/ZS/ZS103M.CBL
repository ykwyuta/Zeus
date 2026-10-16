      *================================================================*
      * ZS103M - JCL PARM= RECEPTION AND STEP RETURN-CODE PROPAGATION  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS103M.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD            PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  WS-PARM-LEN-RAW          PIC X(2).
       01  WS-PARM-LEN-NUM REDEFINES WS-PARM-LEN-RAW PIC S9(4) COMP.
       LINKAGE SECTION.
       01  LK-PARM-BUF.
           05  LK-PARM-LEN          PIC S9(4) COMP.
           05  LK-PARM-TEXT         PIC X(100).
       PROCEDURE DIVISION USING LK-PARM-BUF.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZS103M' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: HALFWORD BIG-ENDIAN PARM LENGTH IS 16 (0010 HEX)
           MOVE 'PARM-HALFWORD-LEN' TO ZCVS-TEST-NAME.
           MOVE LK-PARM-LEN TO WS-PARM-LEN-NUM.
           MOVE WS-PARM-LEN-RAW TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF LK-PARM-LEN = 16 AND ZCVS-HEX-OUT(1:4) = '0010'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0010' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: PARM PAYLOAD TEXT IS 'NIGHTLY,20261016'
           MOVE 'PARM-TEXT-VALUE' TO ZCVS-TEST-NAME.
           IF LK-PARM-TEXT(1:16) = 'NIGHTLY,20261016'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NIGHTLY,20261016' TO ZCVS-EXPECTED
               MOVE LK-PARM-TEXT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 4 TO RETURN-CODE.
           GOBACK.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

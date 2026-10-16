       CBL NSYMBOL(NATIONAL)
      *================================================================*
      * ZN102A - NATIONAL DECIMAL AND NATIONAL NUMERIC/CHARACTER EDIT  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN102A.
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
       01  WS-NAT-DEC-FIELDS.
           05  WS-NDEC              PIC S9(3)V99 NATIONAL
                                    SIGN IS LEADING SEPARATE.
           05  WS-NDEC-RAW REDEFINES WS-NDEC PIC X(12).
           05  WS-NED-NUM           PIC ZZ9.99 NATIONAL.
           05  WS-NED-NUM-RAW REDEFINES WS-NED-NUM PIC X(12).
           05  WS-NED-CH            PIC NBN/0N.
           05  WS-DISP-DEC          PIC S9(3)V99
                                    SIGN IS LEADING SEPARATE.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: NATIONAL DECIMAL BYTE LENGTH IS 12 (6 UTF-16 UNITS)
           MOVE 'NDEC-BYTE-LEN' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-NDEC = 12
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: NATIONAL DECIMAL STORAGE OF -12.34 IN UTF-16BE
           MOVE 'NDEC-UTF16-HEX' TO ZCVS-TEST-NAME.
           MOVE -12.34 TO WS-NDEC.
           MOVE WS-NDEC-RAW TO ZCVS-HEX-IN.
           MOVE 12 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:24) = '002D00300031003200330034'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '002D00300031003200330034' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:24) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: ARITHMETIC AND IS NUMERIC ON NATIONAL DECIMAL
           MOVE 'NDEC-ARITH-NUM' TO ZCVS-TEST-NAME.
           ADD 2.00 TO WS-NDEC.
           MOVE WS-NDEC TO WS-DISP-DEC.
           IF WS-NDEC IS NUMERIC AND WS-DISP-DEC = -10.34
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-10.34' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: NATIONAL NUMERIC-EDITED PIC ZZ9.99 NATIONAL
           MOVE 'NED-NUM-EDIT' TO ZCVS-TEST-NAME.
           MOVE WS-NDEC TO WS-NED-NUM.
           MOVE WS-NED-NUM TO WS-DISP-DEC.
           IF WS-NED-NUM = N' 10.34' AND WS-DISP-DEC = +10.34
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' 10.34' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: NATIONAL CHARACTER EDITING PIC NBN/0N
           MOVE 'NED-CHAR-EDIT' TO ZCVS-TEST-NAME.
           MOVE N'XYZ' TO WS-NED-CH.
           IF WS-NED-CH = N'X Y/0Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'X Y/0Z' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

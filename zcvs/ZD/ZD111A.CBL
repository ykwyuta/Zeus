      * AUDIT: D05; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
       CBL ARITH(EXTEND)
      * SPEC: COMPUTATIONAL ITEMS; AUDIT D05.
      * ZD111A: PACKED sign padding and 1 through 31 digit bounds
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD111A.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORDING MODE F
           RECORD CONTAINS 120 CHARACTERS.
       01  REPORT-RECORD PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  S1 PIC S9(1) COMP-3.
       01  S1-RAW REDEFINES S1 PIC X(1).
       01  U1 PIC 9(1) COMP-3.
       01  U1-RAW REDEFINES U1 PIC X(1).
       01  S2 PIC S9(2) COMP-3.
       01  S2-RAW REDEFINES S2 PIC X(2).
       01  U2 PIC 9(2) COMP-3.
       01  U2-RAW REDEFINES U2 PIC X(2).
       01  S3 PIC S9(3) COMP-3.
       01  S3-RAW REDEFINES S3 PIC X(2).
       01  U3 PIC 9(3) COMP-3.
       01  U3-RAW REDEFINES U3 PIC X(2).
       01  S4 PIC S9(4) COMP-3.
       01  S4-RAW REDEFINES S4 PIC X(3).
       01  U4 PIC 9(4) COMP-3.
       01  U4-RAW REDEFINES U4 PIC X(3).
       01  S5 PIC S9(5) COMP-3.
       01  S5-RAW REDEFINES S5 PIC X(3).
       01  U5 PIC 9(5) COMP-3.
       01  U5-RAW REDEFINES U5 PIC X(3).
       01  S6 PIC S9(6) COMP-3.
       01  S6-RAW REDEFINES S6 PIC X(4).
       01  U6 PIC 9(6) COMP-3.
       01  U6-RAW REDEFINES U6 PIC X(4).
       01  S7 PIC S9(7) COMP-3.
       01  S7-RAW REDEFINES S7 PIC X(4).
       01  U7 PIC 9(7) COMP-3.
       01  U7-RAW REDEFINES U7 PIC X(4).
       01  S8 PIC S9(8) COMP-3.
       01  S8-RAW REDEFINES S8 PIC X(5).
       01  U8 PIC 9(8) COMP-3.
       01  U8-RAW REDEFINES U8 PIC X(5).
       01  S9 PIC S9(9) COMP-3.
       01  S9-RAW REDEFINES S9 PIC X(5).
       01  U9 PIC 9(9) COMP-3.
       01  U9-RAW REDEFINES U9 PIC X(5).
       01  S18 PIC S9(18) COMP-3.
       01  S18-RAW REDEFINES S18 PIC X(10).
       01  U18 PIC 9(18) COMP-3.
       01  U18-RAW REDEFINES U18 PIC X(10).
       01  S19 PIC S9(19) COMP-3.
       01  S19-RAW REDEFINES S19 PIC X(10).
       01  U19 PIC 9(19) COMP-3.
       01  U19-RAW REDEFINES U19 PIC X(10).
       01  S30 PIC S9(30) COMP-3.
       01  S30-RAW REDEFINES S30 PIC X(16).
       01  U30 PIC 9(30) COMP-3.
       01  U30-RAW REDEFINES U30 PIC X(16).
       01  S31 PIC S9(31) COMP-3.
       01  S31-RAW REDEFINES S31 PIC X(16).
       01  U31 PIC 9(31) COMP-3.
       01  U31-RAW REDEFINES U31 PIC X(16).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD111A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'S1-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S1.
           IF S1 = 0
             AND S1-RAW = X'0C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0C' TO ZCVS-EXPECTED
               MOVE S1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S1-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S1.
           IF S1 = 1
             AND S1-RAW = X'1C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1C' TO ZCVS-EXPECTED
               MOVE S1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S1-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9 TO S1.
           IF S1 = 9
             AND S1-RAW = X'9C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9C' TO ZCVS-EXPECTED
               MOVE S1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S1-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S1.
           IF S1 = -1
             AND S1-RAW = X'1D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1D' TO ZCVS-EXPECTED
               MOVE S1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S1-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -9 TO S1.
           IF S1 = -9
             AND S1-RAW = X'9D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9D' TO ZCVS-EXPECTED
               MOVE S1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U1-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U1.
           IF U1 = 0
             AND U1-RAW = X'0F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0F' TO ZCVS-EXPECTED
               MOVE U1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U1-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U1.
           IF U1 = 1
             AND U1-RAW = X'1F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1F' TO ZCVS-EXPECTED
               MOVE U1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U1-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9 TO U1.
           IF U1 = 9
             AND U1-RAW = X'9F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9F' TO ZCVS-EXPECTED
               MOVE U1-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S2-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S2.
           IF S2 = 0
             AND S2-RAW = X'000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000C' TO ZCVS-EXPECTED
               MOVE S2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S2-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S2.
           IF S2 = 1
             AND S2-RAW = X'001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '001C' TO ZCVS-EXPECTED
               MOVE S2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S2-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 99 TO S2.
           IF S2 = 99
             AND S2-RAW = X'099C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '099C' TO ZCVS-EXPECTED
               MOVE S2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S2-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S2.
           IF S2 = -1
             AND S2-RAW = X'001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '001D' TO ZCVS-EXPECTED
               MOVE S2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S2-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -99 TO S2.
           IF S2 = -99
             AND S2-RAW = X'099D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '099D' TO ZCVS-EXPECTED
               MOVE S2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U2-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U2.
           IF U2 = 0
             AND U2-RAW = X'000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000F' TO ZCVS-EXPECTED
               MOVE U2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U2-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U2.
           IF U2 = 1
             AND U2-RAW = X'001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '001F' TO ZCVS-EXPECTED
               MOVE U2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U2-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 99 TO U2.
           IF U2 = 99
             AND U2-RAW = X'099F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '099F' TO ZCVS-EXPECTED
               MOVE U2-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S3-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S3.
           IF S3 = 0
             AND S3-RAW = X'000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000C' TO ZCVS-EXPECTED
               MOVE S3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S3-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S3.
           IF S3 = 1
             AND S3-RAW = X'001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '001C' TO ZCVS-EXPECTED
               MOVE S3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S3-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999 TO S3.
           IF S3 = 999
             AND S3-RAW = X'999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999C' TO ZCVS-EXPECTED
               MOVE S3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S3-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S3.
           IF S3 = -1
             AND S3-RAW = X'001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '001D' TO ZCVS-EXPECTED
               MOVE S3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S3-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -999 TO S3.
           IF S3 = -999
             AND S3-RAW = X'999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999D' TO ZCVS-EXPECTED
               MOVE S3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U3-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U3.
           IF U3 = 0
             AND U3-RAW = X'000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000F' TO ZCVS-EXPECTED
               MOVE U3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U3-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U3.
           IF U3 = 1
             AND U3-RAW = X'001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '001F' TO ZCVS-EXPECTED
               MOVE U3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U3-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999 TO U3.
           IF U3 = 999
             AND U3-RAW = X'999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999F' TO ZCVS-EXPECTED
               MOVE U3-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S4-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S4.
           IF S4 = 0
             AND S4-RAW = X'00000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000C' TO ZCVS-EXPECTED
               MOVE S4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S4-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S4.
           IF S4 = 1
             AND S4-RAW = X'00001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00001C' TO ZCVS-EXPECTED
               MOVE S4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S4-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999 TO S4.
           IF S4 = 9999
             AND S4-RAW = X'09999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '09999C' TO ZCVS-EXPECTED
               MOVE S4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S4-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S4.
           IF S4 = -1
             AND S4-RAW = X'00001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00001D' TO ZCVS-EXPECTED
               MOVE S4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S4-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -9999 TO S4.
           IF S4 = -9999
             AND S4-RAW = X'09999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '09999D' TO ZCVS-EXPECTED
               MOVE S4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U4-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U4.
           IF U4 = 0
             AND U4-RAW = X'00000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000F' TO ZCVS-EXPECTED
               MOVE U4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U4-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U4.
           IF U4 = 1
             AND U4-RAW = X'00001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00001F' TO ZCVS-EXPECTED
               MOVE U4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U4-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999 TO U4.
           IF U4 = 9999
             AND U4-RAW = X'09999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '09999F' TO ZCVS-EXPECTED
               MOVE U4-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S5-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S5.
           IF S5 = 0
             AND S5-RAW = X'00000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000C' TO ZCVS-EXPECTED
               MOVE S5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S5-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S5.
           IF S5 = 1
             AND S5-RAW = X'00001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00001C' TO ZCVS-EXPECTED
               MOVE S5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S5-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 99999 TO S5.
           IF S5 = 99999
             AND S5-RAW = X'99999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999C' TO ZCVS-EXPECTED
               MOVE S5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S5-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S5.
           IF S5 = -1
             AND S5-RAW = X'00001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00001D' TO ZCVS-EXPECTED
               MOVE S5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S5-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -99999 TO S5.
           IF S5 = -99999
             AND S5-RAW = X'99999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999D' TO ZCVS-EXPECTED
               MOVE S5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U5-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U5.
           IF U5 = 0
             AND U5-RAW = X'00000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000F' TO ZCVS-EXPECTED
               MOVE U5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U5-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U5.
           IF U5 = 1
             AND U5-RAW = X'00001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00001F' TO ZCVS-EXPECTED
               MOVE U5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U5-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 99999 TO U5.
           IF U5 = 99999
             AND U5-RAW = X'99999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999F' TO ZCVS-EXPECTED
               MOVE U5-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S6-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S6.
           IF S6 = 0
             AND S6-RAW = X'0000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000C' TO ZCVS-EXPECTED
               MOVE S6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S6-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S6.
           IF S6 = 1
             AND S6-RAW = X'0000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000001C' TO ZCVS-EXPECTED
               MOVE S6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S6-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999 TO S6.
           IF S6 = 999999
             AND S6-RAW = X'0999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0999999C' TO ZCVS-EXPECTED
               MOVE S6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S6-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S6.
           IF S6 = -1
             AND S6-RAW = X'0000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000001D' TO ZCVS-EXPECTED
               MOVE S6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S6-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -999999 TO S6.
           IF S6 = -999999
             AND S6-RAW = X'0999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0999999D' TO ZCVS-EXPECTED
               MOVE S6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U6-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U6.
           IF U6 = 0
             AND U6-RAW = X'0000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000F' TO ZCVS-EXPECTED
               MOVE U6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U6-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U6.
           IF U6 = 1
             AND U6-RAW = X'0000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000001F' TO ZCVS-EXPECTED
               MOVE U6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U6-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999 TO U6.
           IF U6 = 999999
             AND U6-RAW = X'0999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0999999F' TO ZCVS-EXPECTED
               MOVE U6-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S7-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S7.
           IF S7 = 0
             AND S7-RAW = X'0000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000C' TO ZCVS-EXPECTED
               MOVE S7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S7-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S7.
           IF S7 = 1
             AND S7-RAW = X'0000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000001C' TO ZCVS-EXPECTED
               MOVE S7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S7-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999999 TO S7.
           IF S7 = 9999999
             AND S7-RAW = X'9999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999C' TO ZCVS-EXPECTED
               MOVE S7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S7-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S7.
           IF S7 = -1
             AND S7-RAW = X'0000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000001D' TO ZCVS-EXPECTED
               MOVE S7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S7-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -9999999 TO S7.
           IF S7 = -9999999
             AND S7-RAW = X'9999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999D' TO ZCVS-EXPECTED
               MOVE S7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U7-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U7.
           IF U7 = 0
             AND U7-RAW = X'0000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000F' TO ZCVS-EXPECTED
               MOVE U7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U7-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U7.
           IF U7 = 1
             AND U7-RAW = X'0000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000001F' TO ZCVS-EXPECTED
               MOVE U7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U7-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999999 TO U7.
           IF U7 = 9999999
             AND U7-RAW = X'9999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999F' TO ZCVS-EXPECTED
               MOVE U7-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S8-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S8.
           IF S8 = 0
             AND S8-RAW = X'000000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000000C' TO ZCVS-EXPECTED
               MOVE S8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S8-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S8.
           IF S8 = 1
             AND S8-RAW = X'000000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000001C' TO ZCVS-EXPECTED
               MOVE S8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S8-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 99999999 TO S8.
           IF S8 = 99999999
             AND S8-RAW = X'099999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '099999999C' TO ZCVS-EXPECTED
               MOVE S8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S8-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S8.
           IF S8 = -1
             AND S8-RAW = X'000000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000001D' TO ZCVS-EXPECTED
               MOVE S8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S8-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -99999999 TO S8.
           IF S8 = -99999999
             AND S8-RAW = X'099999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '099999999D' TO ZCVS-EXPECTED
               MOVE S8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U8-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U8.
           IF U8 = 0
             AND U8-RAW = X'000000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000000F' TO ZCVS-EXPECTED
               MOVE U8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U8-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U8.
           IF U8 = 1
             AND U8-RAW = X'000000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000001F' TO ZCVS-EXPECTED
               MOVE U8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U8-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 99999999 TO U8.
           IF U8 = 99999999
             AND U8-RAW = X'099999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '099999999F' TO ZCVS-EXPECTED
               MOVE U8-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S9-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S9.
           IF S9 = 0
             AND S9-RAW = X'000000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000000C' TO ZCVS-EXPECTED
               MOVE S9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S9-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S9.
           IF S9 = 1
             AND S9-RAW = X'000000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000001C' TO ZCVS-EXPECTED
               MOVE S9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S9-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999999 TO S9.
           IF S9 = 999999999
             AND S9-RAW = X'999999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999C' TO ZCVS-EXPECTED
               MOVE S9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S9-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S9.
           IF S9 = -1
             AND S9-RAW = X'000000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000001D' TO ZCVS-EXPECTED
               MOVE S9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S9-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -999999999 TO S9.
           IF S9 = -999999999
             AND S9-RAW = X'999999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999D' TO ZCVS-EXPECTED
               MOVE S9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U9-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U9.
           IF U9 = 0
             AND U9-RAW = X'000000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000000F' TO ZCVS-EXPECTED
               MOVE U9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U9-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U9.
           IF U9 = 1
             AND U9-RAW = X'000000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '000000001F' TO ZCVS-EXPECTED
               MOVE U9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U9-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999999 TO U9.
           IF U9 = 999999999
             AND U9-RAW = X'999999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '999999999F' TO ZCVS-EXPECTED
               MOVE U9-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S18-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S18.
           IF S18 = 0
             AND S18-RAW = X'0000000000000000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000000C' TO ZCVS-EXPECTED
               MOVE S18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S18-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S18.
           IF S18 = 1
             AND S18-RAW = X'0000000000000000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000001C' TO ZCVS-EXPECTED
               MOVE S18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S18-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999 TO S18.
           IF S18 = 999999999999999999
             AND S18-RAW = X'0999999999999999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0999999999999999999C' TO ZCVS-EXPECTED
               MOVE S18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S18-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S18.
           IF S18 = -1
             AND S18-RAW = X'0000000000000000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000001D' TO ZCVS-EXPECTED
               MOVE S18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S18-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -999999999999999999 TO S18.
           IF S18 = -999999999999999999
             AND S18-RAW = X'0999999999999999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0999999999999999999D' TO ZCVS-EXPECTED
               MOVE S18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U18-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U18.
           IF U18 = 0
             AND U18-RAW = X'0000000000000000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000000F' TO ZCVS-EXPECTED
               MOVE U18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U18-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U18.
           IF U18 = 1
             AND U18-RAW = X'0000000000000000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000001F' TO ZCVS-EXPECTED
               MOVE U18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U18-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999 TO U18.
           IF U18 = 999999999999999999
             AND U18-RAW = X'0999999999999999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0999999999999999999F' TO ZCVS-EXPECTED
               MOVE U18-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S19-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S19.
           IF S19 = 0
             AND S19-RAW = X'0000000000000000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000000C' TO ZCVS-EXPECTED
               MOVE S19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S19-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S19.
           IF S19 = 1
             AND S19-RAW = X'0000000000000000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000001C' TO ZCVS-EXPECTED
               MOVE S19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S19-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999999999999999999 TO S19.
           IF S19 = 9999999999999999999
             AND S19-RAW = X'9999999999999999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999999999999C' TO ZCVS-EXPECTED
               MOVE S19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S19-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S19.
           IF S19 = -1
             AND S19-RAW = X'0000000000000000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000001D' TO ZCVS-EXPECTED
               MOVE S19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S19-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -9999999999999999999 TO S19.
           IF S19 = -9999999999999999999
             AND S19-RAW = X'9999999999999999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999999999999D' TO ZCVS-EXPECTED
               MOVE S19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U19-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U19.
           IF U19 = 0
             AND U19-RAW = X'0000000000000000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000000F' TO ZCVS-EXPECTED
               MOVE U19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U19-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U19.
           IF U19 = 1
             AND U19-RAW = X'0000000000000000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000000000001F' TO ZCVS-EXPECTED
               MOVE U19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U19-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999999999999999999 TO U19.
           IF U19 = 9999999999999999999
             AND U19-RAW = X'9999999999999999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9999999999999999999F' TO ZCVS-EXPECTED
               MOVE U19-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S30-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S30.
           IF S30 = 0
             AND S30-RAW = X'0000000000000000000000000000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE S30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S30-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S30.
           IF S30 = 1
             AND S30-RAW = X'0000000000000000000000000000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE S30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S30-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999999999999999 TO S30.
           IF S30 = 999999999999999999999999999999
             AND S30-RAW = X'0999999999999999999999999999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '09999999999999999999' TO ZCVS-EXPECTED
               MOVE S30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S30-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S30.
           IF S30 = -1
             AND S30-RAW = X'0000000000000000000000000000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE S30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S30-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -999999999999999999999999999999 TO S30.
           IF S30 = -999999999999999999999999999999
             AND S30-RAW = X'0999999999999999999999999999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '09999999999999999999' TO ZCVS-EXPECTED
               MOVE S30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U30-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U30.
           IF U30 = 0
             AND U30-RAW = X'0000000000000000000000000000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE U30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U30-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U30.
           IF U30 = 1
             AND U30-RAW = X'0000000000000000000000000000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE U30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U30-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 999999999999999999999999999999 TO U30.
           IF U30 = 999999999999999999999999999999
             AND U30-RAW = X'0999999999999999999999999999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '09999999999999999999' TO ZCVS-EXPECTED
               MOVE U30-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S31-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO S31.
           IF S31 = 0
             AND S31-RAW = X'0000000000000000000000000000000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE S31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S31-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO S31.
           IF S31 = 1
             AND S31-RAW = X'0000000000000000000000000000001C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE S31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S31-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999999999999999999999999999999 TO S31.
           IF S31 = 9999999999999999999999999999999
             AND S31-RAW = X'9999999999999999999999999999999C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999999999999999999' TO ZCVS-EXPECTED
               MOVE S31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S31-PACK-3' TO ZCVS-TEST-NAME.
           MOVE -1 TO S31.
           IF S31 = -1
             AND S31-RAW = X'0000000000000000000000000000001D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE S31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'S31-PACK-4' TO ZCVS-TEST-NAME.
           MOVE -9999999999999999999999999999999 TO S31.
           IF S31 = -9999999999999999999999999999999
             AND S31-RAW = X'9999999999999999999999999999999D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999999999999999999' TO ZCVS-EXPECTED
               MOVE S31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U31-PACK-0' TO ZCVS-TEST-NAME.
           MOVE 0 TO U31.
           IF U31 = 0
             AND U31-RAW = X'0000000000000000000000000000000F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE U31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U31-PACK-1' TO ZCVS-TEST-NAME.
           MOVE 1 TO U31.
           IF U31 = 1
             AND U31-RAW = X'0000000000000000000000000000001F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000000000000000' TO ZCVS-EXPECTED
               MOVE U31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U31-PACK-2' TO ZCVS-TEST-NAME.
           MOVE 9999999999999999999999999999999 TO U31.
           IF U31 = 9999999999999999999999999999999
             AND U31-RAW = X'9999999999999999999999999999999F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99999999999999999999' TO ZCVS-EXPECTED
               MOVE U31-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

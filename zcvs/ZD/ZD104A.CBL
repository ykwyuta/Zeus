      *================================================================*
      * ZD104A - RECORD LAYOUT, SLACK BYTES (SYNC), REDEFINES, RENAMES *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD104A.
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
       01  WS-SYNC-REC.
           05  WS-TAG1              PIC X(1).
           05  WS-SYNC-HW           PIC S9(4) COMP SYNC.
           05  WS-TAG2              PIC X(1).
           05  WS-SYNC-FW           PIC S9(9) COMP SYNC.
           05  WS-TAG3              PIC X(1).
           05  WS-SYNC-DW           PIC S9(18) COMP SYNC.
       01  WS-SYNC-RAW REDEFINES WS-SYNC-REC PIC X(24).

       01  WS-NOSYNC-REC.
           05  WS-NS-TAG1           PIC X(1).
           05  WS-NS-HW             PIC S9(4) COMP.
           05  WS-NS-TAG2           PIC X(1).
           05  WS-NS-FW             PIC S9(9) COMP.

       01  WS-REDEF-BASE.
           05  WS-RD-BODY.
               10  WS-RD-PART1      PIC X(4) VALUE 'ABCD'.
               10  WS-RD-PART2      PIC X(4) VALUE '1234'.
               10  WS-RD-PART3      PIC X(2) VALUE 'XY'.
           05  WS-REDEF-SHORT REDEFINES WS-RD-BODY.
               10  WS-RD-S1         PIC X(3).
               10  WS-RD-S2         PIC X(3).
       66  WS-REN-MID RENAMES WS-RD-PART1 THRU WS-RD-PART2.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD104A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: SYNC RECORD TOTAL LENGTH = 24 BYTES
           MOVE 'SYNC-REC-LEN' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-SYNC-REC = 24
               PERFORM ZCVS-PASS
           ELSE
               MOVE '24' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: NOSYNC RECORD TOTAL LENGTH (1 + 2 + 1 + 4 = 8)
           MOVE 'NOSYNC-REC-LEN' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-NOSYNC-REC = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: HALFWORD SYNC OFFSET AT BYTE 3..4 (1-BASED)
           MOVE 'SYNC-HW-OFFSET' TO ZCVS-TEST-NAME.
           MOVE LOW-VALUES TO WS-SYNC-RAW.
           MOVE 'A' TO WS-TAG1.
           MOVE 258 TO WS-SYNC-HW.
           MOVE WS-SYNC-RAW(1:4) TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = 'C1000102'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1000102' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: FULLWORD SYNC OFFSET AT BYTE 9..12 (3 SLACK BYTES)
           MOVE 'SYNC-FW-OFFSET' TO ZCVS-TEST-NAME.
           MOVE 'B' TO WS-TAG2.
           MOVE 16909060 TO WS-SYNC-FW.
           MOVE WS-SYNC-RAW(5:8) TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = 'C200000001020304'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C200000001020304' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: DOUBLEWORD SYNC OFFSET AT BYTE 17..24 (3 SLACK BYTES)
           MOVE 'SYNC-DW-OFFSET' TO ZCVS-TEST-NAME.
           MOVE 'C' TO WS-TAG3.
           MOVE 1 TO WS-SYNC-DW.
           MOVE WS-SYNC-RAW(13:12) TO ZCVS-HEX-IN.
           MOVE 12 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:24) = 'C30000000000000000000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C30000000000000000000001' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:24) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: SHORTER REDEFINES PRESERVES BASE RECORD LENGTH (10)
           MOVE 'REDEF-BASE-LEN' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-REDEF-BASE = 10
             AND LENGTH OF WS-REDEF-SHORT = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10/6' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: SHORTER REDEFINES OVERLAYS FIRST 6 BYTES
           MOVE 'REDEF-OVERLAY' TO ZCVS-TEST-NAME.
           IF WS-RD-S1 = 'ABC' AND WS-RD-S2 = 'D12'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABC/D12' TO ZCVS-EXPECTED
               MOVE WS-REDEF-SHORT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: LEVEL 66 RENAMES THRU LENGTH AND CONTENT
           MOVE 'RENAMES-THRU' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-REN-MID = 8
             AND WS-REN-MID = 'ABCD1234'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABCD1234' TO ZCVS-EXPECTED
               MOVE WS-REN-MID TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 09: MODIFY VIA REDEFINES UPDATES RENAMES AND BASE
           MOVE 'REDEF-MUTATE' TO ZCVS-TEST-NAME.
           MOVE 'XYZ' TO WS-RD-S1.
           IF WS-RD-PART1 = 'XYZD' AND WS-REN-MID = 'XYZD1234'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'XYZD1234' TO ZCVS-EXPECTED
               MOVE WS-REN-MID TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

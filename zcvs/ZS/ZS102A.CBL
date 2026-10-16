      *================================================================*
      * ZS102A - LANGUAGE ENVIRONMENT (LE) CALLABLE SERVICES           *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS102A.
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
       01  WS-VSTR-DATE.
           05  WS-VD-LEN            PIC S9(4) COMP VALUE 8.
           05  WS-VD-TXT            PIC X(8) VALUE '15821015'.
       01  WS-VSTR-PIC.
           05  WS-VP-LEN            PIC S9(4) COMP VALUE 8.
           05  WS-VP-TXT            PIC X(8) VALUE 'YYYYMMDD'.
       01  WS-VSTR-PIC-FMT.
           05  WS-VPF-LEN           PIC S9(4) COMP VALUE 14.
           05  WS-VPF-TXT           PIC X(14) VALUE 'YYYY-MM-DD Www'.
       01  WS-VSTR-TS.
           05  WS-VT-LEN            PIC S9(4) COMP VALUE 14.
           05  WS-VT-TXT            PIC X(14) VALUE '20261016143025'.
       01  WS-VSTR-TS-PIC.
           05  WS-VTP-LEN           PIC S9(4) COMP VALUE 14.
           05  WS-VTP-TXT           PIC X(14) VALUE 'YYYYMMDDHHMISS'.
       01  WS-VSTR-TM-PIC.
           05  WS-VTM-LEN           PIC S9(4) COMP VALUE 19.
           05  WS-VTM-TXT           PIC X(19)
               VALUE 'YYYY/MM/DD HH:MI:SS'.
       01  WS-LILIAN                PIC S9(9) COMP VALUE 0.
       01  WS-SECS                  COMP-2.
       01  WS-DATE-OUT              PIC X(80) VALUE SPACES.
       01  WS-FC.
           05  WS-FC-SEV            PIC S9(4) COMP.
           05  WS-FC-MSG            PIC S9(4) COMP.
           05  WS-FC-FLG            PIC X(1).
           05  WS-FC-FAC            PIC X(3).
           05  WS-FC-ISI            PIC S9(9) COMP.
       01  WS-COND-FIELDS.
           05  WS-C1                PIC S9(4) COMP VALUE 2.
           05  WS-C2                PIC S9(4) COMP VALUE 2518.
           05  WS-CASE              PIC S9(4) COMP VALUE 1.
           05  WS-SEV               PIC S9(4) COMP VALUE 2.
           05  WS-CTL               PIC S9(4) COMP VALUE 1.
           05  WS-FAC               PIC X(3) VALUE 'CEE'.
           05  WS-ISI               PIC S9(9) COMP VALUE 42.
           05  WS-TOK               PIC X(12).
           05  WS-DC-C1             PIC S9(4) COMP VALUE 0.
           05  WS-DC-C2             PIC S9(4) COMP VALUE 0.
           05  WS-DC-CASE           PIC S9(4) COMP VALUE 0.
           05  WS-DC-SEV            PIC S9(4) COMP VALUE 0.
           05  WS-DC-CTL            PIC S9(4) COMP VALUE 0.
           05  WS-DC-FAC            PIC X(3) VALUE SPACES.
           05  WS-DC-ISI            PIC S9(9) COMP VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZS102A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: CEEDAYS epoch 1582-10-15 -> LILIAN DAY 1
           MOVE 'LE-CEEDAYS-EPOCH' TO ZCVS-TEST-NAME.
           MOVE '15821015' TO WS-VD-TXT.
           CALL 'CEEDAYS' USING WS-VSTR-DATE WS-VSTR-PIC
                                WS-LILIAN WS-FC.
           IF WS-FC-SEV = 0 AND WS-LILIAN = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SEV=0/LILIAN=1' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: CEEDAYS + CEEDATE FOR 2026-10-16 -> '2026-10-16 Fri'
           MOVE 'LE-CEEDATE-FMT' TO ZCVS-TEST-NAME.
           MOVE '20261016' TO WS-VD-TXT.
           CALL 'CEEDAYS' USING WS-VSTR-DATE WS-VSTR-PIC
                                WS-LILIAN WS-FC.
           CALL 'CEEDATE' USING WS-LILIAN WS-VSTR-PIC-FMT
                                WS-DATE-OUT WS-FC.
           IF WS-FC-SEV = 0
             AND WS-DATE-OUT(1:14) = '2026-10-16 Fri'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2026-10-16 Fri' TO ZCVS-EXPECTED
               MOVE WS-DATE-OUT(1:14) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: CEESECS + CEEDATM ROUND-TRIP TIMESTAMP
           MOVE 'LE-CEESECS-DATM' TO ZCVS-TEST-NAME.
           CALL 'CEESECS' USING WS-VSTR-TS WS-VSTR-TS-PIC
                                WS-SECS WS-FC.
           CALL 'CEEDATM' USING WS-SECS WS-VSTR-TM-PIC
                                WS-DATE-OUT WS-FC.
           IF WS-FC-SEV = 0
             AND WS-DATE-OUT(1:19) = '2026/10/16 14:30:25'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2026/10/16 14:30:25' TO ZCVS-EXPECTED
               MOVE WS-DATE-OUT(1:19) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: CEEDAYS INVALID DATE 2026-02-30 SETS SEVERITY 3 IN FC
           MOVE 'LE-CEEDAYS-BAD' TO ZCVS-TEST-NAME.
           MOVE '20260230' TO WS-VD-TXT.
           CALL 'CEEDAYS' USING WS-VSTR-DATE WS-VSTR-PIC
                                WS-LILIAN WS-FC.
           IF WS-FC-SEV = 3 AND WS-FC-FAC = 'CEE'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SEV=3/CEE' TO ZCVS-EXPECTED
               MOVE WS-FC-FAC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: CEENCOD AND CEEDCOD CONDITION TOKEN ROUND-TRIP
           MOVE 'LE-CEENCOD-DCOD' TO ZCVS-TEST-NAME.
           CALL 'CEENCOD' USING WS-C1 WS-C2 WS-CASE WS-SEV
                                WS-CTL WS-FAC WS-ISI WS-TOK WS-FC.
           CALL 'CEEDCOD' USING WS-TOK WS-DC-C1 WS-DC-C2 WS-DC-CASE
                                WS-DC-SEV WS-DC-CTL WS-DC-FAC
                                WS-DC-ISI WS-FC.
           IF WS-FC-SEV = 0
             AND WS-DC-C2 = 2518
             AND WS-DC-FAC = 'CEE'
             AND WS-DC-ISI = 42
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2518/CEE/42' TO ZCVS-EXPECTED
               MOVE WS-DC-FAC TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

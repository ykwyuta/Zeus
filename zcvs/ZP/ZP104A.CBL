      *================================================================*
      * ZP104A - CALL BY REFERENCE / CONTENT / VALUE / OMITTED & ENTRY *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP104A.
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
       01  WS-CALL-PARMS.
           05  WS-REF               PIC S9(4) COMP VALUE 10.
           05  WS-CONT              PIC S9(4) COMP VALUE 20.
           05  WS-VAL               PIC S9(9) COMP-5 VALUE 30.
           05  WS-OPT               PIC X(4) VALUE 'ABCD'.
           05  WS-OPT-STAT          PIC X(1) VALUE SPACE.
           05  WS-ENT-IN            PIC X(4) VALUE 'Z999'.
           05  WS-ENT-OUT           PIC X(8) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP104A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: BY REFERENCE, BY CONTENT (UNCHANGED), BY VALUE, PRESENT
           MOVE 'CALL-MODES-PRES' TO ZCVS-TEST-NAME.
           MOVE 10 TO WS-REF.
           MOVE 20 TO WS-CONT.
           MOVE 30 TO WS-VAL.
           CALL 'ZPS04A' USING BY REFERENCE WS-REF
                               BY CONTENT WS-CONT
                               BY VALUE WS-VAL
                               BY REFERENCE WS-OPT
                               BY REFERENCE WS-OPT-STAT.
           IF WS-REF = 60
             AND WS-CONT = 20
             AND WS-OPT-STAT = 'P'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '60/20/P' TO ZCVS-EXPECTED
               MOVE WS-OPT-STAT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: OMITTED ARGUMENT DETECTED VIA ADDRESS OF = NULL
           MOVE 'CALL-OMITTED' TO ZCVS-TEST-NAME.
           MOVE 5 TO WS-REF.
           CALL 'ZPS04A' USING BY REFERENCE WS-REF
                               BY CONTENT WS-CONT
                               BY VALUE WS-VAL
                               OMITTED
                               BY REFERENCE WS-OPT-STAT.
           IF WS-REF = 55 AND WS-OPT-STAT = 'O'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '55/O' TO ZCVS-EXPECTED
               MOVE WS-OPT-STAT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: ENTRY STATEMENT PARAMETER BINDING IN ZPS04B
           MOVE 'CALL-ENTRY-BIND' TO ZCVS-TEST-NAME.
           MOVE SPACES TO WS-ENT-OUT.
           CALL 'ZPS04B' USING WS-ENT-IN WS-ENT-OUT.
           IF WS-ENT-OUT = 'ENT:Z999'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ENT:Z999' TO ZCVS-EXPECTED
               MOVE WS-ENT-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

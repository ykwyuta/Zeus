       CBL TRUNC(OPT)
      * ZA123A: behaviors the IBM documentation leaves undefined or
      * unpublished. Every check has basis zos in basis.json: the
      * expected value is the current Zeus behavior and a run on z/OS
      * confirms or corrects it. Overlapping MOVE, divide by zero
      * without ON SIZE ERROR, TRUNC(OPT) beyond the PICTURE, WORKING-
      * STORAGE without VALUE, RANDOM, decimal constants in COMP-1/2,
      * INTEGER-OF-DATE with an invalid date. NUMVAL with an invalid
      * argument ends the run in Zeus and is in ZA124A.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA123A.
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
       01  WS-S PIC X(6) VALUE 'ABCDEF'.
       01  WS-A PIC S9(4) VALUE 5.
       01  WS-H-G.
           05  WS-H PIC 9(4) COMP.
       01  WS-U-G.
           05  WS-U PIC S9(8) COMP.
           05  WS-UP PIC S9(5) COMP-3.
           05  WS-UD PIC X(4).
       01  WS-R PIC V9(9).
       01  WS-F1-G.
           05  WS-F1 COMP-1.
       01  WS-F2-G.
           05  WS-F2 COMP-2.
       01  WS-D PIC 9V9(9).
       01  WS-N PIC S9(4)V99.
       01  WS-RE PIC .9(9).
       01  WS-DE PIC 9.9(9).
       01  WS-I PIC S9(8).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA123A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'OVERLAP-MOVE-RIGHT' TO ZCVS-TEST-NAME.
           MOVE WS-S(1:5) TO WS-S(2:5).
           IF WS-S = 'AABCDE'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AABCDE' TO ZCVS-EXPECTED
               MOVE WS-S TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'OVERLAP-MOVE-LEFT' TO ZCVS-TEST-NAME.
           MOVE 'ABCDEF' TO WS-S.
           MOVE WS-S(2:5) TO WS-S(1:5).
           IF WS-S = 'BCDEFF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'BCDEFF' TO ZCVS-EXPECTED
               MOVE WS-S TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIVIDE-ZERO-NO-SIZE' TO ZCVS-TEST-NAME.
           MOVE 5 TO WS-A.
           DIVIDE 0 INTO WS-A.
           IF WS-A = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '5' TO ZCVS-EXPECTED
               MOVE WS-A TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'TRUNC-OPT-OVER-PIC' TO ZCVS-TEST-NAME.
           MOVE 12345 TO WS-H.
           MOVE WS-H-G TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '0929'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0929' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'WS-NO-VALUE-BINARY' TO ZCVS-TEST-NAME.
           MOVE WS-U-G TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '00000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000000' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'WS-NO-VALUE-PACKED' TO ZCVS-TEST-NAME.
           MOVE WS-U-G(5:3) TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '00000C'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00000C' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'WS-NO-VALUE-ALNUM' TO ZCVS-TEST-NAME.
           MOVE WS-U-G(8:4) TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '40404040'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '40404040' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RANDOM-SEED-ONE' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION RANDOM(1).
           MOVE WS-R TO WS-RE.
           IF WS-R = .847914421
               PERFORM ZCVS-PASS
           ELSE
               MOVE '.847914421' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'RANDOM-NEXT' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION RANDOM.
           MOVE WS-R TO WS-RE.
           IF WS-R = .250019945
               PERFORM ZCVS-PASS
           ELSE
               MOVE '.250019945' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMP1-DECIMAL-TENTH' TO ZCVS-TEST-NAME.
           MOVE 0.1 TO WS-F1.
           MOVE WS-F1-G TO ZCVS-HEX-IN.
           MOVE 4 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:8) = '4019999A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4019999A' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:8) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMP2-DECIMAL-TENTH' TO ZCVS-TEST-NAME.
           MOVE 0.1 TO WS-F2.
           MOVE WS-F2-G TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = '401999999999999A'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '401999999999999A' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'COMP1-BACK-TO-DEC' TO ZCVS-TEST-NAME.
           MOVE 0.1 TO WS-F1.
           MOVE WS-F1 TO WS-D.
           MOVE WS-D TO WS-DE.
           IF WS-D = 0.100000023
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0.100000023' TO ZCVS-EXPECTED
               MOVE WS-DE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DATE-INVALID-DAY' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION INTEGER-OF-DATE(20230230).
           IF WS-I = 154193
               PERFORM ZCVS-PASS
           ELSE
               MOVE '154193' TO ZCVS-EXPECTED
               MOVE WS-I TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * ZS108A: compiler directives and data and environment elements
      * that NIST CCVS85 does not measure (Enterprise COBOL Language
      * Reference): >>DEFINE, >>IF/>>ELSE/>>END-IF, >>EVALUATE with
      * WHEN OTHER, IS NOT DEFINED and DEFINE OVERRIDE; PACKED-DECIMAL
      * as the standard spelling of COMP-3 (preferred sign C/D, F when
      * unsigned); UPSI switches, which are all off under the default
      * UPSI run-time option (00000000); VOLATILE, which does not
      * change the value; and SECONDS-PAST-MIDNIGHT, which is between
      * 0 and 86399.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZS108A.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           UPSI-0 ON STATUS IS U0-ON OFF STATUS IS U0-OFF.
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
       01  WS-C PIC X VALUE SPACE.
       01  WS-P-G.
           05  WS-P PIC S9(3) PACKED-DECIMAL VALUE -12.
       01  WS-U-G.
           05  WS-U PIC 9(3) PACKED-DECIMAL VALUE 12.
       01  WS-V PIC 9 VOLATILE VALUE 3.
       01  WS-S PIC 9(5)V9(6).
       01  WS-SE PIC 9(5).9(6).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZS108A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'DIR-IF-TRUE' TO ZCVS-TEST-NAME.
           >>DEFINE ZLEVEL AS 2
           MOVE 'X' TO WS-C.
           >>IF ZLEVEL > 1
           MOVE 'H' TO WS-C.
           >>ELSE
           MOVE 'L' TO WS-C.
           >>END-IF
           IF WS-C = 'H'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'H' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIR-IF-ELSE' TO ZCVS-TEST-NAME.
           MOVE 'X' TO WS-C.
           >>IF ZLEVEL > 5
           MOVE 'H' TO WS-C.
           >>ELSE
           MOVE 'L' TO WS-C.
           >>END-IF
           IF WS-C = 'L'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'L' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIR-EVALUATE-WHEN' TO ZCVS-TEST-NAME.
           MOVE 'X' TO WS-C.
           >>EVALUATE ZLEVEL
           >>WHEN 1
           MOVE '1' TO WS-C.
           >>WHEN 2
           MOVE '2' TO WS-C.
           >>WHEN OTHER
           MOVE 'O' TO WS-C.
           >>END-EVALUATE
           IF WS-C = '2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIR-EVALUATE-OTHER' TO ZCVS-TEST-NAME.
           MOVE 'X' TO WS-C.
           >>EVALUATE ZLEVEL
           >>WHEN 7
           MOVE '7' TO WS-C.
           >>WHEN OTHER
           MOVE 'O' TO WS-C.
           >>END-EVALUATE
           IF WS-C = 'O'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'O' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIR-NOT-DEFINED' TO ZCVS-TEST-NAME.
           MOVE 'X' TO WS-C.
           >>IF ZNOSUCH IS NOT DEFINED
           MOVE 'N' TO WS-C.
           >>ELSE
           MOVE 'D' TO WS-C.
           >>END-IF
           IF WS-C = 'N'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'N' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DIR-REDEFINE' TO ZCVS-TEST-NAME.
           >>DEFINE ZLEVEL AS 9 OVERRIDE
           MOVE 'X' TO WS-C.
           >>IF ZLEVEL = 9
           MOVE '9' TO WS-C.
           >>END-IF
           IF WS-C = '9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '9' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PACKED-DECIMAL-SIGN' TO ZCVS-TEST-NAME.
           MOVE WS-P-G TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '012D'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '012D' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'PACKED-DECIMAL-NOSGN' TO ZCVS-TEST-NAME.
           MOVE WS-U-G TO ZCVS-HEX-IN.
           MOVE 2 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:4) = '012F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '012F' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UPSI-DEFAULT-OFF' TO ZCVS-TEST-NAME.
           MOVE 'X' TO WS-C.
           IF U0-OFF AND NOT U0-ON
               MOVE 'F' TO WS-C
           END-IF.
           IF WS-C = 'F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F' TO ZCVS-EXPECTED
               MOVE WS-C TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'VOLATILE-VALUE' TO ZCVS-TEST-NAME.
           ADD 1 TO WS-V.
           IF WS-V = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE WS-V TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'SECONDS-PAST-MIDNT' TO ZCVS-TEST-NAME.
           COMPUTE WS-S = FUNCTION SECONDS-PAST-MIDNIGHT.
           MOVE WS-S TO WS-SE.
           IF WS-S >= 0 AND WS-S < 86400
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0-86399' TO ZCVS-EXPECTED
               MOVE WS-SE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

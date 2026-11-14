      * ZD116A: dynamic-length elementary items (DYNAMIC LENGTH
      * clause). The length is the current number of characters: zero
      * at first, set by MOVE to the sender length (at most LIMIT,
      * truncated on the right, on a character boundary for PIC U), by
      * SET LENGTH OF, and kept by a reference-modified receiver. A
      * figurative constant has the current length, at least one
      * character.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD116A.
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
       01  D1   PIC X DYNAMIC LENGTH.
       01  D2   PIC X DYNAMIC LIMIT 4.
       01  DU   PIC U DYNAMIC LENGTH LIMIT IS 4.
       01  S    PIC S9(4).
       01  F6   PIC X(6).
       01  H    PIC X(24).
       01  L    PIC 9(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD116A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'DL-INITIAL-ZERO' TO ZCVS-TEST-NAME.
           MOVE LENGTH OF D1 TO L.
           IF L = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-MOVE-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 'ABCDEFG' TO D1.
           MOVE LENGTH OF D1 TO L.
           IF L = 7
               PERFORM ZCVS-PASS
           ELSE
               MOVE '7' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-MOVE-CONTENT' TO ZCVS-TEST-NAME.
           MOVE 'ABCDEFG' TO D1.
           IF D1 = 'ABCDEFG'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABCDEFG' TO ZCVS-EXPECTED
               MOVE D1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-SHORTER-MOVE' TO ZCVS-TEST-NAME.
           MOVE 'ABCDEFG' TO D1.
           MOVE 'XY' TO D1.
           MOVE FUNCTION LENGTH(D1) TO L.
           IF L = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-LIMIT-TRUNC' TO ZCVS-TEST-NAME.
           MOVE 'ABCDEFG' TO D2.
           MOVE LENGTH OF D2 TO L.
           IF L = 4 AND D2 = 'ABCD'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4 ABCD' TO ZCVS-EXPECTED
               MOVE D2 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-SENDER-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 'PQR' TO D1.
           MOVE ALL '*' TO F6.
           MOVE D1 TO F6.
           IF F6 = 'PQR   '
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'PQR' TO ZCVS-EXPECTED
               MOVE F6 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-COMPARE-PAD' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO D1.
           IF D1 = 'AB    ' AND D1 < 'ABC'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB < ABC' TO ZCVS-EXPECTED
               MOVE D1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-FIG-CURRENT' TO ZCVS-TEST-NAME.
           MOVE 'ABC' TO D1.
           MOVE SPACES TO D1.
           MOVE LENGTH OF D1 TO L.
           IF L = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-FIG-EMPTY' TO ZCVS-TEST-NAME.
           MOVE 'Z' TO D1.
           SET LENGTH OF D1 TO 0.
           MOVE ZERO TO D1.
           MOVE LENGTH OF D1 TO L.
           IF L = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-SET-TRUNCATE' TO ZCVS-TEST-NAME.
           MOVE 'ABCDEF' TO D1.
           SET LENGTH OF D1 TO 2.
           IF D1 = 'AB' AND FUNCTION LENGTH(D1) = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB' TO ZCVS-EXPECTED
               MOVE D1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-SET-NEGATIVE' TO ZCVS-TEST-NAME.
           MOVE 'ABC' TO D1.
           MOVE -5 TO S.
           SET LENGTH OF D1 TO S.
           MOVE LENGTH OF D1 TO L.
           IF L = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-REFMOD-RECEIVER' TO ZCVS-TEST-NAME.
           MOVE 'ABCDE' TO D1.
           MOVE 'XYZ' TO D1(2:2).
           MOVE LENGTH OF D1 TO L.
           IF D1 = 'AXYDE' AND L = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AXYDE' TO ZCVS-EXPECTED
               MOVE D1 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-UTF8-BOUNDARY' TO ZCVS-TEST-NAME.
           MOVE UX'41C3A9C3A9' TO DU.
           MOVE FUNCTION HEX-OF(DU) TO H.
           IF H = '41C3A9'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '41C3A9' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-UTF8-LENGTH' TO ZCVS-TEST-NAME.
           MOVE UX'41C3A9C3A9' TO DU.
           MOVE FUNCTION LENGTH(DU) TO L.
           IF L = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DL-UTF8-BYTES' TO ZCVS-TEST-NAME.
           MOVE UX'41C3A9C3A9' TO DU.
           MOVE LENGTH OF DU TO L.
           IF L = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

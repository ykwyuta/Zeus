      * ZN116A: UTF-8 data (USAGE UTF-8, PICTURE U). PIC U(n) reserves
      * 4n bytes and holds n characters; PIC U BYTE-LENGTH m holds m
      * bytes. Receivers are padded with UX'20' and truncated on a
      * character boundary. Literals U'..' (with \u escapes) and
      * UX'..'. UTF-8 comparison is bytewise after conversion to
      * UTF-8. Only characters shared by CCSID 037 and 1140 are used.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN116A.
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
       01  UF3  PIC U(3).
       01  UF1  PIC U(1) USAGE UTF-8.
       01  UB6  PIC U BYTE-LENGTH 6 VALUE 'AB'.
       01  UB5  PIC U BYTE-LENGTH 5.
       01  N2   PIC N(2) USAGE NATIONAL.
       01  X2   PIC X(2).
       01  H    PIC X(24).
       01  L    PIC 9(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN116A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'U-FIXED-STORAGE' TO ZCVS-TEST-NAME.
           MOVE LENGTH OF UF3 TO L.
           IF L = 12
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-FIXED-LENGTH-FN' TO ZCVS-TEST-NAME.
           MOVE FUNCTION LENGTH(UF3) TO L.
           IF L = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-BYTE-STORAGE' TO ZCVS-TEST-NAME.
           MOVE LENGTH OF UB6 TO L.
           IF L = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-BYTE-LENGTH-FN' TO ZCVS-TEST-NAME.
           MOVE FUNCTION BYTE-LENGTH(UF3) TO L.
           IF L = 12
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-VALUE-ALNUM' TO ZCVS-TEST-NAME.
           MOVE FUNCTION HEX-OF(UB6) TO H.
           IF H = '414220202020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '414220202020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-LITERAL-ESCAPE' TO ZCVS-TEST-NAME.
           MOVE U'caf\u00E9' TO UB6.
           MOVE FUNCTION HEX-OF(UB6) TO H.
           IF H = '636166C3A920'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '636166C3A920' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-LITERAL-HEX' TO ZCVS-TEST-NAME.
           MOVE UX'C3A9' TO UB6.
           MOVE FUNCTION HEX-OF(UB6) TO H.
           IF H = 'C3A920202020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C3A920202020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-TRUNC-CHARS' TO ZCVS-TEST-NAME.
           MOVE U'abcd' TO UF3.
           MOVE FUNCTION HEX-OF(UF3) TO H.
           IF H = '616263202020202020202020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '61626320202020202020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-TRUNC-BOUNDARY' TO ZCVS-TEST-NAME.
           MOVE UX'C3A9C3A9C3A9' TO UB5.
           MOVE FUNCTION HEX-OF(UB5) TO H.
           IF H = 'C3A9C3A920'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C3A9C3A920' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-SUPPLEMENTARY' TO ZCVS-TEST-NAME.
           MOVE U'\U0001F600' TO UF1.
           MOVE FUNCTION HEX-OF(UF1) TO H.
           IF H = 'F09F9880'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F09F9880' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-LENGTH-BYTE-ITEM' TO ZCVS-TEST-NAME.
           MOVE UX'C3A9' TO UB5.
           MOVE FUNCTION LENGTH(UB5) TO L.
           IF L = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE L TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-HIGH-BYTE-TAIL' TO ZCVS-TEST-NAME.
           MOVE HIGH-VALUE TO UB5.
           MOVE FUNCTION HEX-OF(UB5) TO H.
           IF H = 'F48FBFBF7F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F48FBFBF7F' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-HIGH-FIXED' TO ZCVS-TEST-NAME.
           MOVE HIGH-VALUE TO UF1.
           MOVE FUNCTION HEX-OF(UF1) TO H.
           IF H = 'F48FBFBF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'F48FBFBF' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-LOW-VALUE' TO ZCVS-TEST-NAME.
           MOVE LOW-VALUE TO UB5.
           MOVE FUNCTION HEX-OF(UB5) TO H.
           IF H = '0000000000'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0000000000' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-ZERO' TO ZCVS-TEST-NAME.
           MOVE ZERO TO UB5.
           MOVE FUNCTION HEX-OF(UB5) TO H.
           IF H = '3030303030'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3030303030' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-INITIALIZE' TO ZCVS-TEST-NAME.
           MOVE U'abc' TO UF3.
           INITIALIZE UF3.
           MOVE FUNCTION HEX-OF(UF3) TO H.
           IF H = '202020202020202020202020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '20202020202020202020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-FROM-NATIONAL' TO ZCVS-TEST-NAME.
           MOVE N'xy' TO UF3.
           MOVE FUNCTION HEX-OF(UF3) TO H.
           IF H = '787920202020202020202020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '78792020202020202020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-FROM-ALNUM' TO ZCVS-TEST-NAME.
           MOVE 'a\' TO UB5.
           MOVE FUNCTION HEX-OF(UB5) TO H.
           IF H = '615C202020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '615C202020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-TO-NATIONAL' TO ZCVS-TEST-NAME.
           MOVE UX'C3A9' TO N2.
           MOVE FUNCTION HEX-OF(N2) TO H.
           IF H = '00E90020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00E90020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-NATIONAL-OF' TO ZCVS-TEST-NAME.
           MOVE FUNCTION NATIONAL-OF(UX'C3A9') TO N2.
           MOVE FUNCTION HEX-OF(N2) TO H.
           IF H = '00E90020'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '00E90020' TO ZCVS-EXPECTED
               MOVE H(1:20) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-DISPLAY-OF' TO ZCVS-TEST-NAME.
           MOVE FUNCTION DISPLAY-OF(U'Ab') TO X2.
           IF X2 = 'Ab'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Ab' TO ZCVS-EXPECTED
               MOVE X2 TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-COMPARE-PAD' TO ZCVS-TEST-NAME.
           MOVE U'xy' TO UB6.
           IF UB6 = 'xy'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'EQUAL' TO ZCVS-EXPECTED
               MOVE 'NOT EQUAL' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-COMPARE-UTF8' TO ZCVS-TEST-NAME.
           MOVE U'A' TO UB6.
           IF UB6 < 'a' AND UB6 > '1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A<a, A>1' TO ZCVS-EXPECTED
               MOVE 'EBCDIC ORDER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'U-COMPARE-UNEQUAL' TO ZCVS-TEST-NAME.
           MOVE U'xy' TO UF3.
           IF UF3 NOT = U'xz' AND UF3 < U'xz'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'xy<xz' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

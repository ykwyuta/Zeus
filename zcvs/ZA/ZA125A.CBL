      * ZA125A: Enterprise COBOL intrinsic functions that NIST CCVS85
      * does not measure. Expected values follow the Enterprise COBOL
      * for z/OS Language Reference function definitions: E, PI, EXP,
      * EXP10, SIGN, HEX-OF, HEX-TO-CHAR, BIT-OF, BIT-TO-CHAR, TRIM
      * LEADING/TRAILING, NUMVAL-F, TEST-NUMVAL-F, TEST-DATE-YYYYMMDD,
      * TEST-DAY-YYYYDDD, FORMATTED-DATE, FORMATTED-TIME, FORMATTED-
      * DATETIME, INTEGER-OF-FORMATTED-DATE, SECONDS-FROM-FORMATTED-
      * TIME, COMBINED-DATETIME and TEST-FORMATTED-DATETIME. Integer
      * date 1 is 1601-01-01, a Monday (ISO week 1601-W01-1).
      * Irrational results are checked in a closed range, as CCVS85
      * does.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA125A.
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
       01  WS-R PIC S9(3)V9(9).
       01  WS-RE PIC -9(3).9(9).
       01  WS-I PIC S9(9).
       01  WS-IE PIC -9(9).
       01  WS-X PIC X(20).
       01  WS-L PIC 9(4).
       01  WS-T PIC X(6) VALUE '  AB  '.
       01  WS-LE PIC 9(4).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA125A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'FN-E' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION E.
           MOVE WS-R TO WS-RE.
           IF WS-R >= 2.718281827 AND WS-R <= 2.718281829
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2.718281827-2.718281829' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-PI' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION PI.
           MOVE WS-R TO WS-RE.
           IF WS-R >= 3.141592652 AND WS-R <= 3.141592654
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3.141592652-3.141592654' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-EXP-ONE' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION EXP(1).
           MOVE WS-R TO WS-RE.
           IF WS-R >= 2.718281827 AND WS-R <= 2.718281829
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2.718281827-2.718281829' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-EXP10-TWO' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION EXP10(2).
           MOVE WS-R TO WS-RE.
           IF WS-R >= 99.999999999 AND WS-R <= 100.000000001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '99.999999999-100.000000001' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-EXP10-HALF' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION EXP10(0.5).
           MOVE WS-R TO WS-RE.
           IF WS-R >= 3.162277659 AND WS-R <= 3.162277661
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3.162277659-3.162277661' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-SIGN-NEG' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION SIGN(-5).
           MOVE WS-I TO WS-IE.
           IF WS-I = -1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-1' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-SIGN-ZERO' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION SIGN(0).
           MOVE WS-I TO WS-IE.
           IF WS-I = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-SIGN-POS' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION SIGN(0.001).
           MOVE WS-I TO WS-IE.
           IF WS-I = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-HEX-OF' TO ZCVS-TEST-NAME.
           MOVE FUNCTION HEX-OF('AB') TO WS-X.
           IF WS-X = 'C1C2'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C1C2' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-HEX-TO-CHAR' TO ZCVS-TEST-NAME.
           MOVE FUNCTION HEX-TO-CHAR('C1C2') TO WS-X.
           IF WS-X = 'AB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-BIT-OF' TO ZCVS-TEST-NAME.
           MOVE FUNCTION BIT-OF('A') TO WS-X.
           IF WS-X = '11000001'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '11000001' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-BIT-TO-CHAR' TO ZCVS-TEST-NAME.
           MOVE FUNCTION BIT-TO-CHAR('1100000111000010') TO WS-X.
           IF WS-X = 'AB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TRIM-LEADING-LEN' TO ZCVS-TEST-NAME.
           COMPUTE WS-L = FUNCTION LENGTH(FUNCTION TRIM(WS-T LEADING)).
           MOVE WS-L TO WS-LE.
           IF WS-L = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE WS-LE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TRIM-LEADING-TXT' TO ZCVS-TEST-NAME.
           MOVE FUNCTION TRIM(WS-T LEADING) TO WS-X.
           IF WS-X = 'AB'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TRIM-TRAILNG-LEN' TO ZCVS-TEST-NAME.
           COMPUTE WS-L = FUNCTION LENGTH(FUNCTION TRIM(WS-T TRAILING)).
           MOVE WS-L TO WS-LE.
           IF WS-L = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE WS-LE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-NUMVAL-F' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION NUMVAL-F(' 1.5E+02 ').
           MOVE WS-I TO WS-IE.
           IF WS-I = 150
               PERFORM ZCVS-PASS
           ELSE
               MOVE '150' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-NUMVAL-F-NEG' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION NUMVAL-F('-2.5E-1') * 100.
           MOVE WS-I TO WS-IE.
           IF WS-I = -25
               PERFORM ZCVS-PASS
           ELSE
               MOVE '-25' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-NUMVAL-F-OK' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-NUMVAL-F('1.5E+02').
           MOVE WS-I TO WS-IE.
           IF WS-I = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-NUMVAL-F-BAD' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-NUMVAL-F('1.5X').
           MOVE WS-I TO WS-IE.
           IF WS-I = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-DATE-OK' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-DATE-YYYYMMDD(20240229).
           MOVE WS-I TO WS-IE.
           IF WS-I = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-DATE-YEAR' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-DATE-YYYYMMDD(15001231).
           MOVE WS-I TO WS-IE.
           IF WS-I = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-DATE-MONTH' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-DATE-YYYYMMDD(20241301).
           MOVE WS-I TO WS-IE.
           IF WS-I = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-DATE-DAY' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-DATE-YYYYMMDD(20230229).
           MOVE WS-I TO WS-IE.
           IF WS-I = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-DAY-OK' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-DAY-YYYYDDD(2024366).
           MOVE WS-I TO WS-IE.
           IF WS-I = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-DAY-BAD' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-DAY-YYYYDDD(2023366).
           MOVE WS-I TO WS-IE.
           IF WS-I = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-FMT-DATE-EXT' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-DATE('YYYY-MM-DD', 1) TO WS-X.
           IF WS-X = '1601-01-01'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1601-01-01' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-FMT-DATE-ORD' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-DATE('YYYYDDD', 365) TO WS-X.
           IF WS-X = '1601365'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1601365' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-FMT-DATE-WEEK' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-DATE('YYYYWwwD', 1) TO WS-X.
           IF WS-X = '1601W011'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1601W011' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-FMT-TIME' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-TIME('hhmmss', 3661) TO WS-X.
           IF WS-X = '010101'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '010101' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-FMT-TIME-EXT' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-TIME('hh:mm:ss', 86399) TO WS-X.
           IF WS-X = '23:59:59'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '23:59:59' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-FMT-DATETIME' TO ZCVS-TEST-NAME.
           MOVE FUNCTION FORMATTED-DATETIME('YYYYMMDDThhmmss',
               1, 61) TO WS-X.
           IF WS-X = '16010101T000101'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '16010101T000101' TO ZCVS-EXPECTED
               MOVE WS-X TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-INT-OF-FMT-DATE' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION INTEGER-OF-FORMATTED-DATE(
               'YYYY-MM-DD', '1601-01-02').
           MOVE WS-I TO WS-IE.
           IF WS-I = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '2' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-SEC-FROM-FMT' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION SECONDS-FROM-FORMATTED-TIME(
               'hhmmss', '010101').
           MOVE WS-I TO WS-IE.
           IF WS-I = 3661
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3661' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-COMBINED-DT' TO ZCVS-TEST-NAME.
           COMPUTE WS-R = FUNCTION COMBINED-DATETIME(1, 3661).
           MOVE WS-R TO WS-RE.
           IF WS-R >= 1.036609999 AND WS-R <= 1.036610001
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1.036609999-1.036610001' TO ZCVS-EXPECTED
               MOVE WS-RE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-FMT-DT-OK' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-FORMATTED-DATETIME(
               'YYYY-MM-DD', '2024-02-29').
           MOVE WS-I TO WS-IE.
           IF WS-I = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'FN-TEST-FMT-DT-BAD' TO ZCVS-TEST-NAME.
           COMPUTE WS-I = FUNCTION TEST-FORMATTED-DATETIME(
               'YYYY-MM-DD', '2024/02/29').
           MOVE WS-I TO WS-IE.
           IF WS-I = 5
               PERFORM ZCVS-PASS
           ELSE
               MOVE '5' TO ZCVS-EXPECTED
               MOVE WS-IE TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

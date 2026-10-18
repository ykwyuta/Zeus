      *================================================================*
      * ZD106A - NUMERIC-EDITED AND ALPHANUMERIC-EDITED PICTURE        *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD106A.
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
       01  WS-EDIT-FIELDS.
           05  WS-ED-Z              PIC ZZZ,ZZ9.99.
           05  WS-ED-AST            PIC ***,**9.99.
           05  WS-ED-CUR            PIC $$$,$$9.99.
           05  WS-ED-PLUS           PIC +++,++9.99.
           05  WS-ED-MINUS          PIC ---,--9.99.
           05  WS-ED-CR             PIC ZZZ9.99CR.
           05  WS-ED-DB             PIC ZZZ9.99DB.
           05  WS-ED-BWZ            PIC 9999.99 BLANK WHEN ZERO.
           05  WS-ED-AN             PIC XX/XX/XX.
           05  WS-ED-ANB            PIC XXBXX0XX.
           05  WS-SCALE-HI          PIC 999PP.
           05  WS-SCALE-OUT         PIC 9(5).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD106A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: ZERO SUPPRESSION ZZZ,ZZ9.99 WITH 45.67
           MOVE 'ED-Z-SUPPRESS' TO ZCVS-TEST-NAME.
           MOVE 45.67 TO WS-ED-Z.
           IF WS-ED-Z = '     45.67'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '     45.67' TO ZCVS-EXPECTED
               MOVE WS-ED-Z TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: ZERO SUPPRESSION ZZZ,ZZ9.99 WITH 12345.67
           MOVE 'ED-Z-COMMA' TO ZCVS-TEST-NAME.
           MOVE 12345.67 TO WS-ED-Z.
           IF WS-ED-Z = ' 12,345.67'
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' 12,345.67' TO ZCVS-EXPECTED
               MOVE WS-ED-Z TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: CHECK PROTECTION ***,**9.99 WITH 45.67 + EBCDIC HEX
           MOVE 'ED-AST-PROT' TO ZCVS-TEST-NAME.
           MOVE 45.67 TO WS-ED-AST.
           MOVE WS-ED-AST(1:6) TO ZCVS-HEX-IN.
           MOVE 6 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF WS-ED-AST = '*****45.67'
             AND ZCVS-HEX-OUT(1:12) = '5C5C5C5C5CF4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '*****45.67' TO ZCVS-EXPECTED
               MOVE WS-ED-AST TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: FLOATING CURRENCY $$$,$$9.99 WITH 1234.56
           MOVE 'ED-FLOAT-CUR' TO ZCVS-TEST-NAME.
           MOVE 1234.56 TO WS-ED-CUR.
           IF WS-ED-CUR = ' $1,234.56'
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' $1,234.56' TO ZCVS-EXPECTED
               MOVE WS-ED-CUR TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: FLOATING PLUS SIGN POSITIVE AND NEGATIVE
           MOVE 'ED-FLOAT-PLUS' TO ZCVS-TEST-NAME.
           MOVE +123.45 TO WS-ED-PLUS.
           IF WS-ED-PLUS = '   +123.45'
               MOVE -123.45 TO WS-ED-PLUS
               IF WS-ED-PLUS = '   -123.45'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '   -123.45' TO ZCVS-EXPECTED
                   MOVE WS-ED-PLUS TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '   +123.45' TO ZCVS-EXPECTED
               MOVE WS-ED-PLUS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: FLOATING MINUS SIGN POSITIVE AND NEGATIVE
           MOVE 'ED-FLOAT-MINUS' TO ZCVS-TEST-NAME.
           MOVE +123.45 TO WS-ED-MINUS.
           IF WS-ED-MINUS = '    123.45'
               MOVE -123.45 TO WS-ED-MINUS
               IF WS-ED-MINUS = '   -123.45'
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '   -123.45' TO ZCVS-EXPECTED
                   MOVE WS-ED-MINUS TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '    123.45' TO ZCVS-EXPECTED
               MOVE WS-ED-MINUS TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: CR SIGN INDICATOR NEGATIVE AND POSITIVE
           MOVE 'ED-CR-SIGN' TO ZCVS-TEST-NAME.
           MOVE -12.34 TO WS-ED-CR.
           IF WS-ED-CR = '  12.34CR'
               MOVE +12.34 TO WS-ED-CR
               IF WS-ED-CR = '  12.34  '
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '  12.34  ' TO ZCVS-EXPECTED
                   MOVE WS-ED-CR TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '  12.34CR' TO ZCVS-EXPECTED
               MOVE WS-ED-CR TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: DB SIGN INDICATOR NEGATIVE AND POSITIVE
           MOVE 'ED-DB-SIGN' TO ZCVS-TEST-NAME.
           MOVE -12.34 TO WS-ED-DB.
           IF WS-ED-DB = '  12.34DB'
               MOVE +12.34 TO WS-ED-DB
               IF WS-ED-DB = '  12.34  '
                   PERFORM ZCVS-PASS
               ELSE
                   MOVE '  12.34  ' TO ZCVS-EXPECTED
                   MOVE WS-ED-DB TO ZCVS-ACTUAL
                   PERFORM ZCVS-FAIL
               END-IF
           ELSE
               MOVE '  12.34DB' TO ZCVS-EXPECTED
               MOVE WS-ED-DB TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 09: BLANK WHEN ZERO
           MOVE 'ED-BWZ' TO ZCVS-TEST-NAME.
           MOVE 0 TO WS-ED-BWZ.
           IF WS-ED-BWZ = SPACES
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPACES' TO ZCVS-EXPECTED
               MOVE WS-ED-BWZ TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 10: ALPHANUMERIC EDITING XX/XX/XX AND XXBXX0XX
           MOVE 'ED-ALPHANUM' TO ZCVS-TEST-NAME.
           MOVE '261016' TO WS-ED-AN.
           MOVE 'ABCDEF' TO WS-ED-ANB.
           IF WS-ED-AN = '26/10/16' AND WS-ED-ANB = 'AB CD0EF'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '26/10/16' TO ZCVS-EXPECTED
               MOVE WS-ED-AN TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 11: SCALING P (PIC 999PP)
           MOVE 'ED-SCALE-P' TO ZCVS-TEST-NAME.
           MOVE 12300 TO WS-SCALE-HI.
           MOVE WS-SCALE-HI TO WS-SCALE-OUT.
           IF WS-SCALE-OUT = 12300
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12300' TO ZCVS-EXPECTED
               MOVE WS-SCALE-OUT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

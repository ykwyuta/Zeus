      * SPEC: INITIALIZE STATEMENT; AUDIT X03/D10.
      * ZX106A: INITIALIZE precedence FILLER JUSTIFIED and 2D
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZX106A.
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
       01  REC.
           05  TXT PIC XX VALUE 'AB'.
           05  UNSET-TXT PIC XX.
           05  NUM PIC 99 VALUE 37.
           05  UNSET-NUM PIC 99.
           05  FILLER PIC X VALUE 'F'.
       01  RAW REDEFINES REC PIC X(9).
       01  JUST-TXT PIC X(4) JUSTIFIED RIGHT VALUE 'Q'.
       01  TAB.
           05  ROW-ITEM OCCURS 2 TIMES.
               10  CELL OCCURS 2 TIMES.
                   15  CT PIC X VALUE 'A'.
                   15  CN PIC 9 VALUE 7.
                   15  FILLER PIC X VALUE 'F'.
       01  TAB-RAW REDEFINES TAB PIC X(12).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZX106A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'INIT-PLAIN' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC.
           IF RAW = '    0000Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '    0000Z' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-FILLER' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC WITH FILLER.
           IF RAW = '    0000 '
               PERFORM ZCVS-PASS
           ELSE
               MOVE '    0000 ' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-VALUE-ONLY' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC ALL TO VALUE.
           IF RAW = 'ABZZ3799Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABZZ3799Z' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-VALUE-DEFAULT' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC ALL TO VALUE THEN TO DEFAULT.
           IF RAW = 'AB  3700Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'AB  3700Z' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-ALNUM-VALUE' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC ALPHANUMERIC TO VALUE.
           IF RAW = 'ABZZ9999Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABZZ9999Z' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-REPLACE' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC REPLACING
             ALPHANUMERIC BY 'XY' NUMERIC BY 42.
           IF RAW = 'XYXY4242Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'XYXY4242Z' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-VALUE-REPLACE' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC ALL TO VALUE
             THEN REPLACING ALPHANUMERIC BY 'XY'
             NUMERIC BY 42 THEN TO DEFAULT.
           IF RAW = 'ABXY3742Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABXY3742Z' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-FULL' TO ZCVS-TEST-NAME.
           MOVE 'ZZZZ9999Z' TO RAW.
           INITIALIZE REC WITH FILLER ALL TO VALUE
             THEN REPLACING ALPHANUMERIC BY 'XY'
             NUMERIC BY 42 THEN TO DEFAULT.
           IF RAW = 'ABXY3742F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ABXY3742F' TO ZCVS-EXPECTED
               MOVE RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-JUST-VALUE' TO ZCVS-TEST-NAME.
           MOVE 'Z' TO JUST-TXT.
           INITIALIZE JUST-TXT ALL TO VALUE.
           IF JUST-TXT = 'Q   '
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Q   ' TO ZCVS-EXPECTED
               MOVE JUST-TXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-JUST-REPLACE' TO ZCVS-TEST-NAME.
           INITIALIZE JUST-TXT
             REPLACING ALPHANUMERIC BY 'Z'.
           IF JUST-TXT = '   Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '   Z' TO ZCVS-EXPECTED
               MOVE JUST-TXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-2D-ALL' TO ZCVS-TEST-NAME.
           MOVE ALL 'Z' TO TAB-RAW.
           INITIALIZE TAB WITH FILLER ALL TO VALUE.
           IF TAB-RAW = 'A7FA7FA7FA7F'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'A7FA7FA7FA7F' TO ZCVS-EXPECTED
               MOVE TAB-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-2D-ONE' TO ZCVS-TEST-NAME.
           MOVE ALL 'Z' TO TAB-RAW.
           INITIALIZE CELL(2, 1) WITH FILLER
             ALL TO VALUE THEN TO DEFAULT.
           IF TAB-RAW = 'ZZZZZZA7FZZZ'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'ZZZZZZA7FZZZ' TO ZCVS-EXPECTED
               MOVE TAB-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-2D-NO-FILLER' TO ZCVS-TEST-NAME.
           MOVE ALL 'Z' TO TAB-RAW.
           INITIALIZE TAB.
           IF TAB-RAW = ' 0Z 0Z 0Z 0Z'
               PERFORM ZCVS-PASS
           ELSE
               MOVE ' 0Z 0Z 0Z 0Z' TO ZCVS-EXPECTED
               MOVE TAB-RAW TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

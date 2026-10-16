      *================================================================*
      * ZN106M - JAPANESE CCSID 939 MIXED LITERALS & DBCS IDENTIFIERS  *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN106M.
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
       01  顧客レコード.
           05  顧客番号             PIC 9(4) COMP-3 VALUE 1025.
           05  顧客氏名             PIC G(4) DISPLAY-1 VALUE G'山田太郎'.
       01  売上合計                 PIC S9(7) COMP-3 VALUE 50000.
       01  WS-LOWER-939             PIC X(3) VALUE 'abc'.
       01  WS-MIXED-939             PIC X(8) VALUE 'ID=日1'.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN106M' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

      * TEST 01: DBCS IDENTIFIERS AND PARAGRAPH INVOCATION
           MOVE 'JP939-DBCS-IDENT' TO ZCVS-TEST-NAME.
           PERFORM 合計加算処理.
           IF 売上合計 = 51025 AND 顧客氏名 = G'山田太郎'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '51025' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: CCSID 939 LOWERCASE 'abc' IS 818283 (NO SO/SI)
           MOVE 'JP939-LATIN-LOW' TO ZCVS-TEST-NAME.
           MOVE WS-LOWER-939 TO ZCVS-HEX-IN.
           MOVE 3 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:6) = '818283'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '818283' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:6) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: CCSID 939 MIXED LITERAL 'ID=日1' BYTE LAYOUT
           MOVE 'JP939-MIXED-HEX' TO ZCVS-TEST-NAME.
           MOVE WS-MIXED-939 TO ZCVS-HEX-IN.
           MOVE 8 TO ZCVS-HEX-LEN.
           PERFORM ZCVS-TO-HEX.
           IF ZCVS-HEX-OUT(1:16) = 'C9C47E0E45620FF1'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C9C47E0E45620FF1' TO ZCVS-EXPECTED
               MOVE ZCVS-HEX-OUT(1:16) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

       合計加算処理 SECTION.
       合計加算-P.
           ADD 顧客番号 OF 顧客レコード TO 売上合計.
       合計加算-EXIT.
           EXIT.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

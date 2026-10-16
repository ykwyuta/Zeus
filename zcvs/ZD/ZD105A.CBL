      *================================================================*
      * ZD105A - ODO TABLES, INDEXING, SEARCH, AND SEARCH ALL          *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD105A.
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
       01  WS-ODO-COUNT             PIC 9(2) COMP VALUE 3.
       01  WS-ODO-GROUP.
           05  WS-ODO-HDR           PIC X(4) VALUE 'HDR:'.
           05  WS-ODO-ITEM          PIC X(3)
                                    OCCURS 1 TO 6 TIMES
                                    DEPENDING ON WS-ODO-COUNT
                                    INDEXED BY ODO-IDX.
       01  WS-DEST-BUF              PIC X(22).

       01  WS-ASC-DATA.
           05  FILLER               PIC X(6) VALUE '10AAAA'.
           05  FILLER               PIC X(6) VALUE '20BBBB'.
           05  FILLER               PIC X(6) VALUE '30CCCC'.
           05  FILLER               PIC X(6) VALUE '40DDDD'.
           05  FILLER               PIC X(6) VALUE '50EEEE'.
       01  WS-ASC-TABLE REDEFINES WS-ASC-DATA.
           05  WS-ASC-ENTRY         OCCURS 5 TIMES
                                    ASCENDING KEY IS WS-ASC-KEY
                                    INDEXED BY ASC-IDX.
               10  WS-ASC-KEY       PIC 9(2).
               10  WS-ASC-VAL       PIC X(4).

       01  WS-DSC-DATA.
           05  FILLER               PIC X(6) VALUE '90ZZZZ'.
           05  FILLER               PIC X(6) VALUE '70YYYY'.
           05  FILLER               PIC X(6) VALUE '50XXXX'.
           05  FILLER               PIC X(6) VALUE '30WWWW'.
           05  FILLER               PIC X(6) VALUE '10VVVV'.
       01  WS-DSC-TABLE REDEFINES WS-DSC-DATA.
           05  WS-DSC-ENTRY         OCCURS 5 TIMES
                                    DESCENDING KEY IS WS-DSC-KEY
                                    INDEXED BY DSC-IDX.
               10  WS-DSC-KEY       PIC 9(2).
               10  WS-DSC-VAL       PIC X(4).

       01  WS-IDX-NUM               PIC 9(4) COMP.
       01  WS-FOUND-FLAG            PIC X(1).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD105A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 5 TO WS-ODO-COUNT.
           MOVE 'AA1' TO WS-ODO-ITEM(1).
           MOVE 'BB2' TO WS-ODO-ITEM(2).
           MOVE 'CC3' TO WS-ODO-ITEM(3).
           MOVE 'DD4' TO WS-ODO-ITEM(4).
           MOVE 'EE5' TO WS-ODO-ITEM(5).

      * TEST 01: ODO LENGTH WITH COUNT = 5 (4 + 5 * 3 = 19)
           MOVE 'ODO-LEN-5' TO ZCVS-TEST-NAME.
           IF LENGTH OF WS-ODO-GROUP = 19
               PERFORM ZCVS-PASS
           ELSE
               MOVE '19' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 02: ODO LENGTH WITH COUNT = 2 (4 + 2 * 3 = 10)
           MOVE 'ODO-LEN-2' TO ZCVS-TEST-NAME.
           MOVE 2 TO WS-ODO-COUNT.
           IF LENGTH OF WS-ODO-GROUP = 10
               PERFORM ZCVS-PASS
           ELSE
               MOVE '10' TO ZCVS-EXPECTED
               MOVE 'BAD-LEN' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 03: ODO GROUP MOVE USES CURRENT COUNT (2 -> 'HDR:AA1BB2')
           MOVE 'ODO-GROUP-MOVE' TO ZCVS-TEST-NAME.
           MOVE ALL '*' TO WS-DEST-BUF.
           MOVE WS-ODO-GROUP TO WS-DEST-BUF.
           IF WS-DEST-BUF(1:12) = 'HDR:AA1BB2  '
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HDR:AA1BB2  ' TO ZCVS-EXPECTED
               MOVE WS-DEST-BUF(1:12) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 04: SET INDEX TO / UP BY / DOWN BY
           MOVE 'INDEX-ARITH' TO ZCVS-TEST-NAME.
           MOVE 5 TO WS-ODO-COUNT.
           SET ODO-IDX TO 2.
           SET ODO-IDX UP BY 3.
           SET ODO-IDX DOWN BY 1.
           SET WS-IDX-NUM TO ODO-IDX.
           IF WS-IDX-NUM = 4 AND WS-ODO-ITEM(ODO-IDX) = 'DD4'
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4/DD4' TO ZCVS-EXPECTED
               MOVE WS-ODO-ITEM(ODO-IDX) TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 05: SERIAL SEARCH HIT
           MOVE 'SEARCH-HIT' TO ZCVS-TEST-NAME.
           SET ODO-IDX TO 1.
           MOVE 'N' TO WS-FOUND-FLAG.
           SEARCH WS-ODO-ITEM
               AT END
                   MOVE 'N' TO WS-FOUND-FLAG
               WHEN WS-ODO-ITEM(ODO-IDX) = 'CC3'
                   MOVE 'Y' TO WS-FOUND-FLAG
           END-SEARCH.
           SET WS-IDX-NUM TO ODO-IDX.
           IF WS-FOUND-FLAG = 'Y' AND WS-IDX-NUM = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/3' TO ZCVS-EXPECTED
               MOVE WS-FOUND-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 06: SERIAL SEARCH MISS (AT END)
           MOVE 'SEARCH-MISS' TO ZCVS-TEST-NAME.
           SET ODO-IDX TO 1.
           MOVE 'Y' TO WS-FOUND-FLAG.
           SEARCH WS-ODO-ITEM
               AT END
                   MOVE 'E' TO WS-FOUND-FLAG
               WHEN WS-ODO-ITEM(ODO-IDX) = 'ZZ9'
                   MOVE 'F' TO WS-FOUND-FLAG
           END-SEARCH.
           IF WS-FOUND-FLAG = 'E'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'E' TO ZCVS-EXPECTED
               MOVE WS-FOUND-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 07: SEARCH ALL ASCENDING KEY HIT (FIRST, MIDDLE, LAST)
           MOVE 'SEARCH-ALL-ASC' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-FOUND-FLAG.
           SEARCH ALL WS-ASC-ENTRY
               AT END
                   MOVE 'N' TO WS-FOUND-FLAG
               WHEN WS-ASC-KEY(ASC-IDX) = 40
                   MOVE 'Y' TO WS-FOUND-FLAG
           END-SEARCH.
           IF WS-FOUND-FLAG = 'Y'
             AND WS-ASC-VAL(ASC-IDX) = 'DDDD'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/DDDD' TO ZCVS-EXPECTED
               MOVE WS-FOUND-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 08: SEARCH ALL ASCENDING KEY MISS
           MOVE 'SEARCH-ALL-MISS' TO ZCVS-TEST-NAME.
           MOVE 'Y' TO WS-FOUND-FLAG.
           SEARCH ALL WS-ASC-ENTRY
               AT END
                   MOVE 'M' TO WS-FOUND-FLAG
               WHEN WS-ASC-KEY(ASC-IDX) = 35
                   MOVE 'F' TO WS-FOUND-FLAG
           END-SEARCH.
           IF WS-FOUND-FLAG = 'M'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'M' TO ZCVS-EXPECTED
               MOVE WS-FOUND-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

      * TEST 09: SEARCH ALL DESCENDING KEY HIT
           MOVE 'SEARCH-ALL-DSC' TO ZCVS-TEST-NAME.
           MOVE 'N' TO WS-FOUND-FLAG.
           SEARCH ALL WS-DSC-ENTRY
               AT END
                   MOVE 'N' TO WS-FOUND-FLAG
               WHEN WS-DSC-KEY(DSC-IDX) = 70
                   MOVE 'Y' TO WS-FOUND-FLAG
           END-SEARCH.
           IF WS-FOUND-FLAG = 'Y'
             AND WS-DSC-VAL(DSC-IDX) = 'YYYY'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y/YYYY' TO ZCVS-EXPECTED
               MOVE WS-FOUND-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           STOP RUN.

           COPY ZCVSHEX.
           COPY ZCVSPROC.

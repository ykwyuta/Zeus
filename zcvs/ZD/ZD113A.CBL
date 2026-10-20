      * AUDIT: D08; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * SPEC: ODO VARIABLY LOCATED ITEMS; AUDIT D08.
      * ZD113A: Zero length ODO movable tail grow shrink group MOVE
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD113A.
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
       01  CNT PIC 9 COMP VALUE 0.
       01  FLEX.
           05  PREFIX-TEXT PIC XX.
           05  ITEM-TEXT PIC XX OCCURS 0 TO 4 DEPENDING ON CNT.
           05  TAIL-TEXT PIC XX.
       01  RECEIVER PIC X(12).
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZD113A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ODO-0-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 0 TO CNT.
           MOVE 'HD' TO PREFIX-TEXT.
           MOVE 'TL' TO TAIL-TEXT.
           IF LENGTH OF FLEX = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-0-TAIL' TO ZCVS-TEST-NAME.
           IF TAIL-TEXT = 'TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TL' TO ZCVS-EXPECTED
               MOVE TAIL-TEXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-0-MOVE' TO ZCVS-TEST-NAME.
           MOVE FLEX TO RECEIVER.
           IF RECEIVER = 'HDTL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HDTL' TO ZCVS-EXPECTED
               MOVE RECEIVER TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-1-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 1 TO CNT.
           MOVE 'HD' TO PREFIX-TEXT.
           MOVE 'TL' TO TAIL-TEXT.
           MOVE '11' TO ITEM-TEXT(1).
           IF LENGTH OF FLEX = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-1-TAIL' TO ZCVS-TEST-NAME.
           IF TAIL-TEXT = 'TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TL' TO ZCVS-EXPECTED
               MOVE TAIL-TEXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-1-MOVE' TO ZCVS-TEST-NAME.
           MOVE FLEX TO RECEIVER.
           IF RECEIVER = 'HD11TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HD11TL' TO ZCVS-EXPECTED
               MOVE RECEIVER TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-2-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 2 TO CNT.
           MOVE 'HD' TO PREFIX-TEXT.
           MOVE 'TL' TO TAIL-TEXT.
           MOVE '11' TO ITEM-TEXT(1).
           MOVE '22' TO ITEM-TEXT(2).
           IF LENGTH OF FLEX = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-2-TAIL' TO ZCVS-TEST-NAME.
           IF TAIL-TEXT = 'TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TL' TO ZCVS-EXPECTED
               MOVE TAIL-TEXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-2-MOVE' TO ZCVS-TEST-NAME.
           MOVE FLEX TO RECEIVER.
           IF RECEIVER = 'HD1122TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HD1122TL' TO ZCVS-EXPECTED
               MOVE RECEIVER TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-3-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 4 TO CNT.
           MOVE 'HD' TO PREFIX-TEXT.
           MOVE 'TL' TO TAIL-TEXT.
           MOVE '11' TO ITEM-TEXT(1).
           MOVE '22' TO ITEM-TEXT(2).
           MOVE '33' TO ITEM-TEXT(3).
           MOVE '44' TO ITEM-TEXT(4).
           IF LENGTH OF FLEX = 12
               PERFORM ZCVS-PASS
           ELSE
               MOVE '12' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-3-TAIL' TO ZCVS-TEST-NAME.
           IF TAIL-TEXT = 'TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TL' TO ZCVS-EXPECTED
               MOVE TAIL-TEXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-3-MOVE' TO ZCVS-TEST-NAME.
           MOVE FLEX TO RECEIVER.
           IF RECEIVER = 'HD11223344TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HD11223344TL' TO ZCVS-EXPECTED
               MOVE RECEIVER TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-4-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 1 TO CNT.
           MOVE 'HD' TO PREFIX-TEXT.
           MOVE 'TL' TO TAIL-TEXT.
           MOVE '11' TO ITEM-TEXT(1).
           IF LENGTH OF FLEX = 6
               PERFORM ZCVS-PASS
           ELSE
               MOVE '6' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-4-TAIL' TO ZCVS-TEST-NAME.
           IF TAIL-TEXT = 'TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TL' TO ZCVS-EXPECTED
               MOVE TAIL-TEXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-4-MOVE' TO ZCVS-TEST-NAME.
           MOVE FLEX TO RECEIVER.
           IF RECEIVER = 'HD11TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HD11TL' TO ZCVS-EXPECTED
               MOVE RECEIVER TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-5-LENGTH' TO ZCVS-TEST-NAME.
           MOVE 0 TO CNT.
           MOVE 'HD' TO PREFIX-TEXT.
           MOVE 'TL' TO TAIL-TEXT.
           IF LENGTH OF FLEX = 4
               PERFORM ZCVS-PASS
           ELSE
               MOVE '4' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-5-TAIL' TO ZCVS-TEST-NAME.
           IF TAIL-TEXT = 'TL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TL' TO ZCVS-EXPECTED
               MOVE TAIL-TEXT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'ODO-5-MOVE' TO ZCVS-TEST-NAME.
           MOVE FLEX TO RECEIVER.
           IF RECEIVER = 'HDTL'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'HDTL' TO ZCVS-EXPECTED
               MOVE RECEIVER TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * AUDIT: N05; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZN108A: INITIALIZE categories including national numeric and edi
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZN108A.
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
       01  MIXED-GROUP.
           05  TEXT-FIELD PIC XX.
           05  NUM-FIELD PIC 99.
           05  NAT-FIELD PIC N(2) USAGE NATIONAL.
           05  EDIT-FIELD PIC ZZ9.
           05  NAT-EDIT PIC NBN USAGE NATIONAL.
           05  NAT-NUM PIC 99 USAGE NATIONAL.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZN108A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'INIT-ALPHANUMERIC' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO TEXT-FIELD.
           MOVE 37 TO NUM-FIELD.
           MOVE N'CD' TO NAT-FIELD.
           MOVE 12 TO EDIT-FIELD.
           MOVE N'EF' TO NAT-EDIT.
           MOVE 56 TO NAT-NUM.
           INITIALIZE MIXED-GROUP REPLACING
             ALPHANUMERIC BY 'XY'.
           IF TEXT-FIELD = 'XY'
             AND NUM-FIELD = 37
             AND NAT-FIELD = N'CD'
             AND EDIT-FIELD = ' 12'
             AND NAT-EDIT = N'E F'
             AND NAT-NUM = 56
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-NUMERIC' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO TEXT-FIELD.
           MOVE 37 TO NUM-FIELD.
           MOVE N'CD' TO NAT-FIELD.
           MOVE 12 TO EDIT-FIELD.
           MOVE N'EF' TO NAT-EDIT.
           MOVE 56 TO NAT-NUM.
           INITIALIZE MIXED-GROUP REPLACING
             NUMERIC BY 89.
           IF TEXT-FIELD = 'AB'
             AND NUM-FIELD = 89
             AND NAT-FIELD = N'CD'
             AND EDIT-FIELD = ' 12'
             AND NAT-EDIT = N'E F'
             AND NAT-NUM = 89
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-NATIONAL' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO TEXT-FIELD.
           MOVE 37 TO NUM-FIELD.
           MOVE N'CD' TO NAT-FIELD.
           MOVE 12 TO EDIT-FIELD.
           MOVE N'EF' TO NAT-EDIT.
           MOVE 56 TO NAT-NUM.
           INITIALIZE MIXED-GROUP REPLACING
             NATIONAL BY N'GH'.
           IF TEXT-FIELD = 'AB'
             AND NUM-FIELD = 37
             AND NAT-FIELD = N'GH'
             AND EDIT-FIELD = ' 12'
             AND NAT-EDIT = N'E F'
             AND NAT-NUM = 56
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-NUMERIC-EDITED' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO TEXT-FIELD.
           MOVE 37 TO NUM-FIELD.
           MOVE N'CD' TO NAT-FIELD.
           MOVE 12 TO EDIT-FIELD.
           MOVE N'EF' TO NAT-EDIT.
           MOVE 56 TO NAT-NUM.
           INITIALIZE MIXED-GROUP REPLACING
             NUMERIC-EDITED BY '123'.
           IF TEXT-FIELD = 'AB'
             AND NUM-FIELD = 37
             AND NAT-FIELD = N'CD'
             AND EDIT-FIELD = '123'
             AND NAT-EDIT = N'E F'
             AND NAT-NUM = 56
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-NATIONAL-EDITED' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO TEXT-FIELD.
           MOVE 37 TO NUM-FIELD.
           MOVE N'CD' TO NAT-FIELD.
           MOVE 12 TO EDIT-FIELD.
           MOVE N'EF' TO NAT-EDIT.
           MOVE 56 TO NAT-NUM.
           INITIALIZE MIXED-GROUP REPLACING
             NATIONAL-EDITED BY N'IJ'.
           IF TEXT-FIELD = 'AB'
             AND NUM-FIELD = 37
             AND NAT-FIELD = N'CD'
             AND EDIT-FIELD = ' 12'
             AND NAT-EDIT = N'I J'
             AND NAT-NUM = 56
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'INIT-ALL-CATEGORIES' TO ZCVS-TEST-NAME.
           MOVE 'AB' TO TEXT-FIELD.
           MOVE 37 TO NUM-FIELD.
           MOVE N'CD' TO NAT-FIELD.
           MOVE 12 TO EDIT-FIELD.
           MOVE N'EF' TO NAT-EDIT.
           MOVE 56 TO NAT-NUM.
           INITIALIZE MIXED-GROUP.
           IF TEXT-FIELD = SPACES AND NUM-FIELD = 0
             AND NAT-FIELD = N'  ' AND EDIT-FIELD = '  0'
             AND NAT-EDIT = N'   ' AND NAT-NUM = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPEC RESULT' TO ZCVS-EXPECTED
               MOVE 'MISMATCH' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

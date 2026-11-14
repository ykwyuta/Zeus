      * AUDIT: A09; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZA129A: the Unicode functions ULENGTH, UPOS, USUBSTR,
      * USUPPLEMENTARY, UVALID and UWIDTH on UTF-8 data in alphanumeric
      * items and UTF-16 data in national items, with the examples of
      * the Language Reference: 'Kaefer' as X'4BC3A4666572', a national
      * item with a surrogate pair, the precomposed and decomposed a
      * with diaeresis, and the musical G clef.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZA129A.
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
       01  A PIC X(6) VALUE X'4BC3A4666572'.
       01  B PIC N(8) USAGE NATIONAL
             VALUE NX'005400F6006200750072D858DC6B0073'.
       01  C1 PIC X(3) VALUE X'C3A44B'.
       01  C2 PIC X(4) VALUE X'61CC884B'.
       01  NC PIC N(8) USAGE NATIONAL
             VALUE NX'0054D9C3006200750072D858DC6B0073'.
       01  ND PIC N(8) USAGE NATIONAL
             VALUE NX'005400F60062DC010072D858DC6B0073'.
       01  NG PIC N(4) USAGE NATIONAL VALUE NX'00200020D834DD1E'.
       01  XG PIC X(6) VALUE X'2020F09D849E'.
       01  XBAD PIC X(4) VALUE X'41C04243'.
       01  S2 PIC X(3).
       01  SN PIC N(3) USAGE NATIONAL.
       PROCEDURE DIVISION.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZA129A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'ULENGTH' TO ZCVS-TEST-NAME.
           IF FUNCTION ULENGTH(A) = 5 AND FUNCTION ULENGTH(B) = 7
             AND FUNCTION ULENGTH(C1) = 2 AND FUNCTION ULENGTH(C2) = 3
               PERFORM ZCVS-PASS
           ELSE
               MOVE '5/7/2/3' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UPOS-UTF8' TO ZCVS-TEST-NAME.
           IF FUNCTION UPOS(A 1) = 1 AND FUNCTION UPOS(A 2) = 2
             AND FUNCTION UPOS(A 3) = 4 AND FUNCTION UPOS(A 5) = 6
             AND FUNCTION UPOS(A 6) = 0 AND FUNCTION UPOS(A 0) = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1/2/4/6/0/0' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UPOS-UTF16' TO ZCVS-TEST-NAME.
           IF FUNCTION UPOS(B 2) = 3 AND FUNCTION UPOS(B 6) = 11
             AND FUNCTION UPOS(B 7) = 15
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3/11/15' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'USUBSTR-UTF8' TO ZCVS-TEST-NAME.
           MOVE SPACES TO S2.
           MOVE FUNCTION USUBSTR(A 2 2) TO S2.
           IF S2 = X'C3A466' AND FUNCTION USUBSTR(A 1 2) = X'4BC3A4'
             AND FUNCTION USUBSTR(C2 2 1) = X'CC88'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'C3A466' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'USUBSTR-UTF16' TO ZCVS-TEST-NAME.
           MOVE FUNCTION USUBSTR(B 5 2) TO SN.
           IF SN = NX'0072D858DC6B'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'NX0072D858DC6B' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'USUPPLEMENTARY' TO ZCVS-TEST-NAME.
           IF FUNCTION USUPPLEMENTARY(NG) = 3
             AND FUNCTION USUPPLEMENTARY(XG) = 3
             AND FUNCTION USUPPLEMENTARY(A) = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '3/3/0' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UVALID' TO ZCVS-TEST-NAME.
           IF FUNCTION UVALID(A) = 0 AND FUNCTION UVALID(B) = 0
             AND FUNCTION UVALID(NC) = 2 AND FUNCTION UVALID(ND) = 4
             AND FUNCTION UVALID(XBAD) = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0/0/2/4/2' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'UWIDTH' TO ZCVS-TEST-NAME.
           IF FUNCTION UWIDTH(A 1) = 1 AND FUNCTION UWIDTH(A 2) = 2
             AND FUNCTION UWIDTH(A 3) = 1 AND FUNCTION UWIDTH(B 6) = 4
             AND FUNCTION UWIDTH(A 9) = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '1/2/1/4/0' TO ZCVS-EXPECTED
               MOVE 'OTHER' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

      * AUDIT: P08; SPECIFICATION AND LIMITS IN SCENARIOS.MD.
      * ZP114A: USE FOR DEBUGGING, DEBUG-ITEM and debugging lines
      * Run with PARM='/DEBUG': under CBLOPTS(ON) the runtime options
      * follow the slash, and DEBUG turns on the object-time switch.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZP114A.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SOURCE-COMPUTER. IBM-370 WITH DEBUGGING MODE.
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
       01  HITS PIC 9(4) COMP VALUE 0.
       01  T-COUNT PIC 9(4) COMP VALUE 0.
       01  MODE-FLAG PIC X VALUE 'P'.
       01  D-FLAG PIC X VALUE 'N'.
       01  SAVED-NAME PIC X(30).
       01  SAVED-CONT PIC X(12).
       01  SAVED-SUB PIC X(17).
       LINKAGE SECTION.
       01  OS-PARM.
           05  PARM-LEN PIC S999 COMP.
           05  PARM-TEXT PIC X(20).
       PROCEDURE DIVISION USING OS-PARM.
       DECLARATIVES.
       DBG SECTION.
           USE FOR DEBUGGING ON TARGET-PARA FALL-PARA.
       DBG-1.
           ADD 1 TO HITS.
           MOVE DEBUG-NAME TO SAVED-NAME.
           MOVE DEBUG-CONTENTS(1:12) TO SAVED-CONT.
           MOVE DEBUG-ITEM(39:17) TO SAVED-SUB.
       END DECLARATIVES.
       MAIN-PROC SECTION.
       MAIN-000.
           MOVE 'ZP114A' TO ZCVS-PROG-ID.
           PERFORM ZCVS-BEGIN.

           MOVE 'PARM-RUNTIME-SPLIT' TO ZCVS-TEST-NAME.
           IF PARM-LEN = 0
               PERFORM ZCVS-PASS
           ELSE
               MOVE '0' TO ZCVS-EXPECTED
               MOVE 'NOT 0' TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DEBUG-PERFORM-LOOP' TO ZCVS-TEST-NAME.
           PERFORM TARGET-PARA.
           IF HITS = 1 AND SAVED-NAME = 'TARGET-PARA'
             AND SAVED-CONT = 'PERFORM LOOP' AND T-COUNT = 1
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TARGET-PARA/PERFORM LOOP' TO ZCVS-EXPECTED
               MOVE SAVED-CONT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DEBUG-SUB-SPACES' TO ZCVS-TEST-NAME.
           IF SAVED-SUB = SPACES
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'SPACES' TO ZCVS-EXPECTED
               MOVE SAVED-SUB TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'G' TO MODE-FLAG.
           GO TO TARGET-PARA.
       TARGET-PARA.
           ADD 1 TO T-COUNT.
           IF MODE-FLAG = 'G'
               GO TO AFTER-GO
           END-IF.
       AFTER-GO.
           MOVE 'DEBUG-GO-TO' TO ZCVS-TEST-NAME.
           IF HITS = 2 AND SAVED-NAME = 'TARGET-PARA'
             AND SAVED-CONT = SPACES AND T-COUNT = 2
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'TARGET-PARA/SPACES' TO ZCVS-EXPECTED
               MOVE SAVED-CONT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.
       PRE-FALL.
           MOVE 'F' TO MODE-FLAG.
       FALL-PARA.
           MOVE 'DEBUG-FALL-THROUGH' TO ZCVS-TEST-NAME.
           IF HITS = 3 AND SAVED-NAME = 'FALL-PARA'
             AND SAVED-CONT = 'FALL THROUGH'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'FALL-PARA/FALL THROUGH' TO ZCVS-EXPECTED
               MOVE SAVED-CONT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           MOVE 'DEBUGGING-LINE' TO ZCVS-TEST-NAME.
      D    MOVE 'Y' TO D-FLAG.
           IF D-FLAG = 'Y'
               PERFORM ZCVS-PASS
           ELSE
               MOVE 'Y' TO ZCVS-EXPECTED
               MOVE D-FLAG TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF.

           PERFORM ZCVS-END.
           MOVE 0 TO RETURN-CODE.
           STOP RUN.
           COPY ZCVSHEX.
           COPY ZCVSPROC.

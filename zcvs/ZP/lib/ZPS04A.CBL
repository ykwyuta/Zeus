      *================================================================*
      * ZPS04A - SUBPROGRAM TESTING BY REFERENCE / CONTENT / VALUE     *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS04A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-PTR                   USAGE POINTER.
       LINKAGE SECTION.
       01  LK-REF                   PIC S9(4) COMP.
       01  LK-CONTENT               PIC S9(4) COMP.
       01  LK-VAL                   PIC S9(9) COMP-5.
       01  LK-OPT                   PIC X(4).
       01  LK-OPT-STATUS            PIC X(1).
       PROCEDURE DIVISION USING BY REFERENCE LK-REF LK-CONTENT
                                BY VALUE LK-VAL
                                BY REFERENCE LK-OPT LK-OPT-STATUS.
       MAIN-PARM.
           COMPUTE LK-REF = LK-REF + LK-CONTENT + LK-VAL.
           MOVE 999 TO LK-CONTENT.
      * A BY VALUE PARAMETER MUST ALSO BE A PRIVATE WRITABLE COPY.
           MOVE -888 TO LK-VAL.
           SET WS-PTR TO ADDRESS OF LK-OPT.
           IF WS-PTR = NULL
               MOVE 'O' TO LK-OPT-STATUS
           ELSE
               MOVE 'P' TO LK-OPT-STATUS
           END-IF.
           GOBACK.
       END PROGRAM ZPS04A.

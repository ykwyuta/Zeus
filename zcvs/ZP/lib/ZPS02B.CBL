      *================================================================*
      * ZPS02B - RECURSIVE FIBONACCI SUBPROGRAM                        *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS02B IS RECURSIVE.
       DATA DIVISION.
       LOCAL-STORAGE SECTION.
       01  LS-N1                    PIC S9(9) COMP.
       01  LS-N2                    PIC S9(9) COMP.
       01  LS-R1                    PIC S9(9) COMP.
       01  LS-R2                    PIC S9(9) COMP.
       LINKAGE SECTION.
       01  LK-FN                    PIC S9(9) COMP.
       01  LK-FRES                  PIC S9(9) COMP.
       PROCEDURE DIVISION USING LK-FN LK-FRES.
       FIB-MAIN.
           IF LK-FN <= 0
               MOVE 0 TO LK-FRES
           ELSE IF LK-FN = 1
               MOVE 1 TO LK-FRES
           ELSE
               COMPUTE LS-N1 = LK-FN - 1
               COMPUTE LS-N2 = LK-FN - 2
               CALL 'ZPS02B' USING LS-N1 LS-R1
               CALL 'ZPS02B' USING LS-N2 LS-R2
               COMPUTE LK-FRES = LS-R1 + LS-R2
           END-IF.
           GOBACK.
       END PROGRAM ZPS02B.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. COV1.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I     PIC S9(4) COMP VALUE 0.
       01 N     PIC S9(4) COMP VALUE 0.
       01 MODE-SW PIC X VALUE 'A'.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
               IF I > 3
                   ADD 1 TO N
               ELSE
                   SUBTRACT 1 FROM N
               END-IF
           END-PERFORM
           EVALUATE MODE-SW
               WHEN 'A' DISPLAY 'MODE A'
               WHEN 'B' DISPLAY 'MODE B'
               WHEN OTHER DISPLAY 'OTHER'
           END-EVALUATE
           IF N > 100
               PERFORM NEVER-PARA
           END-IF
           DISPLAY 'N=' N
           STOP RUN.
       NEVER-PARA.
           DISPLAY 'NEVER'.

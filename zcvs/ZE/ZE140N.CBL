      * external float VALUE: reject only this rule.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE140N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ITEM-A PIC +9.9E+99 VALUE 1.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE140N.

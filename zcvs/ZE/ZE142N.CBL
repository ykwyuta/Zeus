      * external float binary: reject only this rule.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE142N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ITEM-A PIC +9.9E+99 COMP.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE142N.

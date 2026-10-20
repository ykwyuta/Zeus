      * external float JUSTIFIED: reject only this rule.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE141N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ITEM-A PIC +9.9E+99 JUSTIFIED.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE141N.

      * blank asterisk: reject only this rule.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE143N.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ITEM-A PIC **9 BLANK WHEN ZERO.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE143N.

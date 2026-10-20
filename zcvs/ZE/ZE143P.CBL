      * blank asterisk: accept only this rule.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZE143P.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ITEM-A PIC ZZ9 BLANK WHEN ZERO.
       PROCEDURE DIVISION.
           STOP RUN.
       END PROGRAM ZE143P.

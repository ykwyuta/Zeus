      *================================================================*
      * ZPS03B - SUBPROGRAM RETURNING CHARACTER ITEM                   *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS03B.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-TEXT                  PIC X(8).
       PROCEDURE DIVISION RETURNING LK-TEXT.
       CHR-MAIN.
           MOVE 'RET-OK! ' TO LK-TEXT.
           GOBACK.
       END PROGRAM ZPS03B.

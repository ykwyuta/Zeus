      *================================================================*
      * ZPS04B - SUBPROGRAM USING ENTRY STATEMENT PARAMETER BINDING    *
      *================================================================*
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZPS04X.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-IN                    PIC X(4).
       01  LK-OUT                   PIC X(8).
       PROCEDURE DIVISION.
       MAIN-ENTRY.
           ENTRY 'ZPS04B' USING LK-IN LK-OUT.
           STRING 'ENT:' LK-IN DELIMITED BY SIZE INTO LK-OUT.
           GOBACK.
       END PROGRAM ZPS04X.

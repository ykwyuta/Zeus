       IDENTIFICATION DIVISION.
       PROGRAM-ID. DLIPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-COUNT              PIC 9(3) VALUE 0.
       01  WS-SUMMARY.
           05  SUM-ACCT          PIC X(8).
           05  SUM-LIMIT         PIC S9(7) COMP-3.
       01  WS-DETAIL.
           05  DTL-TYPE          PIC X.
       LINKAGE SECTION.
       01  LK-ACCT               PIC X(8).
       PROCEDURE DIVISION USING LK-ACCT.
       MAIN-PARA.
           EXEC DLI GU USING PCB(1)
               SEGMENT(PASUM) INTO(WS-SUMMARY)
               WHERE(ACCNTID = LK-ACCT)
           END-EXEC
           EVALUATE DIBSTAT
               WHEN SPACES
                   IF SUM-LIMIT > 1000
                       DISPLAY 'HIGH LIMIT'
                   END-IF
               WHEN 'GE'
                   DISPLAY 'NO SUMMARY'
                   GOBACK
               WHEN OTHER
                   DISPLAY 'DLI ERROR ' DIBSTAT
                   GOBACK
           END-EVALUATE
           PERFORM UNTIL DIBSTAT NOT = SPACES
               EXEC DLI GNP USING PCB(1)
                   SEGMENT(PADTL) INTO(WS-DETAIL)
               END-EXEC
               IF DIBSTAT = SPACES
                   ADD 1 TO WS-COUNT
                   IF DTL-TYPE = 'D'
                       DISPLAY 'DECLINED'
                   END-IF
               END-IF
           END-PERFORM
           IF WS-COUNT > 1
               DISPLAY 'DETAILS ' WS-COUNT
           END-IF
           GOBACK.

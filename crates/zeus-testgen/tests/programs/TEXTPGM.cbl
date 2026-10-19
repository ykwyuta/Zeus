       IDENTIFICATION DIVISION.
       PROGRAM-ID. TEXTPGM.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IN-FILE ASSIGN TO INDD
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  IN-FILE.
       01  IN-REC                PIC X(10).
       WORKING-STORAGE SECTION.
       01  WS-FS.
           05  WS-FS1            PIC X.
           05  WS-FS2            PIC X.
       01  WS-OPT-X              PIC X(2) JUST RIGHT.
       01  WS-OPT                PIC 9(2).
       01  WS-SPACES             PIC 9(2) VALUE 0.
       01  WS-MENU.
           05  FILLER            PIC X(8) VALUE 'LIST    '.
           05  FILLER            PIC X(8) VALUE 'SHOW    '.
           05  FILLER            PIC X(8) VALUE 'ADMIN   '.
       01  WS-MENU-T REDEFINES WS-MENU.
           05  WS-ITEM           PIC X(8) OCCURS 3 TIMES.
       01  WS-DATE.
           05  WS-DATE-LEN       PIC S9(4) BINARY VALUE 8.
           05  WS-DATE-TXT       PIC X(8).
       01  WS-PIC.
           05  WS-PIC-LEN        PIC S9(4) BINARY VALUE 8.
           05  WS-PIC-TXT        PIC X(8) VALUE 'YYYYMMDD'.
       01  WS-LILIAN             PIC S9(9) BINARY.
       01  WS-FC.
           05  WS-FC-TOKEN.
               88  FC-OK         VALUE X'0000000000000000'.
               88  FC-BAD-DATE   VALUE X'000309CC59C3C5C5'.
               10  WS-SEV        PIC S9(4) BINARY.
               10  WS-MSG        PIC S9(4) BINARY.
               10  FILLER        PIC X(4).
           05  FILLER            PIC X(4).
       01  WS-PCB.
           05  PCB-DBD           PIC X(8).
           05  PCB-LEVEL         PIC XX.
           05  PCB-STATUS        PIC XX.
           05  FILLER            PIC X(20).
       01  WS-FUNC               PIC X(4) VALUE 'GN  '.
       01  WS-SEG.
           05  SEG-KIND          PIC X.
           05  SEG-AMOUNT        PIC S9(5) COMP-3.
       01  WS-SEGS               PIC 9(3) VALUE 0.
       LINKAGE SECTION.
       01  LK-OPTION             PIC X(2).
       01  LK-DATE               PIC X(8).
       PROCEDURE DIVISION USING LK-OPTION LK-DATE.
       MAIN-PARA.
      *    a menu option: right justified, spaces to zeros
           MOVE LK-OPTION TO WS-OPT-X
           INSPECT WS-OPT-X TALLYING WS-SPACES FOR ALL SPACE
           INSPECT WS-OPT-X REPLACING ALL SPACE BY '0'
           MOVE WS-OPT-X TO WS-OPT
           IF WS-OPT IS NOT NUMERIC OR WS-OPT = 0 OR WS-OPT > 3
               DISPLAY 'BAD OPTION'
           ELSE
               IF WS-ITEM(WS-OPT) = 'ADMIN'
                   DISPLAY 'ADMIN ONLY'
               ELSE
                   DISPLAY 'OPTION ' WS-ITEM(WS-OPT)
               END-IF
           END-IF
           IF WS-SPACES = 2
               DISPLAY 'NO OPTION'
           END-IF
      *    an LE service's feedback
           MOVE LK-DATE TO WS-DATE-TXT
           CALL 'CEEDAYS' USING WS-DATE WS-PIC WS-LILIAN WS-FC
           EVALUATE TRUE
               WHEN FC-OK
                   DISPLAY 'DATE OK'
               WHEN FC-BAD-DATE
                   DISPLAY 'BAD DATE'
               WHEN OTHER
                   DISPLAY 'DATE ERROR ' WS-MSG
           END-EVALUATE
      *    a status that is not numeric (VSAM)
           OPEN INPUT IN-FILE
           READ IN-FILE
           IF WS-FS NOT NUMERIC
               DISPLAY 'VSAM ERROR'
           END-IF
           CLOSE IN-FILE
      *    a DL/I call: segments until GB
           PERFORM UNTIL PCB-STATUS = 'GB'
               CALL 'CBLTDLI' USING WS-FUNC WS-PCB WS-SEG
               IF PCB-STATUS = SPACES
                   ADD 1 TO WS-SEGS
                   IF SEG-AMOUNT < 0
                       DISPLAY 'CREDIT ' SEG-KIND
                   END-IF
               END-IF
           END-PERFORM
           IF WS-SEGS > 2
               DISPLAY 'SEGMENTS ' WS-SEGS
           END-IF
           GOBACK.

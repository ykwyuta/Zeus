      *================================================================*
      * ZCVSRPT - ZCVS SELF-CHECKING REPORT WORKING-STORAGE FIELDS     *
      *================================================================*
       01  ZCVS-WORK-FIELDS.
           05  ZCVS-PROG-ID         PIC X(8)  VALUE SPACES.
           05  ZCVS-TEST-NAME       PIC X(20) VALUE SPACES.
           05  ZCVS-EXPECTED        PIC X(20) VALUE SPACES.
           05  ZCVS-ACTUAL          PIC X(20) VALUE SPACES.
       01  ZCVS-COUNTERS.
           05  ZCVS-PASS-COUNT      PIC 9(4)  VALUE 0.
           05  ZCVS-FAIL-COUNT      PIC 9(4)  VALUE 0.
           05  ZCVS-DEL-COUNT       PIC 9(4)  VALUE 0.
           05  ZCVS-INSP-COUNT      PIC 9(4)  VALUE 0.
           05  ZCVS-TOTAL-COUNT     PIC 9(4)  VALUE 0.
       01  ZCVS-LINE.
           05  FILLER               PIC X(2)  VALUE SPACES.
           05  ZCVS-STATUS          PIC X(5)  VALUE SPACES.
           05  FILLER               PIC X(5)  VALUE SPACES.
           05  ZCVS-LINE-TEST       PIC X(20) VALUE SPACES.
           05  FILLER               PIC X(2)  VALUE SPACES.
           05  ZCVS-DETAIL          PIC X(86) VALUE SPACES.
       01  ZCVS-TOTAL-LINE          PIC X(120) VALUE SPACES.
       01  ZCVS-HEX-WORK.
           05  ZCVS-HEX-IN          PIC X(16) VALUE SPACES.
           05  ZCVS-HEX-LEN         PIC 9(4) COMP VALUE 0.
           05  ZCVS-HEX-OUT         PIC X(32) VALUE SPACES.
           05  ZCVS-HEX-IDX         PIC 9(4) COMP VALUE 0.
           05  ZCVS-HEX-OUT-POS     PIC 9(4) COMP VALUE 0.
           05  ZCVS-HEX-WORD        PIC 9(4) COMP VALUE 0.
           05  ZCVS-HEX-BYTES REDEFINES ZCVS-HEX-WORD.
               10  ZCVS-HEX-PAD     PIC X.
               10  ZCVS-HEX-CH      PIC X.
           05  ZCVS-HEX-HI          PIC 9(4) COMP VALUE 0.
           05  ZCVS-HEX-LO          PIC 9(4) COMP VALUE 0.
           05  ZCVS-HEX-TABLE       PIC X(16)
               VALUE '0123456789ABCDEF'.

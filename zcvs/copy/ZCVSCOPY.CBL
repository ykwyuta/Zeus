      *================================================================*
      * ZCVSCOPY - PRIMARY COPYBOOK FOR ZX103A COPY/REPLACE TESTS      *
      *================================================================*
       01  :PFX:-COPY-REC.
           05  :PFX:-LIB-NAME       PIC X(8)  VALUE 'PRIMARY '.
           05  :PFX:-COPY-VAL       PIC 9(4)  VALUE 1234.
           COPY ZCVSNEST.

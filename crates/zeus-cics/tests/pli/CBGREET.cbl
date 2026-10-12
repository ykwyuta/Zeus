       IDENTIFICATION DIVISION.
       PROGRAM-ID. CBGREET.
      * LINKed to from PL/I: LINKs to a PL/I program and CALLs a PL/I
      * procedure, all on the COMMAREA in the task's storage.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LEN PIC S9(9) COMP.
       LINKAGE SECTION.
       01 DFHCOMMAREA.
          05 CA-NAME PIC X(10).
          05 CA-TEXT PIC X(40).
          05 CA-LEN  PIC S9(9) COMP.
       PROCEDURE DIVISION.
           EXEC CICS LINK PROGRAM('PLUPPER') COMMAREA(CA-NAME)
                LENGTH(10) END-EXEC
           CALL 'PLLEN' USING CA-NAME WS-LEN
           MOVE WS-LEN TO CA-LEN
           MOVE SPACES TO CA-TEXT
           STRING 'HELLO, ' DELIMITED BY SIZE
                  CA-NAME(1:WS-LEN) DELIMITED BY SIZE
                  '!' DELIMITED BY SIZE
                  INTO CA-TEXT
           EXEC CICS RETURN END-EXEC.

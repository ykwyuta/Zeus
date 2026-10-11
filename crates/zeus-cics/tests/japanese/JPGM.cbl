       IDENTIFICATION DIVISION.
       PROGRAM-ID. JPGM.
      * Japanese screen: the name typed in a SOSI field comes back in
      * a greeting.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       COPY JPMAP.
       01 WS-HELLO  PIC X(40).
       PROCEDURE DIVISION.
           MOVE LOW-VALUES TO JPM1O
           EXEC CICS SEND MAP('JPM1') MAPSET('JPMAP') ERASE END-EXEC
           EXEC CICS RECEIVE MAP('JPM1') MAPSET('JPMAP') END-EXEC
           STRING 'こんにちは、' DELIMITED BY SIZE
                  NAMEI DELIMITED BY SPACE
                  'さん' DELIMITED BY SIZE
                  INTO WS-HELLO
           MOVE WS-HELLO TO MSGO
           EXEC CICS SEND MAP('JPM1') MAPSET('JPMAP') DATAONLY END-EXEC
           EXEC CICS RETURN END-EXEC.

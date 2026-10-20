       IDENTIFICATION DIVISION.
       PROGRAM-ID. EXTPGM.
      * Reads an EXTERNAL indexed file that its caller has opened.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT RATES ASSIGN TO RATES
               ORGANIZATION INDEXED
               ACCESS MODE DYNAMIC
               RECORD KEY RATE-KEY
               FILE STATUS RATES-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  RATES EXTERNAL.
       01  RATE-REC.
           05  RATE-KEY          PIC X(3).
           05  RATE-VALUE        PIC 9(3).
       WORKING-STORAGE SECTION.
       01  RATES-FS              PIC XX EXTERNAL.
       LINKAGE SECTION.
       01  CODE-IN               PIC X(3).
       PROCEDURE DIVISION USING CODE-IN.
           MOVE CODE-IN TO RATE-KEY
           READ RATES
           IF RATES-FS NOT = '00'
               DISPLAY 'NO RATE'
               GOBACK
           END-IF
           IF RATE-VALUE > 500
               DISPLAY 'HIGH RATE'
           ELSE
               DISPLAY 'RATE'
           END-IF
           GOBACK.

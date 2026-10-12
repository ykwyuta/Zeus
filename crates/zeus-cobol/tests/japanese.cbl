       IDENTIFICATION DIVISION.
       PROGRAM-ID. JP1.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-MIXED   PIC X(30) VALUE 'コード変換 ABC 日本語'.
       01 WS-KANJI   PIC G(6) VALUE G'漢字データ'.
       01 WS-NAME    PIC N(4) VALUE N'山田'.
       01 WS-IN      PIC X(40).
       PROCEDURE DIVISION.
           DISPLAY WS-MIXED
           DISPLAY WS-KANJI
           DISPLAY WS-NAME '様'
           DISPLAY FUNCTION LENGTH(WS-KANJI)
           ACCEPT WS-IN
           DISPLAY '入力=' WS-IN
           STOP RUN.

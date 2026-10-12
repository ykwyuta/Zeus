       IDENTIFICATION DIVISION.
       PROGRAM-ID. JP1.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-MIXED   PIC X(30) VALUE 'コード変換 ABC 日本語'.
       01 WS-KANJI   PIC G(6) VALUE G'漢字データ'.
       01 WS-NAME    PIC N(4) VALUE N'山田'.
       01 WS-IN      PIC X(40).
      * DBCS user-defined words
       01 顧客.
          05 顧客番号  PIC 9(4) VALUE 12.
       01 合計       PIC 9(5) VALUE 30.
       PROCEDURE DIVISION.
           DISPLAY WS-MIXED
           DISPLAY WS-KANJI
           DISPLAY WS-NAME '様'
           DISPLAY FUNCTION LENGTH(WS-KANJI)
           ACCEPT WS-IN
           DISPLAY '入力=' WS-IN
           ADD 顧客番号 OF 顧客 TO 合計
           PERFORM 合計表示
           STOP RUN.
       合計表示.
           DISPLAY '合計=' 合計.

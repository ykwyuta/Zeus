# 0004: PL/I コンパイラとランタイム (zeus-pli)

## 1. 目的

z/OS 上の Enterprise PL/I で書かれたバッチプログラムを、ソースからコンパイルして Zeus 上で
実行できるようにする。対象 3 アプリ (CardDemo / GenApp / Bank of Z) は PL/I を使わないため、
検証は本リポジトリのテストプログラムと IBM の言語仕様書の例で行う。

## 2. 方式

- 新しいクレート `zeus-pli` に、ソース読込 (MARGINS(2,72)・`*PROCESS`・`%INCLUDE`)、字句解析、構文解析、
  意味解析、型付き中間表現 (IR)、インタプリタを置く。コマンドは `zpli`。
- COBOL 用の ZIR / VM は使わない。PL/I は再帰手続き・ブロックごとの AUTOMATIC 記憶域・
  内部手続きからの上位参照・ON ユニットの動的探索を必要とし、ZIR (プログラム単位の静的記憶域)
  とは実行モデルが異なるため。共用するもの:
  - `zeus-decimal` (パック 10 進・2 進の符号化)
  - `zeus-ds` (レコード形式、順次データセット、キー順データセット)
  - `zeus-cobol` のコードページ (EBCDIC 037 と日本語 CCSID の変換)
- データは z/OS の表現のまま保持する。
  - CHARACTER は EBCDIC。
  - FIXED DECIMAL はパック 10 進。
  - FIXED BINARY はビッグエンディアン 2 の補数。
  - FLOAT は IBM 16 進浮動小数点 (短・長・拡張)。
  - VARYING は 2 バイト長接頭部付き。
  - 非整列 BIT はビット単位で詰める。
  - ポインタは 4 バイト。NULL は 'FF000000'X、SYSNULL は 0。
- 構造体の写像は言語仕様書のペアリング算法をビット単位で実装する。
  - 仕様書の写像例 (80 バイト、ダブルワード境界から 4 バイト目に開始) をテストで再現している。
- 意味解析で、演算・比較・代入・引数の変換をすべて明示的な変換ノードとして IR に入れる。
  インタプリタは型付きの値を変換規則どおりに扱う。

## 3. 仕様の典拠とオラクル

- 典拠: IBM *Enterprise PL/I for z/OS V5.3 Language Reference* (SC27-8940-02)。仕様書を読んで実装した (クリーンルーム)。
- オラクル: Iron Spring PL/I 1.4.1 (Linux 版、無償配布のバイナリ) を、GnuCOBOL と同じく出力比較にだけ使う。
  - ライブラリのソースは参照しない。
  - 精度規則など IBM と異なる動作がある (`25+1/3` が FIXEDOVERFLOW にならない、BIF の結果精度など)。
    その場合は仕様書を優先する。
- 既定のコンパイラオプション:
  - RULES(IBM)
  - LIMITS(FIXEDDEC(15) FIXEDBIN(31))
  - DEFAULT(HEXADEC)
  - BIFPREC(31)
  - MARGINS(2,72)
- 実行時の既定:
  - 未処理の ERROR は FINISH の後、異常終了 U4038 になる。
  - 戻りコードは PLIRETC で設定する。

## 4. 対応範囲

| 分野 | 対応 |
|---|---|
| 構造 | 外部・内部手続き、RECURSIVE、RETURNS、ENTRY 文、BEGIN ブロック、PACKAGE、OPTIONS(MAIN) と PARM (`/` 以前は実行時オプション) |
| 宣言 | FIXED/FLOAT・BINARY/DECIMAL・精度、CHARACTER/BIT (VARYING・VARYINGZ)、PICTURE、POINTER、LABEL、FILE、ENTRY、配列 (下限・上限、`*` 引数)、構造体 (レベル番号、因数分解、LIKE)、INITIAL (繰返し因数、`*`)、STATIC/AUTOMATIC/BASED/DEFINED (POSITION)、EXTERNAL、VALUE (名前付き定数)、BUILTIN、CONDITION、暗黙宣言と文脈宣言、属性の既定値 (先頭文字 I-N の規則、`BIN(31)` は FLOAT など) |
| 式 | 演算の結果精度 (RULES(IBM) の表)、10 進・2 進の混在、FIXED/FLOAT の混在、`**`、比較 (算術・文字・ビット・ポインタ)、連結、ビット演算 (`&` `\|` 排他的論理和)、文字・ビット・算術間の変換規則 (算術→文字の p+3 形式、E 形式など) |
| 文 | 代入 (複数代入先・複合代入・配列と構造体の代入・BY NAME・SUBSTR/UNSPEC/STRING 疑似変数)、IF、DO (WHILE・UNTIL・TO/BY・REPEAT・複数指定・FOREVER)、SELECT、LEAVE/ITERATE、GO TO (ブロック外へ)、CALL/RETURN (引数は参照渡し、型が合わなければダミー引数)、STOP/EXIT、ALLOCATE/FREE、DISPLAY (REPLY)、DELAY |
| 条件 | ON/REVERT/SIGNAL/RESIGNAL、ON ユニットの動的探索、SYSTEM、条件接頭辞 (SIZE、NOFOFL など)、既定動作 (メッセージ→ERROR→FINISH)、ONCODE・ONFILE・ONKEY・ONLOC・ONSOURCE・ONCHAR、CONVERSION・FIXEDOVERFLOW・SIZE・ZERODIVIDE・STRINGRANGE・SUBSCRIPTRANGE・ENDFILE・KEY・ENDPAGE・UNDEFINEDFILE・CONDITION(名前) |
| ストリーム入出力 | PUT/GET の LIST・EDIT・DATA、SKIP・PAGE・LINE、PRINT ファイル (タブ位置 25/49/73/97、LINESIZE 120、PAGESIZE 60、ENDPAGE)、書式 A・B・F・E・P・X・COLUMN・SKIP・PAGE・LINE・R (FORMAT 文)・繰返し、GET/PUT STRING、繰返し指定 `(X(I) DO I = ...)` |
| レコード入出力 | OPEN/CLOSE (TITLE・LINESIZE・PAGESIZE)、暗黙オープン、READ (INTO・SET・KEY・KEYTO・IGNORE)、WRITE (FROM・KEYFROM)、REWRITE、DELETE。順次データセットと VSAM KSDS (カタログのキー定義) |
| ピクチャ | 9 Z * Y V , . / B $ S + - CR DB T I R F(n)、浮動記号・ゼロ抑制、文字ピクチャ (A X 9) |
| 組込み関数 | 算術 (ABS、MOD、REM、ROUND、TRUNC、CEIL、FLOOR、MAX、MIN、SIGN、ADD/SUBTRACT/MULTIPLY/DIVIDE、PRECISION、BINARY、DECIMAL、FIXED、FLOAT など)、数学関数、文字列 (SUBSTR、INDEX、SEARCH、VERIFY、TALLY、LENGTH、MAXLENGTH、TRIM、LEFT、RIGHT、CENTER、COPY、REPEAT、REVERSE、TRANSLATE、UPPERCASE、LOWERCASE、HIGH、LOW、COLLATE、HEX、BOOL、UNSPEC、STRING、CHAR、BIT)、配列 (DIM、LBOUND、HBOUND、SUM、PROD、ALL、ANY)、記憶域 (ADDR、NULL、SYSNULL、SIZE、CURRENTSIZE、POINTERADD)、日付 (DATE、TIME、DATETIME、DAYS、DAYSTODATE、WEEKDAY)、PLIRETC/PLIRETV |
| JCL | `IBMZPLI` (SYSIN のソース、SYSLIB の %INCLUDE メンバー) → `IEWL` → `EXEC PGM=` で実行。DD はファイル名 (または TITLE) で対応付け、SYSPRINT・SYSIN・SYSOUT (メッセージ)、PARM、戻りコード、U4038 |

## 5. 未対応 (今後)

- プリプロセッサ: %INCLUDE とリスト制御以外の %DECLARE / %IF / %DO / %ACTIVATE / %REPLACE
- CONTROLLED 記憶域、AREA と OFFSET、REFER、iSUB 定義
- COMPLEX、GRAPHIC/WIDECHAR/UCHAR、浮動小数点ピクチャ (E/K)、DEFAULT 文、GENERIC、ORDINAL、HANDLE と DEFINE STRUCTURE
- ENTRY 変数による呼出し、FETCH、別ロードモジュールの外部手続きの呼出し、COBOL との相互呼出し
- EXEC SQL / EXEC CICS / EXEC DLI、マルチスレッド (ATTACH)
- LOCATE、REGIONAL・相対データセット

## 6. テスト

- `crates/zeus-pli/tests/programs/`: 基本機能、条件処理、レコード・ストリーム入出力、リスト出力の各プログラムと、期待する SYSPRINT 出力・戻りコード。
  - モジュールを直列化して読み戻してから実行するので、ロードモジュールの形も検証している。
- 単体テスト:
  - 字句解析、構文解析、精度規則
  - 算術→文字の変換、16 進浮動小数点
  - ピクチャ編集 (仕様書の表の例)
  - 構造体写像 (仕様書の例)
  - 日付 (リリアン日)
- `crates/zeus-jcl/tests/jobs.rs`: 次の流れのジョブで KSDS を更新し、RC 4 で終わることを確かめる。
  1. IDCAMS で KSDS を定義する。
  2. IBMZPLI でコンパイルし、IEWL でバインドする。
  3. ON KEY で新規口座を追加しながら KSDS を更新し、PARM と PLIRETC を確認する。

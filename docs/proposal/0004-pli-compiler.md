# 0004: PL/I コンパイラとランタイム (zeus-pli)

## 1. 目的

z/OS 上の Enterprise PL/I で書かれたバッチプログラムと CICS プログラムを、ソースからコンパイルして
Zeus 上で実行できるようにする。対象 3 アプリ (CardDemo / GenApp / Bank of Z) は PL/I を使わないため、
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

### 2.1 記憶域の統合 (実行単位の記憶域)

COBOL と PL/I は一つの実行単位の記憶域 (アドレス空間) を共有する。

- 実行単位の記憶域は COBOL の VM (`zeus_cobol::vm::Machine`) の記憶域である。PL/I のモジュールは
  実行中だけそれを借りる (`attach` / `detach`)。他の言語を呼ぶときは記憶域ごと渡して、戻ったら受け取る。
- PL/I のモジュールの領域は、すべてこの記憶域に GETMAIN で取る。
  - STATIC 領域: モジュールごとに最初に借りたときに確保し、INITIAL の値を入れる。
  - ヒープ: BASED・CONTROLLED の ALLOCATE とダミー引数。
  - スタック: 256 KB のセグメントを必要に応じて足す。AUTOMATIC はブロックの活性化ごとにここに取り、
    終わると戻す。セグメントは次の呼出しのために残す。
  - アドレス 0〜X'0F' と NULL (X'FF000000') 以降は参照するとエラーになる。
- 言語間の呼出しは引数のアドレスを渡す (`interlang::XRef`: アドレス・長さ・範囲・文字列長)。
  コピーと書き戻しはしない。呼ばれた側が保持したポインタは戻った後も有効で、COBOL の
  LINKAGE SECTION と PL/I の BASED 変数は同じ記憶域を指せる。
- 関数の戻り値だけはバイト列で返す (`XEnd`)。
- CICS のタスクは同じ仕組みで PL/I プログラムに記憶域を貸す。EIB・COMMAREA・SET のポインタは
  COBOL の場合と同じアドレスになる。

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
| 宣言 | FIXED/FLOAT・BINARY/DECIMAL・精度、CHARACTER/BIT (VARYING・VARYINGZ)、PICTURE、POINTER、LABEL、FILE、ENTRY、配列 (下限・上限、`*` 引数)、構造体 (レベル番号、因数分解、LIKE)、INITIAL (繰返し因数、`*`)、STATIC/AUTOMATIC/BASED/DEFINED (POSITION)/CONTROLLED (世代の積み重ね、宣言または ALLOCATE で決める範囲・長さ、`*`)、REFER (BASED 構造体の最後のメンバーの範囲・長さ)、EXTERNAL、VALUE (名前付き定数)、BUILTIN、CONDITION、暗黙宣言と文脈宣言、属性の既定値 (先頭文字 I-N の規則、`BIN(31)` は FLOAT など) |
| 式 | 演算の結果精度 (RULES(IBM) の表)、10 進・2 進の混在、FIXED/FLOAT の混在、`**`、比較 (算術・文字・ビット・ポインタ)、連結、ビット演算 (`&` `\|` 排他的論理和)、文字・ビット・算術間の変換規則 (算術→文字の p+3 形式、E 形式など) |
| 文 | 代入 (複数代入先・複合代入・配列と構造体の代入・BY NAME・SUBSTR/UNSPEC/STRING 疑似変数)、IF、DO (WHILE・UNTIL・TO/BY・REPEAT・複数指定・FOREVER)、SELECT、LEAVE/ITERATE、GO TO (ブロック外へ)、CALL/RETURN (引数は参照渡し、型が合わなければダミー引数)、STOP/EXIT、ALLOCATE/FREE (BASED・CONTROLLED、SET、範囲・長さの指定)、DISPLAY (REPLY)、DELAY |
| 条件 | ON/REVERT/SIGNAL/RESIGNAL、ON ユニットの動的探索、SYSTEM、条件接頭辞 (SIZE、NOFOFL など)、既定動作 (メッセージ→ERROR→FINISH)、ONCODE・ONFILE・ONKEY・ONLOC・ONSOURCE・ONCHAR、CONVERSION・FIXEDOVERFLOW・SIZE・ZERODIVIDE・STRINGRANGE・SUBSCRIPTRANGE・ENDFILE・KEY・ENDPAGE・UNDEFINEDFILE・CONDITION(名前) |
| ストリーム入出力 | PUT/GET の LIST・EDIT・DATA、SKIP・PAGE・LINE、PRINT ファイル (タブ位置 25/49/73/97、LINESIZE 120、PAGESIZE 60、ENDPAGE)、書式 A・B・F・E・P・X・COLUMN・SKIP・PAGE・LINE・R (FORMAT 文)・繰返し、GET/PUT STRING、繰返し指定 `(X(I) DO I = ...)` |
| レコード入出力 | OPEN/CLOSE (TITLE・LINESIZE・PAGESIZE)、暗黙オープン、READ (INTO・SET・KEY・KEYTO・IGNORE)、WRITE (FROM・KEYFROM)、REWRITE、DELETE。順次データセットと VSAM KSDS (カタログのキー定義) |
| ピクチャ | 9 Z * Y V , . / B $ S + - CR DB T I R F(n)、浮動記号・ゼロ抑制、文字ピクチャ (A X 9)、浮動小数点ピクチャ (仮数部 E/K 指数部。仮数の先頭の桁位置が 0 でないように正規化) |
| 型の追加 | COMPLEX (FIXED・FLOAT、虚数定数 `2I`、四則演算と `**`、= と ^= の比較、REAL・IMAG・CONJG・COMPLEX/CPLX・ABS、実数への変換は実部、文字への変換は `実部+虚部I`)。GRAPHIC (DBCS。1 バイト文字 c は X'42c'、空白は X'4040') と WIDECHAR (UTF-16BE): 記憶域にはそのまま置き、式の中では CHARACTER に変換する (同じ種類どうしの代入は変換しない。1 バイトの EBCDIC にない文字は X'3F')、HEX はバイトを示す、GRAPHIC()・WIDECHAR() 組込み関数。ORDINAL (DEFINE ORDINAL、VALUE、TYPE、ORDINALNAME・ORDINALSUCC・ORDINALPRED・FIRST・LAST、DO の UPTHRU/DOWNTHRU)。DEFAULT 文 (RANGE(文字:文字)・RANGE(接頭辞)・RANGE(*)、属性と VALUE による精度・長さの既定)。GENERIC (WHEN の属性で引数の型から選ぶ、OTHERWISE)。DEFINE STRUCTURE と TYPE (構造体の型。メンバーにも TYPE)、DEFINE ALIAS、HANDLE (`h => メンバー`、NEW(:型:)、HANDLE(x)、PLIFREE) |
| 組込み関数 | 算術 (ABS、MOD、REM、ROUND、TRUNC、CEIL、FLOOR、MAX、MIN、SIGN、ADD/SUBTRACT/MULTIPLY/DIVIDE、PRECISION、BINARY、DECIMAL、FIXED、FLOAT など)、数学関数、文字列 (SUBSTR、INDEX、SEARCH、VERIFY、TALLY、LENGTH、MAXLENGTH、TRIM、LEFT、RIGHT、CENTER、COPY、REPEAT、REVERSE、TRANSLATE、UPPERCASE、LOWERCASE、HIGH、LOW、COLLATE、HEX、BOOL、UNSPEC、STRING、CHAR、BIT)、配列 (DIM、LBOUND、HBOUND、SUM、PROD、ALL、ANY)、記憶域 (ADDR、NULL、SYSNULL、SIZE、CURRENTSIZE、ALLOCATION、POINTERADD)、日付 (DATE、TIME、DATETIME、DAYS、DAYSTODATE、WEEKDAY)、PLIRETC/PLIRETV |
| プリプロセッサ | %DECLARE (FIXED・CHARACTER・BIT)、% 代入、%IF/%THEN/%ELSE、%DO (グループ・反復・WHILE/UNTIL・SKIP)、%SELECT、%LEAVE/%ITERATE、%ACTIVATE/%DEACTIVATE (RESCAN・NORESCAN)、%REPLACE、%NOTE、%INCLUDE/%XINCLUDE、プリプロセッサ手続き (引数、RETURNS、ソース中の呼出し) と主な組込み関数、置換後の再走査 |
| 外部呼出し | 別モジュールの外部手続き (PL/I モジュールをロードライブラリから読込み、関数の戻り値、`*` の範囲・長さの記述子)、COBOL プログラムの呼出しと COBOL からの PL/I 手続きの呼出し (戻りコードは RETURN-CODE と PLIRETV で受け渡し)。引数は実行単位の記憶域のアドレスで渡す (2.1 節) |
| AREA・OFFSET | AREA(n) (16 バイトの見出しと割当て域)、OFFSET(領域)、ALLOCATE ... IN(領域) SET(オフセットかポインタ)、POINTER(o, a)・OFFSET(p, a)・EMPTY()、オフセットとポインタの代入による変換、オフセットを位置指定子とする BASED、領域の代入 (オフセットは領域の中の相対位置なので写した先でも使える)、満杯で AREA 条件 |
| 定義・REFER・CONTROLLED | iSUB 定義 (`DEFINED A(1SUB,1SUB)` など。要素ごとにも配列全体の組込み関数にも)。構造体の途中のメンバーの REFER (後のメンバーの位置と構造体の大きさが REFER の値で決まる)。CONTROLLED の仮引数 (呼出し側の世代を ALLOCATE / FREE する) |
| ENTRY 変数・FETCH | ENTRY 変数 (`ENTRY(...) RETURNS(...) VARIABLE`、ENTRY の仮引数、配列の要素) への手続きの代入・比較、関数としての呼出し (`E(x)`、`TAB(K)(x)`) と CALL。呼ばれた手続きの静的な親は、ENTRY 値を取った時点のブロックの活性化。FETCH (TITLE でロードモジュール名) は別モジュールを読み込み、読み込めなければ ERROR (ONCODE 9251)。RELEASE (名前か `*`) はモジュールを解放し、次の FETCH で STATIC 記憶域が初期化される |
| EXEC SQL | INCLUDE SQLCA (コンパイラが宣言する 136 バイトの SQLCA)、INCLUDE メンバー (DCLGEN)、ホスト変数 (`:a.b`、標識変数 `:x :ind` / `INDICATOR`)、DECLARE CURSOR / OPEN / FETCH / CLOSE、SELECT INTO、INSERT / UPDATE / DELETE、COMMIT / ROLLBACK、WHENEVER (SQLERROR・SQLWARNING・NOT FOUND の GO TO / CONTINUE)。ホスト変数の型は CHAR、CHAR VARYING (VARCHAR)、FIXED DEC、FIXED BIN(15/31/63)。COBOL と同じ Db2 インターフェース (zeus-db2、ZEUS_DB2) で実行する |
| EXEC CICS | EXEC CICS を含むプログラムには、コンパイラが `DFHEIPTR` (POINTER STATIC) と `DFHEIBLK` (DFHEIPTR を基底とする UNALIGNED の EIB、COBOL の DFHEIBLK と同じ配置) を宣言する。OPTIONS(MAIN) の手続きの第 1 引数は COMMAREA を指すポインタ (COMMAREA がなければ NULL)。オプションの引数は、変数なら記憶域 (CHAR、FIXED DEC、FIXED BIN、POINTER、CHAR VARYING の現在値、構造体・配列は全体)、ラベル (HANDLE CONDITION / AID / ABEND)、名前、式の値として CICS に渡す。DFHRESP・DFHVALUE は定数になる。SEND MAP の FROM と RECEIVE MAP の INTO は省略すると `<マップ名>O` / `<マップ名>I`。ADDRESS EIB / COMMAREA。%INCLUDE DFHAID・DFHBMSCA は組込みのメンバー (CHAR(1) の STATIC 変数) を使う (インクルードディレクトリにあればそちらが優先)。`zbms --pli` が PL/I のシンボリックマップ (`<マップ名>I` と、それを基底とする `<マップ名>O`) を作る。zcics はロードライブラリの PL/I モジュールを CICS プログラムとして実行し、LINK / XCTL / RETURN TRANSID COMMAREA で COBOL のプログラムと相互に呼び合える。PL/I の EXEC SQL も CICS のタスクの Db2 接続で実行する。タスクの異常終了はそのまま、未処理の ERROR は APLI、その他の実行時エラーは ASRA になる |
| IMS | CALL PLITDLI (0006)。IMS の領域では主手続きの POINTER パラメーターが PCB を指す。構造体の名前のないメンバー (`2 * CHAR(2)`) |
| JCL | `IBMZPLI` (SYSIN のソース、SYSLIB の %INCLUDE メンバー) → `IEWL` → `EXEC PGM=` で実行。DD はファイル名 (または TITLE) で対応付け、SYSPRINT・SYSIN・SYSOUT (メッセージ)、PARM、戻りコード、U4038 |

## 5. 未対応 (今後)

- プリプロセッサ: %GO TO、キーワード引数・STATEMENT 形式の手続き呼出し、MACRO 以外のプリプロセッサ (SQL・CICS の前処理は 5 章の EXEC を参照)
- AREA の中の FREE (領域は EMPTY の代入で空になるまで再利用しない)、配列の要素の構造体の中の REFER
- UCHAR (UTF-8)、GRAPHIC・WIDECHAR の定数 (`'...'G`・`'...'W`)、GRAPHIC・WIDECHAR の 1 バイトの EBCDIC にない文字を式の中で扱うこと、COMPLEX の数学関数と PUT EDIT の C 書式、DEFINE STRUCTURE の型を引数の型とする手続き、PLIFREE による記憶域の再利用
- FETCH の SET オプション (受け付けるが、ポインタは設定しない)
- 言語間呼出しの制約: 呼出し中のモジュールへの再帰的な呼出しはできない。呼ばれた PL/I モジュールは自分の SYSPRINT に書く (行の単位で呼出し側と順序を合わせる)
- EXEC CICS の制約: COBOL から CALL された PL/I 手続き (CICS プログラムとして LINK されたのではないもの) の中の EXEC CICS RETURN / XCTL は異常終了になり、HANDLE CONDITION は呼んだ COBOL プログラムのものと区別されない。翻訳系のオプション (EDF など) と EXEC CICS の引数に書いた PL/I の組込み関数以外の関数呼出しは未確認
- EXEC DLI (PL/I からの IMS の呼出しは CALL PLITDLI だけ。0006 を参照)
- EXEC SQL: FLOAT・PICTURE・BIT のホスト変数、構造体のホスト変数の展開、動的 SQL (PREPARE / EXECUTE)、SQLDA
- マルチスレッド (ATTACH)
- LOCATE、REGIONAL・相対データセット

## 6. テスト

- `crates/zeus-pli/tests/programs/`: 基本機能、条件処理、レコード・ストリーム入出力、リスト出力、プリプロセッサ、CONTROLLED と REFER、EXEC SQL (ZEUS_DB2 があるときだけ実行) の各プログラムと、期待する SYSPRINT 出力・戻りコード。
  - モジュールを直列化して読み戻してから実行するので、ロードモジュールの形も検証している。
- 単体テスト:
  - 字句解析、構文解析、精度規則
  - 算術→文字の変換、16 進浮動小数点
  - ピクチャ編集 (仕様書の表の例)
  - 構造体写像 (仕様書の例)
  - 日付 (リリアン日)
- `crates/zeus-cics/tests/pli.rs` (PL/I の CICS トランザクション、`tests/pli/`): `zbms --pli` のマップ、`zpli` のコンパイル、zcics のスクリプトで、次を確かめる。
  - SEND MAP (ERASE / DATAONLY) と RECEIVE MAP。
  - WRITE / READ FILE と RESP (NORMAL / NOTFND)。
  - RETURN TRANSID COMMAREA で COMMAREA を次のタスクに引き継ぐこと。
  - PL/I → COBOL の LINK。その COBOL から PL/I への LINK と、PL/I 手続きの CALL。COMMAREA は同じ記憶域で書き換わる。
  - HANDLE CONDITION MAPFAIL、EIBAID と DFHAID、ADDRESS EIB / COMMAREA。
- `crates/zeus-jcl/tests/jobs.rs` (言語間呼出し): COBOL 主プログラム → PL/I 関数 → COBOL サブプログラム、PL/I 主プログラム → PL/I 関数 (STATIC の保持) と COBOL サブプログラム。値・戻りコード・出力順を確かめる。
- `crates/zeus-jcl/tests/jobs.rs`: 次の流れのジョブで KSDS を更新し、RC 4 で終わることを確かめる。
  1. IDCAMS で KSDS を定義する。
  2. IBMZPLI でコンパイルし、IEWL でバインドする。
  3. ON KEY で新規口座を追加しながら KSDS を更新し、PARM と PLIRETC を確認する。

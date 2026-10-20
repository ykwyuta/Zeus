# Proposal 0020: CCVS85 方式による z/OS PL/I 互換性テストスイート (ZPVS)

- 状態: スイート実装済み、実機差分検証は未実施
- 作成日: 2026-10-16
- 関連:
  - [0004 PL/I コンパイラとランタイム (zeus-pli)](0004-pli-compiler.md)
  - [0019 CCVS85 補完 z/OS COBOL 互換性テストスイート (ZCVS)](0019-zos-cobol-test-suite.md)

## 1. 背景と目的

COBOL には NIST CCVS85 という公開の標準適合性テストスイートが存在し、[0019](0019-zos-cobol-test-suite.md) ではその未カバー領域（z/OS 固有の内部表現・IBM 拡張）を補完する `ZCVS` を提案した。一方、PL/I（Enterprise PL/I for z/OS V5.3, SC27-8940-02）には、CCVS85 に相当する自由再配布可能なオープンソースの総合互換性スイートが本リポジトリーに存在しない。

現在 `crates/zeus-pli/tests/programs/` には 20 本あまりのテストプログラム (`basics.pli`, `conditions.pli`, `files.pli`, `storage.pli`, `macros.pli` 等) があるが、これらは **`SYSPRINT` のテキスト出力全体を `.out` スナップショットファイルと文字列比較する方式** を採っている。この方式には次の課題がある。

1. **他処理系（実機 z/OS `IBMZPLI` や Iron Spring PL/I）との直接比較が困難**
   - `PUT LIST` / `PUT DATA` や `PRINT` ファイルの空白幅・改頁・メッセージ文言のわずかな差でスナップショット比較全体が失敗し、「言語意味の不一致」と「出力書式の差」を切り分けにくい。
2. **項目単位の合否集計と仕様トレーサビリティの不足**
   - 1 本の `.pli` ファイルに多数の検証が詰め込まれており、どの仕様項目（構造体写像、精度規則、`ON` 条件、`REFER` 等）が何件合格・失敗したかを `zeus-ccvs report` のようなモジュール別集計表で追跡できない。
3. **負系テスト（コンパイル診断）と境界値の体系網羅の不足**
   - `RULES(IBM)` の精度・スケール計算表、ビット単位の構造体写像算法、非整列 `BIT` 列、全変換マトリクスなどを網羅的に自己検証する仕組みが必要である。

そこで本提案では、[0019 (ZCVS)](0019-zos-cobol-test-suite.md) と共通のレポート形式・ハーネス基盤 (`zeus-ccvs`) を用いた **z/OS PL/I 互換性テストスイート「ZPVS (z/OS PL/I Validation Suite)」** を `zpvs/` 以下に構築することを提案する。

---

## 2. 設計原則

### 2.1 CCVS85 / ZCVS 共通の自己検証レポート形式 (`DD_REPORT`)

各テストプログラムは `%INCLUDE ZPVSRPT;` により共通のレポート出力手続きを取り込み、`RECORD OUTPUT ENV(F RECSIZE(120))` の `REPORT` ファイル（環境変数 `DD_REPORT` または JCL の `//REPORT DD`）へテストごとの `PASS` / `FAIL` と末尾の CCVS85 互換集計行を出力する。

```text
ZPVS REPORT - PROGRAM PD101A - MODULE PD (PLI DATA AND STRUCTURE MAPPING)
------------------------------------------------------------------------------------------------------------------------
PASS  PD101A-T001  SC27-8940-CH6   FIXED DEC(7,2) PACKED HEX REPRESENTATION
PASS  PD101A-T002  SC27-8940-CH8   STRUCTURE PAIRING ALGORITHM 80-BYTE EXAMPLE
FAIL  PD101A-T003  SC27-8940-CH6   UNALIGNED BIT(5) PACKING ACROSS BYTE BOUNDARY
      EXPECTED: F840
      ACTUAL:   F800
------------------------------------------------------------------------------------------------------------------------
     2 OF      3  TESTS WERE EXECUTED SUCCESSFULLY
     1   TEST(S) FAILED
    NO   TEST(S) DELETED
```

この形式により、`SYSPRINT` のタブ位置や改行規則の違いに影響されず、`crates/zeus-ccvs/src/report.rs` のパーサーで COBOL (`CCVS85` / `ZCVS`) と全く同じ仕組みで集計・ベースライン比較ができる。`SYSPRINT` 自体はストリーム入出力モジュール (`PF`) の検証対象として個別にテストする。

### 2.2 PL/I 組込み関数 (`HEX` / `UNSPEC` / `SIZE` / `OFFSET`) による記憶域自己検証

z/OS 上の内部表現（EBCDIC、パック 10 進、ビッグエンディアン 2 進、IBM 16 進浮動小数点 HFP、非整列 `BIT`、2 バイト接頭辞付き `VARYING`、構造体の詰めバイト、`AREA` の 16 バイトヘッダー）は、Rust 側のテストコードから検査するのではなく、PL/I 標準の `HEX(x)`、`UNSPEC(x)`、`SIZE(x)`、`CURRENTSIZE(x)`、`ADDR(x)` を用いて **PL/I プログラム自身が検証** する。

### 2.3 実機 z/OS (`IBMZPLI`)・Iron Spring PL/I・Zeus (`zpli`) 間のポータビリティ

- ソースは `MARGINS(2,72)` 準拠（列 1 は空白、列 2–72 にプログラム本文、列 73–80 は一連番号または空白）で統一し、プログラム・インクルード名は 8 文字以内の PDS メンバー互換名 (`PD101A`, `ZPVSRPT`) とする。
- コンパイラーオプションに依存するテストは先頭行の `*PROCESS` 文で明示する。
- クリーンルーム原則に従い、IBM *Enterprise PL/I for z/OS V5.3 Language Reference* (SC27-8940-02) の仕様規則から独自にテストケースを作成し、`GPL-3.0-or-later` で `zpvs/` に収録する。

---

## 3. モジュール体系 (`P*`)

CCVS85 および ZCVS (`Z*`) と区別するため、ZPVS のモジュールは `P` で始まる 2 文字コードとする。

| モジュール | 名称 | 必須/任意 | 検証対象領域 |
|---|---|---|---|
| **`PD`** | Data Types & Structure Mapping | **必須** | 基本データ型 (`CHAR`, `BIT`, `FIXED/FLOAT` `DEC/BIN`, `COMPLEX`, `PICTURE`, `GRAPHIC`, `WIDECHAR`, `ORDINAL`)、属性既定規則 (`DEFAULT`, `I–N` 規則)、構造体のビット単位ペアリング写像算法 (`ALIGNED`/`UNALIGNED`, `LIKE`, `TYPE`/`DEFINE STRUCTURE`) |
| **`PA`** | Expressions, Precision & Conversions | **必須** | `RULES(IBM)` の算術結果精度・スケール規則 (`LIMITS(FIXEDDEC(15/31) FIXEDBIN(31/63))`)、型変換マトリクス (算術⇔`CHAR`⇔`BIT`⇔`PICTURE`⇔`GRAPHIC`/`WIDECHAR`)、配列・構造体演算 (`BY NAME`、断面 `A(*,J)`、擬似変数) |
| **`PC`** | Control, Procedures & ON Conditions | **必須** | 制御文 (`DO` 全形式、`UPTHRU`/`DOWNTHRU`、`SELECT`、`LEAVE`/`ITERATE`、非局所 `GO TO`)、内部手続きと静的スコープ、`RECURSIVE`、`ENTRY` 文・`ENTRY` 変数、`GENERIC`、`FETCH`/`RELEASE`、`ON`/`REVERT`/`SIGNAL`/`RESIGNAL` と動的伝播、`ONCODE`/`ONCHAR`/`ONSOURCE` |
| **`PS`** | Storage Classes, Pointers, `AREA` & `REFER` | **必須** | `AUTOMATIC`/`STATIC`/`BASED`/`CONTROLLED` (世代スタックと `CONTROLLED` 仮引数)、`POINTER` (`NULL`=`'FF000000'X`, `SYSNULL`=`0`, `POINTERADD`)、`HANDLE`/`NEW`、`AREA` と `OFFSET`、自己定義構造体 `REFER` (中間メンバー含む)、`DEFINED` (`POSITION` と `iSUB`) |
| **`PF`** | Stream I/O & Record I/O | **必須** | ストリーム I/O (`GET`/`PUT` `LIST`/`EDIT`/`DATA`、`STRING` Option、`PRINT` ファイルと `ENDPAGE`、書式項目 `A/B/F/E/P/X/COL/SKIP/R`)、レコード I/O (順次、VSAM KSDS/RRDS、`REGIONAL(1)`、`LOCATE` モード) |
| **`PM`** | Preprocessor, BIFs & Inter-Language | **必須** | マクロプリプロセッサー (`%DECLARE`, `%DO`, `%IF`, `%SELECT`, `%GO TO`, `%ACTIVATE`/`RESCAN`, `%REPLACE`, `%INCLUDE`, プリプロセッサー手続き・`STATEMENT` 形式)、組込み関数群 (`BIFPREC(31)`、Lilian 日付関数等)、言語間呼出し (PL/I ⇔ COBOL / HLASM)、`OPTIONS(MAIN)` と `PARM` / `PLIRETC` |
| **`PE`** | Diagnostics & Negative Suite | **必須** | 不正な宣言・型不整合・プリプロセッサー構文エラー・禁止された `REFER`/`DEFINED` 指定などを確実にコンパイルエラーとして診断することを確認する負系テスト群 |

---

## 4. 各モジュールの詳細テスト計画

### 4.1 `PD`: データ型・構造体写像・属性既定値 (Data Types & Structure Mapping)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 (SC27-8940-02) |
|---|---|---|---|
| `PD101A` | 算術データ型の内部表現 | `FIXED DEC(p,q)` (パック 10 進、`C`/`D` 符号)、`FIXED BIN(7/15/31/63)` と符号なし (`UNSIGNED`)、`FLOAT DEC/BIN` (`DEFAULT(HEXADEC)` の短 4B・長 8B・拡張 16B HFP バイト表現) | Ch.3 Data elements |
| `PD102A` | 文字列・ビット列 (`CHAR` / `BIT`) | 固定長 `CHAR`、`CHAR VARYING` (2B 長接頭辞)、`CHAR VARYINGZ` (末尾 `'00'X`)、`ALIGNED` / `UNALIGNED` `BIT(n)` のバイト境界をまたぐビットパッキング、`INITIAL` (繰返し因数・`*`) | Ch.3, Ch.8 |
| `PD103A` | 構造体写像算法 (Structure Mapping) | IBM ペアリング算法による先頭オフセット補正、スラックバイトおよびスラックビットの挿入、80 バイト仕様書例の再現、`UNALIGNED` と `ALIGNED` の混在、`SIZE`/`ADDR`/`OFFSET` 検証 | Ch.8 Structure mapping |
| `PD104A` | `LIKE`・`DEFINE STRUCTURE`・`TYPE` | `LIKE` によるメンバー・属性展開、型付き構造体 (`DEFINE STRUCTURE`, `TYPE`, ネストした `TYPE`)、`DEFINE ALIAS` | Ch.7 Type definitions |
| `PD105A` | `ORDINAL`・`DEFAULT`・暗黙宣言 | `DEFINE ORDINAL` (`VALUE`, `ORDINALNAME`, `ORDINALSUCC`, `ORDINALPRED`, `FIRST`, `LAST`)、`DEFAULT RANGE(...)`、先頭文字 `I–N` 規則と `BIN(31)` の既定属性 | Ch.6, Ch.7 |
| `PD106A` | `COMPLEX`・`GRAPHIC`・`WIDECHAR` | 複素数 (`FIXED`/`FLOAT COMPLEX`, `2I`, `REAL`, `IMAG`, `CONJG`, `CPLX`)、`GRAPHIC` (DBCS `X'4040'`) と `WIDECHAR` (UTF-16BE `X'0020'`) の記憶域表現と変換 | Ch.3, Ch.4 |

### 4.2 `PA`: 式・`RULES(IBM)` 精度規則・型変換・ピクチャ (Expressions & Conversions)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `PA101A` | `RULES(IBM)` 算術結果精度とスケール | `FIXED DEC` / `FIXED BIN` / `FLOAT` の加減乗除・べき乗 (`**`) の結果 `(p,q)` 規則（例: `FIXED DEC` 除算の `(15, 15-p1+q1-q2)`、10 進と 2 進の混在演算）、`ADD`/`MULTIPLY`/`DIVIDE` BIF による精度指定 | Ch.4 Expressions |
| `PA102A` | 型変換マトリクス (算術⇔文字⇔ビット) | 算術→`CHAR` の `p+3` 幅と先行空白・負号位置、算術→`BIT` (整数部の 2 進変換)、`CHAR`→算術 (`CONVERSION` 条件発火境界)、ビット論理演算 (`&`, `\|`, `^`, `BOOL`) | Ch.5 Data conversion |
| `PA103A` | `PICTURE` (数値・文字・浮動小数点) | 編集文字 `9 Z * Y V , . / B $ S + - CR DB`、オーバーパンチ `T I R`、スケール因数 `F(n)`、文字ピクチャ `A X 9`、浮動小数点ピクチャ (`E`/`K` と仮数正規化) | Ch.15 Picture specification |
| `PA104A` | 配列・構造体演算と擬似変数 | 配列全体・断面 `A(*,J)` の演算、`BY NAME` 構造体代入、`SUBSTR`・`UNSPEC`・`STRING` 擬似変数への代入 | Ch.4, Ch.9 |

### 4.3 `PC`: 制御構造・手続き・`ENTRY`・`ON` 条件処理 (Control, Procedures & Conditions)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `PC101A` | `DO` 全形式・`SELECT`・非局所 `GO TO` | `DO` (`TO`/`BY`, `WHILE`, `UNTIL`, `REPEAT`, 複数仕様、序数 `UPTHRU`/`DOWNTHRU`)、`LEAVE ラベル`、`ITERATE ラベル`、内部手続きから上位ブロックへの `GO TO` (スタック巻き戻し) | Ch.9 Statements |
| `PC102A` | 内部手続き・レキシカルスコープ・再帰 | `RECURSIVE` 手続き、再帰呼出し下での上位ブロック `AUTOMATIC` 変数参照 (ディスプレイ/静的リンク)、`BEGIN` ブロックの記憶域寿命 | Ch.6 Program organization |
| `PC103A` | `ENTRY` 文・`ENTRY` 変数・`GENERIC` | 1 手続き内の複数 `ENTRY`、関数 `RETURNS`、静的親環境を保持する `ENTRY VARIABLE` (配列・引数渡し含む)、引数属性で分岐する `GENERIC` | Ch.6 |
| `PC104A` | 外部手続き・`FETCH` / `RELEASE` | 外部モジュール (`lib/PCS01A.PLI`) の動的 `FETCH` (`TITLE` 指定)、`STATIC` 変数の保持と `RELEASE` 後の再初期化、存在しないモジュールの `ERROR` (`ONCODE 9251`) | Ch.6 |
| `PC105A` | `ON` 条件の動的探索・`REVERT`・`RESIGNAL` | 呼出し階層を遡る `ON` ユニットの動的探索、`REVERT` による上位ハンドラー復元、`RESIGNAL`、`CONDITION(name)` ユーザー定義条件 | Ch.16 Condition handling |
| `PC106A` | 計算・文字列・範囲条件と `ONCHAR`/`ONSOURCE` | `CONVERSION` (`ONCHAR`/`ONSOURCE` 擬似変数による入力修復と再変換)、`FIXEDOVERFLOW`、`SIZE`、`ZERODIVIDE`、`SUBSCRIPTRANGE`、`STRINGRANGE`、条件接頭辞 `(SIZE):` / `(NOFOFL):` | Ch.16, Ch.17 |

### 4.4 `PS`: 記憶域クラス・ポインター・`AREA`・`REFER`・`DEFINED` (Storage & Pointers)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `PS101A` | `BASED`・`POINTER`・`HANDLE` | `ALLOCATE` / `FREE`、`NULL` (`'FF000000'X`) と `SYSNULL` (`'00000000'X`) の区別、`POINTERADD`、`HANDLE(:type:)`・`NEW(:type:)`・`=>` 演算子・`PLIFREE` | Ch.7, Ch.10 |
| `PS102A` | `CONTROLLED` 記憶域と仮引数 | `ALLOCATE` による世代のプッシュと `FREE` によるポップ、`ALLOCATION(x)`、範囲・長さの実行時変更、`CONTROLLED` 仮引数に対する被呼出し側での世代操作 | Ch.10 Storage control |
| `PS103A` | `AREA` と `OFFSET` | `AREA(n)` (16B ヘッダー)、`ALLOCATE x IN(A) SET(O)`、`OFFSET`⇔`POINTER` 相互変換、`AREA` 丸ごとの代入（相対オフセット維持）、`EMPTY()`、満杯時の `AREA` 条件 | Ch.10 |
| `PS104A` | `REFER` (自己定義構造体) | 構造体末尾および構造体中間の `REFER` 要素、後続メンバーの動的オフセット計算、`SIZE(s)` と `CURRENTSIZE(s)` | Ch.10 |
| `PS105A` | `DEFINED` (`POSITION` と `iSUB`) | 単純 `DEFINED`、`POSITION(n)` による文字・ビット位置指定、`iSUB` 定義 (`DEFINED A(1SUB, 1SUB)` による対角要素・転置参照と配列組込み関数) | Ch.8 |

### 4.5 `PF`: ストリーム入出力・レコード入出力 (Stream & Record I/O)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `PF101A` | `GET` / `PUT STRING` (`LIST` / `EDIT` / `DATA`) | メモリ上文字列に対する `LIST` / `EDIT` / `DATA` 形式の往復変換、書式項目 (`A`, `B`, `F`, `E`, `P`, `X`, `R(fmt)`)、反復指定 `(X(I) DO I = 1 TO N)` | Ch.12, Ch.13 |
| `PF102A` | ストリームファイルと `PRINT` 制御 | `SYSPRINT` / 任意の `PRINT` ファイルのタブ位置 (25/49/73/97)、`LINESIZE` / `PAGESIZE`、`SKIP` / `PAGE` / `LINE` / `COLUMN`、`ON ENDPAGE` と `PAGENO` / `LINENO` | Ch.12 |
| `PF103A` | レコード I/O (順次・`LOCATE`・`REGIONAL(1)`) | `READ INTO` / `READ SET`、`WRITE FROM`、`LOCATE` モード出力バッファ、`REGIONAL(1)` の 0 始まり領域番号と `DELETE` 時の `'FF'X` ダミーレコード | Ch.11 Record-oriented I/O |
| `PF104M` | VSAM KSDS / RRDS と `ON KEY` / `ON ENDFILE` | KSDS の `KEY` / `KEYFROM` / `KEYTO`、`REWRITE`、`DELETE`、重複・未検出時の `ON KEY` (`ONCODE` と `ONKEY()`)、RRDS の相対レコード番号アクセス | Ch.11 |

### 4.6 `PM`: プリプロセッサー・組込み関数・言語間連携 (Preprocessor, BIFs & Interop)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `PM101A` | マクロプリプロセッサー (変数・制御・再走査) | `%DECLARE` (`FIXED`/`CHAR`/`BIT`)、`%IF`/`%DO`/`%SELECT`/`%GO TO`、`%ACTIVATE`/`%DEACTIVATE` (`RESCAN`/`NORESCAN`)、`%REPLACE`、`%INCLUDE`/`%XINCLUDE` | Ch.21 Preprocessor |
| `PM102A` | プリプロセッサー手続きと `STATEMENT` 形式 | `%PROC ... RETURNS(...)`、位置引数・キーワード引数を持つ `STATEMENT` 形式マクロ、プリプロセッサー組込み関数 (`SUBSTR`, `INDEX`, `LENGTH`, `PARMSET`) | Ch.21 |
| `PM103A` | 組込み関数群 (`BIFPREC(31)`) | 文字列 (`SEARCH`, `VERIFY`, `TALLY`, `TRANSLATE`, `TRIM`, `CENTER`)、配列 (`DIM`, `LBOUND`, `HBOUND`, `SUM`, `PROD`, `ALL`, `ANY`)、Lilian 日付 (`DAYS`, `DAYSTODATE`, `DATETIME`, `WEEKDAY`)、数学関数 | Ch.19 Built-in functions |
| `PM104M` | 言語間呼出し (PL/I ⇔ COBOL / HLASM) と `PARM` | 同一実行単位記憶域での PL/I ⇔ COBOL (`OPTIONS(COBOL)`) および PL/I ⇔ HLASM (`OPTIONS(ASM RETCODE)`) のポインター・構造体共有、`PARM` の `/` 分割、`PLIRETC` / `PLIRETV` | [0004 §2.1](0004-pli-compiler.md) |

### 4.7 `PE`: コンパイル診断・負系テスト (Diagnostics & Negative Suite)

- `PE101N`〜`PE115N`: 不正な構造体レベル・写像不能定義、`GENERIC` の曖昧/不一致記述子、`REFER` の不正参照、未定義条件・不正な `%GO TO` 先、型の自動変換が存在しない代入 (`HANDLE` と算術型など) がコンパイルエラー (`expect = "compile-error"`) になることを検証する。

---

## 5. ディレクトリ構成と自己検証コードの形式

### 5.1 リポジトリー構成 (`zpvs/`)

```text
zpvs/
├─ README.md
├─ manifest.toml             # スイート定義 (toolchain = zpli / zpli-native / ironspring)
├─ copy/
│  ├─ ZPVSRPT.PLI            # %INCLUDE 用のレポートファイル宣言・PASS/FAIL/集計手続き
│  └─ ...
├─ PD/
│  ├─ PD101A.PLI
│  └─ ...
├─ PA/
├─ PC/
│  ├─ lib/
│  │  └─ PCS01A.PLI          # 外部手続き・FETCH 対象サブプログラム
│  └─ PC101A.PLI
├─ PS/
├─ PF/
├─ PM/
└─ PE/
   ├─ PE101N.PLI
   └─ ...
```

### 5.2 自己検証 PL/I プログラムの具体例 (`PD101A.PLI`)

```pli
 /* ZPVS MODULE PD: DATA REPRESENTATION AND STRUCTURE MAPPING */
 PD101A: PROC OPTIONS(MAIN);
   %INCLUDE ZPVSRPT;

   DCL PDEC FIXED DEC(7,2) INIT(-12345.67);
   DCL UBIT(4) BIT(3) UNALIGNED INIT('101'B, '010'B, '111'B, '001'B);

   /* SC27-8940 STRUCTURE MAPPING PAIR TEST */
   DCL 1 S1 ALIGNED,
         2 A CHAR(1),
         2 B FIXED BIN(31),
         2 C BIT(1) ALIGNED,
         2 D FIXED DEC(5);

   CALL ZPVS_INIT('PD101A', 'PD');

   /* TEST 1: FIXED DEC(7,2) PACKED DECIMAL BYTES */
   CALL ZPVS_ASSERT_CHAR('PD101A-001', 'SC27-8940 FIXED DEC(7,2) HEX',
                         '1234567D', HEX(PDEC));

   /* TEST 2: UNALIGNED BIT ARRAY PACKS INTO 12 BITS (2 BYTES: B5 C8) */
   CALL ZPVS_ASSERT_CHAR('PD101A-002', 'SC27-8940 UNALIGNED BIT(3) ARRAY',
                         'B5C8', HEX(STRING(UBIT)));

   /* TEST 3: STRUCTURE ALIGNMENT AND TOTAL SIZE */
   CALL ZPVS_ASSERT_BIN('PD101A-003', 'SC27-8940 ALIGNED STRUCTURE SIZE',
                        12, SIZE(S1));

   CALL ZPVS_FINISH;
 END PD101A;
```

`ZPVS_FINISH` は `REPORT` ファイルに以下を出力してクローズする。

```text
     3 OF      3  TESTS WERE EXECUTED SUCCESSFULLY
    NO   TEST(S) FAILED
    NO   TEST(S) DELETED
```

---

## 6. ハーネス統合と段階的実装計画

`crates/zeus-ccvs` の `Toolchain` に `Zpli` (`zpli compile` + `zpli run`) と `ZpliNative` (`zpli compile --native`)、および比較オラクル用の `IronSpring` (`plic` + リンカー) を追加する。これにより `zeus-ccvs run --suite zpvs --toolchain zpli` で実行・レポート生成が一元化される。

| フェーズ | 内容 | 目標規模 (目安) | 完了条件 |
|---|---|---|---|
| **P0: 共通インクルードとハーネス対応** | `zpvs/copy/ZPVSRPT.PLI` の作成、`zeus-ccvs` への `zpli` / `zpli-native` ツールチェーン追加 | 共通 `%INCLUDE` 1 本 + ハーネス拡張 | `zeus-ccvs run --suite zpvs --toolchain zpli` が動作しレポートが出力される |
| **P1: データ・構造体写像・式・精度 (`PD`, `PA`)** | 全データ型のバイト表現、構造体ペアリング写像、`RULES(IBM)` 精度表、全型変換、ピクチャ編集 | 10 プログラム / 200+ テスト項目 | `PD`, `PA` が FAIL 0。既存の単体テストと `floatpic`/`wide`/`complex` 等を統合 |
| **P2: 制御・手続き・ON 条件・記憶域 (`PC`, `PS`)** | スコープ・再帰・`ENTRY`・`FETCH`、`ON` 動的伝播と `ONCHAR` 修復、`CONTROLLED`・`AREA`/`OFFSET`・`REFER`・`iSUB` | 11 プログラム (+ 外部手続き) / 200+ テスト項目 | `PC`, `PS` が FAIL 0。既存 `conditions`/`storage`/`area`/`refer`/`ctl`/`entries`/`fetch` を包含 |
| **P3: 入出力・プリプロセッサー・BIF・負系 (`PF`, `PM`, `PE`)** | ストリーム/レコード I/O、VSAM KSDS/RRDS、`%PROC`/`STATEMENT` マクロ、BIF 全種、言語間呼出し、負系診断 (`PE`) | 20+ プログラム / 220+ テスト項目 | 全必須モジュール (`PD`, `PA`, `PC`, `PS`, `PF`, `PM`, `PE`) が CI で全件合格 |

## 実装状況 (2026-10-20)

`zpvs/` に全7モジュール、主プログラム68本（実行38本/318判定、負系15本、正常対照15本）を実装しました。

[実行方法・対応範囲](../../zpvs/README.md) / [全シナリオ台帳](../../zpvs/SCENARIOS.md) / [言語横断の検証](../validation-suites.md)。提案本文は設計範囲を示し、各小項目の完全網羅を意味しません。現時点の実測可能な範囲は台帳の判定IDで管理します。

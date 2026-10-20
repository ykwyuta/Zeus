# ZPVS — PL/I Validation Suite

2026-10-20時点で7モジュール、68本の主プログラムを収録します。実行38本・独立判定318件、コンパイル拒否15本、受理を確認する正常対照15本です。補助手続きは本数に含めません。

| モジュール | 対象 | 実行本数 / 判定数 | 負系 / 正常対照 |
|---|---|---:|---:|
| PD | 内部表現・符号付き/符号なし境界・文字/ビット・構造体・序数・複素数 | 7 / 133 | 0 / 0 |
| PA | 精度/スケール・算術・変換・PICTURE・丸め・BOOL | 4 / 31 | 0 / 0 |
| PC | 分岐・反復・手続き・FETCH/RELEASE・ON条件・ONCHAR/ONSOURCE修復 | 8 / 43 | 0 / 0 |
| PS | AUTOMATIC/STATIC/CONTROLLED/BASED・AREA・REFER・DEFINED・HANDLE | 6 / 35 | 0 / 0 |
| PF | ストリーム・PRINT改行/改ページ/再OPEN・固定/可変レコード・キー付き更新 | 5 / 31 | 0 / 0 |
| PM | マクロ前処理・組込み関数・日付・PARM/NOEXECOPS・COBOL/HLASM相互呼出し | 8 / 45 | 0 / 0 |
| PE | 不正宣言/型/精度/構文の診断と1規則だけ修復した正常対照 | 0 / 0 | 15 / 15 |

## 実行

```sh
cargo build -p zeus-ccvs -p zeus-cobol -p zeus-pli -p zeus-native
target/debug/zeus-ccvs run --suite zpvs --toolchain zpli --work work/zpli --out work/zpli.json
target/debug/zeus-ccvs run --suite zpvs --toolchain zpli-native --work work/zpli-native --out work/zpli-native.json
zeus-ccvs report --results work/zpli.json --programs
cargo test -p zeus-ccvs --test language_suites
```

Windowsでは実行ファイルに `.exe` を付けてください。`--programs` に主プログラム名をカンマ区切りで指定すると絞り込めます。各実行の作業ディレクトリーを分けてください。

## 判定と網羅性の維持

[SCENARIOS.md](SCENARIOS.md) は全プログラム・全判定IDを列挙します。[coverage.json](coverage.json) はレビュー対象の固定インベントリーで、ソース/実行レポートの順序・件数と照合します。ソースから期待件数を実行時に再生成しません。期待値は仕様に基づく値/バイト列で、実行結果の採録ではありません。

負系は通常の診断終了と対象メッセージを要求し、パニック、タイムアウト、単なる起動失敗を合格にしません。各 `N` の修復済み `P` を受理させ、処理系の一律拒否を検出します。実行レポートは120バイト固定長EBCDIC、各判定PASS/FAILと末尾集計です。統合テストは全ID、集計、FAIL/DELETED/INSPECTが0であることを照合します。

## z/OSへの持ち出し

```sh
zeus-ccvs export-jcl --suite zpvs --out work/zpvs-zos --hlq USER.VALIDATE
```

`COPY/`, `SRCLIB/`, `JCL/` を生成します。コンパイラーは IBMZPLI、リンカーはIEWLです。追加DDと混在言語の補助手続きも出力します。コンパイラー/LEライブラリー名、PDS属性、DDのDCB/VSAM定義、必要な処理系オプションは設置先に合わせて設定してください。診断文言の照合はZeus側の契約で、実機のメッセージ番号と一致する意味ではありません。

## Hercules（MVS 3.8j / OS/360 PL/I (F)）での検証結果

**Hercules 3.13** 上の **IBM OS/VS2 MVS R3.8**（`PGM=IEMAA` — OS/360 PL/I (F) Compiler V5.5 + `PGM=IEWL` + `SYS1.PL1LIB`）で実行・コンパイル検証を行い、Enterprise PL/I 固有の項目とあわせて全68本（実行38本・318判定 + 診断30本）の期待値を外部オラクルおよび仕様（*Enterprise PL/I for z/OS V5.3 Language Reference* SC27-8940-02）と照合済みです。

| モジュール | 対象プログラム | 実行本数 / 判定数 | Hercules（MVS 3.8j `IEMAA`）検証結果 |
|---|---|---:|---|
| PA | `PA101A`–`PA104A` | 4 / 31 | **4 / 4 完走（31 / 31 PASS — 100%）** — 固定/2進算術・変換・`BOOL`・`PICTURE`・配列断面・`BY NAME`・疑似変数がすべて一致 |
| PD | `PD101A`–`PD107A` | 7 / 133 | `PD101A`（`FIXED DEC(1..15)` 全40件）、`PD102A`（8/9件）、`PD103A`（4/4件）、`PD104A`（`LIKE` 5/5件）、`PD105A`（`DEFAULT` 1件）、`PD106A`（複素数・HFP 6/6件）、`PD107A`（`FIXED BIN(15/31)` 10件）の **計74件が実機PASS**。残り59件は Enterprise PL/I 拡張（下記）で仕様と全件一致 |
| PC | `PC101A`–`PC108A` | 8 / 43 | `PC101A`（`DO` 3件）、`PC102A`（再帰・静的スコープ **11/11件**）、`PC103A`（`ENTRY`・`GENERIC` 4件）、`PC105A`（`CONDITION`/`SIGNAL`/`REVERT` **3/3件**）、`PC106A`（`ZERODIVIDE`/`ONCODE=320`/`SIZE` **3/3件**）、`PC107A`（`ONCHAR` 修復 2/3件）、`PC108A`（`ONSOURCE` 修復 2/3件）の **計28件が実機PASS** |
| PS | `PS101A`–`PS106A` | 6 / 35 | `PS101A`（`BASED`/`NULL`='FF000000'X 5件）、`PS102A`（`CONTROLLED` 世代スタック 6/8件）、`PS103A`（`AREA`/`OFFSET`/`EMPTY` 4件）、`PS104A`（`REFER` 割当て）、`PS105A`（`DEFINED POSITION`・`iSUB` 対角/転置 **5/5件**）の **計21件が実機PASS** |
| PF | `PF101A`–`PF105A` | 5 / 31 | `PF101A`（`STRING` 入出力 **4/4件**）、`PF102A`（`PRINT` `LINE`/`ENDPAGE`/`PAGE`/再OPEN 5/7件）、`PF103A`（固定長レコードI/O **6/6件**）、`PF105A`（可変長レコードI/O **6/6件**）の **計21件が実機PASS**。`PF104M`（VSAM KSDS 8件）は仕様と一致を確認 |
| PM | `PM101A`–`PM108A` | 8 / 45 | `PM101A`（マクロ反復・`%DEACTIVATE` **2/2件**）、`PM102A`（マクロ手続き **3/3件**）、`PM103A`（基本BIF 10件）、`PM104M`（他言語・アセンブラ連係 3件）、`PM105A`（数学BIF 6件）、`PM106A`（マクロ展開 2件）、`PM107A`（`OPTIONS(MAIN)` `PARM` 受渡し 2件）の **計28件が実機PASS** |
| PE | `PE101N/P`–`PE115N/P` | 0 / 0 | 1972年 PL/I (F) 文法範囲内の **12組（24本）で `N` 拒否（`RC>=8`）・`P` 受理（`RC<=4`）が一致**（残り3組 `PE108`/`PE109`/`PE110` は `LEAVE`/`ITERATE` 未実装および `MAIN` での `RETURN(expr)` 扱い差） |

### 1972年 OS/360 PL/I (F) V5.5 と Enterprise PL/I 仕様の主な世代差

* **Enterprise PL/I 拡張型・構文**:
  `PD101A` の `FIXED DEC(31)`（PL/I (F) は最大精度15）、`PD107A` の `FIXED BIN(7/8/16/32/63)` と `UNSIGNED`（PL/I (F) は符号付き `15`/`31` のみ）、`PD104A`/`PD105A`/`PS106A` の `DEFINE STRUCTURE`/`ALIAS`/`ORDINAL`・`HANDLE`/`NEW`、`PD106A` の `WIDECHAR`/`GRAPHIC`、`PC101A` の `DO UNTIL`/`LEAVE`/`ITERATE`/`SELECT`、`PC103A` の `ENTRY VARIABLE`、`PC104A` の動的 `FETCH`/`RELEASE`、`PS101A` の `SYSNULL`/`POINTERADD`、`PM103A`/`PM105A` の文字列・Lilian日付組込み関数（`REVERSE`/`SEARCH`/`TALLY`/`CENTER`/`LEFT`/`RIGHT`/`LOWERCASE`/`UPPERCASE`/`REM`/`DAYS`/`DAYSTODATE`）、`PM106A` の `STATEMENT` マクロ、`PM108A` の `OPTIONS(MAIN NOEXECOPS)` は Optimizing PL/I 以降〜Enterprise PL/I で導入された機能であり、SC27-8940-02 の仕様どおりであることを確認しています。
* **`PD102A` の `VAR-PREFIX`（`UNSPEC(V)`）**:
  OS/360 PL/I (F) では `CHAR(n) VARYING` に対する `UNSPEC` が文字列本体のみを返しましたが、Enterprise PL/I（SC27-8940）では2バイト長接頭辞（`'0003'X`）を含むストレージ表現（`'0003C1C2C3'X`）を返します。
* **`PC107A` / `PC108A` の `CONVERSION-INPUT-KEPT` / `ONSOURCE-INPUT-KEPT`**:
  1972年 PL/I (F) は `ONCHAR` / `ONSOURCE` 疑似変数への代入で変換元変数 `S` 自体を直接書き換えましたが、ANSI / Enterprise PL/I では変換用内部バッファのみを書き換え、元の変数 `S`（`'12X4'` / `'BAD'`）を保持します。
* **`PS102A` の `CTL-SECOND` / `CTL-ARG-PUSH`（`ALLOCATION(X)`）**:
  PL/I (F) の `ALLOCATION(X)` は割当て有無（`0` または `1`）のみを返しましたが、Optimizing / Enterprise PL/I では `CONTROLLED` 変数の世代スタック深さ（2世代積んだ状態で `2`）を返します（`FREE` 後の世代復元 `CTL-RESTORE` / `CTL-ARG-RESTORE` は PL/I (F) でも PASS）。
* **`PF101A` の `GET-STRING-LIST` と `PF102A` の `PRINT-FIRST-SKIP`**:
  PL/I (F) の `GET LIST` は入力ストリーム上の文字列トークンに引用符（`'ALPHA'`）を要求しましたが、Enterprise PL/I では非引用符トークンも `CHAR` 変数へ読み込めます。また `OPEN FILE(F) PRINT` 直後の最初の `PUT SKIP` は、PL/I (F) では2行目（`LINENO=2`）に進みましたが、Enterprise PL/I では最初の見出し行（`LINENO=1`）に配置されます。

### Docker / `zeus-hercdiff` による再実行手順

MVS 3.8j の DASD イメージ（`mvsres.140.cckd` 等を含む `Dasd` ディレクトリ）を指定することで、Docker コンテナ内で CCKD シャドウファイル（元の DASD は不変）を用いた MVS 3.8j (`PGM=IEMAA`) バッチ検証を1コマンドで再現できます。

```sh
# Docker コンテナ内で MVS 3.8j を自動 IPL し、ZPVS の PL/I (F) 検証を実行
tools/hercules-suites.sh zpvs --mvs-dasd /path/to/MVSR38/Dasd

# 特定のプログラムのみを実行する場合
tools/hercules-suites.sh zpvs --mvs-dasd /path/to/MVSR38/Dasd --programs PA101A,PA102A,PA103A,PA104A

# ZHVS（ベアメタル z/Arch）と ZPVS（MVS 3.8j PL/I (F)）を続けて一括実行する場合
tools/hercules-suites.sh all --mvs-dasd /path/to/MVSR38/Dasd

# ホスト環境に hercules がインストール済みの場合（Docker なし）
ZEUS_HERCULES_LOCAL=1 tools/hercules-suites.sh zpvs --mvs-dasd /path/to/MVSR38/Dasd
# または直接:
cargo run --release -p zeus-hercdiff -- zpvs --mvs-dasd /path/to/MVSR38/Dasd --out work/zpvs-hercules.json
```

実行結果は `work/zpvs-hercules.json` にプログラム別の PASS/FAIL 集計および `REPORT` DD の出力行として保存されます。

## 範囲

[提案](../docs/proposal/0020-zos-pli-test-suite.md) の7分野を実行可能なスイートにしました。既存の詳細なRust回帰試験と併用します。これはIBM言語/全命令/全OSサービスの完全網羅や実機認証の主張ではありません。Hercules 3.13（MVS 3.8j / OS/360 PL/I (F) V5.5）で検証可能な203判定および診断12組については実機エミュレーションで検証済みですが、IBM z/OS 実機（Enterprise PL/I）および他社処理系での差分実行は未実施です。全数の組合せを検査するのは明記した境界/変換マトリクスであり、例外条件の全組合せ、全丸めモード、全命令形式を総当たりするものではありません。

クリーンルームで作成した独自ソースです。ライセンスはリポジトリーの GPL-3.0-or-later に従います。

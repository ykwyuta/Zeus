# Proposal 0019: CCVS85 補完 z/OS COBOL 互換性テストスイート (ZCVS)

- 状態: 提案 (Draft)
- 作成日: 2026-10-16
- シナリオ再監査: [実装との対応・不足シナリオ・完了判定](../../zcvs/SCENARIOS.md)
  （プログラム番号は計画時のもの。実装との対応は再監査表を参照）
- 関連:
  - [0003 Rust 製 COBOL コンパイラ (zeus-cobol)](0003-cobol-compiler.md)
  - [0014 IBM Enterprise COBOL 構文適合性の監査](0014-cobol-ibm-conformance.md)
  - [0015 COBOL の句・設定の読み飛ばし監査](0015-cobol-ignored-clauses.md)
  - [0016 RESERVE・物理ブロック・仮想テープ・再帰 CALL](0016-cobol-physical-tape-recursion.md)
  - [0017 COBOL の既知の非互換に対する追加実装](0017-cobol-remaining-compatibility.md)
  - [0018 COBOL 追加互換機能の実装状況](0018-cobol-compatibility-completion.md)

## 実装の現在地（2026-10-20）

実装は158本体プログラムと16補助プログラム。内訳は実行93本・1,431検査、
負系45本、正常コンパイル対照20本である。今回の追加は53本体・7補助で、
実行時766検査、負系12本、正常対照12本を増やした。

PACKED/ZONEDの独立バイト列、TRUNC別名と幅境界、複数算術受信先、
国別分類と4 CCSID、呼出し寿命/再帰/ENTRY、RRDS、部分START、複数AIX、
QSAM状態遷移・EXTEND、4巻テープと物理ラベルを検査する。
新しいSYNC表検査は要素末尾の余白不足を検出し、配置計算も修正した。

件数は実装済みシナリオの規模であり、以下の全要求や実機互換性の達成率ではない。
最新の証拠と残件は[再監査表](../../zcvs/SCENARIOS.md)を正とする。

## 1. 背景と目的

NIST COBOL-85 テストスイート (CCVS85) は、ANSI X3.23-1985 / ISO 1989:1985 規格の核となる制御構造・データ部・ファイル I/O・整列併合・組込み関数を網羅する優れた規格適合性スイートであり、Zeus でも [0003 §5.2](0003-cobol-compiler.md#52-ccvs85-の合格基準) に基づき必須 8 モジュール (`NC`, `SM`, `IC`, `SQ`, `RL`, `IX`, `ST`, `IF`) および任意 4 モジュール (`SG`, `OB`, `DB`, `RW`) の全件合格を達成している。

しかし、CCVS85 は**処理系に依存しないポータブルな COBOL-85 規格の検証**を目的として設計されており、z/OS 上の業務アプリケーションが依存する次の領域を原理的にカバーできない。

1. **COBOL-85 規格が「処理系定義 (implementor-defined)」とした z/OS 固有の内部表現・演算意味**
   - EBCDIC バイト配置、ゾーン 10 進・パック 10 進の符号ニブル (`NUMPROC(NOPFD/PFD/MIG)`、`NUMCLS(PRIM/ALT)`)
   - 2 進項目の格納幅と切り捨て (`COMP`/`BINARY`/`COMP-4` と `COMP-5`、`TRUNC(STD/OPT/BIN)`)
   - 10 進演算の中間結果精度 (`ARITH(COMPAT)` の 18/30 桁と `ARITH(EXTEND)` の 31 桁)
   - IBM 16 進浮動小数点 (`COMP-1`/`COMP-2` の HFP 表現) と外部浮動小数点
   - `SYNCHRONIZED` (`SYNC`) によるスラックバイト挿入位置、`LENGTH OF`、`REDEFINES` 越しのバイト観測
2. **IBM Enterprise COBOL 固有の言語拡張 (4.2 基準 + 5.x/6.x 拡張)**
   - `LOCAL-STORAGE SECTION`、`PROGRAM-ID ... RECURSIVE`、`PROCEDURE DIVISION RETURNING` / `CALL ... RETURNING`、`ENTRY` 文による別名入口
   - `POINTER`、`PROCEDURE-POINTER`、`FUNCTION-POINTER`、`ADDRESS OF`、`SET ... TO ENTRY`
   - `PIC G` (`DISPLAY-1`)、`PIC N` (`NATIONAL`)、`GROUP-USAGE NATIONAL`、`NSYMBOL(NATIONAL/DBCS)`、シフトコード (`0x0E`/`0x0F`)、日本語混在 CCSID (930/939/1390/1399)
   - `CBL` / `PROCESS` コンパイラーオプション、5.x/6.x 拡張 (`EXIT PERFORM CYCLE`、`PERFORM UNTIL EXIT`、`SET ... TO FALSE`、`INITIALIZE` 拡張、部分語 `REPLACE` 等)
3. **z/OS のデータセット・物理ブロック・VSAM・仮想テープ・システム連携**
   - QSAM 物理ブロック (`RECORDING MODE F/V/S/U`、`BLOCK CONTAINS`、`APPLY WRITE-ONLY`、`RESERVE`、`CODE-SET`)
   - VSAM (KSDS/ESDS/RRDS/AIX) の第 2 `FILE STATUS` (6 バイトの return/component/reason コード) と属性不一致 (`FILE STATUS 39`)
   - 仮想テープ (AWSTAPE の標準ラベル `VOL1`/`HDR1`/`HDR2`/`EOF1`/`EOF2`、`NO REWIND`、`REVERSED`、複数ファイル/複数巻)
   - JCL `PARM=`、Special Registers (`RETURN-CODE`, `SORT-RETURN`, `WHEN-COMPILED`)、LE 呼び出し可能サービス (`CEE*`)
4. **不正構文・禁止組み合わせに対するコンパイル診断 (負系テスト)**
   - [0014](0014-cobol-ibm-conformance.md)〜[0018](0018-cobol-compatibility-completion.md) で実装した回帰テスト群 (`crates/zeus-cobol/tests/*.rs`) は Rust のテストコード内に文字列として埋め込まれ、Zeus 内部 API を直接呼んでいるため、**単体の COBOL プログラム集として実機 z/OS (`IGYCRCTL`) や他処理系に持ち込んで比較実行することができない**。

そこで本提案では、CCVS85 を直接補完する **z/OS 版 COBOL 互換性テストスイート「ZCVS (z/OS COBOL Validation Suite)」** をリポジトリー内の独立した COBOL コード集 (`zcvs/`) として構築し、既存の `zeus-ccvs` ハーネスで CCVS85 と同じ形式で実行・集計・比較できるようにする。

---

## 2. 設計原則

### 2.1 CCVS85 互換の自己検証形式 (Self-Checking COBOL)

すべての実行テストプログラムは、CCVS85 と同じく環境変数 `DD_REPORT` (JCL では `//REPORT DD SYSOUT=*` またはデータセット) に 120 バイト固定長の検査レポートを出力し、末尾に CCVS85 互換の集計行を出力する。

```text
ZCVS REPORT - PROGRAM ZD101A - MODULE ZD (ZOS DATA AND STORAGE LAYOUT)
------------------------------------------------------------------------------------------------------------------------
PASS  ZD101A-T001  R42-P246       COMP-5 2-BYTE SIGNED BOUNDARY (-32768)
PASS  ZD101A-T002  R42-P246       TRUNC(STD) PIC 9(4) BINARY OVERFLOW MODULO
FAIL  ZD101A-T003  R42-P236       SYNC SLACK BYTES IN NESTED GROUP
      EXPECTED: 00000014
      ACTUAL:   00000012
------------------------------------------------------------------------------------------------------------------------
     2 OF      3  TESTS WERE EXECUTED SUCCESSFULLY
     1   TEST(S) FAILED
    NO   TEST(S) DELETED
```

末尾の集計行は `crates/zeus-ccvs/src/report.rs` の既存パーサー (`evaluate(Style::Normal, ...)`) がそのまま解釈できる形式とする。これにより、`zeus-ccvs report` のモジュール集計表・ベースライン比較がそのまま利用できる。

### 2.2 実機 z/OS・Zeus・GnuCOBOL で無修正実行できるポータビリティ

- **純粋な固定形式 COBOL ソース**: 全プログラムを固定形式 (列 1–6 一連番号、列 7 標識、列 8–72 A/B 領域、列 73–80 識別領域) で記述する。
- **PDS メンバー名互換の 8 文字以内の命名**: プログラム名・コピーブック名・サブプログラム名はすべて英大文字・数字 8 文字以内 (`ZD101A`, `ZCVSRPT` 等) とし、z/OS の区分データセット (PDS/PDSE) へそのまま格納・コンパイル (`PGM=IGYCRCTL`) できるようにする。
- **バイト列検証の COBOL 内完結**: 記憶域のバイト配置 (`COMP-3` の符号ニブル、`COMP-1` の HFP ビット列、`SYNC` の詰めバイト、`NATIONAL` の UTF-16BE バイト列など) は、Rust 側のメモリ覗き見に頼らず、COBOL 内の `REDEFINES` と共通の 16 進展開ルーチン (`ZCVSHEX` コピーブック) によって 16 進文字列へ変換し、COBOL プログラム自身が期待値と比較して PASS/FAIL を判定する。
- **ソース先頭 `CBL` による自己記述**: `TRUNC(BIN)`、`ARITH(EXTEND)`、`NUMPROC(PFD)`、`NSYMBOL(DBCS)` などコンパイラーオプションに依存するテストは、コマンドライン引数ではなくソース先頭の `CBL` 文で指定する。これにより z/OS の `IGYCRCTL` でも `zcobc` でも同じオプションでコンパイルされる。
- **導入設定・版依存オプションの区別**: `NUMCLS` は実機 z/OS では導入設定である。Zeus の `CBL NUMCLS` で設定を選択するケースはその差を台帳に明記し、実機では該当指定を除いて同じ導入設定で検証する。`NUMPROC(MIG)` のケースは 4.2 対象とし、6.x へ無修正で持ち込めるとは扱わない。

### 2.3 クリーンルーム原則とライセンス

- [CONTRIBUTING.md](../../CONTRIBUTING.md) のクリーンルーム原則に従い、IBM のサンプルコードやマニュアル内の著作権保護されたプログラム例は一切転載しない。
- 公開仕様書 (Enterprise COBOL 4.2 / 6.4 Language Reference、Programming Guide、z/Architecture Principles of Operation) の規則から独自にテストケースと期待値を設計し、Zeus 本体と同じ `GPL-3.0-or-later` ライセンスで `zcvs/` 以下にコミットする。
- 外部ダウンロードが必要な CCVS85 (`newcob.val`) と異なり、`zcvs/` はリポジトリーに同梱されるためネットワーク接続なしで CI・ローカルで常時実行できる。

---

## 3. CCVS85 との役割分担とモジュール体系

CCVS85 の 2 文字モジュール名 (`NC`, `SM`, `IC`, `SQ`, `RL`, `IX`, `ST`, `IF`, `SG`, `OB`, `DB`, `RW`, `CM`) と衝突しないよう、ZCVS のモジュールは `Z` で始まる 2 文字コードとする。

| モジュール | 名称 | 必須/任意 | 補完する CCVS85 モジュール | 検証対象 (z/OS 固有領域) |
|---|---|---|---|---|
| **`ZD`** | z/OS Data & Layout | **必須** | `NC` (データ部) | EBCDIC 表現・照合順序、`BINARY`/`COMP`/`COMP-3`/`COMP-4`/`COMP-5` のバイト配置、`SYNC` アラインメントとスラックバイト、`OCCURS DEPENDING ON` (可変位置項目・複合 ODO)、`REDEFINES`/`RENAMES`、符号分離・オーバーパンチ、編集 PIC |
| **`ZA`** | z/OS Arithmetic & Float | **必須** | `NC`, `IF` (算術・関数) | `TRUNC(STD/OPT/BIN)`、`ARITH(COMPAT/EXTEND)` の 18/30/31 桁中間結果規則、`NUMPROC(NOPFD/PFD/MIG)` と `NUMCLS(PRIM/ALT)` の符号正規化・`NUMERIC` 判定、`COMP-1`/`COMP-2`/128-bit HFP 浮動小数点、外部浮動小数点 PIC、指数リテラル、多倍長数学・統計・金融関数 |
| **`ZN`** | National, DBCS & CCSID | **必須** | (CCVS85 に該当なし) | `PIC G` (`DISPLAY-1`)、`PIC N` (`NATIONAL` UTF-16BE)、`NSYMBOL(NATIONAL/DBCS)`、`GROUP-USAGE NATIONAL`、NATIONAL 固定小数点・数値編集・national-edited、SO/SI (`0x0E`/`0x0F`)、`NATIONAL-OF`/`DISPLAY-OF`、日本語混在 CCSID (930/939/1390/1399) |
| **`ZP`** | Program, Recursion & Pointers | **必須** | `IC` (プログラム間連絡) | `LOCAL-STORAGE` (呼出しごとの初期化・非再帰/再帰)、`PROGRAM-ID ... RECURSIVE`、`CALL ... BY REFERENCE/CONTENT/VALUE`、`OMITTED`、`RETURNING`、`ENTRY` 文 (別名入口・個別引数)、`POINTER`/`PROCEDURE-POINTER`/`FUNCTION-POINTER`、`ADDRESS OF`、入れ子プログラムの `COMMON`/非 `COMMON` 可視性・`GLOBAL`/`EXTERNAL` |
| **`ZF`** | z/OS Files, VSAM & Tape | **必須** | `SQ`, `RL`, `IX` (ファイル) | QSAM 物理ブロック (`RECORDING MODE F/V/S/U`、`BLOCK CONTAINS`、`APPLY WRITE-ONLY`、`RESERVE`、`CODE-SET`)、VSAM KSDS/ESDS/RRDS/AIX と第 2 `FILE STATUS` (6 バイトフィードバック)、属性照合 (`FILE STATUS 39`)、仮想テープ AWSTAPE (標準ラベル `VOL1`/`HDR`/`EOF`、`NO REWIND`、`REVERSED`、複数ファイル/EOV、ユーザーラベル) |
| **`ZS`** | Directives & z/OS Environment | **必須** | `SM`, `IF`, `ST` | `COPY`/`REPLACE` のプログラム境界スコープ、`CBL`/`PROCESS`、リスト制御 (`EJECT`/`SKIP1..3`/`TITLE`)、`WHEN-COMPILED` (特殊レジスター 16 文字と関数 21 文字)、世紀窓関数 (`DATE-TO-YYYYMMDD` 等)、`RETURN-CODE`/`SORT-RETURN`/`TALLY`、JCL `PARM=` 受領、LE サービス (`CEE3ABD`/`CEEDAYS`) |
| **`ZX`** | COBOL 5.x/6.x Extensions | **任意** | `NC`, `SM`, `IF` | Enterprise COBOL 5.x/6.x で追加され Zeus が実装する拡張: `EXIT PERFORM [CYCLE]`、`PERFORM UNTIL EXIT`、`WHEN SET TO FALSE` / `SET ... TO FALSE`、`INITIALIZE` (`WITH FILLER`/`TO VALUE`/`TO DEFAULT`)、部分語 `COPY REPLACING`/`REPLACE` (`LEADING`/`TRAILING`)、`TRIM`/`BYTE-LENGTH`/`TEST-NUMVAL` |
| **`ZE`** | Diagnostics & Negative Suite | **必須** | (CCVS85 に該当なし) | IBM 仕様がコンパイルエラーと定める不正構文・禁止組み合わせ (再帰プログラムでの `ALTER`/`INITIAL`/入れ子、`EXTERNAL` な `LOCAL-STORAGE`、不正な `END PROGRAM`、不正な `SAME AREA`、未対応・矛盾する `CBL` 指定など) の拒否確認 |

---

## 4. 各モジュールの詳細テスト計画

### 4.1 `ZD`: データ表現・記憶域配置 (z/OS Data & Storage Layout)

CCVS85 の `NC` モジュールは表示上の値 (`DISPLAY` や文字比較) のみを検査し、内部バイト表現を問わない。`ZD` は z/OS の記憶域バイト列そのものを検証する。

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZD101A` | EBCDIC 表意定数・文字照合 | `SPACE` (`X'40'`), `ZERO` (`X'F0'`), `QUOTE` (`X'7F'`/`X'7D'`), `HIGH-VALUE` (`X'FF'`), `LOW-VALUE` (`X'00'`)、英小文字 < 英大文字 < 数字の EBCDIC 照合順序と `PROGRAM COLLATING SEQUENCE` | R42 Ch.1–4, Ch.12 |
| `ZD102A` | 2 進項目 (`BINARY`/`COMP`/`COMP-4`/`COMP-5`) | 1〜4 桁 (2B)、5〜9 桁 (4B)、10〜18 桁 (8B) のビッグエンディアン 2 の補数表現、符号付き/符号なし境界値 | R42 pp.246–249 |
| `ZD103A` | パック 10 進 (`COMP-3`) とゾーン 10 進 (`DISPLAY`) | 偶数桁・奇数桁の `COMP-3` パディング、正数 (`C`)・負数 (`D`)・符号なし (`F`) ニブル、`SIGN LEADING/TRAILING [SEPARATE]`、オーバーパンチ文字 (`{`, `}`, `A`–`R`) | R42 pp.229–235, 247 |
| `ZD104A` | `SYNCHRONIZED` (`SYNC`) と `LENGTH OF` | `COMP`/`COMP-1`/`COMP-2`/`COMP-5`/`POINTER`/`INDEX` の 2/4/8 バイト境界整列、スラックバイト挿入位置、`OCCURS` 要素間の整列パディング、`SYNC LEFT/RIGHT` | R42 pp.236–242 |
| `ZD105A` | `OCCURS DEPENDING ON` (ODO) と可変長グループ | ODO 対象が表外・表内にある配置、ODO の後ろに続く可変位置項目 (variably located items)、送信側・受信側 ODO のグループ `MOVE` 時の長さ評価順序 | R42 pp.197–202, App.B |
| `ZD106A` | `REDEFINES`・`RENAMES` (66)・`88` レベル | 多段 `REDEFINES`、長さが異なる `REDEFINES`、グループをまたぐ `RENAMES THRU`、16 進リテラル・範囲指定を含む `88` レベル条件名 | R42 pp.222–228, 250–255 |
| `ZD107A` | 編集 PICTURE・`BLANK WHEN ZERO`・`JUSTIFIED` | 通貨記号、`+`/`-`/`CR`/`DB`、`Z`/`*`、浮動挿入、ゼロ抑制、逆編集 (`MOVE` edited to numeric)、`JUSTIFIED RIGHT` の切り詰めと空白詰め | R42 pp.206–221 |

### 4.2 `ZA`: 算術精度・切り捨て・浮動小数点 (z/OS Arithmetic & Float)

[0017](0017-cobol-remaining-compatibility.md) および [0018](0018-cobol-compatibility-completion.md) で扱った `TRUNC`、`ARITH`、`NUMPROC`、`NUMCLS`、HFP 浮動小数点を体系的に検証する。

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZA101A` | `TRUNC(STD)` (既定) | `CBL TRUNC(STD)` 下での `BINARY`/`COMP` への `VALUE`・`MOVE`・算術結果の PICTURE 桁数切り捨て、`ON SIZE ERROR` 発火境界、`DISPLAY` 桁数 | R42 p.248, PG TRUNC |
| `ZA102A` | `TRUNC(BIN)` と `COMP-5` | `CBL TRUNC(BIN)` 下での 2/4/8 バイト全幅保持 (-32768〜32767 等)、`ON SIZE ERROR` の 2 進幅判定、5/10/20 桁の `DISPLAY` 出力、`TRUNC(STD)` 下の `COMP-5` との一致 | R42 p.248, [0017 R03](0017-cobol-remaining-compatibility.md) |
| `ZA103A` | `TRUNC(OPT)` | PICTURE 範囲内の有効値に対する演算・移送の整合確認 | PG TRUNC |
| `ZA104A` | `ARITH(COMPAT)` 中間結果規則 | `CBL ARITH(COMPAT)` 下での最大 18 桁数値項目、30 桁中間結果、`COMPUTE` での乗除算の小数桁保持・切り捨て、`ROUNDED` と `ON SIZE ERROR` | R42, PG Intermediate Results |
| `ZA105A` | `ARITH(EXTEND)` 31 桁演算 | `CBL ARITH(EXTEND)` 下での 19〜31 桁の `DISPLAY`/`COMP-3`、31 桁中間結果、境界値演算。`BINARY`/`COMP-5` の PICTURE は 1〜18 桁・格納幅は 2/4/8 バイトとして別に検査する | R42, PG ARITH(EXTEND) |
| `ZA106A` | `NUMPROC(NOPFD / PFD / MIG)` と `NUMCLS` | 非優先符号 (`A`–`F`) の入力データに対する `IF NUMERIC`、`IF POSITIVE/NEGATIVE/ZERO` (`-0` を含む)、算術・`MOVE` 後の符号修復有無 | PG NUMPROC / NUMCLS |
| `ZA107A` | 内部浮動小数点 (`COMP-1` / `COMP-2`) | IBM HFP 形式 (基数 16 指数・仮数) のバイト表現 (`REDEFINES` で検証)、指数リテラル (`1.5E+02`, `-3.125E-01`)、10 進⇔HFP 変換、`ARITH(EXTEND)` 時の高2倍精度 | R42 pp.37, 247 |
| `ZA108A` | 外部浮動小数点と数学・統計組込み関数 | `PIC +9(2).9(4)E+99` (`DISPLAY` および `NATIONAL`)、`SQRT`/`LOG`/`LOG10`/`SIN`/`COS`/`TAN`/`ASIN`/`ACOS`/`ATAN`/`ANNUITY`/`PRESENT-VALUE`/べき乗 (`**`) の精度検証 | R42 pp.214, Ch.22 |

### 4.3 `ZN`: 国別データ・DBCS・コードページ (National, DBCS & CCSID)

CCVS85 には DBCS や UTF-16 (`USAGE NATIONAL`) のテストが一切存在しない。`ZN` は z/OS の日本語・国際化対応を検証する。

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZN101A` | `PIC G` (`DISPLAY-1`) と DBCS リテラル | `G"..."` リテラル、DBCS 空白 (`X'4040'`)、`MOVE`、`INSPECT`、比較、シフトコード検査 | R42 pp.38–40, 247 |
| `ZN102A` | `PIC N` (`USAGE NATIONAL`) と `NSYMBOL` | `CBL NSYMBOL(NATIONAL)` (既定) での UTF-16BE 格納 (`X'0020'` 空白、`NX"..."`) と `CBL NSYMBOL(DBCS)` (`ZN102B`) での DBCS 切り替え | R42, [0015 I01](0015-cobol-ignored-clauses.md) |
| `ZN103A` | 国別数値 (`NATIONAL` 固定小数点) と編集項目 | `PIC S9(5)V99 USAGE NATIONAL`、`SIGN LEADING/TRAILING SEPARATE`、四則演算、`NUMERIC` 判定、数値編集 (`USAGE NATIONAL`)、national-edited (`PIC NN/NNB00`) | R42, [0017 R04](0017-cobol-remaining-compatibility.md) |
| `ZN104A` | `GROUP-USAGE NATIONAL` | 集団項目への `GROUP-USAGE NATIONAL` 指定による暗黙 `NATIONAL` 継承、集団 `MOVE`・比較・部分参照 (`(pos:len)`)・`INITIALIZE` | R42 p.203, [0017 R05](0017-cobol-remaining-compatibility.md) |
| `ZN105A` | `INITIALIZE` の文字カテゴリー分離 | `ALPHANUMERIC` / `NATIONAL` / `DBCS` / `EGCS` / `NATIONAL-EDITED` / `NUMERIC` を個別に `REPLACING` した際の相互非干渉と初期値 (`X'40'`, `X'0020'`, `X'4040'`) | R42 pp.368–370, [0014 C01/C02](0014-cobol-ibm-conformance.md) |
| `ZN106M` | `NATIONAL-OF` / `DISPLAY-OF` と日本語 CCSID | CCSID 037 および CCSID 930/939/1390/1399 での EBCDIC (SO/SI 混在) ⇔ UTF-16 変換、代替文字引数 | R42 pp.513, 527 |

### 4.4 `ZP`: プログラム構造・再帰・ポインター・入口 (Program, Recursion & Pointers)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZP101A` | `LOCAL-STORAGE` と `WORKING-STORAGE` | 通常サブプログラム (`lib/ZPS01A.CBL`) と入れ子プログラムにおける `LOCAL-STORAGE` の呼出しごとの `VALUE` 再初期化、`WORKING-STORAGE` の状態保持、`CANCEL` によるリセット | R42 pp.163–164, [0014 C08/C09](0014-cobol-ibm-conformance.md) |
| `ZP102A` | `PROGRAM-ID ... RECURSIVE` | 再帰 `CALL` における `WORKING-STORAGE` 共有と `LOCAL-STORAGE` / `LINKAGE` / `PERFORM` 復帰スタックのフレーム分離 | R42 pp.107–108, [0016](0016-cobol-physical-tape-recursion.md) |
| `ZP103A` | `CALL ... RETURNING` / `PROCEDURE DIVISION RETURNING` | `LINKAGE SECTION` 01/77 結果項目の返却、数値・文字・集団の転送、`RETURNING` 指定時の呼出し元 `RETURN-CODE` 不変、呼出し失敗 (`ON EXCEPTION`) 時の受信項目保持 | R42 p.266, [0017 R02](0017-cobol-remaining-compatibility.md) |
| `ZP104A` | `ENTRY` 文 (別名入口と個別引数) | サブプログラム内の複数 `ENTRY` 文、入口ごとに異なる `USING` (`BY REFERENCE`/`VALUE`) 引数リスト、入口位置からの実行開始 | R42 p.338, [0018](0018-cobol-compatibility-completion.md) |
| `ZP105A` | `POINTER` / `PROCEDURE-POINTER` / `FUNCTION-POINTER` | 4/8 バイト配置、`ADDRESS OF`、`SET ptr TO ADDRESS OF`、`SET proc-ptr TO ENTRY`、ポインター間 `SET`、`NULL` 比較、ポインター経由の間接 `CALL` (主入口および `ENTRY` 別名) | R42 pp.246, 447, [0017 R06](0017-cobol-remaining-compatibility.md) |
| `ZP106A` | 入れ子プログラムの可視性と `GLOBAL` / `EXTERNAL` | `COMMON` 属性の有無による兄弟プログラムからの `CALL` 可否、親の `GLOBAL` データ・条件名・ファイルの参照、複数プログラム間の `EXTERNAL` データ共有 | R42 pp.107, 200, [0018](0018-cobol-compatibility-completion.md) |

### 4.5 `ZF`: ファイル・物理ブロック・VSAM・仮想テープ (z/OS Files, VSAM & Tape)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZF101A` | QSAM 記録形式 `F` / `V` と `BLOCK CONTAINS` | `RECORDING MODE F` / `V`、`BLOCK CONTAINS n RECORDS/CHARACTERS`、`RECORD VARYING IN SIZE DEPENDING ON` の読み書きとレコード長復元 | R42 pp.183–191, [0016](0016-cobol-physical-tape-recursion.md) |
| `ZF102A` | QSAM スパンレコード (`S`) と `APPLY WRITE-ONLY` | `RECORDING MODE S` (SDW 分割・結合) でブロック境界をまたぐ長大レコードの WRITE/READ、`APPLY WRITE-ONLY` による可変長ブロック充填 | R42 pp.157, 190, [0016](0016-cobol-physical-tape-recursion.md) |
| `ZF103A` | `RESERVE` と `CODE-SET` | `RESERVE n AREAS`、`CODE-SET IS ASCII-ALPHABET` (`STANDARD-1`/`STANDARD-2`) による外部 ASCII ファイルと内部 EBCDIC の自動変換 | R42 pp.141, 191, [0015 I04/I17](0015-cobol-ignored-clauses.md) |
| `ZF104A` | VSAM KSDS / RRDS / ESDS と第 2 `FILE STATUS` | 主キー・代替キー (`WITH DUPLICATES`) アクセス、重複キー・未検出・終端時の第 2 `FILE STATUS` (6 バイト: return=8, component=0/2, reason=4/8/16)、VSAM `SAME AREA` | R42 pp.150–155, [0017 R07](0017-cobol-remaining-compatibility.md) |
| `ZF105A` | ファイル属性照合と `FILE STATUS 39` | 既存データセット/DD 属性と COBOL FD (`LRECL`, `RECFM`, キー長/位置, ラベル種別) の不一致時に `OPEN` が `FILE STATUS 39` を返すこと | R42 p.408, [0018](0018-cobol-compatibility-completion.md) |
| `ZF201M` | 仮想テープ (AWSTAPE) 標準ラベル・位置制御 | `.aws` / `UNIT=3480` 上での `VOL1`/`HDR1`/`HDR2`/`EOF1`/`EOF2` 生成・検証、`RECORDING MODE U`、複数論理ファイル、`OPEN ... WITH NO REWIND`、`OPEN REVERSED`、`CLOSE REEL/UNIT FOR REMOVAL` | R42 pp.187, 322, 407, [0016](0016-cobol-physical-tape-recursion.md) |
| `ZF202M` | テープ複数巻 (EOV)・ユーザーラベル・`RERUN` | 複数巻境界での EOV ラベルと次巻継続、`USE BEFORE/AFTER STANDARD LABEL` 手続きの実行、`RERUN` チェックポイント設定 | R42 pp.153, 187, 578, [0018](0018-cobol-compatibility-completion.md) |

### 4.6 `ZS`: コンパイラー指示・組込み拡張・システム連携 (Directives & z/OS Environment)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZS101A` | `COPY` / `REPLACE` スコープとリスト指示文 | 1 ファイル内の複数プログラムにおける `REPLACE` の独立コンパイル境界での終了と入れ子への継承、文途中の `EJECT` / `SKIP1` / `SKIP2` / `SKIP3` / `TITLE` | R42 pp.564–571, [0014 C06/C11](0014-cobol-ibm-conformance.md) |
| `ZS102A` | `WHEN-COMPILED` と世紀窓日付関数 | 特殊レジスター `WHEN-COMPILED` (16 バイト `MM/DD/YYhh.mm.ss`) と `FUNCTION WHEN-COMPILED` (21 バイト `YYYYMMDDhhmmsshh+0000`)、`YEAR-TO-YYYY` / `DATE-TO-YYYYMMDD` / `DAY-TO-YYYYDDD` | R42 pp.22, 548, [0014 C10](0014-cobol-ibm-conformance.md), [0017 R01](0017-cobol-remaining-compatibility.md) |
| `ZS103M` | JCL `PARM=`・`RETURN-CODE`・LE サービス | 主プログラム `LINKAGE SECTION` での 2 バイト長 + `PARM` 文字列受領、`CEEDAYS` 等の LE 呼び出し可能サービス、`RETURN-CODE` 設定の終了コード反映 | PG Coding Linkage, [0003](0003-cobol-compiler.md) |

### 4.7 `ZX`: Enterprise COBOL 5.x/6.x 拡張モジュール (任意)

4.2 基準とは切り離し、5.x/6.x 互換機能 ([0014 X01〜X05](0014-cobol-ibm-conformance.md)) を独立モジュールとして検証する。

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `ZX101A` | `EXIT PERFORM [CYCLE]` と `PERFORM UNTIL EXIT` | 多重 inline ループからの最内側脱出、`VARYING ... AFTER` および `TEST AFTER` での `CYCLE` 増分順序、inline / out-of-line `PERFORM UNTIL EXIT` | R64 pp.343–345, 417 |
| `ZX102A` | `WHEN SET TO FALSE` と `SET ... TO FALSE` | 条件名の `FALSE` 値設定 (省略形・添字付き条件名・`SYMBOLIC CHARACTERS`・`NATIONAL` 定数・`GLOBAL` 条件名) | R64 SET, [0014 X03](0014-cobol-ibm-conformance.md) |
| `ZX103A` | `INITIALIZE` 6.x 拡張 | `WITH FILLER`、`ALL TO VALUE`、`THEN REPLACING`、`THEN TO DEFAULT` の優先順位、`JUSTIFIED` 項目の初期配置復元、多次元テーブル全要素への適用 | R64 pp.350–352, [0014 X04](0014-cobol-ibm-conformance.md) |
| `ZX104A` | 部分語 `COPY REPLACING` / `REPLACE` と追加関数 | `LEADING` / `TRAILING` 擬似テキスト置換 (識別子・ハイフン・PICTURE トークン・空置換)、`TRIM` / `BYTE-LENGTH` / `TEST-NUMVAL` / `TEST-NUMVAL-C` / `ABS` | R64 pp.688–702, [0014 X05](0014-cobol-ibm-conformance.md) |

### 4.8 `ZE`: コンパイル診断・負系テスト (Diagnostics & Negative Suite)

CCVS85 には不正な COBOL プログラムをコンパイラーが正しく拒否するかを組織的に検査するモジュールがない。[0014](0014-cobol-ibm-conformance.md)〜[0018](0018-cobol-compatibility-completion.md) で「読み飛ばしを廃止し診断へ変更」した規則を 1 ファイル 1 規則の負系 COBOL ソースとして保持する。

| プログラム群 | 検証するコンパイルエラー (正常に拒否されること) |
|---|---|
| `ZE101N`〜`ZE105N` | `END PROGRAM` 名不一致・ピリオド欠落・入れ子を含む親の終端欠落、空の `88` `VALUE`、空の `REPLACE` 置換元 |
| `ZE106N`〜`ZE110N` | `LOCAL-STORAGE` 内の `EXTERNAL`、`RECURSIVE` と `INITIAL` / 入れ子 / `ALTER` / `GO TO` (宛先なし) / セグメント / `USE FOR DEBUGGING` の併用 |
| `ZE111N`〜`ZE115N` | `INITIALIZE` の重複カテゴリー (`DBCS` と `EGCS` 等)、`RENAMES` や条件名に対する `TO VALUE`、`WHEN SET TO FALSE` なしの `SET ... TO FALSE`、`FALSE` 値への変数指定 |
| `ZE116N`〜`ZE120N` | `GROUP-USAGE NATIONAL` 下の不正 `USAGE`/`JUSTIFIED`、`LINE SEQUENTIAL` への `RESERVE`、`SCREEN SECTION`、不正・未知の `CBL`/`PROCESS` オプション |
| `ZE121N`〜`ZE125N` | `PROCEDURE DIVISION RETURNING` と `ENTRY` の併用・`USING` 重複・受信型不一致、非 `COMMON` 兄弟プログラムへの `CALL`、部分語 `REPLACE` の非擬似テキスト指定 |

---

## 5. ディレクトリ構成と自己検証コードの形式

### 5.1 リポジトリー構成 (`zcvs/`)

`ccvs85/` で展開される `work/suite` と同じディレクトリ構造を採ることで、`zeus-ccvs` の実行エンジンをそのまま共用する。

```text
zcvs/
├─ README.md                 # スイートの構成・実行方法・仕様対応表
├─ manifest.toml             # プログラム別の実行属性・期待結果・仕様書参照
├─ copy/
│  ├─ ZCVSRPT.CBL            # レポート用 FD / WORKING-STORAGE カウンター・作業領域
│  ├─ ZCVSPROC.CBL           # ZCVS-OPEN / ZCVS-PASS / ZCVS-FAIL / ZCVS-CLOSE-SUMMARY 手続き
│  └─ ZCVSHEX.CBL            # 任意のバイト列を 16 進文字列 (EBCDIC) に変換する共通ルーチン
├─ copyalt/
│  └─ ...                    # 代替ライブラリーテスト用
├─ ZD/
│  ├─ ZD101A.CBL
│  └─ ...
├─ ZA/
├─ ZN/
├─ ZP/
│  ├─ lib/
│  │  ├─ ZPS01A.CBL          # 呼び出されるサブプログラム (LOCAL-STORAGE / ENTRY 等)
│  │  └─ ...
│  ├─ ZP101A.CBL
│  └─ ...
├─ ZF/
│  ├─ ZF101A.CBL
│  ├─ ZF201M.CBL
│  └─ ZF201M.JCL             # JCL 駆動モード用の DD/UNIT/DCB 定義
├─ ZS/
├─ ZX/
└─ ZE/
   ├─ ZE101N.CBL
   └─ ...
```

### 5.2 自己検証 COBOL プログラムの具体例

各テストプログラムは共通コピーブック `ZCVSRPT` と `ZCVSPROC` を `COPY` して記述する。以下は `ZD102A.CBL` の構造例である。

```cobol
       CBL TRUNC(STD)
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ZD102A.
      *****************************************************************
      * ZCVS MODULE ZD: BINARY / COMP-5 AND SYNC STORAGE LAYOUT       *
      * SPEC: IBM ENTERPRISE COBOL 4.2 LR PP.236-249                  *
      *****************************************************************
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO REPORT.
       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE
           RECORD CONTAINS 120 CHARACTERS
           RECORDING MODE IS F.
       01  REPORT-RECORD              PIC X(120).
       WORKING-STORAGE SECTION.
           COPY ZCVSRPT.
       01  BIN-GROUP.
           05  STD-BIN-4              PIC S9(4) BINARY VALUE 12345.
           05  STD-BIN-4-BYTES REDEFINES STD-BIN-4 PIC X(2).
           05  COMP5-BIN-4            PIC S9(4) COMP-5 VALUE 12345.
           05  COMP5-BIN-4-BYTES REDEFINES COMP5-BIN-4 PIC X(2).
       01  SYNC-GROUP.
           05  SYNC-CHAR              PIC X VALUE 'A'.
           05  SYNC-FULLWORD          PIC S9(9) COMP SYNC VALUE 1.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 'ZD102A' TO ZCVS-PROG-ID
           MOVE 'ZD'     TO ZCVS-MOD-ID
           PERFORM ZCVS-OPEN-REPORT

      * TEST 1: TRUNC(STD) 2-BYTE BINARY TRUNCATES 12345 TO 2345 (X'0929')
           MOVE 'ZD102A-001' TO ZCVS-TEST-ID
           MOVE 'R42-P248 TRUNC(STD) PIC S9(4) BINARY' TO ZCVS-DESC
           MOVE STD-BIN-4-BYTES TO ZCVS-HEX-IN
           MOVE 2 TO ZCVS-HEX-LEN
           PERFORM ZCVS-TO-HEX
           MOVE '0929' TO ZCVS-EXPECTED
           MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
           PERFORM ZCVS-ASSERT-EQ

      * TEST 2: COMP-5 RETAINS FULL 2-BYTE VALUE 12345 (X'3039')
           MOVE 'ZD102A-002' TO ZCVS-TEST-ID
           MOVE 'R42-P248 COMP-5 FULL HALFWORD' TO ZCVS-DESC
           MOVE COMP5-BIN-4-BYTES TO ZCVS-HEX-IN
           MOVE 2 TO ZCVS-HEX-LEN
           PERFORM ZCVS-TO-HEX
           MOVE '3039' TO ZCVS-EXPECTED
           MOVE ZCVS-HEX-OUT(1:4) TO ZCVS-ACTUAL
           PERFORM ZCVS-ASSERT-EQ

      * TEST 3: SYNC INSERTS 3 SLACK BYTES BEFORE 4-BYTE COMP (1+3+4=8)
           MOVE 'ZD102A-003' TO ZCVS-TEST-ID
           MOVE 'R42-P236 SYNC FULLWORD SLACK BYTES' TO ZCVS-DESC
           IF LENGTH OF SYNC-GROUP = 8
               PERFORM ZCVS-PASS
           ELSE
               MOVE '8' TO ZCVS-EXPECTED
               MOVE LENGTH OF SYNC-GROUP TO ZCVS-NUM-EDIT
               MOVE ZCVS-NUM-EDIT TO ZCVS-ACTUAL
               PERFORM ZCVS-FAIL
           END-IF

           PERFORM ZCVS-CLOSE-REPORT
           STOP RUN.
           COPY ZCVSPROC.
           COPY ZCVSHEX.
```

この構成により:
1. **外部ランナー非依存**: プログラム単体を z/OS の `IGYCRCTL` でコンパイルし `//REPORT DD SYSOUT=*` を割り当てて実行するだけで、同じ PASS/FAIL レポートと集計行が得られる。
2. **失敗原因の即座の特定**: 失敗時にはテスト ID、仕様書のページ番号 (`R42-P248`)、期待値 (`EXPECTED`)、実測値 (`ACTUAL`) がレポートに出力される。

---

## 6. `zeus-ccvs` ハーネスの拡張設計

既存の `crates/zeus-ccvs` を拡張し、CCVS85 (`work/suite-zeus`) と ZCVS (`zcvs/`) の両方を同一の CLI で実行・集計できるようにする。

### 6.1 スイート種別とマニフェスト (`zcvs/manifest.toml`)

CCVS85 では特別な扱いが必要なプログラムを `crates/zeus-ccvs/src/special.rs` にハードコードしているが、ZCVS ではスイート直下の `manifest.toml` から宣言的に読み込むようにする (`manifest.toml` が存在しない場合は従来どおり CCVS85 として動作する)。

```toml
[suite]
name = "ZCVS"
required_modules = ["ZD", "ZA", "ZN", "ZP", "ZF", "ZS", "ZE"]
optional_modules = ["ZX"]

# 特殊な実行条件を持つプログラムの定義 (省略時は通常のコンパイル＋実行＋レポート集計)
[programs.ZN106M]
ccsid = 939

[programs.ZS103M]
parm = "NIGHTLY,20261016"
expect_exit_code = 4

[programs.ZF201M]
jcl = "ZF201M.JCL"

[programs.ZE101N]
expect = "compile-error"
diagnostic_contains = "END PROGRAM"
```

### 6.2 `Outcome` の拡張 (負系テストの集計)

`crates/zeus-ccvs/src/runner.rs` の `Outcome` に、コンパイルエラーを期待する負系テスト (`expect = "compile-error"`) の判定を追加する。

- 期待どおりコンパイルエラーとなり、かつ `diagnostic_contains` (指定時) が診断メッセージに含まれていれば `Counts { total: 1, pass: 1, .. }` として `Ran` (OK) に計上する。
- 誤ってコンパイルに成功してしまった場合、または期待と異なる診断だった場合は `Counts { total: 1, fail: 1, .. }` として失敗に計上する。

### 6.3 ツールチェーンと JCL 連携

```sh
# 1. ZCVS 単体の実行 (インタプリタ / ネイティブ)
zeus-ccvs run --suite zcvs --toolchain zeus --out work/zcvs-zeus.json
zeus-ccvs run --suite zcvs --toolchain zeus-native --out work/zcvs-native.json

# 2. 比較オラクル (GnuCOBOL -std=ibm) との比較レポート出力
zeus-ccvs run --suite zcvs --toolchain gnucobol --out work/zcvs-gnucobol.json
zeus-ccvs report --results work/zcvs-zeus.json --baseline work/zcvs-gnucobol.json --programs

# 3. CCVS85 と ZCVS の両方を通した統合レポート
zeus-ccvs report --results work/ccvs85-zeus.json --results work/zcvs-zeus.json
```

- `toolchain = gnucobol` で ZCVS を走らせる際は、`manifest.toml` が存在する場合は `-std=ibm` を付与する。GnuCOBOL と IBM Enterprise COBOL の仕様差 (EBCDIC バイト値、HFP 表現、`ARITH(EXTEND)` 等) により GnuCOBOL 側が不合格になる項目は `docs/oracle/gnucobol-differences.md` に記録し、ZCVS 側の期待値 (IBM 仕様および Hercules 10 進命令オラクル) を正とする。
- `jcl = "..."` が指定されたプログラム (`ZF201M` など、物理ブロック・VSAM カタログ・仮想テープ DD を伴うテスト) は、`zjob` を通じて JCL をサブミットし、スプールまたは `REPORT` DD の出力を `report::evaluate` に渡す。

---

## 7. 既存テスト資産との役割分担と移行方針

現在 `crates/zeus-cobol/tests/` にある統合テスト群と ZCVS は次のように役割を分担する。

| 区分 | 置き場所 | 役割 |
|---|---|---|
| **ZCVS (`zcvs/`)** | `zcvs/<MOD>/*.CBL` | **COBOL 言語・実行互換性の正本スイート**。実機 z/OS (`IGYCRCTL`) でも実行可能な自己検証 COBOL プログラムとして記述し、`zeus-ccvs` でモジュール別合格率・件数を追跡する |
| **Rust 内部 API テスト** | `crates/zeus-cobol/tests/*.rs` | ZIR の JSON シリアライズ往復、スレッドローカル状態の並行コンパイル、VM API の直接呼び出しなど、COBOL ソース単体では表現できない Rust クレート内部の契約テストに絞る |
| **Cargo テスト連携** | `crates/zeus-ccvs/tests/zcvs.rs` | `cargo test --workspace` 実行時にも `zcvs/` の全モジュールをインプロセスまたは `zcobc` 経由で走らせ、CI で常時全件合格を強制する |

既存の次のテストは、検証ロジックを `ZCVSRPT` 形式の COBOL コードへ昇格させて `zcvs/` に収録する。

1. `crates/zeus-cobol/tests/ibm_conformance.rs` (44 件: C01〜C11, X01〜X05) → `ZD`, `ZN`, `ZP`, `ZS`, `ZX`, `ZE` へ展開
2. `crates/zeus-cobol/tests/ignored_clauses.rs` (13 件: I01〜I26) → `ZN`, `ZF`, `ZS`, `ZE` へ展開
3. `crates/zeus-cobol/tests/physical_recursive.rs` (I04/I11〜I14/I19/I20/I25) → `ZP`, `ZF` へ展開
4. `crates/zeus-cobol/tests/remaining_compatibility.rs` (R01〜R08 + [0018](0018-cobol-compatibility-completion.md) の `ARITH`/`NUMPROC`/HFP/`ENTRY`/可視性) → `ZA`, `ZN`, `ZP`, `ZF`, `ZS`, `ZE` へ展開

---

## 8. 段階的実装ロードマップ

| フェーズ | 内容 | 目標規模 (目安) | 完了条件 |
|---|---|---|---|
| **Z0: ハーネス・共通基盤** | `zcvs/copy/` (`ZCVSRPT`, `ZCVSPROC`, `ZCVSHEX`) の作成、`crates/zeus-ccvs` の `manifest.toml` 対応 (`Z*` モジュール認識・`expect = "compile-error"`・`ccsid`・`parm`・`jcl` 対応) | 共通 copybook 3 本 + ハーネス拡張 | `zeus-ccvs run --suite zcvs --toolchain zeus` と `zeus-native` が動作し、`cargo test` から実行できる |
| **Z1: データ・算術モジュール (`ZD`, `ZA`)** | EBCDIC バイト配置、`COMP`/`COMP-3`/`COMP-5`、`SYNC`、ODO、`TRUNC(STD/OPT/BIN)`、`ARITH(COMPAT/EXTEND)`、`NUMPROC`/`NUMCLS`、HFP 浮動小数点・数学関数 | 15 プログラム / 250+ テスト項目 | `ZD`, `ZA` が `zeus` / `zeus-native` で FAIL 0。10 進・HFP 期待値を `zeus-hercdiff` (Hercules) と照合済み |
| **Z2: 国別・プログラム構造 (`ZN`, `ZP`)** | `PIC G`/`PIC N`、`NSYMBOL`、`GROUP-USAGE NATIONAL`、CCSID 変換、`LOCAL-STORAGE`、`RECURSIVE`、`RETURNING`、`ENTRY`、各種ポインター、入れ子可視性 | 12 プログラム (+ サブプログラム 8 本) / 180+ テスト項目 | `ZN`, `ZP` が FAIL 0。既存 `ibm_conformance` / `remaining_compatibility` の該当ケースをすべて包含 |
| **Z3: ファイル・環境・拡張・負系 (`ZF`, `ZS`, `ZX`, `ZE`)** | QSAM 物理ブロック (`F/V/S/U`, `AWO`, `RESERVE`, `CODE-SET`)、VSAM 第 2 `FILE STATUS`・`STATUS 39`、AWSTAPE、`WHEN-COMPILED`、世紀窓、5.x/6.x 拡張 (`ZX`)、コンパイル診断 (`ZE`) | 35+ プログラム / 300+ テスト項目 | [0018](0018-cobol-compatibility-completion.md) の残件実装と合わせて全必須モジュール (`ZD`, `ZA`, `ZN`, `ZP`, `ZF`, `ZS`, `ZE`) および `ZX` が全件合格 |
| **Z4: 実機 z/OS 実行用 JCL パッケージとオラクル台帳** | 実機 z/OS (`IGYCRCTL` + `IEWL`) へ一括転送・実行できる JCL 生成コマンド (`zeus-ccvs export-jcl`)、および GnuCOBOL `-std=ibm` との差異台帳更新 | 全モジュール一括 JCL + `docs/oracle/` 更新 | 生成した JCL と PDS メンバーが標準の z/OS JCL 構文に準拠し、比較結果が `docs/oracle/` に記録される |

## 9. シナリオ網羅性の受入基準

§4の機能名ごとに代表プログラムが存在するだけでは完了としない。
[シナリオ再監査表](../../zcvs/SCENARIOS.md)で、要求群ごとの入力分割・
境界・異常系・状態遷移・オプション間の相互作用を管理する。
CCVS85に委ねるケースには既存の検査IDを、ZCVSで検査するケースには
COBOLプログラムと検査IDを対応付ける。未対応のものは明示的な残件とする。

### 9.1 各シナリオに必要な情報

1. 対象の仕様版・節と、正常系/診断系/処理系依存/未定義入力の区分。
2. CBL/PROCESS、CCSID、ABI、呼出し先、DD/DCB/VSAM/テープ属性の前提。
3. 初期状態、入力、実行する操作、期待値とその独立した導出根拠。
4. 値・バイト列・長さ・STATUS/feedback・終了RC・非対象領域保持の観測点。
5. 実行済み処理系と結果。ソースの存在、コンパイル成功、ZeusでのPASS、
   実機比較済みは区別し、代替可能とはしない。

### 9.2 分割と組合せの選び方

- 数値は符号有無、0/正/負、桁数と物理幅の各境界、丸め直前/一致/直後。
- 文字は空白/LOW/HIGH、短い/等長/長い、先頭/末尾、文字単位/バイト単位。
- 配列・ODOは最小/最大/要素1つ、カウント変更後、送受両側の長さと隣接領域。
- CALLとI/Oは初回/再使用、成功→失敗→成功、再OPEN/CANCEL、共有/分離領域。
- 負系は違反1件と正常対照。タイムアウト、内部エラー、別規則の診断を合格にしない。
- TRUNC×USAGE×受信先、ARITH×中間精度、NUMPROC×有効なNUMCLS×符号、
  NSYMBOL×CCSID、再帰×LS/LINKAGE、REPLACE×プログラム境界、
  RECFM×BLOCK×AWO、VSAM操作×アクセスモードなど、意味が変わる交互作用は必須。
  全直積を省略するときも、採用する組合せと除外の理由を記録する。

### 9.3 退行防止

レビュー済みの実装済み検査IDを `zcvs/coverage.json` に保持し、Cargo連携で
ソース・manifest・実レポートの対応と正確な件数を照合する。
インタプリタ/ネイティブの双方で全プログラム・全検査の実行を確認し、
空モジュール、検査の削除、目視扱いへの変更をPASSにしない。
この台帳の一致はシナリオの十分性を証明しないため、再監査表の残件も併せて評価する。

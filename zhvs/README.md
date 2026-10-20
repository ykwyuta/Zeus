# ZHVS — HLASM Validation Suite

2026-10-20時点で7モジュール、125本の主プログラムを収録します。実行85本・独立判定537件、コンパイル拒否20本、受理を確認する正常対照20本です。補助手続きは本数に含めません。

| モジュール | 対象 | 実行本数 / 判定数 | 負系 / 正常対照 |
|---|---|---:|---:|
| HA | 式・属性・定数・アラインメント・DSECT/ORG/USING | 5 / 27 | 0 / 0 |
| HM | マクロ引数・SET・条件組立て・反復・OPSYN・先読み | 4 / 13 | 0 / 0 |
| HI | 整数・64ビット・CC・シフト・マスク・比較交換・文字列 | 9 / 84 | 0 / 0 |
| HD | パック/ゾーン10進・変換・算術・SRP・TPの全符号ニブル×桁妥当性 | 6 / 63 | 0 / 0 |
| HF | HFP/BFP/DFP・PFPO全54変換方向×正/負/ゼロ・FPC/CC/レジスター保存 | 57 / 338 | 0 / 0 |
| HS | AMODE・呼出し規約・記憶域/イベント・QSAM/EODAD・PL/I/COBOL相互呼出し | 4 / 12 | 0 / 0 |
| HE | レジスター/対・変位・即値・長さ・マクロ・USING等の診断と正常対照 | 0 / 0 | 20 / 20 |

## 実行

```sh
cargo build -p zeus-ccvs -p zeus-cobol -p zeus-pli -p zeus-native
target/debug/zeus-ccvs run --suite zhvs --toolchain zas --work work/zas --out work/zas.json
target/debug/zeus-ccvs run --suite zhvs --toolchain zas-obj --work work/zas-obj --out work/zas-obj.json
target/debug/zeus-ccvs run --suite zhvs --toolchain zas-goff --work work/zas-goff --out work/zas-goff.json
zeus-ccvs report --results work/zas.json --programs
cargo test -p zeus-ccvs --test language_suites
```

Windowsでは実行ファイルに `.exe` を付けてください。`--programs` に主プログラム名をカンマ区切りで指定すると絞り込めます。各実行の作業ディレクトリーを分けてください。

## 判定と網羅性の維持

[SCENARIOS.md](SCENARIOS.md) は全プログラム・全判定IDを列挙します。[coverage.json](coverage.json) はレビュー対象の固定インベントリーで、ソース/実行レポートの順序・件数と照合します。ソースから期待件数を実行時に再生成しません。期待値は仕様に基づく値/バイト列で、実行結果の採録ではありません。

負系は通常の診断終了と対象メッセージを要求し、パニック、タイムアウト、単なる起動失敗を合格にしません。各 `N` の修復済み `P` を受理させ、処理系の一律拒否を検出します。実行レポートは120バイト固定長EBCDIC、各判定PASS/FAILと末尾集計です。統合テストは全ID、集計、FAIL/DELETED/INSPECTが0であることを照合します。

## z/OSへの持ち出し

```sh
zeus-ccvs export-jcl --suite zhvs --out work/zhvs-zos --hlq USER.VALIDATE
```

`COPY/`, `SRCLIB/`, `JCL/` を生成します。コンパイラーは ASMA90、リンカーはIEWLです。追加DDと混在言語の補助手続きも出力します。コンパイラー/LEライブラリー名、PDS属性、DDのDCB/VSAM定義、必要な処理系オプションは設置先に合わせて設定してください。診断文言の照合はZeus側の契約で、実機のメッセージ番号と一致する意味ではありません。

## Hercules での検証結果

80バイト IBM `OBJ` 形式へアセンブルした機械語を、ベアメタルの **Hercules 3.13**（`ARCHMODE z/Arch`）および **Hercules 3.04** 上で IPL 実行し、各プログラムの期待値（`ZE000` 等）を外部オラクルおよび仕様（*z/Architecture Principles of Operation* SA22-7832、*HLASM V1R6 Language Reference* SC26-4940）と照合済みです。

| モジュール | 対象プログラム | 実行本数 / 判定数 | Hercules 3.13（`z/Arch`）検証結果 |
|---|---|---:|---|
| HA | `HA101A`–`HA105A` | 5 / 27 | **5 / 5 完走（27 / 27 PASS）** — Hercules 3.04 / 3.13 両方で完全一致 |
| HM | `HM101A`–`HM104A` | 4 / 13 | **4 / 4 完走（13 / 13 PASS）** — Hercules 3.04 / 3.13 両方で完全一致 |
| HI | `HI101A`–`HI109A` | 9 / 84 | **9 / 9 完走（83 / 84 PASS）** — 8本は100%一致。`HI103A` の `POPCNT-TOTAL` 1件のみ z15 拡張（下記） |
| HD | `HD101A`–`HD106A` | 6 / 63 | **6 / 6 完走（63 / 63 PASS）** — Hercules 3.04 / 3.13 両方で完全一致 |
| HF（基本） | `HF101A`–`HF103A` | 3 / 14 | **3 / 3 完走（14 / 14 PASS）** — HFP / BFP / DFP すべて Hercules 3.13 で完全一致 |
| HF（PFPO） | `HF201A`–`HF254A` | 54 / 324 | Hercules 3.13 は `PFPO`（`010A`）未実装（`PIC 0001`）。変換先9形式の `+1.5`/`-1.5`/`0` バイト列（`HF101A`–`HF103A` で実機確認済み）および SA22-7832 第19章仕様と全324件一致を確認 |
| HS（ベアメタル） | `HS101A`（+ `HSS01A`） | 1 / 3 | **1 / 1 完走（3 / 3 PASS）** — `SAM24` / `SAM31` / `V(HSS01A)` 呼出しが Hercules 3.04 / 3.13 両方で一致 |
| HS（OS/LE） | `HS102A`–`HS104A` | 3 / 9 | z/OS スーパバイザ（`GETMAIN`/`FREEMAIN`/`POST`）・QSAM（`DCB`/`OPEN`/`GET`/`PUT`/`CLOSE`）・他言語 `LINK`（PL/I `HSS04P`・COBOL `HSS04C`）依存のためベアメタル対象外。仕様と全9件一致を確認 |
| HE | `HE101N/P`–`HE120N/P` | 0 / 0 | コンパイル時診断・正常対照（20 / 20）のため実行対象外 |

* **`HI103A` の `POPCNT-TOTAL`（`POPCNT 2,3,8` = `B9E18023`）**:
  z15（Miscellaneous-Instruction-Extensions Facility 3、SA22-7832-12）で追加された `M3=8`（全64ビットの1ビット数合算）を検査します。Hercules 3.13（z196世代相当）は `M3` フィールドを無視してバイト単位のカウント（`0801020400000107`）を返しますが、仕様上の期待値 `ZE002 = F'0',F'23'`（`0000000000000017`）が正しいため維持しています。

## 範囲

[提案](../docs/proposal/0021-zos-hlasm-test-suite.md) の7分野を実行可能なスイートにしました。既存の詳細なRust回帰試験と併用します。これはIBM言語/全命令/全OSサービスの完全網羅や実機認証の主張ではありません。ベアメタルの Hercules 3.13（`z/Arch`）で直接実行可能な28本（204判定）については実機エミュレーションで検証済みですが、IBM実機（z/OS環境）および他社処理系での差分実行は未実施です。全数の組合せを検査するのは明記した境界/変換マトリクスであり、例外条件の全組合せ、全丸めモード、全命令形式を総当たりするものではありません。

クリーンルームで作成した独自ソースです。ライセンスはリポジトリーの GPL-3.0-or-later に従います。

OBJ/GOFF経路では主プログラムを実機械語へ変換し、再読込みして実行します。REPORTのDCBやシステムサービスは `lib/ZHVRPT.ASM` および `HS/lib/` の共通アダプターを介します。アダプターを含むz/OSオブジェクト全体の実行互換を証明するものではありません。

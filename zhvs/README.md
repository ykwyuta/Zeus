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

## 範囲

[提案](../docs/proposal/0021-zos-hlasm-test-suite.md) の7分野を実行可能なスイートにしました。既存の詳細なRust回帰試験と併用します。これはIBM言語/全命令/全OSサービスの完全網羅や実機認証の主張ではありません。IBM実機・Hercules・他社処理系との差分実行は未実施です。全数の組合せを検査するのは明記した境界/変換マトリクスであり、例外条件の全組合せ、全丸めモード、全命令形式を総当たりするものではありません。

クリーンルームで作成した独自ソースです。ライセンスはリポジトリーの GPL-3.0-or-later に従います。

OBJ/GOFF経路では主プログラムを実機械語へ変換し、再読込みして実行します。REPORTのDCBやシステムサービスは `lib/ZHVRPT.ASM` および `HS/lib/` の共通アダプターを介します。アダプターを含むz/OSオブジェクト全体の実行互換を証明するものではありません。

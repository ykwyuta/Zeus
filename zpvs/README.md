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

## 範囲

[提案](../docs/proposal/0020-zos-pli-test-suite.md) の7分野を実行可能なスイートにしました。既存の詳細なRust回帰試験と併用します。これはIBM言語/全命令/全OSサービスの完全網羅や実機認証の主張ではありません。IBM実機・Hercules・他社処理系との差分実行は未実施です。全数の組合せを検査するのは明記した境界/変換マトリクスであり、例外条件の全組合せ、全丸めモード、全命令形式を総当たりするものではありません。

クリーンルームで作成した独自ソースです。ライセンスはリポジトリーの GPL-3.0-or-later に従います。

# z/OS で通れば互換性を担保できる検査スイートにする計画

2026-11-12 着手。ZCVS (COBOL)・ZPVS (PL/I)・ZHVS (HLASM) を、**z/OS の実機で全件
通ったとき「網羅台帳に載せた言語要素について互換性が担保できた」と言える**ものに
することを目標にする。

## 「担保」の意味

検査がすべて通っても、担保できるのは次の 2 つが揃った範囲だけである。

1. **網羅台帳に載っている**: 言語要素 (文・句・データの種類・命令・マクロ機能・
   条件・サービス) を Language Reference / Principles of Operation の章立てから
   列挙し、各要素に対応する検査がある ([網羅台帳](#網羅台帳))。載っていない要素は
   担保の外であり、台帳に「未網羅」として明記する。
2. **期待値の根拠が確定している**: 各検査の期待値は、次のどれかの根拠を
   `basis.json` に持つ。

| 根拠 | 意味 | z/OS で FAIL したら |
| --- | --- | --- |
| `spec` | 公式文書 (Language Reference、Programming Guide、Principles of Operation、HLASM Language Reference) で値が一意に決まる | 検査の誤り。文書を読み直して検査を直す |
| `zos` | 公式文書から決まらない (処理系定義、未定義の結果、文書に無い番号や形)。期待値は **いまの Zeus の振る舞い** | Zeus が実機と違う。実機の値を正解として検査と Zeus を直し、`spec` 相当の確定値として記録する |
| `zeus` | Zeus 固有の選択 (例: `CBL NUMCLS`) に依り、z/OS ではそのまま流せない | 流し方を z/OS に合わせる (注記に書く) |

`basis.json` は検査 ID ごとに根拠を持ち、プログラムの既定を検査単位で上書きできる。
`zos` と `zeus` には、何を確かめるか・どう流すかの注記 (`note`) を必ず付ける。
全検査が台帳に載っていることは `cargo test -p zeus-ccvs` が強制する
(`crates/zeus-ccvs/tests/common/basis.rs`)。

既存の検査は、各 README が「期待値は仕様に基づく」としているので既定を `spec` に
した。ただしこれは README の主張をそのまま写したもので、網羅台帳を作るときに
モジュールごとに読み直し、公式文書から決まらないものは `zos` に移す。

## 網羅台帳

`docs/inventory/` に言語ごとの台帳を置き、`tools/suite-inventory.py [--ccvs85 newcob.val]`
で作り直す。道具は要素ごとに、検査プログラムの原文がその要素を使うかを機械的に照合する
(報告の仕組みのコピー・マクロ・書き手は数えない)。最後の列「確認」は人が読んで書き、
作り直しても保たれる。

- `zcvs-elements.md` — Enterprise COBOL for z/OS Language Reference の章立て
- `zpvs-elements.md` — Enterprise PL/I for z/OS Language Reference の章立て
- `zhvs-elements.md` — HLASM Language Reference と Principles of Operation の命令

各行は「要素 / 検査 (プログラムと検査 ID) / 状態」を持つ。状態は
`網羅` (代表値・境界・内部表現まで検査がある)、`一部` (使っているが境界や
組合せが無い)、`未網羅`、`対象外` (理由を書く。例: CCVS85 が測る COBOL-85 の
基本、OS の外の製品) のどれか。COBOL の基本機能は NIST CCVS85 でも測るので、
CCVS85 のどのモジュールが測るかも書く (ZCVS は z/OS 固有の決めごとを測る)。

## 実機確認シナリオ

公式から決まらない挙動は、黙って抜かすのではなく **`zos` の検査として書く**。
z/OS で流したときに Zeus との差が数として出るようにするためである。例:

- 10 進から HFP への変換の丸め、HFP の正規化されない値
- PL/I の ONCODE の番号、条件の既定の処置の細部
- COBOL の国別項目から英数字への MOVE の可否 (文書に明記された表を確認できない)
- PL/I の AREA の管理域の形、入口の値の記憶域の形
- SIZE ERROR が無いときの桁あふれの結果、未定義とされる重なりの MOVE
- 診断の重大度 (RC) — 文言と番号は Zeus の契約で、実機の番号とは一致させない

## z/OS で流して結果を記録する

1. `zeus-ccvs export-jcl --suite <suite> --out work/<suite>-zos --hlq <HLQ>` で
   COPY/SRCLIB/JCL を作る。`zeus` の検査は注記に従って直す。
2. z/OS で流し、各プログラムの REPORT メンバー (120 byte の PASS/FAIL 行) を
   プログラム名のファイルとして 1 つのディレクトリーに回収する。データセットの
   まま (EBCDIC の固定長レコード) でも、テキストとしてダウンロードしたものでもよい。
3. ジョブの出力から、プログラムごとのコンパイルの RC と実行ステップの完了
   (RC の数、または `S0C7`・`U4038` のような異常終了コード) を step ファイルに書く。

   ```
   PROGRAM  COMPILE-RC  COMPLETION
   ZA124A   0           U4038
   HS105A   0           S0C7
   ZE101N   12          -
   ```

4. 記録する。検査 ID ごとに PASS / FAIL / MISSING (報告に無い) が付く。

   ```sh
   tools/zos-results.py record --suite zcvs --reports DIR --steps steps.txt \
       --env compiler="Enterprise COBOL 6.4" --env options="ARITH(EXTEND)" \
       --env ccsid=1140 --date 2026-12-01
   ```

   `<suite>/zos-results/<日付>.json` ができる。これはコミットする。
5. 突き合わせる。`tools/zos-results.py compare --suite zcvs --results <file>` が
   根拠ごとに数え、FAIL の意味を分けて出す。

   | 出力 | 意味 | すること |
   | --- | --- | --- |
   | `spec FAIL` | 文書の読みが実機と合わない | 文書を読み直して検査を直す |
   | `zos FAIL` | Zeus が実機と違う | 実機の値を正解にして検査と Zeus を直す |
   | `zos PASS` | Zeus の値が実機で確かめられた | 根拠を `spec` 相当に移し、注記に実行日を書く |
   | `STEP` | 負系が受理された、異常終了するはずのものが RC 0 で終わった等 | manifest と検査を見直す |
   | `MISSING` | 報告が無い・途中で終わった | ジョブの出力を見る |

   manifest の `expect_exit_code` は Zeus の終了コードである。z/OS では異常終了
   コードになるので、比較では「RC 0 で終わらないこと」だけを見る。

道具そのものは `crates/zeus-ccvs/tests/zos_results.rs` が検査する。Zeus の実行結果
(`zeus-ccvs run --work W` の W) も同じ道具で記録でき、2026-11-12 に 3 スイートを
記録したところ `spec`・`zos` の FAIL と MISSING は 0 件だった。

## 進め方と記録

| 日付 | 内容 | ZCVS | ZPVS | ZHVS |
| --- | --- | --- | --- | --- |
| 2026-11-12 | 根拠台帳を置く。既存は既定 `spec`、`zeus` 4 本 (NUMCLS)、`zos` 2 検査 (KEY の ONCODE) | 162 本 / 1,464 | 72 本 / 340 | 128 本 / 582 |
| 2026-11-12 | 網羅台帳を作る (`tools/suite-inventory.py`)。要素のうち検査が使うもの / 未網羅 | 208 / 129 | 212 / 199 | 176 / 447 |
| 2026-11-12 | ZCVS の実機確認シナリオ `ZA123A`/`ZA124A`/`ZN114A` (根拠 `zos`、期待値は Zeus) | 165 本 / 1,481 | | |
| 2026-11-12 | ZPVS の実機確認シナリオ `PD109A`/`PC110A`。Zeus の浮動小数点が HFP の範囲で OVERFLOW/UNDERFLOW を起こさない不備を直す | | 74 本 / 370 | |
| 2026-11-12 | ZHVS の実機確認シナリオ `HD107A`/`HS105A` | | | 130 本 / 594 |
| 2026-11-12 | z/OS の結果を記録・突き合わせる道具 `tools/zos-results.py`。Zeus の実行を記録して `spec`/`zos` の FAIL 0 件を確認 | 165 本 / 1,481 | 74 本 / 370 | 130 本 / 594 |
| 2026-11-12 | Enterprise COBOL 5/6 の組込み関数 22 個を `ZA125A` (spec 36 検査) で測り、Zeus に実装。未網羅 128 → 106 | 166 本 / 1,517 | | |
| 2026-11-12 | `ZS108A` (指示文・PACKED-DECIMAL・UPSI・VOLATILE・SECONDS-PAST-MIDNIGHT、spec 11 検査)。Zeus に VOLATILE と SECONDS-PAST-MIDNIGHT を実装。未網羅 106 → 100 | 167 本 / 1,528 | | |
| 2026-11-12 | `PM109A` (組込み関数 spec 36 判定)。Zeus に IAND 等 20 個を実装。道具が DCL/PROC/STG/CSTG の略記を数えるようにした。未網羅 193 → 144 | | 75 本 / 406 | |
| 2026-11-12 | `HA106A`・`HM106A`・`HM107A` (DC 型・CCW・条件付きアセンブリの関数とシステム変数、spec 53 判定)。Zeus に CCW/CCW0/CCW1 を実装。道具がアドレス定数を数えるようにした。未網羅 443 → 390 | | | 133 本 / 647 |
| 2026-11-12 | 機械命令 `HI112A`–`HI116A` (加減算・論理加減算・比較・乗除算・ロード、spec 90 検査)。Zeus に MS/MSY を実装。道具から STORAGE/STIMER/XCTL (マクロ) を外した | | | 138 本 / 737 |
| 2026-11-12 | 機械命令 `HI117A`–`HI121A` (ストア・挿入、反転・条件付き、AND/TM、OR/XOR、3 オペランドの論理演算、spec 81 検査)。Zeus のアセンブラーに LAY を足した | | | 143 本 / 818 |
| 2026-11-12 | 機械命令 `HI122A`–`HI124A` (シフト・回転、分岐、インターロック、spec 52 検査)。Zeus に LPQ/STPQ と、TS・SSF 形式 (CSST/LPD/LPDG) のオブジェクト形式の書き出しと解読を実装 | | | 146 本 / 870 |

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
2. z/OS で流し、各プログラムの REPORT (120 byte の PASS/FAIL 行) を回収する。
3. `<suite>/zos-results/<日付>.json` に、検査 ID ごとの PASS/FAIL と環境
   (コンパイラーの版・オプション・CCSID) を記録する。
4. `zos` の検査が PASS ならその根拠を「実機で確定」とし、FAIL なら実機の値を正解に
   して検査と Zeus を直す。`spec` が FAIL なら検査を読み直す。

## 進め方と記録

| 日付 | 内容 | ZCVS | ZPVS | ZHVS |
| --- | --- | --- | --- | --- |
| 2026-11-12 | 根拠台帳を置く。既存は既定 `spec`、`zeus` 4 本 (NUMCLS)、`zos` 2 検査 (KEY の ONCODE) | 162 本 / 1,464 | 72 本 / 340 | 128 本 / 582 |
| 2026-11-12 | 網羅台帳を作る (`tools/suite-inventory.py`)。要素のうち検査が使うもの / 未網羅 | 208 / 129 | 212 / 199 | 176 / 447 |
| 2026-11-12 | ZCVS の実機確認シナリオ `ZA123A`/`ZA124A`/`ZN114A` (根拠 `zos`、期待値は Zeus) | 165 本 / 1,482 | | |

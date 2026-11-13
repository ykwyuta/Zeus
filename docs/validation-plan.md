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

## 出典 (裏付け)

根拠が `spec` でも、どの文書のどこで値が決まるかを書かなければ裏付けにならない。
各検査は `basis.json` の `refs` に出典を持つ (検査に無ければプログラムの出典を使う)。

```json
"ZA118A": {
  "basis": "spec",
  "refs": [
    "COB-LRM: SIZE ERROR phrases | If the resultant identifier is defined with usage BINARY, ..."
  ]
}
```

- 出典は `"文書ID: 見出し"` か `"文書ID: 見出し | 本文の引用"`。文書 ID は
  [`docs/sources.json`](sources.json) に版 (資料番号) と URL と sha256 で固定する。
- `zos` の検査は、値を決めていない (未定義・処理系定義と書いてある、または書いていない)
  箇所を出典にする。
- 文書はリポジトリに入れない。`tools/basis-refs.py fetch --docs DIR` で取り、
  `tools/basis-refs.py check --docs DIR` で見出しが文書にあること、引用がその見出しの
  あとの本文にそのままあることを確かめる。`report` は出典の付いた検査を数える。
- 照合で文書と食い違った検査は、文書に合わせて検査と Zeus を直す (例: `ZA118A`)。
- 出典の無い検査は `cargo test -p zeus-ccvs` が落とす (`crates/zeus-ccvs/tests/common/basis.rs`)。

台帳の文書 (`docs/sources.json`): Enterprise COBOL 6.4 LRM/PG と 4.2 PG、
Enterprise PL/I 6.1 LRM/PG/Messages and Codes、HLASM 1.6 LR/PG、
z/Architecture Principles of Operation、z/OS 3.1 LE Programming Reference・
DFSMS Macro Instructions for Data Sets・MVS Assembler Services Reference 第 1/2 巻。

| スイート | 検査 | 出典あり | spec | zos | zeus |
| --- | --- | --- | --- | --- | --- |
| ZCVS | 1,620 | 1,620 | 1,343 | 53 | 224 |
| ZPVS | 547 | 547 | 534 | 13 | 0 |
| ZHVS | 1,032 | 1,032 | 1,027 | 5 | 0 |

(負系・正常対照はプログラムを 1 検査と数える。)

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
| 2026-11-12 | 機械命令 `HI125A`–`HI131A` (変換・長い移動・TRTR・アクセスレジスター・AMODE、HFP 短/長/拡張、BFP、DFP、spec 113 検査)。Zeus に CVBG/CVDG・LAM/STAM と、MVCLE/CLCLE/TRTR の OBJ/GOFF の書き出しと解読を実装。機械命令の未網羅は CSST と TDG*T の 4 個 (理由を台帳に記載) | | | 153 本 / 983 |
| 2026-11-12 | 網羅台帳の道具: `>>DEFINE` 等の照合漏れを直し、CCVS85 だけが測る要素を状態「CCVS85」に分けた。`ZS109A`/`ZA126A` (spec 18 検査)。Zeus に FORMATTED-CURRENT-DATE、ZWB/NOZWB、INTDATE を実装。ZCVS の未網羅 100 → 34 (CCVS85 50) | 169 本 / 1,546 | | |
| 2026-11-12 | `PM110A`/`PD110A`/`PC111A` (spec 35、zos 1)。Zeus の STRINGSIZE・PRESENT/OMITTED・BYVALUE を直し、IEEE/LITTLEENDIAN/NONNATIVE を黙って読み捨てず断るようにした。ZPVS の未網羅 144 → 107 | | 78 本 / 442 | |
| 2026-11-12 | `HA107A` (LOCTR・CCW0・S/SY/V/VD 型・WXTRN、spec 8)、`HM107A` に SLA。網羅台帳に「対象外」の区分 (確認列が対象外で始まる行) を足し、リスト・目的コード・出口の制御 16 個を対象外にした。ZHVS の未網羅 55 → 27 | | | 154 本 / 992 |
| 2026-11-12 | `PM111A` (spec 13)。Zeus に ADDRDATA・BITLOCATION・CS/CDS・POINTERSUBTRACT・OFFSET 系・RANK・EXPONENT・SCALE・SECSTODATE を実装。リスト制御・ダンプ・再始動など 14 個を対象外にした。ZPVS の未網羅 107 → 80 | | 79 本 / 455 | |
| 2026-11-12 | `PM112A` (spec 37)。Zeus に BETWEEN 系・INLIST・IFTHENELSE・ICLZ・IRLL/IRRL・LOWER2・POPCNT・ISIGNED/IUNSIGNED・ROUNDAWAYFROMZERO/ROUNDTOEVEN・MAXVAL/MINVAL・PRED/SUCC・PLACES・PRECVAL/SCALEVAL・Y4DATE 系・REPLACEBY2・HEXIMAGE・COMPARE・LOCATION・ERF/ERFC/GAMMA/LOGGAMMA・ONCONDID/ONCONDCOND を実装 (期待値は LRM 6.1 の本文と例) | | 80 本 / 492 | |
| 2026-11-12 | `PM113A` (spec 25)。Zeus に文字列とバッファーの関数 27 個 (COLLAPSE・SQUEEZE・INDEXR・REPLACE・SCRUBOUT・UNHEX・HEX8・MEM 系・WS 系・HEXENCODE/DECODE・BASE64 系・WHEREDIFF) を実装 | | 81 本 / 517 | |
| 2026-11-12 | 出典の仕組み (`docs/sources.json`・`tools/basis-refs.py`・`refs`)。ZA/ZD の 41 本を文書と照合。`ZA118A` は LRM「SIZE ERROR phrases」(BINARY/COMP-5 の上限は TRUNC に依らず PICTURE) と食い違っていたので検査と Zeus を直した。`ZA120A` の表示形 (仮数の桁の置き方は文書に無い)、`ZA113A`/`ZA114A` の +0 を `zos`/注記つきにした | 169 本 / 1,555 | | |
| 2026-11-12 | ZCVS の全 170 本を文書と照合し、全 1,620 検査に出典を付けた。`ZN114A` (国別→英数字の MOVE) は LRM 表 51 で無効と決まるので負系 `ZE146N/P` に移し、Zeus に表 51 の MOVE の可否の診断を足した。VSAM の戻りコードは DFSMS Macro Instructions を台帳に足して裏付けた。DBCS の符号位置 (CCSID の変換表) と、COBOL が VSAM を呼ばずに出す 47 のときの VSAM 状態は文書に無いので `zos` にした | 170 本 / 1,553 | | |
| 2026-11-12 | ZPVS の全 81 本を LRM・PG・Messages and Codes と照合し、全 547 検査に出典を付けた。ONCODE は Messages and Codes の条件コード表で決まるので `PC110A`・`PC111A`・`PF106A` を `spec` に移し、食い違った 3 つ (CONVERSION 612、DD 無しの UNDEFINEDFILE 84、見つからない FETCH 9250) は Zeus と検査を直した。`PD109A` の STORAGE と NULL も LRM の表 84 と PG の既定で決まった | | 81 本 / 547 | |
| 2026-11-12 | ZHVS の全 154 本を HLASM LR/PG・Principles of Operation・DFSMS・MVS Assembler Services と照合し、全 1,032 検査に出典を付けた。機械命令は POP の命令一覧で記号から命令名に引いた。`HD107A` の固定小数点の端数 (整数に丸める)、P の小数点 (無視)、Z の符号 (ゾーン) は LR で決まるので `spec` に移した。出典の無い検査を cargo test で落とすようにした | | | 154 本 / 1,032 |
| 2026-11-12 | `PM114A` (spec 27)。Zeus に日付・時刻の組込み関数 (MICROSECS・SECS・MICROSECSTODATE・DAYSTOMICROSECS・MICROSECSTODAYS・TIMESTAMP・UTCDATETIME・UTCSECS・UTCMICROSECS・MAXDATE・MINDATE・STCKTODATE・STCKETODATE・JULIANTOSMF・SMFTOJULIAN) と PLISTCK 系 10 個を実装し、DAYS/DAYSTODATE/SECSTODATE を表 65 の型 (Mmm・DDD・ZY・999999・窓) に対応させた。UNSIGNED FIXED BIN(64) を受け付けるようにした。ZPVS の未網羅 329 → 304 | | 82 本 / 559 | |
| 2026-11-12 | `PM115A` (spec 35)。Zeus に WHIGH/WLOW・SUBTO・POLY・WSCOLLAPSE16/WSREPLACE16・BASE64ENCODE16/DECODE16・PROCEDURENAME・PACKAGENAME・SOURCEFILE/SOURCELINE・ISMAIN・MAINNAME・GETENV/PUTENV・UUID4・VALID・ONPROCEDURE・PLIOVER を実装し、名前だけあった COUNT を実装した。UNSPEC が WIDECHAR を文字に変換してから取っていた不備を直した。ZPVS の未網羅 304 → 268 | | 83 本 / 594 | |
| 2026-11-12 | `PM116A` (spec 20)。SHA の組込み関数 44 個 (SHA1/SHA2/SHA3 の DIGEST・INIT・UPDATE・FINAL) を実装した。期待値は EBCDIC の 'abc' 等の FIPS 180-4/202 のハッシュ。文書台帳に FIPS 180-4 と FIPS 202 を足した。ZPVS の未網羅 268 → 232 | | 84 本 / 614 | |
| 2026-11-12 | `PM117A` (spec 21)。前処理系: Zeus に ANSWER と CALL (前処理手続きの文)、%INSCAN/%XINSCAN、COMMENT・COMPILETIME・COLLATE・MACNAME・MACLMAR/MACRMAR・SYS*SIZE を実装し、TRIM/INDEX/VERIFY/TRANSLATE の省略できる引数、SYSVERSION と COMPILEDATE の形を LRM に合わせた。網羅台帳の道具: 前処理の RETURN/ANSWER/CALL は % を付けない文、引数の無い前処理組込み関数は括弧なしで照合する。ZPVS の未網羅 232 → 196 | | 85 本 / 635 | |
| 2026-11-12 | `PC112A` (spec 13、zos 3)。Zeus に ASSERT 文 (ASSERT(CONDITION) のときだけ。ASSERT(ENTRY) は断る) と ASSERTION 条件、ONTEXT・ONACTUAL・ONEXPECTED・ONOPERATOR・ONPACKAGE・ONSUBSCRIPT・ONLBOUND・ONHBOUND・ONLINE・ONAREA を実装した。ASSERTION の ON 単位から普通に戻ったら次の文から続く (LRM)。最初は ERROR に進めてしまい、検査で見つかった。ZPVS の未網羅 196 → 180 | | 86 本 / 651 | |
| 2026-11-12 | `PM118A` (spec 13)。Zeus に INARRAY・WCHARVAL・PICSPEC・CODEPAGE を実装し、RANK を WIDECHAR(1) に広げた。`*PROCESS CODEPAGE(n)` を読むようにした (Zeus の既定は 037 で、PL/I の既定 CODEPAGE(1140) と違うので、検査は明示する)。ZPVS の未網羅 180 → 176 | | 87 本 / 664 | |
| 2026-11-12 | `PA106A` (spec 7、zos 4)。Zeus に QUICKSORT・QUICKSORTX・BINSEARCH・BINSEARCHX を実装した。X の形は比較関数 (OPTLINK、POINTER BYVALUE 2 つ) を処理系の中から呼ぶ。LRM は QUICKSORT を組込み関数の表に挙げるが値を書いていないので、Zeus は CALL 文で呼ぶ形だけを受け、その形は zos とした。ZPVS の未網羅 176 → 171 | | 88 本 / 675 | |
| 2026-11-12 | `PD111A` (spec 11、zos 2)。Zeus に ALLOCATE/ALLOC31 (組込み関数)・ALLOCNEXT・AVAILABLEAREA・ALLOCSIZE・CHECKSTG・UNALLOCATED を実装した。ヒープの割り当てを記録し、後ろに 'FF'X を 8 バイト置く (LRM CHECKSTG)。BASED の FREE で記録を外す。AREA の割り当ては 8 の倍数に丸めるので、LRM の例 (1000 → 896) と合う。ZPVS の未網羅 171 → 164 | | 89 本 / 688 | |
| 2026-11-12 | `PM119A` (spec 8)。Zeus に XMLCHAR (XML 関連の属性なし)・XMLSCRUB・XMLSCRUB16 を実装した。XMLCHAR は LRM の例の 72 バイトとそのまま一致した。値の中の & < > " ' を実体参照にするかは文書に無く、検査では使っていない。Zeus の WIDECHAR は多くの組込み関数で EBCDIC を経由して変換されるので、HEX(SUBSTR(w, ...)) は w の 16 ビットの値を見せない (既知の制約、今回は直していない)。ZPVS の未網羅 164 → 161 | | 90 本 / 696 | |
| 2026-11-12 | `PF107A` (spec 4、zos 1)。KEYED SEQUENTIAL BUFFERED ファイルの KEYTO・KEY のあとの順読み・ONKEY・SAMEKEY。Zeus に足したのは SAMEKEY だけで、ほかは前から動いていた。Zeus のキー付きデータセットのキーは一意なので SAMEKEY は '0'B になる。ZPVS の未網羅 161 → 156 | | 91 本 / 701 | |
| 2026-11-12 | ZCVS `ZP114A` (spec 6)。USE FOR DEBUGGING の PERFORM/GO TO/FALL THROUGH と DEBUG-ITEM、デバッグ行。Zeus の COBOL 実行時は PARM をそのままプログラムに渡し、オブジェクト時スイッチを環境変数 ZEUS_DEBUG からしか取っていなかった。CBLOPTS(ON) (既定) に合わせて PARM を最後の `/` で分け、実行時オプション DEBUG/NODEBUG を読むようにした。ZCVS の未網羅 43 → 42 | | 171 本 / 1559 | |
| 2026-11-12 | 網羅台帳の道具の不具合。HLASM の属性参照 (T' L' S' I' K' N' D' O') の名前を二重引用符つきで持っていたので、どの検査にも当たらなかった。名前を直し、記号か & の前の属性参照だけに当てる形にした。使用ありになったのは L' S' I' K' N' O' の 6 つで、T' と D' は本当に使われていない。ZHVS の未網羅 73 → 67 | | — | |
| 2026-11-12 | ZHVS `HM108A` (spec 13、zos 2)。型属性 T' (DS の各型、G、J、I、WXTRN の $、U、N、O)、定義属性 D'、SYSATTRA/SYSATTRP。Zeus は 10 進・16 進・2 進の自己定義項のマクロ・オペランドの T' を U としていたので N に直した。DS F の記号の SYSATTRA は F で、LRM の「DC/DS の記号は型拡張を含む」に合う (最初の検査は空と書いていた)。ZHVS の未網羅 67 → 65 | | 155 本 / 1007 | |
| 2026-11-12 | ZHVS `HM109A` (spec 11、zos 1)。システム変数記号のうち LRM が形を決めているもの (&SYSCLOCK・&SYSSEQF・&SYSIN_MEMBER・&SYSM_SEV/&SYSM_HSEV・&SYSSTMT・&SYSOPT_DBCS/XOBJECT/OPTABLE、&SYSVER などの T')。Zeus を 3 か所直した: &SYSM_SEV/&SYSM_HSEV を 3 桁に、&SYSSTMT を「次の文番号」(条件付きアセンブリの文も数える) に、OPTABLE の既定を HLASM と同じ UNI に (Zeus の UNI と ZEUS は同じ命令の集まりで、名前だけが変わる)。&SYSSTMT の単体テストは誤った規則 (3) を固定していたので 4 に直した。走らせる道具は翻訳の RC が 4 以下なら実行へ進む (書き出す JCL の COND=(4,LT) と同じ)。ZHVS の未網羅 65 → 51 | | 156 本 / 1019 | |
| 2026-11-12 | ZHVS `HA108A` (spec 2)・`HA109A` (spec 5)。AMODE 31/RMODE 24 (LA が 1-7 ビットを残す)、COPY、ENTRY と V 型定数、DXD・Q 型定数・CXD、RSECT と &SYSSTYP。Zeus の OBJ/GOFF 書き出しは 1 つの制御セクションに平らにするので、RSECT を含むもの、外部ダミー・セクション、同じモジュールの ENTRY への V 型定数を断る。黙って書かないのは正しいので、HA109A は manifest の object_formats = ["zeus"] で Zeus 形式だけで流し、OBJ/GOFF では「Zeus はこの形式で書けない」と報告する (z/OS では普通に流れる)。ZHVS の未網羅 49 → 41 | | 158 本 / 1026 | |
| 2026-11-12 | ZHVS `HA110A` (spec 2)。ICTL 2,71,16 で 1 桁目を読まない (迷い込んだ 9 が無視される)、ラベルは 2 桁目から。Zeus は最初から通った。ICTL は *PROCESS より前の最初の文でなければならず (LRM)、Zeus もそう断る。ZHVS の未網羅 41 → 40 | | 159 本 / 1028 | |
| 2026-11-12 | ZHVS `HM110A` (spec 5、zos 2)。データセットの系の変数記号のうち LRM が決めている形 (&SYSIN_DSN が空でない、原始マクロの &SYSLIB_* は &SYSIN_* と同じ、ライブラリー・マクロの &SYSLIB_MEMBER はそのメンバー、_VOLUME は 6 文字以下、OBJECT のときの &SYSLIN_DSN、使わない SYSPUNCH/SYSTERM/SYSADATA は空)。Zeus を 2 か所直した: 原始マクロの &SYSLIB_* が空だった (単体テストも空を固定していた)、一覧ファイルが無いときの &SYSPRINT_DSN を DUMMY と同じ NULLFILE にした。ZHVS の未網羅 40 → 20 | | 160 本 / 1035 | |
| 2026-11-13 | ZHVS `HM111A` (spec 7)。Zeus に AREAD と AINSERT を実装した。AREAD はマクロ呼出しの次の原始レコードを 80 文字で読み (順序欄も含む)、読んだレコードはアセンブルしない。AINSERT はマクロの中では最外のマクロ呼出しが終わるまで溜め (FRONT は前、BACK は後ろ)、開いたコードではすぐ次の文になる。AREAD は溜まった AINSERT のレコードを先に読む。CLOCKB/CLOCKD も。ZHVS の未網羅 20 → 18 | | 161 本 / 1042 | |
| 2026-11-13 | ZHVS `HA111A` (spec 3)。EXTRN した記号の A 型定数で別にアセンブルした下位プログラム HAS11A (HA/lib) を呼ぶ、EXTRN 記号の T' は T、先頭の *PROCESS OPTABLE(UNI,NOLIST)。Zeus は最初から通った。書き出す JCL は下位プログラムを LOAD に結合し、主プログラムの結合はその LOAD を自動呼出しの SYSLIB に持つ。ESYM は z/OS のシステム記号を返す関数で Zeus には系が無いので対象外にした。ZHVS の未網羅 18 → 15 | | 162 本 / 1045 | |
| 2026-11-13 | ZHVS `HI132A` (spec 14)。TEST DATA GROUP (TDGET・TDGDT・TDGXT) の 6 つのデータグループと符号のマスク (POP 図 20-31)。全ビット 0 は最小指数の +0 (極端な指数のゼロ)、X'..01' は最小指数の 1 (極端な指数の非ゼロ)、桁いっぱいの値は先頭桁が 0 でない。Zeus は 3 形式とも最初から通った。Hercules 3.13 では DFP のプログラムが完走しない (HI131A と同じ) ので、期待値は POP だけによる。ZHVS の未網羅 15 → 12 | | 163 本 / 1059 | |
| 2026-11-13 | ZHVS `HI133A` (spec 5)。COMPARE AND SWAP AND STORE (CSST) の FC 0/1 と SC 0-3。パラメーター表は 4 倍語境界で、置換値が 0 バイト目、格納値が 16 バイト目 (POP 図 7-195)。Zeus は 3 形式とも通った。この環境の Hercules では HI101A も含めて完走せず、外の基準では確かめられない。機械命令の未網羅は 0。ZHVS の未網羅 12 → 11 | | 164 本 / 1064 | |
| 2026-11-13 | ZHVS `HA109A` に QD・QY の 2 検査 (spec 1、zos 1)。QD は Q と同じオフセットを 8 バイトで持つ。QY は 20 ビットの符号付き長変位の並びで、4 ビットの 0・DL 12 ビット・DH 8 ビットと読んだ (並びの図が LRM に無いので zos)。COM は Zeus に無く、結合プログラムで同じ名前の共通セクションをまとめる機能が要るので、理由を台帳に書いて残した。ZHVS の未網羅 11 → 9 | | 164 本 / 1066 | |
| 2026-11-13 | ZCVS `ZP115A` (spec 3)。NODYNAM のもとでの >>CALLINTERFACE STATIC/DYNAMIC と CANCEL。静的に呼んだプログラムは CANCEL しても何も起きず WORKING-STORAGE が続く (LRM: 動的に呼ばれていなければ CANCEL は何もしない)。DYNAMIC にすれば CANCEL で初期状態に戻る。引数なしの >>CALLINTERFACE はコンパイラー・オプションの規則に戻る。Zeus は 2 つの翻訳系とも最初から通った。ZCVS の未網羅 42 → 41 | | 172 本 / 1562 | |
| 2026-11-13 | ZCVS `ZA127A` (spec 4、zos 1)。Zeus の COBOL に FUNCTION UUID4 を実装した (36 文字、小文字の 16 進、版 4、変種 10、参照ごとに違う値、実行ごとに違う値)。ROUNDED MODE は Enterprise COBOL 6.4 の文書に無い (ISO 2002 の機能) ので網羅台帳で対象外にした。ZCVS の未網羅 41 → 39 | | 173 本 / 1567 | |
| 2026-11-13 | ZCVS `ZN115A` (spec 4)。CBL CODEPAGE(1140) で X'9F' がユーロ記号 (U+20AC) になり、NATIONAL-OF の CCSID 引数 37 では通貨記号 (U+00A4) になる。Zeus は最初から通った。PG は COBOL の既定を CODEPAGE(1140) と書くが、Zeus の既定は 037 (PL/I と同じ食い違い、X'9F' だけが違う)。ZCVS の未網羅 39 → 38 | | 174 本 / 1571 | |
| 2026-11-13 | ZCVS `ZP116A` (spec 3)・`ZE147N/P`。Zeus に QUALIFY(COMPAT|EXTEND) を実装した。Zeus は「修飾なしの 01 レベルの名前が勝つ」規則を最初から持っていたが、PG ではそれは QUALIFY(EXTEND) のときだけ一意になる。既定の COMPAT では一意でなければ拒否し、EXTEND では完全な修飾 (含むすべての水準を挙げた修飾) を持つ 1 つに決める。NIST CCVS85 の全プログラムで結果が変わらないことを確かめた (変更前後で 434 本の結果が同じ)。ZCVS の未網羅 38 → 37 | | 177 本 / 1574 | |
| 2026-11-13 | ZCVS `ZP117A` (spec 6)。Zeus の COBOL に ALLOCATE と FREE を実装した。LINKAGE 記録の ALLOCATE は番地を決め (RETURNING にも入る)、INITIALIZED は INITIALIZE WITH FILLER ALL TO VALUE THEN TO DEFAULT と同じ。CHARACTERS INITIALIZED は 2 進のゼロ、端数は切り上げ、0 以下は NULL。FREE は ALLOCATE で取った領域を放して NULL にする。READ PREVIOUS は Enterprise COBOL 6.4 に無いので対象外にした。ZCVS の未網羅 37 → 34 | | 178 本 / 1580 | |
| 2026-11-13 | ZCVS `ZA128A` (spec 3、zos 1)。Zeus の COBOL に FUNCTION CONTENT-OF を足した。引数 1 つの MAX と同じく、引数の内容をその型 (英数字・国別・数値) と大きさのまま返す。OO/Java 連携の 4 要素 (INVOKE ほか) は Zeus の範囲の外 (提案 0007) なので網羅台帳で対象外にした。ZCVS の未網羅 34 → 29 | | 179 本 / 1584 | |
| 2026-11-13 | 利用者の判断で JSON/XML の対応を範囲の外とした。網羅台帳の COBOL の 16 要素 (JSON/XML の文、XML-SCHEMA、JSON/XML の特殊レジスター) と PL/I の 23 要素 (JSON* と XMLUCHAR の組込み関数、ONJSONNAME、PLISAXA-D) を対象外にした。ZCVS の未網羅 29 → 13、ZPVS 156 → 133 | | | |

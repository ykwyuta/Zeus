# ZCVS シナリオ網羅性の再監査

監査基準: [提案0019](../docs/proposal/0019-zos-cobol-test-suite.md) §4.1–4.8。
監査起点: `master` の `0f8d697`。プログラム名ではなく、ソース中の
比較式・分岐・観測対象を確認した。提案のプログラム番号と実装の番号は
一致しないため、以下では実際のファイル名を記す。

**全件PASSはシナリオ網羅を意味しない。** 起点の66プログラム・248検査は
すべて通るが、提案に列挙した機能の一部しか検証していない。
この変更後は104プログラム、693検査（実行660件、負系33件）、
正常コンパイル対照8本となる。これらはZeusのインタプリタとネイティブで
同じ期待値を使う。実機z/OS・Herculesとの比較を完了したという意味ではない。

以下の表は**要求群ごとの不足シナリオ**であり、行数をテスト件数として数えない。
「部分」は右欄の未実装シナリオが残ること、「未」は対応する自己検証がないこと、
「追加」は今回明示した限定的なシナリオ集合を実行できることを示す。
元の要求群全体を「追加」で完了扱いにしない。

## 判定方法

各ケースには、前提（コンパイラー版・オプション・CCSID・DD属性）、入力、操作、
期待結果、観測方法を持たせる。境界では原則として直前・一致・直後を区別する。
状態を持つ操作では初回だけでなく、成功→失敗→成功、再OPEN、再CALLを検査する。
値・バイト列・長さ・ステータス・非対象領域の保持を必要に応じて独立に観測する。

1. オプションの作用域を跨ぐ組合せは別のコンパイル単位にする。
2. 仕様が動作を定める有効入力と、未定義入力に対するZeusの堅牢性を分離する。
   例: `TRUNC(OPT)` のPICTURE範囲外入力をIBM互換性の必須期待値にしない。
3. 負系は1規則1ソース、対象規則を示す診断を確認する。可能なものには、
   違反だけを取り除いた正常対照を付ける。単なるコンパイラー異常終了を合格にしない。
4. ラウンドトリップだけでは相殺する不具合を検出できない。既知のバイト列・
   日付通算日・定数などの片方向オラクルも照合する。
5. EBCDIC、HFP、ホスト依存ポインターなどについてGnuCOBOLを絶対オラクルにしない。
   実装間一致、公開仕様との一致、実機との一致は別々に記録する。

## ZD: 表現・配置

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| D01 表意定数 | `ZD107A` のSPACE/ZERO/HIGH/LOW/QUOTE/ALL | 部分 | QUOTE/APOSTオプション別、長さ1/複数、比較に使う場合 |
| D02 照合順序 | `ZD109A/110A` 10検査 | 部分 | 通常の小文字/大文字/数字・HIGH/LOW・空白補充、独自順序Z/A/9、変数/リテラル比較と生バイト不変を追加。集団内の異なる桁位置、SORT/MERGEとの一致は残る |
| D03 BINARY別名・桁幅 | `ZD103A`、`ZA101A`–`ZA103A` | 部分 | PIC 1/4/5/9/10/18各境界でCOMP/BINARY/COMP-4、VALUE/MOVE/算術の差 |
| D04 COMP-5物理幅 | `ZD108A` 39検査 | 追加 | 2/4/8バイト×符号有無、0/1/255/256/最大、符号付き-1/-256/最小を値と固定HEXで照合。幅を超える演算は別途必要 |
| D05 PACKED | `ZD101A` 18検査 | 部分 | 偶数桁の0/最大/最小、1桁、31桁全体のHEX、小数スケール境界、MOVEと算術の符号 |
| D06 DISPLAY符号 | `ZD102A` 16検査 | 部分 | 先頭/末尾×分離/非分離の0/正/負、全数字のオーバーパンチ、負ゼロの生バイトと比較 |
| D07 SYNC | `ZD104A` のCOMP 2/4/8 | 部分 | COMP-1/2/5/INDEX/POINTER、入れ子、OCCURSの要素間余白、LEFT/RIGHT。余白の未規定内容を固定しない |
| D08 ODO | `ZD105A` の長さ2/5・送出MOVE | 部分 | 下限/上限、0許容宣言、後続可変位置、表内/表外カウント、送受両側の長さ評価、入れ子ODO |
| D09 REDEFINES/RENAMES | `ZD104A` の単段上書き・THRU | 部分 | 多段、拡大型/縮小型、集団跨ぎ、書戻し方向、88条件名の単値/範囲/HEX |
| D10 編集PIC | `ZD106A` の通貨/符号/Z/*/CR/DB/BWZ | 部分 | 各編集の0/正/負/最大、逆編集、JUSTIFIEDの短い/等長/長い移送、スケールPの両方向 |

## ZA: 数値オプション・演算

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| A01 TRUNC(STD) | `ZA101A` | 部分 | 2/4/8バイト×符号、VALUE/MOVE/ADD/SUB/MUL/DIV/COMPUTE、PICTURE上限±1、DISPLAY出力 |
| A02 TRUNC(BIN) | `ZA103A`、`ZD108A` | 部分 | 物理幅の演算上限/下限±1、ON SIZE ERROR有無、COMP-5との同値性、送出先が数値/英数字の差 |
| A03 TRUNC(OPT) | `ZA102A` | 部分 | 有効PICTURE範囲の0/正負境界、複合式、格納幅跨ぎ。範囲外値の結果保証は要求しない |
| A04 ARITH(COMPAT) | `ZA104A` の18桁と複合式 | 部分 | 30桁中間結果の整数/小数配分3分岐、桁落ちする式/しない式、演算順序 |
| A05 ARITH(EXTEND) | `ZA110A` 6検査、`ZE131N/P` | 部分 | 31桁PACKED最大/負値・DISPLAY往復・30桁受信先エラー・中間乗除算は追加。19/30桁、NATIONAL/編集、31桁中間結果の配分各分岐は残る |
| A06 SIZE ERROR | `ZA105A`、`ZA109A` 19検査 | 部分 | 今回ADD/SUB/MUL/DIV/COMPUTE成功/エラー、受信先保持、正負丸め直前/一致/直後、余りの符号を追加。複数受信先、丸め桁上がり×型、余り側のみのエラーは残る |
| A07 NUMPROC/NUMCLS | `ZA111A`–`ZA115A` 228検査、`ZA106A` | 部分 | NOPFD/PFD×PRIM/ALTの4単位でPACKED/DISPLAY・符号有無×A–F/不正0/9、桁不正、符号なしゼロ・負ゼロ、MOVE/ADD後の優先符号を追加。MIGは4.2の優先符号対照4件。先頭符号/分離符号、MIG固有の非優先符号、NUMERIC後の入力不変は残る |
| A08 HFP | `ZA107A` のCOMP-1/2定数と演算 | 部分 | 基数16指数境界、丸め前後、最小/最大指数、10進との片方向変換、EXTENDの128bit中間精度、外部浮動小数点DISPLAY/NATIONAL |
| A09 組込み関数 | `ZA108A` のSQRT/SIN/COS/LOG10等 | 部分 | LOG/TAN/ASIN/ACOS/ATAN、ANNUITY/PRESENT-VALUE、べき乗の正/負/0指数、非自明引数、誤差許容と定義域の明記 |

## ZN: NATIONAL・DBCS・CCSID

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| N01 PIC G/DBCS | `ZN103A`、`ZN102B` | 部分 | 比較の長さ差、INSPECT、切詰め/空白補充、DBCS文字境界、SO/SIが要る混在領域との区別 |
| N02 NSYMBOL | `ZN101A`、`ZN102B` | 部分 | 同じPIC N宣言をNATIONAL/DBCSでコンパイルして対比、明示USAGEとの優先関係、版/CCSIDの固定 |
| N03 国別数値・編集 | `ZN102A` | 部分 | SIGN先頭/末尾、0/負/最大、四則各文、NUMERIC不正入力、逆編集、national-editedの部分参照 |
| N04 GROUP-USAGE | `ZN101A` | 部分 | 入れ子への継承、集団MOVE/比較、国別の数値子項目、部分参照、INITIALIZEの文字/数値別初期値 |
| N05 INITIALIZE分類 | `ZN103A`、`ZN105A`、`ZD107A` | 部分 | ALPHANUMERIC/NATIONAL/DBCS/EGCS/NATIONAL-EDITED/NUMERIC全カテゴリを同居させ、各REPLACINGで非対象が不変、FILLER/REDEFINES/OCCURS |
| N06 CCSID変換 | `ZN104A`、`ZN106M` は主に939 | 部分 | 037/930/939/1390/1399×SBCSのみ/DBCSのみ/混在/空白、片方向既知バイト、代替文字、変換不能。明示引数/コンパイル既定を分離 |
| N07 UTF-16境界 | `ZN107A` 10検査、`ZN105A` | 追加 | パディング/切詰め/LOW、先頭/末尾/省略長/書込み部分参照、初期化、サロゲート2単位と文字数/バイト数を追加。孤立サロゲートは別の仕様確認対象 |

## ZP: 呼出し・記憶域・可視性

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| P01 WS/LS/CANCEL | `ZP101A` の初回/2回目/CANCEL後 | 部分 | 入れ子LS、数値/文字/集団のVALUE、CANCEL対象の独立性、INITIALプログラムとの差 |
| P02 再帰 | `ZP102A` の階乗/Fibonacci | 部分 | 深さ0/1/複数、WS共有の直接観測、各フレームのLS/LINKAGE、再帰内PERFORM復帰、再帰呼出し失敗 |
| P03 RETURNING | `ZP103A` | 部分 | 77/集団、符号・長さ・型変換、RETURN-CODEと受信項目保持を成功/ON EXCEPTIONで個別確認 |
| P04 ENTRY | `ZP104A` とhelperの別名1件 | 部分 | 複数別名の交互呼出し、入口ごとの異なる引数個数/順序/BY VALUE、入口より前が実行されないこと |
| P05 CALL引数 | `ZP104A`、`ZP107A` 4検査 | 部分 | 今回0/負値/OMITTED→PRESENTの再束縛、CONTENT/VALUE/非対象領域保持を追加。同一項目を複数引数へ渡すalias、集団、別名入口との組合せは残る |
| P06 ポインター | `ZP105A`、`ZP108A` 7検査 | 部分 | 既存のPROCEDURE→FUNCTIONコピー後のNULL化に加え、データポインターのコピー、別名ENTRYの動的入口名、NULL化・別入口への再束縛後もコピーが有効、別バッファ保持を追加。ABI別サイズ、引数個数の異なる複数ENTRY、再帰フレーム別ポインターは残る |
| P07 可視性 | `ZP106A` のCOMMON/GLOBAL/EXTERNAL | 部分 | 非COMMON兄弟の拒否、同名の遮蔽、GLOBAL条件名/ファイル、EXTERNALの再呼出し/別プログラム/CANCEL後 |

## ZF: ファイル・VSAM・テープ

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| F01 QSAM F/V | `ZF101A`、`ZF106A` 12検査 | 部分 | 今回V最小/最大/最小+1、各I/OのSTATUS、EOF分岐、再OPEN先頭復帰を追加。Fの空/1件/満杯/最終不完全ブロック、RECORDS/CHARACTERS単位、V範囲外のSTATUSは残る |
| F02 QSAM S/U | `ZF102A`、`ZF201M` | 部分 | Sで分割直前/一致/直後、複数セグメント、最大長、Uのブロック境界、物理BDW/RDW/SDWの外部検査 |
| F03 AWO/RESERVE | `ZF105A` は共有領域の観測 | 部分 | AWO有無で物理充填差、可変長レコードの並び順と末尾ブロック、RESERVE 0/1/複数の意味を観測。構文受理だけで合格にしない |
| F04 CODE-SET | `ZF103A` のASCII片方向/往復 | 部分 | STANDARD-1/2、英数/記号/空白/LOW/HIGH、F/V、RDW非変換、符号分離数値 |
| F05 VSAM主キー | `ZF104A`、`ZF107A/108A` 48検査 | 部分 | KSDSの成功→主/代替キー重複→成功→未検出→成功、REWRITE拒否時のデータ保持、DELETE、EOF後復帰、再OPENと拡張6Bを追加。RRDSでの同じ遷移、SEQUENTIAL/RANDOMモード別、START不等号は残る |
| F06 VSAM代替キー・ESDS | `ZF107A/108A` のUNIQUE/重複許可AIX | 部分 | 代替キーREAD/START/NEXT、主キー順と異なる重複挿入順、WRITE/REWRITE/READの02と正常分岐、キー変更/削除後の索引更新・永続化を追加。ESDS、複数AIX、部分キーSTARTは残る |
| F07 属性不一致39 | `ZF102A`、`ZF109A` 6検査 | 部分 | 同じ標準ラベルテープのLRECL/RECFMを別FDで不一致にして39、正常FDで再OPENして内容不変を追加。DD/DCB属性、VSAMキー長/位置、ラベル不一致は残る |
| F08 SAME AREA | `ZF105A` のSAME RECORD AREA | 部分 | 複数FDの交互OPENと上書き、VSAM SAME AREA、異なる長さ、コンパイル負系との対応 |
| F09 テープラベル | `ZF201M` の逆順READ/モードU | 部分 | VOL1/HDR1/HDR2/EOF1/EOF2の実バイト・長さ・順序・ファイルマーク。読戻しだけでラベル仕様を保証しない |
| F10 テープ位置 | `ZF201M` のNO REWIND→REVERSED | 部分 | 同一巻に2論理ファイル、位置指定、REWINDとの差、EOF後、CLOSE REEL/UNIT FOR REMOVAL |
| F11 EOV/ユーザーラベル/RERUN | `ZF202M` が存在しない | 未 | 強制小容量で巻境界前/一致/後、次巻継続、BEFORE/AFTERラベル手続き順と内容、チェックポイント再開後の重複/欠落なし |

## ZS: 指示文・日時・システム

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| S01 COPY/REPLACEスコープ | `ZX103A`、`ZS105A` 7検査 | 部分 | 独立PROGRAM終端で解除、入れ子への識別子/リテラル置換継承、OFF、新REPLACEによる前ルール解除、単語一致・リテラル非置換、部分語空置換を追加。COPY REPLACINGとの交互作用、ALSO/LAST OFF、複数段入れ子は残る |
| S02 コンパイル指示文 | `ZX104A` | 部分 | CBL/PROCESSの同値性と後勝ち/矛盾規則、文途中EJECT/SKIP1/2/3/TITLE、継続行・72列境界、一覧制御の実際の効果 |
| S03 WHEN-COMPILED | `ZS101A` は区切りと年4桁だけ | 部分 | 16/21文字全フィールド、日付/時刻範囲、同一コンパイル単位で不変、再実行でも不変。現在時刻との厳密一致は要求しない |
| S04 世紀窓 | `ZS101A` は26/98年のみ | 部分 | 固定した窓末年に対する始年/末年/隣接年、00/99、日付/通日関数との同値性。実行年を使う場合は窓を固定する計算を明記 |
| S05 Gregorian境界 | `ZS104A` 8検査 | 追加 | epoch/翌日、1900/2000/2100世紀、2000閏日前後、9999年末を独立通算日・通日・逆変換で照合 |
| S06 PARM | `ZS103M` の固定文字列・2バイト長 | 部分 | 空/1/最大長、カンマ/空白/引用符、DDとPARMの独立、切詰め規則。exportしたJCLでも同じ値を受領 |
| S07 特殊レジスター | `ZS101A` のTALLY/RETURN-CODE | 部分 | CALL成功/失敗/RETURNINGによるRC保持、ジョブRCへの伝播、SORT成功/失敗時SORT-RETURN |
| S08 LE | `ZS102A` のCEEDAYS等 | 部分 | 日付下限/閏日/不正月日/書式不一致、feedback全体、失敗時出力保持、CEE3ABDの異常終了をプロセス/JCL結果で観測 |

## ZX: 5.x/6.x拡張

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| X01 EXIT/CYCLE | `ZX101A`、`ZX105A` 4検査 | 部分 | 今回多重ループで最内側への作用、TEST AFTER CYCLE、初回EXITを追加。VARYING AFTER繰上げ、out-of-line UNTIL EXIT、EXITとCYCLEの混在は残る |
| X02 SET FALSE | `ZX102A` の通常/添字付き条件名 | 部分 | 省略形、複数対象、SYMBOLIC CHARACTERS、NATIONAL、GLOBAL、範囲VALUEのFALSE設定と他条件名の非干渉 |
| X03 INITIALIZE | `ZX102A`、`ZX106A` 13検査 | 部分 | 通常/FILLER有無、ALL/ALPHANUMERIC TO VALUE、VALUE→REPLACING→DEFAULTの各分岐、JUSTIFIEDのVALUE配置とMOVEとの差、2次元全体/指定要素と隣接要素保持を追加。REDEFINES除外、国別/編集カテゴリのVALUE分岐は残る |
| X04 部分語置換 | `ZX103A`、`ZS105A`、`ZE128N/P`、`ZE133N/P` | 部分 | LEADING/TRAILING識別子とLEADINGの空置換、単語全体一致/非一致を追加。TRAILING空置換、PICTURE、COPY REPLACING別の部分語ケースは残る |
| X05 追加関数 | `ZX107A` 16検査、既存BYTE-LENGTH | 部分 | TRIM LEADING/TRAILING/BOTHの値/結果長・内部空白・全空白、TEST-NUMVAL/Cの正常/不正位置/通貨/小数点、ABSの正負0を追加。NATIONAL引数、通貨記号引数、DECIMAL-POINT COMMA、最大桁数は残る |

## ZE: 負系の独立性

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| E01 終端/空VALUE/REPLACE | `ZE101N/102N`、新規`ZE126N/P`・`ZE127N/P` | 部分 | END PROGRAMのピリオド欠落、入れ子の名前入替え、親終端のみ欠落。正常対照も必要 |
| E02 再帰/記憶域禁止 | `ZE109N/110N/111N`、新規`ZE129N/P` | 部分 | RECURSIVE+INITIAL/入れ子/USE FOR DEBUGGING、LS EXTERNALの正常対照 |
| E03 INITIALIZE/条件名 | `ZE103N`–`ZE108N` | 部分 | EGCS/DBCS別名重複、他の重複カテゴリ、条件名受信、NATIONAL FALSE不適合。既存負系の正常対照 |
| E04 NATIONAL/POINTER | `ZE114N`–`ZE116N` | 部分 | GROUP-USAGE下JUSTIFIED、他の禁止USAGE、符号位置、ポインターVALUE NULL正常対照 |
| E05 ファイル禁止句 | `ZE117N`–`ZE123N` | 部分 | LINE SEQUENTIAL+RESERVE、不正SAME AREA、属性不一致と構文違反の区別、各規則の正常対照 |
| E06 RETURNING/可視性 | `ZE112N/113N` | 部分 | USING重複、受信型不適合、非COMMON兄弟CALL、01/77以外の結果項目 |
| E07 指示文/桁数 | `ZE124N/125N`、新規`ZE128N/P`・`ZE130N/P`・`ZE131N/P`・`ZE133N/P` | 部分 | 未知オプション、矛盾オプション、境界桁数のUSAGE別、部分語の複数語・空語。COMPAT19桁/EXTEND32桁と正常対照は追加 |
| E08 UNTIL EXIT禁止形 | `ZE105N`、新規`ZE132N/P` | 部分 | TEST BEFORE/AFTER/WITH表記、VARYING禁止、単にEXIT未定義と診断する場合と専用診断を区別 |

## 実行基盤自体の検査

`coverage.json` はレビュー対象の固定されたプログラム/検査ID台帳。
`crates/zeus-ccvs/tests/zcvs.rs` が、ソースとの一致、モジュール/プログラムの
欠落なし、検査IDの重複なし、全IDの実レポート出現順、正確な件数、
削除/目視検査0、負系の期待診断、正常対照のコンパイル成功を検証する。
既存実行ファイルがある場合もCargoで鮮度を確認する。

この台帳は**実装済みシナリオの退行防止**であり、上表の不足を埋めたことを
証明するものではない。新規ケースを追加するときは、対応する要求ID・入力分割・
仕様の期待値をレビューし、台帳と本表の証拠/残件を一緒に更新する。

次の基盤シナリオは残件として追跡する:

- 空/途中で切れた/不整合なレポート、重複ID、誤集計、実行されないモジュール。
  台帳との照合は追加済みだが、ハーネスへこれらを注入する独立した負系が必要。
- 負系コンパイルのタイムアウト/シグナル/内部エラーと通常の診断終了の区別。
- コンパイル時と実行時のタイムアウト、異常RC、期待RC、標準出力のみの場合。
- manifestの参照先不足/未知プログラム/誤CCSID/無効JCL、jobs=1/複数での同値性。
- JCL exportのコンパイル/リンク/実行ステップ、負系の期待RC検査、
  SUBPROGRAM/ENTRY解決、CCSID/PARM/DD属性の保持を実機で検証する。

## 公開仕様から見直したテスト前提

- [ARITH](https://www.ibm.com/docs/en/cobol-zos/6.5.0?topic=options-arith):
  EXTENDで拡張される十進項目と、2/4/8バイトのBINARY/COMP-5を区別する。
  提案§4.2の「19–31桁BINARY/COMP-5」は十進項目と同じ要求にしない。
- [中間結果の精度](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=precision-fixed-point-data-intermediate-results):
  中間結果の整数/小数配分と桁落ちを計算してから最終受信先を判定する。
  `ZA110A` は31桁値の30桁受信先への代入でSIZE ERRORを検査する。
- [SIZE ERROR](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=operations-size-error-phrases):
  ROUNDEDの適用後に最終受信先の桁あふれを判定する。エラー有無だけでなく
  受信先保持とNOT ON SIZE ERRORの排他性を検査する。
- [NUMCLS](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-numcls):
  NOPFDで有効な設定と、他のNUMPROC設定の前提を分離する。
  `ZA111A`–`ZA114A` のCBL NUMCLSはZeusの設定選択方法であり、実機では
  対応する導入設定に揃えて当該指定を除く。PFD×PRIM/ALTの同じ期待値で
  NUMCLSがPFDへ作用しないことも検査する。`ZA115A`のMIGは4.2専用。
- [不正プログラム/データ](https://community.ibm.com/community/user/blogs/dan-zhang1/2020/04/02/pitfalls-of-invalid-programs-and-data):
  PFD前提に違反するデータの任意の実測結果をIBMの保証結果にしない。
- [国別の部分参照](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=reference-modification):
  位置・長さの単位と生バイト列の観測を分離する。
- [ファイル状態](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=operations-common-processing-facilities)、
  [代替キー](https://www.ibm.com/docs/en/cobol-zos/6.4?topic=section-alternate-record-key-clause):
  重複許可の代替キーWRITE/REWRITEでは02、READでは次の同一参照キーが
  ある場合だけ02。主キーREADと重複の最後のREADは00である。
  `ZF108A`はこの区別と、02がINVALID KEY/AT ENDではないことを検査する。
- [REPLACE](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=statements-replace-statement)、
  [INITIALIZE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statements-initialize-statement):
  置換の終端と入れ子継承、VALUE/REPLACING/DEFAULTの選択順を独立に検査する。
- [TRIM](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-trim)、
  [組込み関数一覧](https://www.ibm.com/docs/SSCH39_1.0.0/com.ibm.ent.cbl.zos.doc/lr/ref/rlinffun.html?view=kc):
  MOVE後の空白補充だけではTRIMの結果長を判定できないためLENGTHも比較する。
  TEST-NUMVAL/Cは不正文字位置と、文字が欠ける場合の入力長+1を区別する。

## 完了判定

この監査の結論は、**元の提案§4の網羅は未達**である。
今回、A07のオプション別検査、F06の代替索引、S01/X03のスコープ/初期化を追加した。
優先して閉じる残件はF06のESDS/複数AIX、F07のDD/VSAM属性、
F09–F11（ラベル/複数ファイル/複数巻）、A07のMIG固有規則と分離符号である。
全行の未検証分割に実行可能な証拠を付けるか、対象外の仕様根拠を示して初めて
提案の完了条件を評価する。プログラム数・PASS率だけでZ1–Z4完了を宣言しない。

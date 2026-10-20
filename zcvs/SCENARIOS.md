# ZCVS シナリオ網羅性の再監査

監査基準: [提案0019](../docs/proposal/0019-zos-cobol-test-suite.md) §4.1–4.8。
監査起点: `master` の `0f8d697`。プログラム名ではなく、ソース中の
比較式・分岐・観測対象を確認した。提案のプログラム番号と実装の番号は
一致しないため、以下では実際のファイル名を記す。

**全件PASSはシナリオ網羅を意味しない。** 起点の66プログラム・248検査は
すべて通るが、提案に列挙した機能の一部しか検証していない。
この変更後は158プログラム、1,476検査（実行1,431件、負系45件）、
正常コンパイル対照20本となる。これらはZeusのインタプリタとネイティブで
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
| D03 BINARY別名・桁幅 | `ZD103A`、`ZA116A/117A` 252検査 | 部分 | PIC 1/4/5/9/10/18×COMP/BINARY/COMP-4の0/±1/±最大、STDの桁切詰め、OPTの有効入力を追加。各VALUE初期値・全演算との直積は残る |
| D04 COMP-5物理幅 | `ZD108A`、`ZA118A` | 部分 | 2/4/8バイトの既知HEXに加え、BINの符号有無×上限/下限演算とSIZE ERROR保持を追加。COMP-5別名との差・エラー句なしは残る |
| D05 PACKED | `ZD101A`、`ZD111A` 104検査 | 部分 | 1/2/3/4/5/6/7/8/9/18/19/30/31桁×符号有無の0/1/最大/負値、偶数桁の余剰ニブル・全31桁HEXを追加。小数スケール境界、演算後の符号は残る |
| D06 DISPLAY符号 | `ZD102A`、`ZD112A` 80検査 | 部分 | 先頭/末尾×分離/非分離×0–9の正負を独立HEXで追加。MOVE後のゼロは優先正符号。負ゼロ入力の生バイト/比較は残る |
| D07 SYNC | `ZD104A`、`ZD114A/115A` 17検査 | 部分 | 表の要素長/末尾余白、2次元入れ子、非整列開始、COMP-1/2/5、PACKEDでSYNC無効を追加。INDEX/POINTER・LEFT/RIGHTは残る。余白内容は固定しない |
| D08 ODO | `ZD105A`、`ZD113A` 18検査 | 部分 | 0/1/2/4→1→0で長さ・送出MOVE・後続項目の可変位置を追加。表内カウント・送受両側評価・入れ子ODOは残る |
| D09 REDEFINES/RENAMES | `ZD104A` の単段上書き・THRU | 部分 | 多段、拡大型/縮小型、集団跨ぎ、書戻し方向、88条件名の単値/範囲/HEX |
| D10 編集PIC | `ZD106A` の通貨/符号/Z/*/CR/DB/BWZ | 部分 | 各編集の0/正/負/最大、逆編集、JUSTIFIEDの短い/等長/長い移送、スケールPの両方向 |

## ZA: 数値オプション・演算

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| A01 TRUNC(STD) | `ZA101A`、`ZA116A` 144検査 | 部分 | 3別名×PIC 1/4/5/9/10/18の有効境界、上限ADD、正負の桁超過MOVEを追加。各型のVALUE/SUB/MUL/DIV/COMPUTEは残る |
| A02 TRUNC(BIN) | `ZA103A`、`ZD108A`、`ZA118A` 18検査 | 部分 | 2/4/8バイト×符号有無の上限-1/上限/上限+1、符号付き下限で演算成功/エラー保持を追加。エラー句なし・送出先の型差は残る |
| A03 TRUNC(OPT) | `ZA102A`、`ZA117A` 108検査 | 部分 | 3別名×PIC 1/4/5/9/10/18の0/±1/±最大と有効演算を追加。複合式・格納幅跨ぎは残る。範囲外入力の結果保証は要求しない |
| A04 ARITH(COMPAT) | `ZA104A` の18桁と複合式 | 部分 | 30桁中間結果の整数/小数配分3分岐、桁落ちする式/しない式、演算順序 |
| A05 ARITH(EXTEND) | `ZA110A` 6検査、`ZE131N/P` | 部分 | 31桁PACKED最大/負値・DISPLAY往復・30桁受信先エラー・中間乗除算は追加。19/30桁、NATIONAL/編集、31桁中間結果の配分各分岐は残る |
| A06 SIZE ERROR | `ZA105A/109A`、`ZA119A` 11検査 | 部分 | ADD/SUB/MUL/DIV/COMPUTEの狭い/広い受信先を両順序で実行し、エラー側保持・正常側格納・分岐を追加。余り側だけの桁あふれも追加。丸め桁上がり×型は残る |
| A07 NUMPROC/NUMCLS | `ZA111A`–`ZA115A` 228検査、`ZA106A` | 部分 | NOPFD/PFD×PRIM/ALTの4単位でPACKED/DISPLAY・符号有無×A–F/不正0/9、桁不正、符号なしゼロ・負ゼロ、MOVE/ADD後の優先符号を追加。MIGは4.2の優先符号対照4件。先頭符号/分離符号、MIG固有の非優先符号、NUMERIC後の入力不変は残る |
| A08 HFP | `ZA107A`、`ZA120A` 21検査 | 部分 | 外部浮動小数点DISPLAY/NATIONALの0/正負/小さい値/指数変化、NUMERIC・独立文字列・十進復元を追加。HFP指数限界・丸め・128bit精度は残る |
| A09 組込み関数 | `ZA108A`、`ZA121A` 14検査 | 部分 | LOG/TAN/ASIN/ACOS/ATAN、非自明SIN/COS、ANNUITY/PRESENT-VALUE、正負/0/小数べき乗を追加。定義域内・絶対誤差1E-9を明記。関数ごとの全境界は残る |

## ZN: NATIONAL・DBCS・CCSID

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| N01 PIC G/DBCS | `ZN103A`、`ZN102B` | 部分 | 比較の長さ差、INSPECT、切詰め/空白補充、DBCS文字境界、SO/SIが要る混在領域との区別 |
| N02 NSYMBOL | `ZN101A`、`ZN102B` | 部分 | 同じPIC N宣言をNATIONAL/DBCSでコンパイルして対比、明示USAGEとの優先関係、版/CCSIDの固定 |
| N03 国別数値・編集 | `ZN102A` | 部分 | SIGN先頭/末尾、0/負/最大、四則各文、NUMERIC不正入力、逆編集、national-editedの部分参照 |
| N04 GROUP-USAGE | `ZN101A`、`ZN109A` 7検査 | 部分 | 国別集団の数値子項目、集団MOVE・既知UTF-16 bytes・LENGTH・部分参照・INITIALIZE分類を追加。深い入れ子と明示USAGE優先は残る |
| N05 INITIALIZE分類 | `ZN103A/105A`、`ZN108A` 6検査 | 部分 | 英数字/数値/国別/数値編集/国別編集/国別数値の同居で各REPLACINGと非対象保持、既定初期化を追加。DBCS/EGCS同居、各カテゴリ×FILLER/REDEFINES/OCCURSは残る |
| N06 CCSID変換 | `ZN104A/106M`、`ZN110A`–`ZN113A` 24検査 | 部分 | 930/939/1390/1399の別単位でSBCSのみ/DBCSのみ/混在を独立既知bytesで両方向検査。共有文字だけなのでコードページ固有文字・037・空白・代替/変換不能は残る。明示引数と既定を同一視しない |
| N07 UTF-16境界 | `ZN107A` 10検査、`ZN105A` | 追加 | パディング/切詰め/LOW、先頭/末尾/省略長/書込み部分参照、初期化、サロゲート2単位と文字数/バイト数を追加。孤立サロゲートは別の仕様確認対象 |

## ZP: 呼出し・記憶域・可視性

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| P01 WS/LS/CANCEL | `ZP101A`、`ZP109A` 12検査 | 部分 | 2プログラムのWS独立性、数値/文字LSの毎回VALUE復帰、CANCEL対象外保持とINITIAL毎回復帰を追加。入れ子LS・集団VALUE・EXTERNALの組合せは残る |
| P02 再帰 | `ZP102A`、`ZP112A` 10検査 | 部分 | 深さ0–8、累積WS呼出数、LS/LINKAGEのフレーム隔離、再帰内PERFORM復帰、CANCEL後再計算を追加。再帰呼出し失敗・ポインターのフレーム別観測は残る |
| P03 RETURNING | `ZP103A`、`ZP110A` 7検査 | 部分 | 集団RETURNINGの連続結果、77から01への同属性数値結果、成功/ON EXCEPTIONの受信先・RETURN-CODE保持を追加。型不一致は正常変換扱いしない。数値/文字/浮動属性別は残る |
| P04 ENTRY | `ZP104A`、`ZP111A` 10検査 | 部分 | 主入口/引数2個の別入口/BY VALUE別入口を交互実行。入口前のコード不実行、引数再束縛・参照更新・値引数保持・入口ポインターを追加。OMITTEDと入口の組合せは残る |
| P05 CALL引数 | `ZP104A`、`ZP107A` 4検査 | 部分 | 今回0/負値/OMITTED→PRESENTの再束縛、CONTENT/VALUE/非対象領域保持を追加。同一項目を複数引数へ渡すalias、集団、別名入口との組合せは残る |
| P06 ポインター | `ZP105A`、`ZP108A` 7検査 | 部分 | 既存のPROCEDURE→FUNCTIONコピー後のNULL化に加え、データポインターのコピー、別名ENTRYの動的入口名、NULL化・別入口への再束縛後もコピーが有効、別バッファ保持を追加。ABI別サイズ、引数個数の異なる複数ENTRY、再帰フレーム別ポインターは残る |
| P07 可視性 | `ZP106A` のCOMMON/GLOBAL/EXTERNAL | 部分 | 非COMMON兄弟の拒否、同名の遮蔽、GLOBAL条件名/ファイル、EXTERNALの再呼出し/別プログラム/CANCEL後 |

## ZF: ファイル・VSAM・テープ

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| F01 QSAM F/V | `ZF101A/106A`、`ZF113A/114A` 40検査 | 部分 | Fの0–5件（BLOCK 2 RECORDS）、再作成・EOF・EXTEND、閉状態/二重OPEN/誤モード/EOF連続READ/復帰を追加。物理充填の証明とV範囲外STATUSは残る |
| F02 QSAM S/U | `ZF102A`、`ZF201M` | 部分 | Sで分割直前/一致/直後、複数セグメント、最大長、Uのブロック境界、物理BDW/RDW/SDWの外部検査 |
| F03 AWO/RESERVE | `ZF105A` は共有領域の観測 | 部分 | AWO有無で物理充填差、可変長レコードの並び順と末尾ブロック、RESERVE 0/1/複数の意味を観測。構文受理だけで合格にしない |
| F04 CODE-SET | `ZF103A` のASCII片方向/往復 | 部分 | STANDARD-1/2、英数/記号/空白/LOW/HIGH、F/V、RDW非変換、符号分離数値 |
| F05 VSAM主キー | `ZF104A/107A/108A`、`ZF110A/111A` 49検査 | 部分 | RRDS欠番/重複/REWRITE/DELETE/再挿入/START/NEXT/EOF復帰、KSDS =/>/>= の一致/間隙/前後境界・部分キー・失敗後復帰を追加。ACCESSモード別の直積は残る |
| F06 VSAM代替キー・ESDS | `ZF107A/108A`、`ZF112A` 26検査 | 部分 | 重複許可/一意の2代替キー、書込み順の重複検索、主キーREAD00、更新拒否の原データ/旧索引保持、両キー変更・削除・永続化を追加。ESDSは未実装、代替キー側の部分STARTは残る |
| F07 属性不一致39 | `ZF102A`、`ZF109A` 6検査 | 部分 | 同じ標準ラベルテープのLRECL/RECFMを別FDで不一致にして39、正常FDで再OPENして内容不変を追加。DD/DCB属性、VSAMキー長/位置、ラベル不一致は残る |
| F08 SAME AREA | `ZF105A` のSAME RECORD AREA | 部分 | 複数FDの交互OPENと上書き、VSAM SAME AREA、異なる長さ、コンパイル負系との対応 |
| F09 テープラベル | `ZF201M/202M` とRust物理画像検査 | 部分 | 4巻全体で80-byte VOL1/HDR1/HDR2/EOV1/EOV2/EOF1/EOF2の順、HDR2 F/BLKSIZE/LRECL、区切りマーク、部分データ、巻別block数を外部検査。全ラベル属性・ユーザーラベルは残る |
| F10 テープ位置 | `ZF201M`、`ZF202M` 18検査 | 部分 | CLOSE REEL/UNIT FOR REMOVAL後もOPENを維持、空巻を跨ぐ自動継続/明示巻移動、再OPEN/最終UNIT後EOFを追加。同一巻に2論理ファイル・MULTIPLE FILE位置指定は残る |
| F11 EOV/ユーザーラベル/RERUN | `ZF202M` と4巻のAWS検査 | 部分 | CLOSE REEL/UNITで4巻（3巻目空）を明示生成し、不完全ブロックをフラッシュ・次巻継続を追加。容量上限による自動EOV・ユーザーラベル手続き・RERUN復帰は未検証 |

## ZS: 指示文・日時・システム

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| S01 COPY/REPLACEスコープ | `ZX103A`、`ZS105A` 7検査、`ZS106A`（リテラル/タグ置換） | 部分 | 独立PROGRAM終端で解除、入れ子への識別子/リテラル置換継承、OFF、新REPLACEによる前ルール解除、単語一致・リテラル非置換、部分語空置換を追加。COPY REPLACINGとの交互作用、ALSO/LAST OFF、複数段入れ子は残る |
| S02 コンパイル指示文 | `ZX104A`、`ZS106A`（バッチ内の単位別TRUNC） | 部分 | CBL/PROCESSの同値性と後勝ち/矛盾規則、文途中EJECT/SKIP1/2/3/TITLE、継続行・72列境界、一覧制御の実際の効果 |
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
| X05 追加関数 | `ZX107A`、`ZX108A/109A` 22検査 | 部分 | NATIONALのTRIM値/結果長、TEST-NUMVAL不正位置、NUMVAL正負、DECIMAL-POINT COMMAとNUMVAL-C桁区切りを追加。浮動結果はROUNDEDで小数2桁に格納。通貨記号引数・最大桁数は残る |

## ZE: 負系の独立性

| 要求ID | 現在の証拠 | 判定 | 追加すべき独立シナリオ |
|---|---|---|---|
| E01 終端/空VALUE/REPLACE | `ZE101N/102N`、新規`ZE126N/P`・`ZE127N/P` | 部分 | END PROGRAMのピリオド欠落、入れ子の名前入替え、親終端のみ欠落。正常対照も必要 |
| E02 再帰/記憶域禁止 | `ZE109N/110N/111N/129N/P`、`ZE145N/P` | 部分 | LOCAL-STORAGE EXTERNALの違反だけを除いた正常対照を追加。RECURSIVE+INITIAL/入れ子/USE FOR DEBUGGINGと対応対照は残る |
| E03 INITIALIZE/条件名 | `ZE103N`–`ZE108N` | 部分 | EGCS/DBCS別名重複、他の重複カテゴリ、条件名受信、NATIONAL FALSE不適合。既存負系の正常対照 |
| E04 NATIONAL/POINTER | `ZE114N`–`ZE116N`、`ZE134N/P`–`ZE143N/P` | 部分 | JUSTIFIED・BLANK WHEN ZERO・SIGN・手続ポインターPIC/VALUE・国別PIC・外部浮動VALUE/JUSTIFIED/USAGEを1規則ずつ拒否/受理で追加。GROUP-USAGE下JUSTIFIED・国別符号の対照は残る |
| E05 ファイル禁止句 | `ZE117N`–`ZE123N` | 部分 | LINE SEQUENTIAL+RESERVE、不正SAME AREA、属性不一致と構文違反の区別、各規則の正常対照 |
| E06 RETURNING/可視性 | `ZE112N/113N` | 部分 | USING重複、受信型不適合、非COMMON兄弟CALL、01/77以外の結果項目 |
| E07 指示文/桁数 | `ZE124N/125N/128N/P/130N/P/131N/P/133N/P`、`ZE144N/P` | 部分 | ODOの小数カウント拒否と整数カウント正常対照を追加。未知/矛盾オプション、部分語の複数語・空語、USAGE別の境界桁数は残る |
| E08 UNTIL EXIT禁止形 | `ZE105N`、新規`ZE132N/P` | 部分 | TEST BEFORE/AFTER/WITH表記、VARYING禁止、単にEXIT未定義と診断する場合と専用診断を区別 |

## 実行基盤自体の検査

`coverage.json` はレビュー対象の固定されたプログラム/検査ID台帳。
`crates/zeus-ccvs/tests/zcvs.rs` が、ソースとの一致、モジュール/プログラムの
欠落なし、検査IDの重複なし、全IDの実レポート出現順、正確な件数、
削除/目視検査0、負系の期待診断、正常対照のコンパイル成功を検証する。
既存実行ファイルがある場合もCargoで鮮度を確認する。
`ZF202M`の4巻は本番のAWSデコーダーを使わず独立に解析し、ヘッダーの
前ブロック長・flags・label長/順序・HDR2属性・EOV/EOF・ファイルマーク・
既知EBCDICデータ・巻別block数まで照合する。Rust物理検査は実行1,431件に含めない。

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

- [SYNCの余白規則](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=clause-slack-bytes-within-records):
  OCCURSの各要素末尾にも必要な余白を加え、次の要素で最大境界を維持する。
  PACKEDにSYNCを指定しても同じ整列効果は要求しない。
- [CLOSEのファイル種別ごとの作用](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=statement-effect-close-file-types):
  REEL/UNITではOPEN状態を維持する。出力切替・入力切替・最終巻を分ける。
- [標準テープラベル2](https://www.ibm.com/docs/en/zos/3.1.0?topic=format-standard-data-set-label-2-hdr2eov2eof2):
  HDR2/EOV2/EOF2のレコード形式・ブロック長・レコード長を、読戻しとは別に
  EBCDICの固定バイト列と比較する。
- [CALL RETURNING](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statements-call-statement):
  呼出側/呼出先の属性一致が必要。異なる桁数を自動変換の正常ケースにしない。
- [NUMVAL](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=functions-numval):
  NUMVALの浮動結果から小数2桁への格納にはROUNDEDを使い、二進/十六進近似の
  下位誤差と文字解析の誤りを混同しない。

## 完了判定

この監査の結論は、**元の提案§4の網羅は未達**である。
今回の追加は53本（実行29本766検査、負系12本、正常対照12本）と補助7本。
複数AIX・RRDS・部分START・複数巻・物理ラベル・入れ子SYNCの証拠を補った。
SYNCを含むOCCURSで末尾余白が不足する不具合も検出し、要素長を最大必要境界へ
丸める修正を行った。独立した入れ子/2-byte/4-byte/8-byte配置検査で退行を防ぐ。
優先して閉じる残件はF06のESDS、F07のDD/VSAM属性、F10の同一巻複数ファイル、
F11の自動容量EOV/ユーザーラベル/RERUN、A07のMIG固有規則と分離符号である。
全行の未検証分割に実行可能な証拠を付けるか、対象外の仕様根拠を示して初めて
提案の完了条件を評価する。プログラム数・PASS率だけでZ1–Z4完了を宣言しない。

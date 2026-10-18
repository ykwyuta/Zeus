# COBOL追加互換機能の実装状況

基準はEnterprise COBOL 4.2の公開仕様。0017の残件からXML、OO/Java、DATEPROCを除いた範囲を対象とする。
作業順序は、対象機能の実装と開発中の回帰テスト、その後にCI全体のエラー解消とする。
この文書は作業中の一覧であり、IBM実機との互換性認証ではない。

最新の残件再確認は[0019: 残件再監査](0019-cobol-compatibility-reaudit.md)を参照。この文書の後半は実装段階ごとの記録であり、SQLIMSやCI等の先行時点の未対応記述をそのまま現在の状態としない。

| 項目 | 状態 | 残る作業 |
|---|---|---|
| ARITH COMPAT/EXTEND | 実装・検証中 | 18/31桁の入力、30/31桁の中間結果、64/128ビットHFPを接続。複合演算・境界値の追加確認 |
| NUMPROC / NUMCLS | 実装・検証中 | PFD/NOPFD/MIGとPRIM/ALTの数値クラス判定を追加。不正データの結果はIBM仕様の定義範囲で扱う |
| 浮動小数点リテラル | 実装・検証中 | 小数点、仮数1～16桁、指数1～2桁とHFP範囲を検査 |
| 浮動小数点関数・小数指数 | 実装・検証中 | 多倍長のSQRT/LOG/LOG10/三角関数・べき乗・統計・金融関数を接続。EXTENDの30小数桁と、大きな近接値の分散を回帰確認。IBM HFP命令の境界丸めは追加確認が必要 |
| 整数関数・数値文字列・RANDOM | 実装・検証中 | INTEGER/PART・MOD/REM・数値のORD系を多倍長化。FACTORIALのARITH別範囲、NUMVAL系のHFP・NATIONAL・通貨・小数点、RANDOMの省略シードと開区間を追加 |
| 外部浮動小数点PICTURE | 実装・検証中 | DISPLAY/NATIONAL、実小数点/V、MOVE、演算、表示、数値クラス、VALUE等の禁止事項を追加 |
| ENTRY | 実装・検証中 | 別名、開始位置、別々の引数、ポインター、外部オブジェクト検索、重複検査を追加。BY VALUEのバイナリー・浮動小数点・文字表現を回帰確認。ABIはZeusの仮想アドレス空間での契約 |
| 入れ子の可視性 | 実装・検証中 | 非COMMONの兄弟・他の系統への呼出しを制限。再帰・外部名の回帰確認 |
| データ・文の意味制約 | 一部実装・残件あり | SAMEの宣言制約、SAME SORT AREAの使用中の排他、EXTERNAL/GLOBAL、複合ODO、結果項目の下位属性、JUSTIFIED/BLANK WHEN ZERO/SIGN等のカテゴリー制約を追加。他の意味制約は監査継続 |
| テープ複数巻・EOV | 実装・検証中 | SL/NLの巻集合、容量分割、F/Sの継続読取り、HDR/EOV/EOFの照合、巻境界イベントを追加。複数ファイルで容量を共有し、先行ファイルのREELラベルも保持。CLOSE REEL/UNITは部分ブロックを排出し、開いたまま巻を切り替える |
| ユーザーラベル | 実装・検証中 | FILEおよびREEL/UNITのBEGINNING/ENDING、共有80バイト領域、UHL/UTL、MORE-LABELSを追加。巻末UTLと次巻UHLを実際の物理巻に保存。同じ位置・属性の共通ラベル名をモード手続きから参照可能にし、重複手続きも診断。4.2の構文はAFTER |
| RERUN | 実装・検証中 | レコード周期、SL/NLの巻末、SORT入力収集後・並べ替え後の保存・再開を追加。CLOSE REEL直後の保存から書込み・後続ファイルを継続できる。Zeus独自のチェックポイント形式。外部トランザクション環境、対話コンソール、IBM/JCLのチェックポイントイメージとの相互運用は対象外で明示的に診断 |
| FD/DD/媒体属性 | 一部実装・残件あり | 長さ・形式・ラベル種別不一致のFILE STATUS 39、EXTERNALファイル間の属性照合を追加。全ラベル属性・媒体状態の照合は未完了 |
| コンパイラー制御 | 一部実装・残件あり | DEFINE/IF/EVALUATE、COPY内定義、無効分岐の字句解析抑止、APOST/QUOTE・AWO・BLOCK0・SSRANGE・CODEPAGE・DYNAM・OPTIMIZE・SQL、ARCH以外の定義済み変数、SOURCE/MAP/LIST/OFFSET/XREF/VBREFを実装。CLI/JCLの呼出し設定も反映。指令へのCOPY置換を抑止し、入れ子連鎖の重複REPLACINGを診断。条件付きコンパイルは6.x拡張。DLL/EXPORTALL・THREAD・LP(64)を追加。ARCH・SQLIMS、IBM固有リスト・診断の再現は残る |

参照資料: [IBM Enterprise COBOL 4.2 Language Reference](https://publibfp.dhe.ibm.com/epubs/pdf/igy3lr50.pdf)、
[Programming Guide](https://www.ibm.com/docs/en/SS6SG3_6.5/pdf/pgmvs.pdf)。
後者はARITH等の補足確認に使い、版ごとの差を4.2の仕様へ無条件に取り込まない。

追加回帰テストは `crates/zeus-cobol/tests/remaining_compatibility.rs`。
CI成功と機能実装完了を区別し、未完了の行を残したまま「全対応済み」と報告しない。

## 複合OCCURS DEPENDING ON

可変件数の表に続く項目の位置、可変長の要素を持つ表の要素間隔、包含するグループの長さを参照ごとに計算する。
複数のODOと、GLOBAL経由の参照にも適用する。件数の変更で既存データを自動的に移動する処理ではない。
可変位置にある項目のVALUEと既定値は初期件数に応じた位置へ設定し、グループ内にODO対象の件数項目があるMOVE受信側は最大長を使用する。
件数項目を可変位置または表の下位に置く定義は診断する。最大領域は引き続き静的に確保する。
現在このメタデータを持つプログラムはネイティブ生成を省略し、同じ実行モジュール内のインタープリターで実行する。

参照: [IBM Complex OCCURS DEPENDING ON](https://www.ibm.com/docs/en/cobol-zos/6.3?topic=tables-complex-occurs-depending)、
[位置・長さと件数項目の制約](https://www.ibm.com/docs/en/cobol-zos/6.3?topic=depending-example-complex-odo)。
資料のサンプルコードは転記せず、独自の回帰プログラムで検証している。

## 数値関数の追加確認

ABS、SUM、MAX/MIN、MEAN、MEDIAN、MIDRANGE、RANGE、VARIANCE、STANDARD-DEVIATION、ANNUITY、PRESENT-VALUEを多倍長数値処理へ接続した。
ALL引数の展開にはODOの現在件数を使う。分散では大きな値同士の差分をホストのf64へ変換せずに計算し、関数結果をARITHのHFP幅へ変換する。
ANNUITYの非負利率・正の整数期間、PRESENT-VALUEの-1より大きい利率も検査する。
参考: [ANNUITY](https://www.ibm.com/docs/en/cobol-zos/6.3?topic=functions-annuity)、
[VARIANCE](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-variance)、
[PRESENT-VALUE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=functions-present-value)。

HFP加減算では、16進1桁のガード桁より下のビットを桁合わせ時に捨てる処理をCOBOLとZeusのHLASM実行系で共有した。
COMPAT/EXTENDで微小な値を引く境界を回帰確認している。
根拠は [IBM z/Architecture Principles of Operation](https://www.ibm.com/docs/en/module_1678991624569/pdf/SA22-7832-14.pdf?cp=HW11W) のADD/SUBTRACT NORMALIZEDのガード桁仕様。
IBMコンパイラーの最適化命令選択や全数値境界の一致を確認済みという意味ではない。

NATIONAL/DBCSのJUSTIFIEDも、句の受理だけでなくMOVE時の右寄せ・左側切捨てを実行する。
初期VALUEには右寄せを適用せず、参照変更で受ける場合も通常の配置規則を使う。文字の2バイト境界を維持する。
現在、この属性を持つプログラムはインタープリターへフォールバックする。

## 未完了作業の順序

1. 残る媒体属性・ラベル照合と意味制約。
2. 数値境界・ABI・コンパイラー設定・意味制約の残件監査と実装。
3. 対象実装を揃えた後にworkspace全体のCIを実行して修正する。

コンパイラー制御ではOPTIMIZE、リスト出力、ARCH以外の定義済みIGY変数を追加した。未対応設定・IBM固有出力の境界は後述する。
CODEPAGEは37・1140・930・939・1390・1399に対応する。これらのEBCDICソースを行区切り／固定80バイト形式で直接入力できる。その他のCCSIDは残る。
RERUNのON file-name形式とDDの既定規則も追加確認対象。

この段階では未実装と追加確認が残るため、用途次第の機能以外がすべて実装済みという状態ではない。

## RERUNで保存する状態

公式の[RERUN句](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=section-rerun-clause)と
[チェックポイントと再始動](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=processing-interrupts-checkpointrestart)では、
指定したレコード周期または巻末で実行状態を保存し、その位置から再開する。
Zeusで実装する場合も、WORKING-STORAGEだけでなく、実行中のプログラムと命令位置、CALL/PERFORMの復帰先、
LINKAGE・LOCAL-STORAGE・EXTERNALの参照関係、数値一時値、ALTER、Report Writer、
開いているファイルの内容・読取り位置・更新状態、DDとテープ巻位置が保存対象になる。
上記の状態に加えて、ヒープ領域、条件ハンドラー、乱数、供給済みSYSINの未消費行も保存・復元する。
索引ファイルは削除によって空いた書込み順序番号も保持し、再開後の重複代替キーの位置を変えない。
外部トランザクションホスト・別言語ランタイム・HLASMを含む実行、対話コンソールを既に使用した実行は、保存できない状態があるため診断する。
標準出力へ既に出した内容は取り消さない。ファイル出力は保存したバッファ・更新状態から再開する。
Zeusの保存形式はJSON形式の版付き実行イメージであり、IBMのチェックポイントデータセット形式ではない。
複数保存時は版付きのZeusコンテナーにJSONイメージを格納する。

利用例:

```text
zcobc run --dd INFILE=input.dat --dd OUTFILE=output.dat --dd CKPT=step.ckpt main.zo
zcobc restart step.ckpt
```

ソースでは `RERUN ON CKPT EVERY 100 RECORDS OF F` または
`RERUN ON CKPT EVERY END OF UNIT OF F` を指定する。SORT用は `RERUN ON CKPT`。
SORTは入力収集と並べ替え、出力を別々のVM命令に分け、収集後・並べ替え後に保存する。
出力手続きで障害を起こした回帰テストでは、入力手続きを再実行せずに再開できることを確認した。
APIは `Machine::restart_checkpoint`、保存済みパスの取得は `Machine::checkpoint_files`。
RERUNを含むプログラム（SORT用はそのモジュール）は現在インタープリターで実行する。

チェックポイントDDの`DISP=MOD`は複数イメージを保持し、その他は最新イメージで置き換える。
APIでは`IoAttributes.checkpoint_multiple`を指定する。保存成功時はRETURN-CODEを0にし、その値を保存する。
`zcobc checkpoints step.ckpt`で番号を表示し、`zcobc restart --checkpoint 2 step.ckpt`で番号を選べる。
番号省略時は最新、従来の単独JSONは番号1。APIは`checkpoint_ids`と`restart_checkpoint_at`。
純粋なCOBOLのJCL実行では、未使用の別言語リンカーが設定済みでも保存できる。実際に別言語へ移行した実行は診断する。
根拠: [IBMのチェックポイント設定](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=checkpointrestart-setting-checkpoints)、
[DFSMS Checkpoint/Restart](https://publibz.boulder.ibm.com/epubs/pdf/dgt2c410.pdf)。識別番号とコンテナーはZeus独自。

## SSRANGEとNATIONAL文字列関数

`SSRANGE`/`SSR`、`NOSSRANGE`/`NOSSR`を実装した。既定はNOSSRANGE。
各添字の値ではなく計算後のアドレスを最大表領域と照合する。多次元表の個別の添字が範囲外でも、実効アドレスが表内なら参照できる。
可変長項目、ALL展開時のODO件数、参照変更の開始位置・長さも検査する。
GLOBALの表は定義元の領域・ODO件数で照合し、NATIONALの参照変更は2バイト単位で判定する。
チェックに失敗した場合は診断して停止する。SSRANGE付きプログラムはインタープリターへ切り替える。
4.2のCHECK(ON/OFF)相当として`Machine::set_range_checking`と`zcobc run --check-range ON|OFF`を用意し、状態をチェックポイントにも保存する。
後続版で追加されたSSRANGEサブオプションや、6.xのCHECK(OFF)では無効化できない動作は今回の4.2基準へ取り込まない。
根拠: [IBMのSSRANGE説明と版差](https://www.ibm.com/docs/en/SS6SG3_6.3.0/pdf/cg.pdf)、
[Language EnvironmentのCHECK](https://www.ibm.com/docs/en/zos/2.5.0?topic=ulero-check-cobol-only)。

UPPER-CASE、LOWER-CASE、REVERSE、TRIM、MAX/MINのNATIONAL結果はUTF-16の型を保持する。
大文字小文字変換はUnicode 17.0.0のUnicodeData.txtの単純変換欄を用い、文字列長を維持する。
REVERSEはサロゲートペアを分断しない。MAX/MINとORD-MAX/MINはNATIONALの空白補完・順序で比較し、混合カテゴリーを診断する。
ALLと固定添字を組み合わせた関数引数も各次元を保持し、現在のODO件数だけを展開する。
関数の引数数・数値カテゴリー・数値関数の参照変更・スカラー関数のALLを検査する。
根拠: [UPPER-CASE](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-upper-case)、
[REVERSE](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-reverse)、
[MAX](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-max)、
[TRIM](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-trim)、
[Unicode公式変換データ](https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt)。IBMとUnicodeの版が異なる場合の新規文字の変換差は残る。

OPEN REVERSEDはF形式かつ単一リールのファイルに制限し、複数巻へ継続するファイルは開く前に診断する。
別ファイルからなる巻集合のうち、対象ファイルが単一リールなら利用できる。SL/NLの両方を回帰確認した。
根拠: 4.2 Language ReferenceのOPEN、pp.407–408。

## CODEPAGE

`CODEPAGE(ccsid)`／`CP(ccsid)`をコンパイル単位へ反映する。APIでは`FrontOptions.codepage`で指定できる。
CCSID 1140は37のX'9F'をユーロへ変更した変換を追加した。対応は37・1140・930・939・1390・1399で、未対応CCSIDは診断する。
COPY・条件付きコンパイル・リテラルの初期値生成に指定を適用し、モジュールにCCSIDを保存する。
コンパイル中の指定をスレッド内のスコープで保持し、終了・エラー後には戻す。
実行時はCALL先のモジュールのCCSIDへ切り替え、呼出元へ戻す。NATIONAL-OF／DISPLAY-OFの省略CCSIDもこれに従う。
実行時の明示的な`--ccsid`、`ZEUS_CCSID`、`codepage::set_active`は従来どおり上書きとして扱い、チェックポイントにも保持する。
Zeusの既定37は変更しない。IBMの既定1140との差は明示指定で解消する。ソースの既定はUTF-8入力で、EBCDIC入力は後述の形式指定で利用できる。
根拠: [IBM CODEPAGE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-codepage)。

## 巻集合とCLOSE REEL

Zeusで巻を分割したときはSL/NLともに
`名前.aws.volumes.json` を併せて保存する。このファイルには版と各巻の最終ファイルが次巻へ継続するかを記録する。
自然なファイル終端で次巻へ進む場合と、同じファイルの継続巻の場合を区別し、論理ファイルを誤って連結しない。
既存のNL巻を明示的な `IoAttributes.volumes` のみで指定する場合は、途中巻の最終ファイルが次巻へ継続する設定として扱う。
巻パスが省略されている場合は、標準ラベルと同じ `.vol002.aws` 等の命名規則を使う。
NLについてもF/Sの巻末RERUNからの再開を回帰確認している。巻集合情報は媒体と一緒に保管する。

先行ファイルが使用した容量を後続ファイルの書込みに反映し、最初のブロックが入らない場合は新しい巻のセクション1から開始する。
先行ファイルの継続位置・REELラベルを保持して再保存するため、NO REWINDで後続ファイルを書いても失わない。
`CLOSE REEL/UNIT` は部分ブロックをその巻へ確定し、空巻も含めて巻を切り替え、ファイルは開いたままとする。
入力では現在の巻の未読レコードを飛ばし、次巻の読取りを可能にする。最終巻では切替を行わない。
`FOR REMOVAL` はファイルを閉じて以後のWRITEを禁止する指定として扱わない。
根拠: [IBM CLOSEのファイル種別ごとの動作](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=statement-effect-close-file-types)。

## 整数関数・数値文字列・コンパイラー設定の追加

INTEGER/INTEGER-PART、MOD/REM、数値のORD-MAX/MINは多倍長値を直接処理する。巨大なHFP値から固定桁の一時値への変換を挟まない。
FACTORIALは4.2のCOMPATでは0～28、EXTENDでは0～29の整数引数を検査する。
ORD系などのALL展開にもODOの現在件数を使用する。
NUMVAL/NUMVAL-Cは18/31桁を検査し、64/128ビットHFPの近似値を返す。
NATIONAL、通貨文字列の任意の長さと大文字小文字、DECIMAL-POINT IS COMMAを反映し、TEST-NUMVAL系も同じ解析器で文字位置を返す。
RANDOMは省略時の初回シード0、指定による列の再開始、非負整数引数、0と1を含まない値域を実装する。生成アルゴリズムはZeus独自でIBMと同じ列は保証しない。
大きなシードも従来のZeusの64ビット対応を維持する。IBMの「0～2,147,483,645で異なる列を保証する」という制約を、その範囲への剰余変換の指定とは解釈しない。先行変更の剰余変換は既存アプリケーションの生成データを変えていたため取り消し、大きなシードの再開始と従来列を回帰試験で確認する。
根拠は4.2 Language ReferenceのFACTORIAL pp.525–526、NUMVAL/NUMVAL-C pp.535–538、RANDOM pp.540–541、および[MOD](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=functions-mod)。

APOST/QUOTEはVALUE・MOVE・比較・文字定義のQUOTE/QUOTESへ反映し、モジュール保存後も選択を保持する。
AWOはQSAMのV形式の充填を切り替え、BLOCK0は明示ブロック指定とU形式を除いてシステム決定のブロック化を有効にする。
根拠: [オプション比較のAPOST/QUOTE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=appendixes-option-comparison)、[AWOの動作](https://www.ibm.com/docs/en/SS6SG3_6.4.0/pdf/perfguide.pdf)、[BLOCK0](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=options-block0)。
条件付き指令内の文字列にはCOPY置換を適用せず、DEFINEのみを含むEVALUATEの分岐も本文として扱う。
根拠: [条件付きコンパイルの処理順序](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=directives-conditional-compilation)、[入れ子のCOPY REPLACING制約](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statement-comparison-replacement-rules)。

## 今回の開発回帰確認

- COBOLとデータセット層の全開発テスト: EBCDICソース入力追加後の合計845件成功、1件は外部コーパスが必要なため既定で除外。
- `rerun` 11件: CALL/PERFORM/RETURNING・入力と出力・残SYSINの復元、SL/NLのF/S巻末再開、CLOSE REEL直後の書込み再開、SORT入力の再実行抑止、複数保存と番号選択、CODEPAGE上書きとSSRANGEの保持、DYNAMの取り消し済み領域とNODYNAMの結合の保持、無効宣言、DDパス衝突、別作業ディレクトリーでの再開、任意語省略と周期の重複検査。
- 最新の重点確認は `remaining_compatibility` 54件、`physical_recursive` 19件、`dynamic_calls` 8件、`rerun` 11件、`callinterface` 6件成功。`zeus-ds` 16件、HFPの共通化時には `hlasm_float` 82件も成功。
- `compiler_control` 9件: 定義済み変数とCOPY、設定優先順位、再定義拒否、各リスト内容・抑止、CLIの失敗時リスト、手続き・ファイルのXREF、SQL/NOSQLの切替、FLAGの閾値・埋込みと抑止時のRC保持を確認。
- JCL実行の15件も成功。DDのDISP=NEWとDISP=MODによる単独保存・履歴保存、空テープの新規割当、コンパイルPARMとSYSPRINTへの反映、警告付きSYSLINとRC=4、EBCDICのSYSIN/SYSLIBとUTF-8メンバーの併用を回帰確認。
- 実際の `zcobc restart` コマンドでも、保存したSORTを再開し、保存済み検索パスから未ロードの外部COBOLを読み込めることを確認。
- `zeus-native --test native` の24件も成功し、APOSTの実際のネイティブ生成と既存の単独実行ファイル経路、SSRANGEの診断保持、DYNAM/NODYNAMとCALLINTERFACEの状態の違い、OPTIMIZE 0/1/2、EBCDICのDBCSソースからの生成と実行を確認。CALLINTERFACEの手続きポインターも追加で重点確認。
- `cargo fmt --all` と `git diff --check` を実行。workspace全体のCI確認は残機能の実装後に行う。

## DYNAM、CANCEL、INITIAL

`DYNAM`/`DYN`、`NODYNAM`/`NODYN`を実装した。既定はNODYNAM。
NODYNAMのCALL literalとSET TO ENTRY literalについて、利用可能なCOBOLの結合先をプログラム開始前に確定する。
DYNAMのリテラル指定とCALL identifierは動的な対象として管理する。
動的に呼び出したCOBOLオブジェクトのCANCELでは、内部ファイルをUSE手続きなしで閉じ、内包状態を取り消し、主名・ENTRY別名の名前解決を解除する。
外部`.zo`の取り消しでは、同じオブジェクトに含まれる静的な依存先も取り消す。次の呼出しでは更新後の`.zo`を読み込む。
EXTERNALデータ・外部ファイルの領域は保持し、私有データ領域・ファイル管理スロットは後続ロードで再利用する。
取り消した対象への手続き／関数ポインターは無効化する。使う前にSET TO ENTRYし直す。
静的な結合で保持される対象、未呼出し、取り消し済みのCANCELは処理しない。活動中の呼出元・対象を取り消す場合は診断して停止する。
従来はNODYNAMのリテラルCALL先もCANCELで初期化していたが、IBMの既定に合わせて修正した。旧動作に依存するソースはDYNAMを明示して再コンパイルする。
INITIALの復帰時には、直接・間接に内包するプログラムの状態と内部ファイルも初期状態へ戻す。
これらの結合情報と再利用領域はRERUNにも保存する。更新された同名オブジェクトへ古いネイティブ命令列を適用しないよう、動的なCOBOLオブジェクトはインタープリターで実行する。
CICS指定およびEXEC CICSによる自動翻訳とのDYNAM併用を診断する。

暗黙CLOSEは通常のCLOSEと分け、CANCEL・INITIALの終了ではUSEラベル手続きを実行しない。暗黙CLOSEの書込み失敗は実行エラーとして伝える。
JCLで新規割当した0バイトの仮想テープは、最初のOPEN OUTPUTで初期化できる。SL/NLともに空媒体のOPEN INPUTはFILE STATUS 39とし、継続巻の検査は緩和しない。

ZeusはIBMのバインダー形式を再現するリンカーではない。実行前に見つからない静的依存先と別言語・HLASMの呼出しは、既存の実行時ローダー／ホストサービスへ解決を委ねる拡張を残す。
単一のソースモジュール内に埋め込まれたプログラムはホストファイルから再ロードせず、状態を取り消して再利用する。
同じ対象に静的・動的CALLを混在させるケースはIBMでも状態が保証されない。Zeusでは外側から静的に結合された対象を保持する。
CALLINTERFACEのSTATIC/DYNAMICと既定への復帰を追加した。DLLのエクスポート／サイドファイルとCALLINTERFACE DLLは追加対応対象として残す。
根拠: [IBM DYNAM](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=options-dynam)、
4.2 Language ReferenceのCANCEL（pp.335–336）、
[手続き・関数ポインターの結合](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=subprograms-using-procedure-function-pointers)、
[静的・動的呼出しの混在](https://www.ibm.com/docs/en/SS6SG3_6.5/pdf/pgmvs.pdf)。

## コンパイラー設定・定義済み変数・リスト

CLIの `--options TEXT`、APIの `FrontOptions.compiler_options`、JCLのIGYCRCTLのPARMを、CBL/PROCESSと同じ解析器へ接続した。
呼出し時の設定よりソース先頭のCBL/PROCESSを優先し、コンパイルごとに設定を初期化する。
以前JCLで読み飛ばしていた未対応PARMもエラーにするため、既存JCLに未対応指定があれば修正が必要。

条件付きコンパイルの次の9変数と、IGY接頭辞なしの旧名を定義する。

| 変数 | Zeusでの値 |
| --- | --- |
| IGY-CICS / IGY-DYNAM / IGY-SQL | 有効なコンパイラー設定の真偽値 |
| IGY-DLL / IGY-THREAD | 有効なDLL／THREAD設定。LP(64)では両設定はIBMの規則に従い無効化し、DLL対応自体は自動的に有効 |
| IGY-SQLIMS | 偽。NOSQLIMSを受理し、SQLIMSは未対応として診断 |
| IGY-LP | 32または64。Zeusの仮想COBOL ABI |
| IGY-OPTIMIZE | 有効な0、1、2 |
| IGY-COMPILER-VRM | Zeus自身のパッケージ版をVVRRMMで表現。現在の0.1.0は000100 |

これらの再定義、OFF、OVERRIDE、PARAMETERによる上書きを拒否する。ユーザー定義でIGY接頭辞を使うことも診断する。
IGY-ARCHは未定義とする。ZeusのVM／ホスト向け生成へIBM CPU世代の値を割り当てず、ARCH設定も未対応として診断する。
EXEC CICS/SQLの自動翻訳は従来のZeus拡張として残るが、変数は明示されたコンパイラー設定を表す。
NOSQLを明示した場合、活性分岐のEXEC SQLを拒否する。
根拠: [IBMの定義済み変数](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=compilation-predefined-variables)。

OPTIMIZE(0/1/2)、NOOPTIMIZE、OPTIMIZE(STD/FULL)を受理し、値をモジュールへ保存する。
Zeusのネイティブ生成では順にCraneliftのnone / speed_and_size / speedへ接続する。
インタープリターでは同じ命令を実行する。IBMの最適化パスや機械命令の一致を表す設定ではない。
根拠: [IBM OPTIMIZE](https://www.ibm.com/docs/en/cobol-zos/6.5.0?topic=options-optimize)。

リストはCLIの `zcobc compile --options 'MAP,LIST' --listing main.lst main.cbl`、
APIの `compile_file_with_listing`、JCLのSYSPRINTで取得できる。コンパイル失敗時も診断とRCを保存する。

| 設定 | 出力 |
| --- | --- |
| SOURCE / SOURCE(DEC/HEX) | 入力ファイルの行、およびCOPY/REPLACE後の活性トークン。行番号を10進／16進表示 |
| MAP | データの仮想配置・型・定義位置、動的位置・長さ、手続き・ENTRYの位置 |
| LIST | ZIR命令と初期データの16進表示 |
| OFFSET | ソース位置とZIR命令番号の対応 |
| XREF / XREF(FULL/SHORT) | 解決済みデータ参照、生成された分岐・PERFORMの手続き参照、ファイル操作、リテラルCALL/SET ENTRY。SHORTは未参照データを省略 |
| VBREF | 活性トークンにある文の動詞と参照位置 |

既定はSOURCEとXREF(FULL)。NOSOURCE、NOMAP、NOLIST、NOOFFSET、NOXREF、NOVBREFで抑止する。
LISTとOFFSETは同じ設定階層で併用できない。上位階層で一方を指定すると下位の他方を解除する。
COPYファイル一覧、診断、生成時刻、CCSID、OPTIMIZEも表示する。
IBMの機械命令アドレス・PPA・ESDやIBM製品のリスト書式・全診断を再現するものではない。
PCはZIR命令番号、データ位置はZeusの仮想バイト位置である。
根拠: [IBMコンパイラー設定](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=program-compiler-options)。

## I-O-CONTROLの追加確認

4.2 Language Reference pp.152–156の構文図と本文を再確認した。
SAME句は単一ファイルを許可し、AREA・FORを省略できる。SAME SORT/SORT-MERGEでも単一のSDを許可する。
MULTIPLE FILEのTAPE・CONTAINSも省略可能。ファイル名を保持し、同じプログラムで宣言されたQSAMのFDか検査する。
この句自体は公式仕様どおり配置を変更せず、物理配置はDDのLABELで制御する。
RERUNではON・EVERY・OFの任意性を反映した。レコード周期が異なっても、同じファイルへレコード周期を重複指定すると診断する。
同じファイルへレコード周期と巻末周期を一つずつ指定する組合せは許可する。
実ファイルへの書込み、任意語省略で保存したチェックポイントの再始動、無効ファイル指定を回帰確認した。
`ignored_clauses` 15件、`rerun` 11件が成功。

ON file-name形式からのチェックポイントDDの選択規則は引き続き確認対象であり、DD名や既定値を推測して実装しない。
追加の対応対象はARCH、SQLIMS、RERUN ON file-nameの既定規則、媒体属性・意味制約・数値／ABI境界の残件監査である。DLL・THREAD・LP(64)・54種類のCCSIDは下記の範囲を追加した。
用途次第として除外したXML、OO/Java、DATEPROCと区別して管理する。

## CALLINTERFACEと警告付きコンパイル

`>>CALLINTERFACE STATIC`／`DYNAMIC`と略記`>>CALLINT`を追加した。引数を省略すると有効なDYNAM/NODYNAM設定へ戻る。
PROCEDURE DIVISIONの文境界に独立した行で指定し、以後のCALL／SET TO ENTRYへ適用する。
指令をCOPY展開後の順序で処理し、COPY REPLACING・REPLACEによる変更から保護する。
無効な条件分岐内の指定は適用しない。別の最外プログラムから始まるコンパイル単位へ持ち越さない。
文の途中、COPY/REPLACEの内部、不正なサブオプションを診断する。
CALL identifierはSTATICを指定しても動的なままであり、CANCEL自体の意味は切り替えない。
結合情報は命令に保存するため、モジュール保存・RERUN・単独実行でも保持する。

同じコンパイル単位で同じリテラル呼出先にSTATIC/DYNAMICを混在させた場合は、最初のCALLの方式へ統一する。
Zeusではこの修復をWレベルの診断として保存し、CLIのcompile/check、リスト、JCLのIGYCRCTLへRC=4を伝える。
警告付きでも`.zo`／SYSLINを生成する。診断の型・位置・メッセージはモジュールにも保存し、旧形式のモジュールでは空とみなす。
IBMの診断番号や診断レベルの完全な一致を保証するものではない。
根拠: [CALLINTERFACE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=directives-callinterface)、
[呼出し規約が混在する場合の処理](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=application-using-dll-linkage-dynamic-calls-together)。

大域的なDYNAMオプションは引き続きCICSとの併用を診断するが、個別のCALLINTERFACE DYNAMICはCICS下でも使用できる。
根拠: [統合CICS変換のコンパイラー設定](https://www.ibm.com/docs/en/cobol-zos/6.3?topic=translator-compiler-options-integrated-cics)。
DLL指定はDLL機構の残件として明示的に診断する。

## FLAGの実効化

これまで受理するだけだったFLAGをリスト出力に接続した。
`FLAG(x)`は指定レベル以上の診断をリスト末尾へ出し、`FLAG(x,y)`はさらにy以上を該当ソース行の直後へ埋め込む。
I/W/E/S/U、略記F/NOF、NOFLAGに対応し、既定はFLAG(I,I)。yはx以上とし、明示的な2段階指定にはSOURCEを必要とする。
NOFLAGや表示下限によって成果物の有無・診断の保存・戻り値は変更しない。
CLIの標準エラーへの報告は保持する。コンパイラー設定のエラーはリストでも抑止しない。
COPY/REPLACE処理中の診断は末尾表示の対象だがソース行へ埋め込まず、Uレベルも埋め込まない。
JCLでは同じリストをSYSPRINTへ出力し、抑止したエラーを後段で重複表示しない。
根拠: [IBM FLAG](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=options-flag)。

重点テストは`callinterface` 6件、`compiler_control` 9件、`dynamic_calls` 8件、`rerun` 11件が成功。
呼出先の差替え前の静的結合、CANCEL後の再ロード、ポインター、COPYと置換、無効配置、混在時の方式統一とRC=4のCLI/JCL成果物、FLAGの表示抑止・埋込みを確認した。
全体の開発回帰結果は上記一覧に記載。DECLARATIVES境界の指令と、無関係な`>>`を含む疑似テキストも重点確認した。

## EBCDICソースとCOPYの直接入力

`FrontOptions.source_encoding`にUtf8（既定）／Ebcdic／Ebcdic80を追加し、CLIでは`--source-encoding utf8|ebcdic|ebcdic80`で指定する。
compile/check/inventoryで共通に適用する。EbcdicはNL（X'15'）・LF（X'25'）とホストのLF/CRLFで区切ったレコード、Ebcdic80は連結された固定80バイトレコードを表す。
SourceEncodingはZeus側の入力表現の選択であり、IBMの新しいコンパイラー設定として扱うものではない。

CODEPAGEまたはCPで、37・1140・930・939・1390・1399のSBCS／混在DBCSを復号する。
最初にCBL/PROCESSのCODEPAGEを確認し、選択されたCCSIDで全体を再読して、DEFINEの文字列等も正しく解析する。
DBCS文字・SO/SIの消費バイト数を保持したまま、1～6桁・7桁・8～72桁・73桁以降の各領域を分ける。
DBCS名、混在／DBCSリテラル、条件付きコンパイル、COPY REPLACING、継続行へ接続し、73桁以降の識別欄が文に混入しないようにした。
プログラム領域の不正なSO/SI、未対応の文字対応、途中で切れたDBCSをファイル名・行・元の桁位置付きで診断する。固定80バイト形式の端数も診断する。

外部COPYは既定で親ソースと同じ入力形式を使う。APIの`FrontOptions.copy_encodings`で、メンバーのパスごとに形式を上書きできる。
組込みCOPYは引き続き内部のUTF-8定義を使う。ソース一覧と埋込み診断は、入力時に保存した復号済みの内容を使用する。
UTF-8のBOMを受理し、従来のUTF-8不正列を黙って置換する処理は診断に変更した。

JCLのIGYCRCTLもデータセットのSYSIN／SYSLIBを固定レコードのまま渡し、事前のLatin-1変換で日本語や物理桁を失わないようにした。
インストリームとTEXTのSYSINはUTF-8として保持する。SYSLIBはDDのレコード形式を外して、UTF-8のホストファイルとEBCDICメンバーを個別に指定する。
SYSINの連結でTEXT／インストリームとバイナリーの入力表現が混在する場合は、文字コードを推測せず診断する。
COPYの異なる入力表現の混在は対応済みである。

重点テスト`source_input` 7件で全6CCSID、各改行、固定80バイト、継続、COPY・日本語名・Gリテラル、組込みCOPY、BOM、誤った入力、CLIと復号済みリストを確認した。
JCLではEBCDICのSYSIN、インストリームのSYSIN、CBLとPARMのCODEPAGE、UTF-8/EBCDICの混在SYSLIBを通してコンパイル・結合・実行を確認した。
EBCDICソースから生成した単独実行ファイルも、ネイティブ生成を含めて日本語の出力が解釈実行と一致した。
既存のCCSID 37のLatin-1コンソール表現と、他の対応CCSIDのUTF-8コンソール表現は変更していない。

根拠: [IBM CODEPAGE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-codepage)、
[参照形式](https://www.ibm.com/docs/en/cobol-zos/6.5.0?topic=structure-reference-format)、
[文字の規則](https://www.ibm.com/docs/en/cobol-zos/6.4?topic=structure-characters)、
[継続行](https://www.ibm.com/docs/en/cobol-zos/6.5.0?topic=b-continuation-lines)。

これらはZeus内の回帰結果であり、IBM実機の実行結果との一致を示すものではない。

## DLL・THREAD・LP(64)・追加CCSID（2026-10-18）

- DLL/NODLL、EXPORTALL/NOEXPORTALL（EXP/NOEXP）、CALLINTERFACE DLLを実装。DLLとDYNAMの矛盾を診断し、IGY-DLLに有効値を反映する。
- EXPORTALLのPROGRAM-IDとENTRYをZeusの側ファイル`ZEUS-DLL-1`として保存する。CLIのsave_moduleとJCLのバインダーが生成し、SYSDEFSDへの出力にも対応。ロード時は実際のモジュールの公開属性も検査する。未公開名はCALLのON EXCEPTIONまたは実行エラーになる。
- DLLの公開ENTRY間は同じWSを共有し、結合情報でCANCELによる解放を防ぐ。DLL経由でロードしたコードは別のネイティブコードと取り違えない。SET TO ENTRYで選んだ方式もポインターとチェックポイントへ保存する。
- THREAD/NOTHREADとIGY-THREADを実装。RECURSIVE必須を検査する。ThreadedRunUnitは複数ホストスレッドの要求を所有スレッドで直列化し、一つのMachineのWS・EXTERNAL・ファイルを共有する。LOCAL-STORAGEは毎回初期化する。STOP RUN・未処理の実行エラーは実行単位を終了する。
- RECURSIVEのRETURN-CODE・TALLY・SORT-RETURNとLINKAGE表の索引を呼出しごとに分離した。WS表の索引は共有し、LOCAL-STORAGE表の索引は通常の非再帰呼出しも含めて呼出しごとに分離する。新しい領域はチェックポイントにも保存される。これらを持つプログラムの直接機械語生成は省略し、単独実行ファイルのVMで実行する。
- LP(32/64)とIGY-LP、POINTER-32を追加。LP(64)ではPOINTER／FUNCTION-POINTER／INDEXと索引名を8バイトにし、PROCEDURE-POINTERは従来通り8バイト。LENGTH OFの暗黙の9/18桁BINARYと、ADDRESS OF・LENGTH OFの引数格納幅も反映する。エントリー識別子を64ビットへ拡張し、DLL結合のLP不一致を診断する。LP(64)は直接機械語生成を省略する。IBMのELF／GOFF／XPLINKやホストCの関数記述子を実装したという意味ではない。
- 既存6CCSIDに48種類を追加し、計54種類へ拡張した。IBMのCODEPAGE表の全11混在CCSID（日本語・韓国語・簡体中国語・繁体中国語）と、欧州・各国のSBCSを含む。全一覧と固定版UCMはcodepages/README.mdとSOURCESに記載する。変換表とRustの一覧を同じマニフェストから生成する。追加の変換ライブラリは実行時に不要。
- CALL/CANCELで許される名前のカテゴリーを検査する。バイナリー・パック・NATIONAL・編集・データポインター等を名前として受理せず、CALLのエントリーポインターを別扱いする。SET TO ENTRYのalphanumericグループ名も受理する。呼出先名の復号にプログラムのCCSIDを適用する。
- 標準テープのブロック数を下位6桁と上位4桁で読み書きする。上位の先頭空白を扱い、100万以上のブロックで下位値を999999へ切り詰めない。IBMが検査しない予約領域などを一律に比較する変更は行わない。

根拠: [DLL](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-dll)、
[EXPORTALL](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-exportall)、
[CALLINTERFACE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=directives-callinterface)、
[DLLと動的CALL](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=application-using-dll-linkage-dynamic-calls-together)、
[THREAD](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=multithreading-choosing-thread-support)、
[特殊レジスター](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=literals-special-registers)、
[LP(64)のポインター](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=programs-pointer-data-items-amode-64)、
[CODEPAGE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-codepage)、
[テープラベル1](https://www.ibm.com/docs/en/zos/3.1.0?topic=format-standard-data-set-label-1-hdr1eov1eof1)。

開発回帰ではcallinterface 12件、compiler_control 10件、dynamic_calls 8件、source_input 9件、LP(64) 3件、THREAD 5件、remaining_compatibility 54件を確認した。JCLのDLL結合とSYSDEFSD、単独実行ファイルのDLL呼出しも確認した。
全体CIは実装完了後に行う。関連パッケージの回帰で、既存のzeus-nativeの直接実行検査がNumericContextでフォールバックを検出する残件も判明している。

### 現在の未完了一覧

| ID | 残件 | 完了条件 |
|---|---|---|
| K01 | ARCHとIGY-ARCH：移植用設定として完了 | ユーザー合意によりARCH(6～15)の受理と条件付きコンパイルへの反映を対応範囲とする。IBM Zの命令生成は対象外。省略時の未指定値0等の契約は[0019](0019-cobol-compatibility-reaudit.md)を参照 |
| K02 | SQLIMS | IMS SQLの翻訳、IMSデータ・メタデータへの問い合わせ／更新、状態と診断の実装。EXEC SQLをDb2へ転送する代用で済ませない |
| K03 | RERUN ON file-nameとDDの既定 | 4.2の構文図とProgramming Guideからチェックポイントの割当規則を確定し、媒体保存・再開まで検証 |
| K04 | FD/DD/ラベル・媒体状態 | IBMが実際に照合する属性を列挙し、未照合の項目を実装。予約・未使用領域まで過剰検査しない |
| K05 | データ・文・数値・ABIの監査残件 | 0017/0018の未検証項目を具体的な合法／不合法条件と境界試験へ落とし、未実装を解消 |
| K06 | CI | 上記完了後、fmt・全workspace clippy/testsと実行できるCI統合シナリオを検証し、全エラーを解消 |

以下の追加節は段階ごとの記録であり、後続の記録を現在の状態として優先する。2026-10-18の区切りでは、テープのLBI・VSEラベル入力・保持期限・ホスト認可、ASSIGN接頭辞、動的PATHと既存カタログDSNまで実装した。K01～K06全体は完了していない。

XML、OO/Java、DATEPROCは従来合意した対象外のまま。それ以外の残件を完了扱いへ変更していない。


DLLの結合監査では、JCLバインダーがCOBOLのSYSLIN連結の先頭だけを保存していた不足も修正した。複数オブジェクトを同じプログラムオブジェクトへ結合し、非公開の内部名も相互に呼べるようにした。入れ子の親番号、GLOBALデータとファイル所有者の番号、ソース位置を再配置する。各単位のCCSIDとWHEN-COMPILED日時を維持し、公開名の重複は診断する。混在した異種のオブジェクトを黙って捨てる処理は行わない。


SQLIMS監査では、従来のパーサーがEXEC SQLIMSをEXEC SQLとして扱い、Db2へ渡してしまう誤動作を発見した。IMS SQL機能の実装完了まで、DATA／PROCEDURE両DIVISIONで明示的に診断するよう修正した。これはSQLIMSの実装完了を示すものではなく、K02は未完了のままである。


CALLのBY VALUE監査では、文字リテラルをUTF-8バイト数で検査していた点をCCSID変換後の格納幅へ修正した。1バイトの16進リテラル、DISPLAY／NATIONALの1文字の参照変更、計算した長さ、長さ省略の末尾スライス、SYMBOLIC CHARACTERS、SHIFT-IN／SHIFT-OUTも接続した。計算した参照変更の長さが実行時に1文字でなければCALLの前に診断する。SHIFT-IN／SHIFT-OUTは読み取り専用で、再定義や受信先としての利用を拒否する。


開発回帰の直列実行 `cargo test -p zeus-cobol -p zeus-ds -p zeus-jcl` は **903 passed / 1 ignored / 0 failed**。optional corpusを除く。関連する単独実行ファイル25件も成功した。これは全workspaceのCI完了を意味しない。最後に追加したLENGTH OFのBY CONTENT/BY VALUE幅もLP(32/64)の重点試験で成功。


ThreadedRunUnitのfinish()は開いたファイルとカバレッジの保存を待ってから戻る。STOP RUNと実行エラー時も、終了処理の後にホストへ応答する。終了を共有するフラグで終了後の呼出しを拒否し、繰り返しのfinishを安全に扱う。GOBACKで出力を開いたまま返るプログラムを使って、finish後に出力が保存されていることを検証した。


CODE-SETの監査では、[IBMのQSAM ASCII仕様](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=files-processing-qsam-ascii-tape)からCODEPAGEとASCII/EBCDIC変換が独立していることを確認した。CCSID 37以外でのCODE-SETを一律に拒否していた制約を解消し、既存のASCII/37ファイル変換表を独立して使う。全54CCSIDでRDWを保持したASCIIレコードの書込み・読取りを確認した。ignored_clausesの16件が成功。

## SORT制御と媒体属性の追加対応（2026-10-18）

SORT-CORE-SIZE、SORT-FILE-SIZE、SORT-MODE-SIZE、SORT-MESSAGE、SORT-CONTROLを追加した。数値項目は仕様の桁数に応じたBINARY、DD名は8バイトDISPLAYとし、初期値・GLOBAL属性・再帰呼出し別の格納域を反映する。入れ子では最外部プログラムのレジスターを参照する。

- SORT開始時にレジスターと制御DDを読み、実行中の設定を確定する。設定はチェックポイントにも保存する。
- 制御DDのOPTION MAINSIZE、RESINV、FILSZ、MSGDDN、およびSMSを対応するレジスターより優先し、上書きの診断を選択されたメッセージDDに出す。テキストDDとEBCDICの固定／可変レコードDDを読む。EBCDICカードではラベル、注記、カンマによる継続、72桁目によるオペランド／注記の継続、73～80桁の識別欄を処理する。未対応の制御キーワードは診断し、読み飛ばさない。
- 正のCORE-SIZEは並べ替え作業領域の予算、負値は既定予算からの予約領域として扱う。ゼロとMAX相当の値にはZeusの既定64MiBを使う。ホスト全体の空きメモリーやIBMリージョンサイズを表す値ではない。
- 予算を超えると安定ソートしたランを一時ファイルへ退避し、二分マージする。ファイルと読み出し先頭レコードの数が入力件数に比例しないよう、マージ段階ごとに統合する。データセットと最終結果の全レコードは従来のVM内モデルに保持するため、SORT全体のメモリー上限をCORE-SIZEで保証するという意味ではない。
- FILE-SIZEは件数の見積りとして確保量に用い、実件数の上限にはしない。制御DDのFILSZ=nは正確な件数を検査し、不一致ではICE047A相当のメッセージとSORT-RETURN 16で終了する。FILSZ=En/Unは見積りとして扱う。MAINSIZEとRESINVにはK/M単位も指定でき、RESINVはMAX指定時に適用する。MODE-SIZEを可変長レコードの作業量見積りに使う。テーブルSORTには適用しない。
- SORT-RETURNは開始時にゼロへ戻す。入力／出力手続きが16を設定した場合、次のRELEASE／RETURNで手続きを抜け、SORTの後へ制御を戻す。未実行のGIVINGを行わない。
- OPTION CKPT／CHKPTからSORTCKPTへのZeusチェックポイントを作成できる。並べ替え後の保存から入力手続きを繰り返さずに再開する。IBM DFSORTの内部作業ファイルやチェックポイント画像との相互運用を提供するものではない。
- レジスターへの設定にはWレベル診断を出す。

根拠: [SORT制御](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=files-controlling-sort-behavior)、[特殊レジスター](https://www.ibm.com/docs/en/SS6SG3_6.2.0/pdf/lrmvs.pdf)、[DFSORT OPTION](https://www.ibm.com/docs/en/zos/3.2.0?topic=statements-option-control-statement)。

### 媒体属性の監査

| 対象 | 今回の対応・照合 |
|---|---|
| データセット識別子 | 出力は外部データセット名の末尾17文字。JCLの解決済みDSNを保持し、入力時に明示された名前と照合 |
| データセットシーケンス | 1～9999のEBCDIC数字と、10000～65535の拡張バイナリー形式を読書き。範囲外を診断。最初のHDR1が1より大きい番号を持つ媒体では、その値を基準に位置決めする。要求が1の場合は最初のデータセットを読む |
| U形式のレコード長 | HDR2／EOF2にはゼロを書き、入力でもゼロと照合。実ブロック長を保持 |
| ブロック長のマージ | FD/DDで未指定またはゼロならHDR2から補う。読取り・追記・巻境界処理・保存／再開で、確定した物理形式を保持 |
| ブロック属性 | VとVS/VBS、F、Uで許されるHDR2の属性を照合 |
| 巻シリアル | 明示された巻集合のVOL1と照合。不一致はFILE STATUS 39 |
| 巻集合の識別・順序 | HDR1の初巻シリアルと巻連番の不整合はIEC709I相当の警告。データセット名・シーケンス・形式・ブロック数の不一致はエラー |
| 世代・版・予約領域・所有者等 | IBMが入力時に検査しない領域を無条件に一致検査しない。非GDGの世代・版は空白で出力 |

根拠: [VOL1](https://www.ibm.com/docs/en/zos/3.1.0?topic=format-standard-volume-label-vol1)、[ラベル1](https://www.ibm.com/docs/en/zos/3.2.0?topic=format-standard-data-set-label-1-hdr1eov1eof1)、[ラベル2](https://www.ibm.com/docs/en/zos/3.2.0?topic=format-standard-data-set-label-2-hdr2eov2eof2)。後年のDFSMS仕様は拡張仕様として扱う。

[コンパイラー上限表](https://www.ibm.com/docs/en/cobol-zos/6.5.0?topic=guide-compiler-limit-comparison)に基づき、RESERVE 255、RERUN周期16777215、RERUN句32767、SELECT名65535、SAME RECORD AREAの255ファイルという上限も検査する。

SORTキーがSDに属すること、OCCURS／可変配置や不許可のカテゴリーを持たないことを検査する。RELEASE／RETURNはSDと実行中の入力／出力段階を照合し、THREADでのファイルSORT/MERGEは診断する。HFPキーの比較は多倍長の数値経路を使い、1E60等が狭い数値表現へ落ちないようにした。NATIONALキーにはalphanumericの照合順序を適用しない。MERGEはUSINGごとの順序を検査し、不整列またはOPEN失敗ではSORT-RETURN 16で終了する。USINGにも通常のOPEN/CLOSEと媒体属性の検査を適用する。

LP(64)の追加監査では、CICSオプションを有効値から除外し、EXEC CICS、ALTER、対象名を省いたGO TOを診断する。省略GO TOが専用命令へ変換される経路でも検査する。根拠: [AMODE 64の制約](https://www.ibm.com/docs/en/cobol-zos/6.4?topic=programs-using-compiler-options-compile-amode-64)、[SORT](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statements-sort-statement)、[RELEASE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statements-release-statement)、[DFSORTカード規則](https://www.ibm.com/docs/en/zos/3.1.0?topic=statements-general-coding-rules)。

関連3パッケージの開発回帰は923 passed / 1 ignored / 0 failed。その後の重点回帰ではsort_controls 11件、physical_recursive 25件、rerun 11件、LP(64) 4件、JCL jobs 17件が成功。データセット単体18件と単独実行ファイル25件も成功。全体CIは実装完了後に行い、K01～K06の残件をこの追加対応だけで完了とはしない。

K04の追加実装でLBI／BLKSZLIMに対応した。32760バイトを超えるテープブロック、V形式の拡張BDW、65535バイトを超えるAWSブロックの分割セグメント出力、HDR2/EOF2/EOV2の10桁ブロック長を扱う。BLOCK CONTAINS 0とBLKSZLIMでは、仮想テープの自動ブロック長を上限256 KiBとし、Fではレコード長の倍数へ切り下げる。明示したBLKSIZEや既存HDR2の値を優先する。非テープの大ブロックは状態90、OPEN I-Oは37で拒否し、スパン形式に大ブロックを許可しない。JCLのBLKSIZEはK/M、BLKSZLIMはK/M/Gを解釈し、不正な数値やBUFNOの範囲外を黙って無視しない。データセット単体20件、媒体関連26件、JCL単体14件が成功した。保護・有効期限のある媒体、VSEでHDR2を持たない入力は引き続き残件である。根拠: [ブロックサイズ](https://www.ibm.com/docs/en/cobol-zos/6.3?topic=cobol-setting-block-sizes)、[拡張BDW](https://www.ibm.com/docs/en/zos/2.5.0?topic=records-block-descriptor-word-bdw)、[JCL ReferenceのBLKSIZE／BLKSZLIM](https://www.ibm.com/docs/en/SSLTBW_3.1.0/pdf/ieab600_v3r1.pdf)。

[4.2 Programming Guide](https://publibfp.dhe.ibm.com/epubs/pdf/igy3pg50.pdf)の231ページと625～627ページも取得して確認した。SORTのIBM側の手順は、SORTCKPT DD、RERUN ON assignment-name、制御DDのOPTION CKPT／CHKPTの組合せである。この組合せからの保存・再開もZeusの重点試験で成功した。RERUN単独／OPTION単独でのZeusの保存は、IBM DFSORTの手順を再現したという意味ではない。通常のチェックポイントDDはassignment-nameのddname部分に対応し、入力データセットにチェックポイントを埋め込む方式ではない。図のfile-name分岐から特定の既定DD名へ割り当てる規則は確認できておらず、K03を未完了のまま維持する。ASSIGN／RERUNのlabel-・S-等を含む割当名の分解と、環境変数によるPATH／DSN動的割当も、既存の直接DD／パス指定とは区別して監査・実装する必要がある。

SQLIMSの仕様監査では、COBOLからの利用はBMP／IFP／MPR等の対応リージョンでの動的SQLであり、通常のDBバッチやCICSからの利用、およびSQL COMMIT／ROLLBACKはIBMでも対応しないことを確認した。実装時には、IMSカタログのメタデータ、同じPSBのDL/Iと共有するトランザクション、SQLIMSCA／SQLIMSDA、単一の有効カーソルという制約を接続する。SQLIMSをDb2へ転送する代用は行わない。根拠: [IMS SQLの制約](https://www.ibm.com/docs/en/ims/15.5.0?topic=sql-considerations-restrictions-cobol)、[SQL文一覧](https://www.ibm.com/docs/en/ims/15.5.0?topic=reference-sql-statements)。

SQLIMSの通信領域については、[IMS 15.1 Application Programming APIs](https://public.dhe.ibm.com/software/IMS/documentation/ims15pdfs/IMS_15_Application_Programming_APIs.pdf)の公開されたフィールド表を参照する。SQLIMSCAは224バイトで、SQLIMSCODEは4バイト、SQLIMSERRMCは158バイト、SQLIMSERRDは6個の整数、SQLIMSSTATEは5バイト。SQLIMSDAは16バイトのヘッダーと44バイトずつのSQLIMSVARからなり、同梱宣言の上限は750個。これらは実装に使う仕様の確認結果であり、IBMの同梱コピー句やサンプルコードを転用しない。LP(64)でのSQLIMSはIBMでも未対応なので、31ビットの領域を64ビット対応としてそのまま公開しない。

### 割当名・VSE入力・テープの保持と認可（2026-10-18追加）

ASSIGN／RERUNの`label-`、`S-`、`AS-`接頭辞から末尾のDD名を取り出し、DD割当とI/O属性の双方へ接続した。チェックポイントDDも同じ規則で解決し、データDDとの重複を接頭辞の違いで回避できないようにした。既存のZeusの直接ホストパスと、割当名全体への明示的なエイリアス指定は拡張として維持する。RERUNの接頭辞付きDDによる保存・再開を含む12件、データセット単体21件が成功した。環境変数のPATH／DSN割当とRERUN ON file-nameの既定先は別の残件である。根拠: [ASSIGN](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=section-assign-clause)。

HDR2を持たないVSE形式の標準ラベルテープを受理する。FD／DDの形式と、AWSの実ブロック境界で復号し、ラベル2を架空に補わない。出力は引き続きラベル1と2を生成する。

EXPDT（yyddd／yyyy/ddd）とRETPD（0～93000日）をJCLから標準ラベルへ反映する。うるう年と2155年上限を検査し、RETPDの計算結果は2155年末で上限に達する。99365／99366は永久保持として扱う。INPUTでは有効期限を照合せず、OUTPUTで既存の保持期限を確認する。保護コード1は読み書き、3は書込みで認可を要求し、継続巻のHDR1も確認する。保護された書込み先の名前も検査する。

Zeusのホストは`Machine::set_tape_authorizer`でオペレーター／パスワード検証サービスを接続できる。認可が必要で接続がない場合はOPENを状態37で拒否し、既存媒体を変更しない。要求には媒体パス、データセット名、読書き区分、保護レベル、期限内かを渡す。これはIBMのPASSWORDデータセットやRACF自体を内蔵するという意味ではない。ホスト側が実際の本人確認と許可判定を担う。根拠: [ラベル1の有効期限と保護](https://www.ibm.com/docs/en/zos/3.2.0?topic=format-standard-data-set-label-1-hdr1eov1eof1)、[ラベル2とVSE](https://www.ibm.com/docs/en/zos/3.2.0?topic=format-standard-data-set-label-2-hdr2eov2eof2)、[JCLのEXPDT／RETPD](https://www.ibm.com/docs/en/SSLTBW_3.1.0/pdf/ieab600_v3r1.pdf)。

### 動的割当の実装範囲（2026-10-18）

DD未割当時は、ASSIGNのname部分に対応する環境変数をOPENのたびに評価する。`PATH(絶対パス)`、および既存カタログの`DSN(完全名)`とOLD／SHRを解決する。PDSメンバーとカタログのレコード属性も使用する。JCLではそのジョブのカタログ、単独実行では`$ZEUS_HOME/catalog`（既定`./zeus/catalog`）を使用し、組込みAPIの`set_dynamic_catalog`で変更できる。

DD割当は環境変数より優先する。不正な値、存在しないDSN、未対応の割当オプション、使用中のファイルの再割当は状態98になる。動的割当のマッピングは実行単位終了時に解除する。チェックポイントでは現在の割当を保持しつつ、将来のOPENを固定DDへ書き換えない。`set_file_environment`は実行単位内だけの環境値を設定するAPIで、ホストプロセスの環境を共有する複数実行単位でも使える。

DSNのNEW／MOD、SPACE／UNIT／VOL／SMSクラス等の割当属性、割当解除時のDELETE／CATALOG等、およびRERUNの環境変数割当は残件である。これらを構文だけ受理して読み飛ばす処理にはしていない。根拠: [ASSIGN](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=section-assign-clause)、[割当規則と状態98](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=files-allocating)。

### 区切りの検証結果（2026-10-18）

`cargo test -p zeus-cobol -p zeus-ds -p zeus-jcl`を直列実行し、75ターゲット合計 **940 passed / 0 failed / 1 ignored**。ignoredは任意コーパス。`cargo fmt --all --check`と`git diff --check`も成功した。この区切りは全workspaceのCI完了やK01～K06の実装完了を意味しない。全体CIと既知のzeus-native NumericContext検査の修正は、合意した順序に従い残件実装後に行う。

### リモート試験基盤との統合

先行する`e4e7ba8`のZCVS試験基盤を取り込んだ。統合時の差異は試験の前提を修正して解消した。CANCELで再初期化を確認するZP101AはDYNAMを明示し、ZPS04AはBY VALUEの仮引数を宣言する。ENTRY試験はPROGRAM-IDと別の入口名を用いる。NUMVAL／NUMVAL-Cは浮動小数点近似値を返すため、十進小数4桁を照合するZA108AではROUNDEDを明示する。未実装を前提としていた負例は、不完全なRERUN、未定義ラベル、無効なARITH値を検査するように整理した。ARITH(EXTEND)の有効指定をエラー扱いしない。

現行CLIを構築後、`cargo test -p zeus-ccvs`は **24 passed / 0 failed**。ZCVSの66プログラムを実行する統合試験、ソース桁数、JCL出力の検証を含む。全workspaceのCI完了を示すものではない。

根拠: [動的CALLとCANCEL](https://www.ibm.com/docs/en/SS6SG3_6.5/pdf/pgmvs.pdf)、[CALLのBY VALUE対応](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statements-call-statement)、[NUMVAL系の戻り値と精度](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=functions-converting-numbers-numval-numval-c-numval-f)。

### 動的DSNの作成・追記・解除と終了処理（2026-10-18追加）

動的DSNのNEW／MODを順次データセットへ接続した。NEWは既存データを置換せず、新規ファイルを排他的に作成する。MODは存在しなければ作成し、OUTPUTで既存レコードへ追記する。JCLのDISP=MODにも同じOUTPUT追記処理を適用した。既存PDSのメンバーは`DSN(データセット(メンバー))`と従来の外側括弧形式で割り当てられる。PDS自体の新規作成とVSAMのNEW／MODは許可しない。

KEEP／DELETE／CATALOG／UNCATALOGは、環境値の変更による再割当または実行単位の終了時に実行する。CATALOGはFD由来のレコード属性を保存し、UNCATALOGはデータを保持してカタログ項目だけを外す。PDSメンバーのDELETEでは親PDSを削除しない。同じ環境値の再OPENは既存割当を再利用する。使用中の割当では、解除や削除を行う前に変更を拒否する。Zeus側で省略時に選ぶ割当状態はOLD、解除時の処理はKEEPであり、IBM側のすべてのOS既定値との一致を示すものではない。

RERUNのチェックポイント名も環境変数のPATH／既存DSNで解決できるようにした。保存・再開後の終了処理でも開いた出力と動的割当を確定する。通常終了時のCLOSE／カタログ操作のエラーは実行結果へ返し、ホスト駆動の実行単位では`Machine::try_finish`で確認できる。ThreadedRunUnitのfinishとSTOP RUNも終了時のエラーを返す。従来の戻り値を持たないMachine::finishは互換APIとして維持する。

重点回帰ではCOBOL単体78件、媒体34件、RERUN13件、THREAD6件、データセット26件、JCL単体15件、JCLジョブ18件が成功した。SPACE／UNIT／VOL／SMSクラス等の動的割当属性とRERUN ON file-nameの既定先は引き続き残件で、無条件に対応済みとはしない。

根拠: [QSAMの環境変数割当とPDS作成の制約](https://www.ibm.com/docs/en/SS6SG3_6.5/pdf/pgmvs.pdf)、[OUTPUTとDISP=MODによる拡張](https://www.ibm.com/docs/en/zos/3.1.0?topic=volumes-multiple-volume-considerations-sequential-data-sets)。

この追加実装後の関連3パッケージ回帰（`cargo test -p zeus-cobol -p zeus-ds -p zeus-jcl`）は **948 passed / 0 failed / 1 ignored**。全workspace CIは残件実装後に行う。

### SQLIMSの実行経路と動的SQL（2026-10-18追加、K02進行中）

EXEC SQLIMSを独立した命令種別として翻訳し、IMSの既存DL/Iセッションへ接続した。SQLIMS／NOSQLIMSとIGY-SQLIMSを反映し、LP(64)ではSQLIMSを無効にする。JCLのBMPと既存のメッセージ処理経路で有効化し、DLI／DBBではエラーを返す。Db2には転送しない。

実装した範囲は次のとおり。

- PREPARE、EXECUTE、EXECUTE IMMEDIATE、DECLARE CURSOR、OPEN、FETCH、CLOSE、DESCRIBE [OUTPUT]、PREPARE INTO。
- SELECT／INSERT／UPDATE／DELETE、比較述語とAND／OR、同じPCB・階層パスのテーブル参照、GROUP BY／ORDER BY、COUNT／SUM／AVG／MIN／MAX。AVGはIBM仕様に従い整数なら小数を捨て、DECIMALなら引数と同じ小数桁数を用いる。
- SQLIMSCAの224バイト、SQLIMSDAの16バイトのヘッダーと44バイトの記述子。公開のフィールド表から独自生成した宣言をINCLUDEで提供する。DESCRIBEはレコードの読み取りや更新を行わず、SQLIMSN不足は+239を返す。FETCH INTO DESCRIPTORはZeusの32ビット仮想ポインターを用いる。
- 文字・整数・パック／ゾーン10進、ホスト構造体、VARCHAR、標識配列、NULL・文字切り詰めの通知。WHENEVERはソース上の指定位置以降の命令へ分岐先を付ける。
- PCB・セグメント・フィールドのEXTERNALNAME、仮想外部キーの検索と子INSERTの親特定。仮想外部キーのUPDATEはSQLIMSSTATE 23504とする。DFSMARSHの組み込み10進変換と文字コードを参照する。Javaのカスタム変換器は自動実行しない。
- DL/Iと同じ未確定変更を読み書きし、SYNC／CHKP／ROLBで同じ作業単位を確定・取り消す。SQL文の失敗ではその文の変更だけを取り消す。DL/Iの検索位置は変更しない。カーソルは同時に1個とし、同期点で閉じる。

K02はまだ完了扱いにしない。日時型とDFSMARSHのPATTERN、フィールド単位の感応性・メタデータ制約、宣言と修飾・述語の意味制約、診断の網羅を引き続き監査・実装する。SQLIMSCODEは0／100／239、-122／-199の仕様を反映し、数値の対応を確認できていないエラーは暫定的にZeusの-1と公開仕様のSQLIMSSTATEを返す。IBMの全数値コードとの一致は主張しない。IFPの専用スケジューラーも未追加であり、I/O PCBの存在だけでSQLを許可しない。

構文解析5件とSQLIMS統合16件を確認した。統合試験にはCOBOLソースからの実行、DL/Iとの未確定データ共有、子INSERT、文の途中での失敗、記述子、WHENEVER、同期点、警告・NULLを含む。途中時点の関連3パッケージ回帰（COBOL／IMS／JCL）は980件成功。その後のメタデータ拡張については追加の関連回帰を実施する。ユーザー指定の順序に従い、SQLIMS対応後に全体CIのエラーを解消する。

根拠: [IMS Application Programming APIs 第8章](https://public.dhe.ibm.com/software/IMS/documentation/ims15pdfs/IMS_15_Application_Programming_APIs.pdf)、[COBOL SQLの制約](https://www.ibm.com/docs/en/ims/15.5.0?topic=sql-considerations-restrictions-cobol)、[COBOLの集計関数](https://www.ibm.com/docs/en/ims/15.4.0?topic=programs-sql-aggregate-functions-supported-cobol)、[SQLIMSSTATE](https://www.ibm.com/docs/en/ims/15.4.0?topic=codes-sqlimsstate-values-common-error)、[型指定とDFSMARSH](https://www.ibm.com/docs/en/ims/15.4.0?topic=utilities-specifying-data-types-application-programs)。

### SQLIMSの日時・バイナリー・公式診断の追加（2026-10-18）

DATE／TIME／TIMESTAMPをSQLの文字表現と物理フィールドへ変換する。DFSMARSHのLONGでは1970-01-01からのミリ秒、CHARではPATTERNを用いる。西暦1～9999年、負のエポック、世紀のうるう年規則を検査する。CHARのパターンはyyyy、MM、dd、HH、mm、ss、1～9桁のSと引用リテラルに対応し、未知の指定は診断する。LONGではミリ秒未満を切り捨て、CHARではマイクロ秒まで保持する。固定バイナリー列はUSING、比較、FETCHで生バイトのまま転送する。

IMSのMessages and Codes Volume 4で確認できた数値コードを追加した。例として未準備のOPEN／DESCRIBE／EXECUTEはそれぞれ-514／-516／-518、未知のPCBは-204、パラメーター数不一致は-313、感応性は-350、外部キーは-530／-531、階層・フィールド・述語制約は-8010～-8025を用いる。重複キーや不許可操作はPCB状態のSQLエラー-8004／58030とし、Db2のコードを代用しない。公開一覧で数値を確認できなかった条件は引き続きZeusの-1であり、診断全体の一致は未保証。

末尾のセミコロン以降は実行せず、PREPAREの+98／01568を返す。UPDATE／DELETEで該当行がなければ+100／02000、INSERTでシーケンスキーが指定されなければ-407／23502を返す。WHEREの整数に対する連続AND上限、DECIMALのOR制約、セグメントをまたぐOR、同じセグメントの述語が連続しない指定を検査する。型の異なるフィールドが重なる位置への書き込みは-8023／23613で拒否する。PREPAREのメタデータ検査はデータを更新しない。

DATA DIVISIONの実行SQL、対応しないカーソル宣言、DECLARE SECTIONの位置・対応、SQLIMSCAの長さをコンパイル時に検査する。K02の残件は、SENFLD・SENSEGの詳細設定を含むフィールド単位の感応性、宣言・名前・メタデータ制約の残りの監査、未確認の数値診断等。K02と全体CIをこの追加だけで完了とはしない。

重点試験はSQLIMS統合21件、コンパイラー制御11件、日時変換2件・SQL解析5件が成功した。

根拠: [IMS Messages and Codes Volume 4 第17章](https://public.dhe.ibm.com/software/IMS/documentation/ims15pdfs/IMS_15_Messages_and_Codes_Volume_4_IMS_Component_Codes.pdf)、[DFSMARSHの日時・数値変換](https://www.ibm.com/docs/en/ims/15.4.0?topic=utilities-specifying-data-types-application-programs)。

### SQLIMSの感応性・宣言順序・数値受け渡しとCIへの移行（2026-10-18）

SENFLDのフィールド選択、STARTによるDL/I入出力の並べ替え、REPLACE=NOを実装した。REPLは保持中レコードを基に感応フィールドだけを戻し、非感応フィールドと変更不可フィールドを保持する。SQLも同じPSBのフィールド選択・置換制約を適用する。SENSEGのPROCOPTでPCBの処理権限を制限でき、Kは入出力領域へデータを移さず、SSAのないGNで読み飛ばす。SENFLDの範囲・重なり、挿入時に必要なキー、255フィールド／SENSEG・10000フィールド／PSBを検査する。MSDB／DEDB、GSAM、キーのみのSENSEGにSENFLDを指定する等、対象外の組み合わせは拒否する。

カーソルは同じプログラム内で使用前に宣言し、同じstatement-nameを別カーソルへ重複指定できない。ホスト変数の予約接頭辞、SQLIMSCAの配置・長さ、DECLARATIVESでの使用を検査する。SQLコメント内のEND-EXECやホスト名を解釈せず、文字リテラル内の空白を保持する。同期点ではカーソルと準備状態を破棄する。出力先なしのFETCHと、文書に記載されたINTO DESCRIPTOR／USING DESCRIPTORを扱う。

二進整数へのFETCHは、IBMのSQLIMS仕様どおりTRUNC(BIN)相当で記憶域の全幅を使う。COMP-1／COMP-2のホスト変数は、IEEE doubleを経由せずにHFPと10進数を変換する。型に合わない数値表現を診断し、SUMの中間オーバーフローを検出する。ここまでの重点試験はSQLIMS統合26件、コンパイラー制御12件、HFPの下位ビット検査1件が成功した。

先行段階の「SENFLD・宣言の残件」はこの追加で対応した。公開コード一覧で数値を確認できない条件、対応PATTERN以外の書式、IFP専用スケジューラー等、Zeusの既存IMS環境との境界は残る。z/OS上の全IMS機能との同一性やK01～K06全体の完了は主張しない。ユーザーの最新指定に従い、このSQLIMS追加実装を区切りとして全体CIのエラー解消へ進んだ。

CIの静的検査で、動的割当の初期化、物理ブロックの戻り値型、条件式、POINTER-32のレイアウト表示の漏れを修正した。ネイティブ生成試験は整数の精度設定を算術フォールバックと誤認していたため、decimals=0かつfloating=falseの設定命令のみを記録処理として認めた。算術・MOVEや他の精度設定のフォールバックを拒否する検査と、生成後の全データ値の検査は維持する。通常の `cargo clippy --all-targets -- -D warnings` が成功し、全テストとGitHub Actionsの確認を進めている。

根拠: [SENSEGの処理オプション](https://www.ibm.com/docs/en/ims/15.4.0?topic=statements-senseg-statement)、[SENFLDのレイアウト・置換指定](https://www.ibm.com/docs/sr/SS9NWR_3.3.0/com.ibm.ims.explorer33.doc/wb_senfld.htm)、[COBOL SQLの宣言・名前・二進整数への代入規則](https://www.ibm.com/docs/en/ims/15.5.0?topic=programs-sql-statements-in-cobol)。

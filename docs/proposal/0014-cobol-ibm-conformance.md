# Proposal 0014: IBM Enterprise COBOL 構文適合性の監査

- 状態: **C01〜C11・X01〜X05 対応完了・回帰検証済み**。IBM 方言全体の準拠監査は継続。
- 確認日: 2026-10-16
- 監査開始時点: `bf4ee09688e89a4d2df92ff76004a1da06d7b535`
- 対象: `crates/zeus-cobol` のソース読込、前処理、AST、意味解析、ZIR、VM。
- 基準: [0003](0003-cobol-compiler.md) と同じ Enterprise COBOL **4.2**。
  追加実装した 5.x/6.x 系の構文は拡張として別に記載する。

## 参照した IBM 公式資料

以下の文書の目次と該当節を、実装と照合した。ページ番号は冊子のページであり、
PDF ビューアのページ番号とは異なる。本文の転載・仕様書の同梱は行わない。

| ID | 資料 | 使用箇所 |
|---|---|---|
| R42 | [Enterprise COBOL for z/OS 4.2 Language Reference](https://publibfp.dhe.ibm.com/epubs/pdf/igy3lr50.pdf), SC23-8528-01, Second Edition, August 2009 | 全体の章・文・句・関数の索引、4.2 基準の照合 |
| R64 | [Enterprise COBOL for z/OS 6.4 Language Reference](https://publibfp.dhe.ibm.com/epubs/pdf/igy6lr40.pdf), SC27-8713-03, 28 June 2024 update | EXIT pp.343–345、INITIALIZE pp.350–352、PERFORM pp.414–423、COPY pp.688–695、REPLACE pp.700–702 |
| LIB | [Enterprise COBOL documentation library](https://www.ibm.com/support/pages/enterprise-cobol-zos-documentation-library) | 4.2/6.4 の版と公開 PDF の確認 |
| LS | [LOCAL-STORAGE SECTION](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=overview-local-storage-section) | 呼出しごとの VALUE 初期化、EXTERNAL 制限 |
| EP | [End program markers](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=area-end-program-end-class-end-method-markers) / [Program structure](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=structure-cobol-program) | 終端名と区切りピリオド |
| SF | [SET for condition-names](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statement-format-4-set-condition-names), [APAR PI97160](https://www.ibm.com/support/pages/apar/PI97160) | SET TO FALSE と対応する VALUE 句。4.2 との差分 |
| I61 | [Changes in Version 6 Release 1](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=sccc-changes-in-enterprise-cobol-zos-version-6-release-1) | INITIALIZE の FILLER/VALUE/DEFAULT 拡張 |
| CP | [COPY comparison/replacement rules](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statement-comparison-replacement-rules) / [REPLACE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statements-replace-statement) | 部分語置換とその制約 |
| PE | [PERFORM with UNTIL phrase](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=statement-perform-until-phrase) | UNTIL EXIT と TEST/VARYING の併用制限、out-of-line 形式の終了 |
| IN | [INITIALIZE statement](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=statements-initialize-statement) | 新しい句でも適用される受信項目の制約 |

R42 は公式配布元から取得して本文を抽出した。取得 PDF の SHA-256:
`0be04068649dc68474e6bef1daab873993835beefcecbbc83ded10d0508acf86`。
参照用キャッシュは `.cache/cobol-specs/`。ネットワークを使わず実行できる
回帰テストには、独自に記述した小さな COBOL プログラムを使用する。

## 判定方法と結論

「キーワードを読み取れる」「コンパイルできる」「実行結果が一致する」を区別する。
単に読み飛ばす句や実行時エラーになる文は、実行まで対応済みとは数えない。

| 表記 | 意味 |
|---|---|
| 実装 | AST・コード生成・実行経路がある。全形式・制約の適合保証ではない |
| 修正 | 今回、差分を修正し専用の回帰テストを追加した |
| 部分 | 解析のみ、一部の形式・意味が欠落、または具体的制限を確認した |
| 未対応 | 該当する文法・実行モデルがない |
| 未検証 | 実装はあるが、IBM 固有の全制約や実機結果は確認していない |

COBOL-85 の手続き文とデータ配置には広い実装がある。一方、IBM 4.2 全体に対しては
XML、OO/Java、DATEPROC、特殊なデータカテゴリ、コンパイラオプション等が不足する。
したがって **「Enterprise COBOL 4.2 に完全準拠」「6.4 に完全準拠」とは判定できない**。
文の数だけで合格率を算出すると未対応の形式や意味を隠すため、準拠率は算出しない。

既存の CCVS85 合格実績は [0003](0003-cobol-compiler.md#進捗-2026-10) の記録を参照する。
今回の単体・統合テストの結果と、以前の CCVS85 実績を混同しない。
IBM コンパイラによる実行比較は未実施であり、今回は公式仕様から期待結果を定義した検証である。

## 4.2 基準の一覧

実装根拠の主なファイル（以下の表で略記する）:

- `front/source.rs`, `front/lexer.rs`, `front/copy.rs`: ソース、字句、前処理
- `front/parser/{mod,data,expr,stmt}.rs`, `front/ast.rs`: 構文と AST
- `sema/{data,picture,operand,codegen}.rs`: 配置、型、名前解決、コード生成
- `vm/{mod,data,eval,text,func,files}.rs`, `zir.rs`: 実行

### ソース・プログラム構造・環境部

R42 Chapters 1–17 と実装を照合した。

| 項目 | 判定 | 実装の到達点・制限 |
|---|---|---|
| 固定形式、継続行、コメント、デバッグ行 | 実装／未検証 | `source.rs`。列幅・継続等のテストあり。Area A/B 違反等の全診断を保証しない |
| 数値・英数字・16進・DBCS・NATIONAL 定数 | 部分 | `lexer.rs`。浮動小数点リテラルの指数表記は不足。長さ・字句制約の網羅検証なし |
| 表意定数、ALL、NULL、LENGTH OF、ADDRESS OF | 実装 | `parser/expr.rs`, `sema/operand.rs`。型制約の全診断は未検証 |
| データ名の修飾、添字、参照変更 | 実装 | テーブル認識、名前解決、参照変更。7 次元等の上限・境界診断は未検証 |
| PROGRAM-ID、INITIAL、COMMON、END PROGRAM | 修正 | 終端名・引用符形式・ピリオドを確認。入れ子を持つ親の終端を要求 |
| 入れ子プログラム、GLOBAL、独立コンパイル | 実装 | parser/sema/VM に経路あり。COMMON の可視性など全制約は未検証 |
| RECURSIVE | 部分 | 属性を解析するが VM は active program の CALL を拒否する |
| CLASS-ID、METHOD-ID、FACTORY、OBJECT、INVOKE | 未対応 | OO/Java の構文・実行モデルなし。0003 の初期範囲外だが IBM 4.2 全体には存在する |
| SOURCE-COMPUTER、OBJECT-COMPUTER | 部分 | debugging/collating は保持。一部の語は読み飛ばす |
| SPECIAL-NAMES: ALPHABET、CLASS、SYMBOLIC、DECIMAL-POINT | 実装 | `parser/mod.rs`, `codegen.rs`。全制約は未検証 |
| CURRENCY SIGN | 部分 | 単一の通貨記号のみ。複数定義・WITH PICTURE SYMBOL 等は不足 |
| XML-SCHEMA、REPOSITORY | 未対応 | XML スキーマ・クラス定義への対応なし |
| SELECT/ASSIGN、ORGANIZATION、ACCESS、KEY、FILE STATUS | 実装／部分 | 順次・相対・索引の経路あり。ASSIGN の複数名は先頭のみ使用。2 個目の状態項目等は不足 |
| I-O-CONTROL | 部分 | SAME RECORD AREA は実装。一部のテープ・RERUN/APPLY 等は読み飛ばす |

### データ部

R42 Chapters 17–19 と `parser/data.rs`, `sema/data.rs`, `picture.rs` を照合した。

| 項目 | 判定 | 実装の到達点・制限 |
|---|---|---|
| FILE SECTION、FD、SD | 実装／部分 | 記録域・ファイル定義あり。RECORDING MODE は解析後に値を破棄する |
| WORKING-STORAGE、LINKAGE | 実装 | 記憶域・USING・アドレス設定の経路あり |
| LOCAL-STORAGE | 修正／部分 | 呼出しごとの VALUE 再初期化、EXTERNAL 禁止を修正。再帰プログラムは引き続き未対応 |
| レベル 01–49/66/77/88、FILLER、VALUE、RENAMES | 実装 | 空の 88 VALUE は今回診断を追加。すべてのレベル・句の合法な組合せ検査は未検証 |
| REDEFINES、EXTERNAL、GLOBAL | 実装 | 共有配置・外部記憶域・親の大域項目。組合せ制約は未検証 |
| OCCURS、DEPENDING ON、KEY、INDEXED BY | 実装 | 配置・添字・ODO。可変位置項目など IBM の全制限は未検証 |
| PICTURE、SIGN、SYNC、JUSTIFIED、BLANK WHEN ZERO | 実装／部分 | 表示・編集 PIC と配置あり。全記号・順序・桁数制限を保証しない |
| DISPLAY、COMP/BINARY/COMP-4、COMP-3、COMP-5 | 実装 | `NumType` と decimal runtime。TRUNC/NUMPROC 等による挙動切替は不足 |
| COMP-1、COMP-2 | 実装／部分 | HFP 記憶域と変換あり。指数リテラル等の構文が不足 |
| DISPLAY-1、NATIONAL、NSYMBOL | 実装／部分 | DBCS、UTF-16、変換・移送テストあり。national-edited、numeric NATIONAL 等は不足 |
| GROUP-USAGE NATIONAL | 未対応 | グループカテゴリの指定なし |
| POINTER、INDEX | 実装 | 31-bit 仮想アドレス・索引。IBM の実アドレス/ABI そのものではない |
| PROCEDURE-POINTER、FUNCTION-POINTER | 未対応 | `usage_word` と呼出し規約に経路がない |
| OBJECT REFERENCE、USAGE NATIVE | 未対応 | OO 用データ型なし |
| DATE FORMAT と日付フィールド | 未対応 | DATEPROC 関連の型情報・演算規則なし |

### 手続き文

R42 Chapter 21 の文単位で照合。以下の「実装」は、文全体の完全準拠を示さない。
各文の通常形式に経路があることを示し、IBM 固有の不足は右欄に記載する。

| 文 | 判定 | 根拠・制限 |
|---|---|---|
| ACCEPT | 実装 | 入力・DATE/DAY/TIME 等。全装置名・環境名は未検証 |
| ADD | 実装 | 通常/GIVING/CORRESPONDING、丸め・サイズエラー |
| ALTER | 実装 | GO TO の変更。再帰/THREAD の制約は未検証 |
| CALL | 部分 | REFERENCE/CONTENT/VALUE、OMITTED、RETURNING、例外。再帰・procedure/function pointer 呼出しが不足 |
| CANCEL | 実装 | プログラムの状態リセット |
| CLOSE | 部分 | ファイルクローズ。REEL/UNIT/LOCK 等のホスト意味は未検証 |
| COMPUTE | 実装 | 式、複数受信、丸め・サイズエラー |
| CONTINUE | 実装 | 操作なし |
| DELETE | 実装 | ファイル削除・INVALID KEY |
| DISPLAY | 実装／部分 | 文字/数値/DBCS/NATIONAL、NO ADVANCING。装置別のレコード分割・長さ制限は未検証 |
| DIVIDE | 実装 | INTO/BY/GIVING/REMAINDER、丸め・サイズエラー |
| ENTRY | 部分 | 引数の名前・エントリ登録。BY VALUE 等の全形式は未検証 |
| EVALUATE | 実装 | 複数 subject、ALSO、THRU、ANY、OTHER |
| EXIT、EXIT PROGRAM、GOBACK | 実装 | VM の呼出し復帰経路。EXIT METHOD は未対応 |
| GO TO | 実装／部分 | 通常/DEPENDING/ALTER。MORE-LABELS は未対応 |
| IF、NEXT SENTENCE | 実装 | 文終端の分岐。IBM のすべてのスコープ合法性検査は未検証 |
| INITIALIZE | 修正／部分 | NATIONAL/DBCS 選択、float、FILLER 集団内の名前付き項目を修正。national-edited 型は不足 |
| INSPECT | 実装 | TALLYING/REPLACING/CONVERTING。DBCS/NATIONAL の全組合せは未検証 |
| INVOKE | 未対応 | OO/Java 呼出しモデルなし |
| MERGE | 実装 | キー、USING、出力手続き/GIVING |
| MOVE | 実装 | 通常/CORRESPONDING、各記憶形式の移送 |
| MULTIPLY | 実装 | 通常/GIVING、丸め・サイズエラー |
| OPEN | 実装／部分 | INPUT/OUTPUT/I-O/EXTEND。テープ/ラベル制御は未検証 |
| PERFORM | 実装 | Inline/out-of-line、THRU、TIMES、UNTIL、TEST、VARYING/AFTER |
| READ | 実装 | NEXT/KEY/INTO と終了・キー条件 |
| RELEASE | 実装 | ソート入力・FROM |
| RETURN | 実装 | ソート出力・INTO・AT END |
| REWRITE | 実装 | ファイル更新・FROM・INVALID KEY |
| SEARCH | 実装 | 逐次/ALL、VARYING、WHEN、AT END |
| SET | 実装／部分 | 条件名 TRUE、索引、ポインタ、switch。手続きポインタ・OO 形式は不足 |
| SORT | 実装 | キー、入出力手続き、USING/GIVING |
| START | 実装 | キー関係条件、INVALID KEY |
| STOP | 実装／部分 | STOP RUN/定数を解析。定数形式のオペレータ対話は未検証 |
| STRING | 実装 | DELIMITED、POINTER、OVERFLOW |
| SUBTRACT | 実装 | 通常/GIVING/CORRESPONDING、丸め・サイズエラー |
| UNSTRING | 実装 | 複数区切り・DELIMITER/COUNT・POINTER/TALLYING |
| WRITE | 実装 | FROM、ADVANCING、EOP、INVALID KEY |
| XML GENERATE / XML PARSE | 未対応 | 文法・XML 特殊レジスタ・イベント処理経路なし |

REPORT SECTION / INITIATE / GENERATE / TERMINATE / SUPPRESS は Zeus の追加機能。
COMMUNICATION SECTION / ENABLE / DISABLE / SEND / RECEIVE / PURGE の解析もあるが、
後者は実行時に失敗する。これらを IBM 4.2 準拠の達成項目として加算しない。

### 組込み関数・コンパイラ指示

| 項目 | 判定 | 実装の到達点・制限 |
|---|---|---|
| COBOL-85 の数学・統計・文字・日付関数 | 実装／未検証 | `sema/operand.rs::function`, `vm/func.rs`。IBM の引数個数・カテゴリ・境界値診断の網羅性なし |
| NATIONAL-OF / DISPLAY-OF | 実装 | コードページ変換テストあり |
| DATE-TO-YYYYMMDD / DAY-TO-YYYYDDD / YEAR-TO-YYYY | 修正／部分 | 2026 固定を廃止し実行時の年を使用。世紀窓の範囲を検査。時計は既存日付関数と同じ UTC 基準。全引数の検査は未完 |
| DATEVAL / UNDATE / YEARWINDOW | 未対応 | DATEPROC とともに未実装 |
| WHEN-COMPILED | 部分 | VM が CURRENT-DATE と同じ処理を使用。コンパイル時刻をモジュールに保存していない |
| COPY / REPLACING / REPLACE / OFF | 修正／部分 | 別プログラムへの REPLACE 漏れを修正。separator 単独の置換・前処理時の PICTURE 文脈等に未検証部分がある |
| CBL/PROCESS | 部分 | CICS/NSYMBOL 等を拾うが、全オプションの構文検証・実行意味はない |
| EJECT / SKIP1/2/3 / TITLE | 修正／部分 | 単独行の正しい形式を実行対象から除外。IBM 互換のリスト出力・全診断は未実装 |
| ENTER / READY/RESET TRACE | 未対応 | 予約語としての専用処理がない |
| BASIS / INSERT / DELETE（ライブラリ文） | 未対応 | ソース更新前処理なし |
| SERVICE LABEL / RELOAD | 未対応 | LE 向けの制御処理なし |
| USE | 実装／部分 | エラー・デバッグ・report の経路あり。LABEL/全 GLOBAL 優先規則は未検証 |
| EXEC CICS / SQL / DLI | 部分 | 解析からホストへの委譲まである。各製品仕様との完全互換性は本監査の対象外 |
| コンパイラ上限・診断・コードページ・照合順序 | 部分 | EBCDIC/DBCS 実装あり。IBM 診断コード、全上限、TRUNC/ARITH/NUMPROC 等の保証なし |

現在 `sema/operand.rs::function` が認識する関数名の全一覧（関数名の認識と、
全引数・全結果の適合性は別判定）:

| 系統 | 認識する名前 |
|---|---|
| 数学 | ABS, ACOS, ANNUITY, ASIN, ATAN, COS, FACTORIAL, INTEGER, INTEGER-PART, LOG, LOG10, MOD, PRESENT-VALUE, RANDOM, REM, SIN, SQRT, TAN |
| 集計 | MAX, MEAN, MEDIAN, MIDRANGE, MIN, ORD-MAX, ORD-MIN, RANGE, STANDARD-DEVIATION, SUM, VARIANCE |
| 文字・変換 | CHAR, ORD, LENGTH, BYTE-LENGTH, LOWER-CASE, UPPER-CASE, REVERSE, TRIM, NATIONAL-OF, DISPLAY-OF, NUMVAL, NUMVAL-C, TEST-NUMVAL, TEST-NUMVAL-C |
| 日付 | CURRENT-DATE, WHEN-COMPILED, DATE-OF-INTEGER, DAY-OF-INTEGER, INTEGER-OF-DATE, INTEGER-OF-DAY, DATE-TO-YYYYMMDD, DAY-TO-YYYYDDD, YEAR-TO-YYYY |

ABS / BYTE-LENGTH / TRIM / TEST-NUMVAL / TEST-NUMVAL-C は R42 の関数一覧外の拡張。
R42 の DATEVAL / UNDATE / YEARWINDOW は認識しない。R64 の追加関数全体の目録は未完。

## 今回解消した差分と回帰テスト

テストは `crates/zeus-cobol/tests/ibm_conformance.rs`。
実行結果の検証と不正構文の診断検証を含む。最初の 8 件は修正前の失敗を確認した。

**C01〜C11 は、以下に定義した修正対象についてすべて対応完了。**
追加確認では既存の修正コードを再確認し、6 件の統合回帰テストと世紀窓の境界値検査を補強した。
DBCS/EGCS は受理だけでなく実際の格納バイトを確認し、表の初期化は 4.2 の通常形式でも
複数次元・特定要素の両方を確認した。以下の完了判定は、別節の残存課題まで解消したという意味ではない。

| ID | 修正 | 基準・テスト |
|---|---|---|
| C01 | INITIALIZE の NATIONAL と ALPHANUMERIC を区別 | R42 pp.368–370、`initialize_national_category_does_not_select_alphanumeric` |
| C02 | DBCS/EGCS の選択・重複カテゴリ診断 | R42 pp.368–369、`initialize_egcs_is_dbcs_category_and_duplicate_alias_is_invalid`, `initialize_rejects_duplicate_categories` |
| C03 | float 初期化、FILLER 集団内の名前付き子項目 | R42 pp.369–370、`initialize_float_and_filler_group` |
| C04 | INITIALIZE で再定義項目・索引・ポインタを除外、表全要素の処理 | `initialize_excludes_redefinitions_and_index_pointer_storage`, `initialize_value_initializes_every_table_occurrence` |
| C05 | END PROGRAM の名前照合・引用符・必須ピリオド | R42 p.55 / EP、`end_program_name_and_separator_are_checked` |
| C06 | REPLACE を別コンパイルの次プログラムに漏らさず、入れ子では維持 | R42 pp.566–568、`replace_scope_ends_at_separately_compiled_program`, `replace_remains_active_across_nested_programs` |
| C07 | 空の置換元・ピリオド欠落・空の 88 VALUE の診断 | `malformed_replace_and_condition_value_are_diagnosed` |
| C08 | LOCAL-STORAGE の呼出しごとの初期化と EXTERNAL 禁止 | R42 pp.163–164 / LS、`local_storage_value_is_reinitialized_without_resetting_working_storage`, `local_storage_cannot_be_external` |
| C09 | 外部言語からの CALL にも LOCAL-STORAGE 初期化を適用し ZIR に保存 | `local_storage_is_reset_for_external_language_calls_and_serialized_modules` |
| C10 | 日付変換の現在年固定を廃止し世紀窓の範囲を検査 | R42 pp.548–549、`vm::func::tests::ibm_year_to_yyyy_reference_examples_and_window_boundaries` |
| C11 | EJECT / SKIP1/2/3 / TITLE を文のスコープに影響させず処理 | R42 pp.564,570–571、`listing_directives_do_not_affect_program_or_sentence_scope` |
| X01 | EXIT PERFORM / CYCLE、最内側の inline ループへの適用 | R64 pp.343–345、`exit_perform_exits_innermost_inline_loop`, `exit_perform_cycle_runs_increment_and_test`, `exit_perform_cycle_test_after_and_once` |
| X02 | PERFORM UNTIL EXIT、TEST との不正な併用を診断 | R64 pp.417–418、`perform_until_exit_and_times_cycle` |
| X03 | WHEN SET TO FALSE と SET condition-name TO FALSE、句欠落の診断 | SF、`set_false_uses_declared_value_and_true_uses_first_range_bound`, `set_false_requires_false_value` |
| X04 | INITIALIZE WITH FILLER / TO VALUE / REPLACING / TO DEFAULT の優先順位、JUSTIFIED 項目の初期配置復元 | I61 / R64、`initialize_value_replacing_default_precedence_and_filler`, `initialize_to_value_restores_justified_initial_layout` |
| X05 | COPY/REPLACE の LEADING/TRAILING、空文字への置換 | CP、`replace_partial_words_respects_edges_order_and_literals`, `copy_replacing_partial_word_and_deletion` |

追加した完了確認の実行テスト:

| ID | 追加確認 | テスト名 |
|---|---|---|
| C02 | DBCS/EGCS が DBCS の格納域だけを空白にし、英数字・NATIONAL を保持 | `initialize_dbcs_and_egcs_replace_only_dbcs_storage` |
| C03 | COMP-1/COMP-2 の両方で NUMERIC REPLACING と通常のゼロ初期化 | `initialize_numeric_replacement_includes_both_float_usages` |
| C04 | 通常形式で 2 次元表の全要素・特定要素を初期化し FILLER を保持 | `initialize_plain_form_visits_nested_tables_and_preserves_fillers` |
| C05 | 入れ子の終端名の不一致と親プログラムの終端欠落を診断 | `end_program_checks_nested_names_and_requires_containing_marker` |
| C08 | 入れ子プログラム再入時に LOCAL-STORAGE の表だけを再初期化 | `local_storage_nested_reentry_reinitializes_groups_and_tables` |
| C10 | YEAR/DATE/DAY の 3 関数をコンパイル・実行し、世紀窓の端点と日付部分を確認 | `date_conversion_functions_use_runtime_year_and_preserve_date_components` |

C10 の単体テストには、窓の終了年 1700/9999 の受理、1699/10000 の拒否、負の 2 桁年の拒否も追加した。

### X01〜X05 の追加対応

X 系も上表で定義した修正対象について対応完了。既存の実装に対して 14 件の回帰テストを追加し、
そのうち検証で見つかった X03〜X05 の不足を修正した。

| ID | 追加対応・確認 | 回帰テスト |
|---|---|---|
| X01 | CYCLE が最内側のループに作用し、VARYING/AFTER の増分・繰り上がりまで実行することを確認 | `exit_perform_cycle_handles_after_varying_and_nested_loops` |
| X01 | TEST AFTER VARYING とループ外の CYCLE の動作を確認 | `exit_perform_cycle_test_after_varying_tests_before_increment` |
| X02 | out-of-line UNTIL EXIT が EXIT PARAGRAPH で終了せず GOBACK で終了することを確認（PE） | `perform_until_exit_out_of_line_reenters_until_goback` |
| X02 | TEST BEFORE/AFTER、WITH の有無、VARYING との不正な組合せを確認（PE） | `perform_until_exit_rejects_test_phrases_and_varying` |
| X03 | FALSE 句の WHEN/SET/TO/IS 省略形を受理（SF） | `condition_false_phrase_allows_ibm_optional_words` |
| X03 | 複数の添字付き条件名に対する SET TO FALSE/TRUE を確認 | `set_false_accepts_multiple_subscripted_condition_names` |
| X03 | FALSE 値にデータ項目を指定した宣言を、SET が実行されなくても診断 | `condition_false_value_requires_a_literal_even_if_not_executed` |
| X03 | NATIONAL/SYMBOLIC の FALSE 定数を確認し、SYMBOLIC は宣言時に定数へ解決 | `set_false_preserves_national_and_symbolic_literal_values` |
| X03 | GLOBAL 条件名を入れ子プログラムから設定する場合も宣言元の定数を保持 | `set_false_global_condition_keeps_declaring_program_symbolic_literal` |
| X04 | TO VALUE/TO DEFAULT に RENAMES 項目を指定した場合に診断（IN） | `initialize_to_value_rejects_renames_receiver` |
| X04 | 複数カテゴリの VALUE 選択を確認。条件名・INDEX・POINTER を直接の受信項目に指定した場合に診断（IN） | `initialize_value_phrases_select_categories_and_reject_invalid_receivers` |
| X05 | 部分語置換時に PICTURE トークンの形式を保持し、単独ハイフンの部分語を処理。PICTURE 全体を削除したときは空トークンを残さない | `partial_word_replace_preserves_picture_token_and_hyphen_fragments` |
| X05 | 部分語の置換元・置換先に必須の疑似テキスト区切りを検査（CP） | `partial_word_replace_requires_pseudotext_delimiters` |
| X05 | COPY REPLACING でも PICTURE の変換と、置換結果を再置換しない順序を確認 | `copy_partial_replacement_handles_picture_and_does_not_cascade` |

X01/X02 は既存の実行経路が追加テストにも合格し、製品コードの追加変更は不要だった。
X03 は parser と宣言の意味解析、X04 は受信項目の意味解析、X05 は COPY/REPLACE 処理を修正した。
NATIONAL-EDITED/UTF-8 のデータ型や IBM 方言全体の残存課題は、引き続き別節で追跡する。

`X*` は新しい版の拡張であり、4.2 の欠陥としては数えない。
REPLACE ALSO / LAST OFF は今回参照した IBM の文法にないため実装対象にしない。
部分語置換は SBCS の識別子・数字・ハイフンを含む部分語と PICTURE を対象にし、
DBCS 語・文字列リテラル内部は置換しない。

LOCAL-STORAGE は非再帰呼出しで記憶域を再利用し初期値を復元する。
値を持たない項目の初期内容は IBM でも未定義。既存 `.zo` は追加メタデータを持たないため、
この修正を適用するにはソースを再コンパイルする。再帰対応は完了扱いにしない。

## 残存する未準拠の解消順序

今回の修正だけでは、上の「部分」「未対応」「未検証」は解消されない。
次の単位で専用の仕様テストを増やして解消する。既存設計で初期範囲外だった機能も隠さず追跡する。

| 優先 | 作業 | 完了条件 |
|---|---|---|
| P1 | 数値リテラル・引数検査・WHEN-COMPILED・日付のタイムゾーン・前処理の文脈/境界 | 公式例と境界・不正入力の期待診断をテスト化し通過 |
| P1 | RECURSIVE、CALL の形式/モード、procedure/function pointer | 呼出しフレーム・記憶域・復帰値まで仕様テスト通過 |
| P1 | NATIONAL-EDITED、numeric NATIONAL、GROUP-USAGE | 配置と MOVE/比較/INITIALIZE の組合せを検証 |
| P1 | CBL/PROCESS と TRUNC/ARITH/NUMPROC/SSRANGE、PIC/名前/レベル制約 | 未実装オプションの黙殺をなくし、受理した設定を実行に反映 |
| P2 | 特殊なファイル句、ラベル、テープ、装置別 I/O、USE 優先規則 | ホスト依存箇所を含む形式別のテストが通過 |
| P2 | リスト/ライブラリ/LE 向けのコンパイラ指示 | 専用構文処理と効果/診断を検証 |
| P2 | DATEPROC、DATE FORMAT、関連関数 | 日付型と演算・比較の一貫した意味を実装 |
| P2 | XML GENERATE/PARSE、XML レジスタ、XML-SCHEMA | 解析・生成・イベント・例外まで実装 |
| P3 | OO/Java、CLASS/METHOD/INVOKE、OBJECT REFERENCE | 構文だけでなくオブジェクトと JVM 連携を含む実装 |
| P3 | 6.4/6.5 の追加全体（JSON、UTF-8、ALLOCATE/FREE、ユーザー関数等） | 対象版を固定し、4.2 からの差分目録を拡張して検証 |

## 検証コマンド

2026-10-16 のローカル検証結果:

| 検証 | 結果 |
|---|---|
| 専用の IBM 仕様回帰テスト（X01〜X05 追加対応後） | 44 passed |
| zeus-cobol のライブラリテスト | 77 passed（世紀窓の公式例テストを含む） |
| programs / national / japanese の統合テスト | 5 / 1 / 1 passed。上記と合わせて 128 passed |
| X01〜X05 の最終コードでの `cargo test --workspace` | 945 passed、失敗なし |
| `cargo clippy -p zeus-cobol --all-targets -- -D warnings` | X01〜X05 の最終コードで成功 |
| `cargo fmt --all --check` / `git diff --check` | 成功 |

全体テストは、最初のサンドボックス内実行では IMS TM の一時ファイル置換に対する
アクセス拒否で 6 件失敗した。該当テストをサンドボックス外で再実行して 8 件成功した後、
最終コードの全体テストもサンドボックス外で再実行して成功した。
C01〜C11 の追加確認時点では専用 30 件・関連合計 114 件に合格した。
今回の X01〜X05 の追加対応後は、専用 44 件と全体 945 件、および全 target の clippy を再実行し成功した。
外部サービスを要求するテストは、各テストの既存の接続設定・スキップ条件に従う。
今回は CCVS85 全件と IBM 処理系との実機比較は再実行していない。

```sh
cargo test --workspace
cargo test -p zeus-cobol --test ibm_conformance
cargo test -p zeus-cobol --lib --test programs --test national --test japanese
cargo clippy -p zeus-cobol --all-targets -- -D warnings
cargo fmt --all --check
```

専用テストは通常の `cargo test --all` に含まれるため CI に追加設定は不要。
アプリの構文使用一覧は [docs/inventory/apps.md](../inventory/apps.md) を参照する。
これは使用している構文の一覧であり、本書の仕様準拠一覧とは目的が異なる。

句・コンパイラー設定を読み飛ばしていた実装の追加監査と対応範囲は
[0015-cobol-ignored-clauses.md](0015-cobol-ignored-clauses.md) を参照する。

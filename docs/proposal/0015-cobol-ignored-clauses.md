# COBOL の句・設定の読み飛ばし監査

確認日: 2026-10-16。対象: `zeus-cobol` の COBOL フロントエンド、意味解析、ファイル VM。
0014 の C/X 系の追加監査。**IBM 全構文・実行環境との完全互換を示すものではない。**

RESERVE・物理ブロック・テープ・再帰 CALL の追加実装は
[0016-cobol-physical-tape-recursion.md](0016-cobol-physical-tape-recursion.md) を参照。

## 参照資料と判断基準

- [IBM Enterprise COBOL for z/OS V4.2 Language Reference, SC23-8528-01](https://publibfp.dhe.ibm.com/epubs/pdf/igy3lr50.pdf) を基準とする。以下のページは冊子ページ（PDF ページではない）。
- [NSYMBOL](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=options-nsymbol) は N リテラルと PIC N の意味、既定 NATIONAL、略記 NS/NAT の確認に使用。
- [ALPHABET](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=section-alphabet-clause) は STANDARD-1/2、NATIVE/EBCDIC の定義の補足確認に使用。
- [SYNCHRONIZED](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=entry-synchronized-clause) は LEFT/RIGHT に実行上の効果がないことの補足確認に使用。
- [コンパイラーオプション一覧](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-cobol-compiler) は実行意味を変える設定とリスト出力設定の区別に使用。新しい版の全設定を V4.2 の対応範囲へ取り込むものではない。

「構文検査するが実行には影響しない」と IBM が定義する句と、Zeus が実装不足で指定を捨てていた句を区別する。
前者に実行命令を追加する必要はない。ただし IBM の構文・意味制約まで全て検査できたとは限らない。
実装できない実行機能については、受理して無視する動作を可能な範囲で明示的な診断へ変更する。

## 該当箇所一覧

パスはリポジトリー相対。状態の「一部」は後述の制約を含む。

| ID | 句・設定 / 実装箇所 | 監査前の動作 | IBM の仕様 | 今回の結果 |
|---|---|---|---|---|
| I01 | `CBL` / `PROCESS`, `src/front/mod.rs::process_statements`, `source_tokens` | CICS という単語以外を破棄。NSYMBOL も無効 | コンパイル設定。NSYMBOL は N リテラルと PIC N を変更 | NSYMBOL/NS の NATIONAL/NAT/DBCS を字句解析前に適用。CICS/NOCICS を順に処理。未知・未対応・不正なオプションは診断 |
| I02 | `SOURCE-COMPUTER`, `OBJECT-COMPUTER`, `front/parser/mod.rs::computer_paragraph` | 不明トークンも無条件に消費 | LR pp.116–118。前者は DEBUGGING MODE、後者は COLLATING SEQUENCE だけ実行に影響 | コンピューター名を読み、句を構文解析。MEMORY の数値・単位、SEGMENT-LIMIT の 1–49 を検査。不明句を拒否 |
| I03 | 複数 `ASSIGN` 名, `front/parser/mod.rs::select` | 最初以外を破棄 | LR p.137。最初以外は構文検査だけ | 実行上の扱いを維持。後続名を任意の語として受けるため、予約語等の厳密な検査は残る |
| I04 | `RESERVE`, `select` → AST `reserve` | 値を保存するが下流で不使用 | LR p.141。I/O バッファー数を指定。LINE SEQUENTIAL では不可 | OPEN 中の I/O バッファープールに反映。RESERVE > DD BUFNO > Zeus 既定 2。LINE SEQUENTIAL と EXTERNAL 不一致を診断。詳細は 0016 |
| I05 | `PADDING CHARACTER`, `select` → AST `padding` | 保存するが参照しない | LR p.144。実行への効果なし。ただし 1 文字の英字・英数字・国別データ等の制約あり | 実行上は無処理を維持。1 文字リテラル／非 FILE データ、EXTERNAL 制約の一部を意味解析で検査 |
| I06 | `RECORD DELIMITER`, `select` | 名前を捨て、存在だけ保存 | LR p.144。実行への効果なし。可変長レコード等の制約あり | 無処理を維持。装置種別・可変長条件の厳密な検査は残る |
| I07 | `SAME AREA`, `front/parser/mod.rs::io_control` | RECORD 以外の指定とファイル名を破棄 | LR pp.154–155。QSAM は文書、VSAM は SAME RECORD AREA 相当。同時 OPEN に制約 | QSAM と VSAM を区別し、VSAM のレコード領域共有と同時 OPEN の実行エラーを追加。2 ファイル以上を検査 |
| I08 | `SAME SORT/SORT-MERGE AREA`, `io_control` | ファイル名を破棄 | LR pp.156–157。実行への効果なし。ただし参照先等の制約あり | 基本構文と 2 ファイル以上を検査。参照先の種別、SORT 中の非 SORT ファイルの OPEN 禁止等は残る |
| I09 | `MULTIPLE FILE TAPE`, `io_control` | ほぼ任意の語・数字を破棄 | LR p.156。実行への効果なし。DD LABEL が処理 | FILE/TAPE、ファイル名、POSITION 整数を構文解析。媒体・DD LABEL の実現はデータセット環境の課題 |
| I10 | `RERUN`, `io_control` | 任意トークンとして消費 | LR pp.153–154。チェックポイントを取得 | **未実装を診断**。チェックポイント取得・再始動は提供しない |
| I11 | `APPLY WRITE-ONLY`, `io_control` | 任意トークンとして消費 | LR p.157。QSAM 可変長ブロックのバッファー充填を変更 | V 形式のブロック充填に実装。次レコードの実長を基準に充填する。詳細は 0016 |
| I12 | `RECORDING MODE`, `front/parser/data.rs::file_description` | モード名を捨て、長さだけから形式決定 | LR pp.190–191。QSAM の F/V/U/S。VSAM は無視 | F/V/S/U の物理ブロックを実装。S は SDW による分割・復元、U は AWS のブロック境界を使用。既存ホスト形式の扱いは 0016 |
| I13 | `BLOCK CONTAINS`, `file_description` → AST `block_contains` | 保存するが不使用 | LR pp.183–185。QSAM の物理ブロックサイズ。VSAM 等では効果なし | 物理ブロックの生成・復元に使用。CHARACTERS/RECORDS、0、DD BLKSIZE を反映。従来の最大 32760 バイトを対象。詳細は 0016 |
| I14 | `LABEL RECORDS`, `file_description` | STANDARD の真偽だけ保存。ユーザーラベル名を破棄 | LR pp.187–188。QSAM のラベル処理、VSAM/SD は実行への効果なし | AWS 上の標準 VOL1/HDR1/HDR2/EOF1/EOF2 と OMITTED を実装。ユーザーラベルの処理は未対応として診断。詳細は 0016 |
| I15 | `VALUE OF`, `file_description` → AST `value_of` | 保存するが不使用 | LR p.188。実行への効果なし | 無処理を維持。データ参照の定義域・型の厳密な検査は残る |
| I16 | `DATA RECORDS`, `file_description` → AST `data_records` | 保存するが不使用 | LR p.188。文書としてだけ働く。同名 01 が存在する必要もない | 無処理を維持 |
| I17 | `CODE-SET`, `file_description` → AST `code_set` | 保存するが変換しない | LR pp.191–192。外部文字集合との変換。全項目 DISPLAY、符号付き数字は SIGN SEPARATE。SD では効果なし | STANDARD-1/2 の ASCII と内部 EBCDIC の入出力変換を追加。NATIVE/EBCDIC は変換不要。アルファベット定義、DISPLAY、符号分離を検査。現在内部 CCSID 37 に限定。他 CCSID は診断 |
| I18 | `SYNC LEFT/RIGHT`, `front/parser/data.rs::data_entry` | 方向を捨て SYNC だけ保持 | LR p.236。LEFT/RIGHT は実行への効果なし | 方向の無処理を維持。SYNC 自体の既存アラインメント処理を使用 |
| I19 | `OPEN REVERSED`, `WITH NO REWIND`, `front/parser/stmt.rs` | 修飾を捨て通常の OPEN | LR OPEN statement。入力順序／テープ位置を指定 | 仮想テープの NO REWIND と固定長 REVERSED を実装。非テープでは修飾の効果なし。詳細は 0016 |
| I20 | `CLOSE REEL/UNIT FOR REMOVAL`, 同上 | REMOVAL と REEL/UNIT の区別を捨てる | LR CLOSE statement。媒体処理 | 現在の仮想巻を閉じる／取り外す基本経路を追加。自動複数巻交換・継続は未対応。非テープの REEL は既存 status 07。詳細は 0016 |
| I21 | `SCREEN SECTION`, `front/parser/data.rs::data_division` | セクション全体を読み飛ばす | IBM Enterprise COBOL の対象外の拡張 | **コンパイル診断へ変更** |
| I22 | `COMMUNICATION SECTION` / CD と ENABLE/DISABLE/SEND/RECEIVE/PURGE, parser data/stmt | CD 句を読み飛ばし領域だけ確保。文は Unsupported | IBM Enterprise COBOL の対象外。CCVS の旧 Communication Module 向け拡張 | 既存拡張として区別。文の実行はエラー。IBM 対応実績に含めない |
| I23 | AUTHOR 等のコメント段落、EJECT/SKIP/TITLE, `front/source.rs`, parser | 内容をコード化しない | LR の comment-entry / compiler-directing statements。文書・リスト用 | 正しい無処理を維持 |
| I24 | `COPY SUPPRESS`, `front/copy.rs::expand_copy` | キーワードだけ消費 | LR COPY statement。コピー内容のソースリスト出力抑制 | 実行意味は不変。IBM 形式のリスト出力自体は未提供 |
| I25 | `PROGRAM-ID ... RECURSIVE`, `front/parser/mod.rs` → AST `recursive` | 属性を保存するが ZIR に伝播しない | LR pp.111–112。アクティブなプログラムへの再入を許可 | 共有 WS と呼出し別 LS/LINKAGE/PERFORM を持つ再帰 CALL を実装。外部言語経由の再入も対応。細かな制約とネイティブ実行の範囲は 0016 |
| I26 | `PROCEDURE DIVISION RETURNING`, parser → AST `procedure.giving` | 受け取り項目を保存するが呼び出し規約に接続しない | LR p.266。LINKAGE の level 01/77 を結果項目にする。主／入れ子プログラム等には制約 | **未実装**。既存 CALL RETURNING / RETURN-CODE による戻り値処理とは別機能。IBM の関数結果 ABI は未対応 |

`src/` は `crates/zeus-cobol/src/`。上表の parser パスも同ディレクトリー配下。
オプション処理は新設 `front/options.rs`。ファイルの実行処理は `sema/codegen.rs` → `zir.rs` → `vm/files.rs`。

## 実装の範囲と残る制約

- CBL/PROCESS は先頭の連続した文だけを処理する既存構成を維持。バッチ内の途中の再設定、継続行、全オプションの略記、CICS のサブオプションは未対応。
- LIST/NOLIST、SOURCE/NOSOURCE、MAP/NOMAP、OFFSET/NOOFFSET、XREF/NOXREF、VBREF/NOVBREF は実行に影響しない設定として受理する。IBM 形式のコンパイラーリスト生成を実装したわけではない。
- TRUNC、ARITH、NUMPROC、CODEPAGE、YEARWINDOW、APOST など、サポートしていないソース内の設定を黙って捨てない。今回から明示指定はコンパイルエラーになる。既定時の全演算意味が IBM の既定に一致するとの保証でもない。
- NSYMBOL の内部設定をプロセス全体の atomic からスレッドローカルへ変更した。並行コンパイル時に異なる指定が混ざる問題を回避する。同一ソースの解析・意味解析は現在と同じスレッドで実施する前提。
- 従来の F/V ホスト表現を維持しつつ、明示的なブロック指定・テープ等には物理ブロック層を追加した。詳しい選択条件、DD 属性との関係、残る FILE STATUS 39 検査は 0016 を参照。
- CODE-SET はホスト上の順次レコード媒体へ適用する。磁気テープだけに許可する装置検査、EXTERNAL 定義間の一致、変換不能文字の IBM と同一の取り扱いは未検証。試験対象は CCSID 37 と 7-bit ASCII 範囲。
- SAME の重複グループ、EXTERNAL/GLOBAL 制約、SORT グループの参照整合性などは完全には検査していない。同時 OPEN 違反は Zeus の実行エラーであり、IBM の診断メッセージ／番号の再現ではない。
- RESERVE、基本的な物理ブロック、標準媒体ラベル、再帰 CALL は 0016 で追加対応した。複数巻交換、ユーザーラベル、RERUN などの残件は 0016 を参照。
- PROCEDURE DIVISION RETURNING の ABI は引き続き未対応。再帰 CALL と分けて扱う。

## 検証

`crates/zeus-cobol/tests/ignored_clauses.rs` に追加した回帰試験:

- F/V と等長 RECORD VARYING の実ファイルバイト列。
- NSYMBOL によるリテラルと PIC N のカテゴリー、略記、ソース指定優先、シーケンス番号付き CBL。
- GLOBAL ファイルの CODE-SET を宣言元の環境で解決すること、SIGN SEPARATE のグループからの継承。
- 未対応・不正・括弧不整合のコンパイラーオプション。
- MEMORY SIZE / SEGMENT-LIMIT の構文と範囲。
- CODE-SET STANDARD-1/2、NATIVE/EBCDIC の読み書き、RDW 非変換、ZIR の JSON 保存・復元、型制約。
- VSAM SAME AREA の共有オフセットと同時 OPEN 禁止、QSAM との区別。
- PADDING の長さ、ユーザーラベル、SCREEN の診断、MULTIPLE FILE TAPE の基本構文。

IBM コンパイラーを使う差分実行や実機テープ試験は実施していない。

実行結果: 専用 13 テスト、既存 IBM 準拠 44 テストを含む `cargo test --workspace` は
958 passed / 0 failed。`cargo clippy -p zeus-cobol --all-targets -- -D warnings`、
`cargo fmt --all --check`、`git diff --check` も通過した。
Windows の既存 IMS ファイル置換試験を実行するため、全体試験はサンドボックス外で実施した。

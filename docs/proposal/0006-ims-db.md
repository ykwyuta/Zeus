# 0006: IMS DB (zeus-ims)

## 1. 目的

IMS DB を使う COBOL プログラムを、元の DBD・PSB・プログラム・JCL のまま Zeus で動かす。
対象は CardDemo の承認モジュール (app-authorization-ims-db2-mq) と、Bank of Z の IMS 資材。

## 2. 由来

データベースの核は gnucobol-zosplus (オーナーは Zeus と同じ) の `extras/imsdb` (Rust) を流用した
(コミット 61df3c72f09a)。

- 流用したもの: `catalog`、`store`、`dli`、`call_adapter`、`gsam`、`tm`、`unload`、`csv_load` と CLI (`zims`、元は `imsdbctl`)、
  およびそのテスト。
  - DBD/PSB マクロの解析。
  - B 木索引と、チェックサム付き WAL による永続化と回復。
  - DL/I の位置付けと更新。
  - ルート単位のロックと UOW。
  - GSAM、IMS TM のキュー。
  - DFSURGU0 アンロードの読み込み。
- 流用しなかったもの: GnuCOBOL 用の C ABI (`ffi.rs`、`cbltdli.c` など) と Windows 用の unsafe コード。
  Zeus では COBOL の実行時から直接呼ぶので不要。
- 変更点:
  - GSAM の DD を環境変数ではなくジョブステップの割り当てから受け取る。
  - EBCDIC 表を Zeus のコードページ表にした。
  - PSBGEN の CMPAT を保持する。

## 3. Zeus への組み込み (`zeus_ims::runtime`)

| 項目 | 内容 |
|---|---|
| ライブラリ | DBD と PSB はマクロのソースをメンバーとして持つ PDS (DBDLIB・PSBLIB)。カタログ上の FB 80 の EBCDIC レコードも、ホストのテキストファイルも読める |
| データベース | 全 DBD を 1 つの WAL に保持する。zjob は `ZEUS_IMS_WAL`、なければカタログの隣の `ims/ims.wal`。zcics は `--ims-wal` (既定は同じ場所)。WAL は複数のプロセス (zcics とバッチジョブ) が同時に開ける。各プロセスは他のプロセスがコミットしたフレームを、データベースを読むたびに追って適用する。更新する作業単位は、最初のルートのロックから終わりまで WAL のファイルロック (排他) を持つので、他のプロセスの更新は待つ (ロックの待ち時間を過ぎると、ルートのロックと同じくタイムアウト)。プロセス内の作業単位どうしは、従来どおりルート単位でロックする |
| `EXEC PGM=DFSRRC00` | `PARM='DLI\|DBB\|BMP,プログラム,PSB'`。ライブラリは IMS DD の連結。プログラムには PCB マスクを USING で渡す (BMP と CMPAT=YES では I/O PCB が先頭)。正常終了でコミット、異常終了でバックアウト。`PARM=(ULR,DFSURGL0,DBD)` は DD DFSUINPT のアンロード (DFSURGU0 形式) から再ロードする |
| SSA | コマンドコード (D: パス呼出しで親のセグメントも入出力域に入れる、F: 親の下の最初から、L: 最後のもの、C: 連結キーで修飾、P: 親子関係をその階層に置く。U・V・Q・N・- は受け付ける)、修飾文を AND (`*`・`&`・`#`) と OR (`+`・`\|`) で結んだブール修飾、比較演算子 EQ・NE・GT・GE・LT・LE (`= `・`¬=`・`>=` などの記号も)。パック 10 進のフィールドは数値で、ほかはバイトで比べる。GU・GN・GNP はどの階層の SSA も受け付け、上位の SSA は親のセグメントの条件になる。親子関係は GU・GN が取り出したセグメントに置く (GNP は変えない) |
| CALL 'CBLTDLI' | 引数の個数 (省略可)、関数、PCB、入出力域、SSA。DB PCB: GU/GN/GNP と GHU/GHN/GHNP、ISRT/REPL/DLET (可変長セグメントは先頭の長さで書く)。GSAM PCB: OPEN/CLSE/GN/GU/ISRT と RSA。I/O PCB: CHKP (ID。領域の長さと番地を続けると記号チェックポイントで、その領域を保存する)、XRST (I/O 域の ID が空白なら通常の開始、ID か LAST なら記号チェックポイントの領域を戻す)、SYNC、ROLB。バッチでは GU/GN は QC |
| EXEC DLI | SEGMENT、WHERE (フィールド・比較演算子・COBOL のデータ項目かリテラル)、INTO/FROM、PCB(n)、CHKP ID、SYMCHKP・XRST (ID、AREA1〜7、LENGTH1〜7)、SCHD PSB/TERM。結果は DIB (DIBSTAT、DIBSEGM、DIBSEGLV、DIBKFBL、DIBDBDNM、DIBDBORG) に返す。取得コマンドは保持付きの取得として扱う。SEGMENT で現在のパス上の親を指定した DLET/REPL は、キーフィードバックからその親を保持し直してから実行する (子を GNP で読んだ後に親を削除する CardDemo の書き方) |
| CICS (zcics) | `--ims-lib` (カタログの PDS 名かディレクトリ、複数可) と `--ims-wal`。タスクの EXEC DLI SCHD で PSB をスケジュールし (二重のスケジュールは TC)、TERM・SYNCPOINT・タスク終了で作業単位を確定する |
| コンパイラ | EXEC DLI の WHERE を解析し、比較の右辺の COBOL 項目を実行時に渡す。DLZDIB (DIB) を EXEC DLI の結果の受け取り先として渡す。PROCEDURE DIVISION USING がなければ、最初の ENTRY 文 (ENTRY 'DLITCBL' USING) のパラメーターをプログラムのパラメーターにする |
| IMS TM (メッセージ処理) | `EXEC PGM=CSLUSPOC` の `CREATE TRAN NAME(t) SET(PGM(p))` がトランザクションを定義する (カタログの隣の `ims/transactions`、PSB はプログラムと同名)。`zjob ims send t テキスト [--hex] [--lterm L]` が入力メッセージをキュー (`ims/tmq`、`zeus_ims::tm`) に入れ、`DFSRRC00 PARM='MSG'` のメッセージ処理領域がキューのメッセージごとにプログラムを (I/O PCB を先頭に) 実行する。I/O PCB: GU (次のメッセージ。前のメッセージの作業単位をコミットし、LTERM・日付・時刻・順序番号・利用者 ID を PCB に入れる。なければ QC)、GN (次のセグメント、なければ QD)、ISRT (LL の長さのセグメントを入力元の LTERM へ)、PURG。代替 PCB: CHNG・ISRT・PURG。プログラムが正常に終われば出力をキューに入れて入力を消し、異常終了なら DB をバックアウトして入力をキューに残す。`zjob ims receive L` が LTERM の出力を 16 進とテキストで表示する |
| PL/I | `CALL PLITDLI(個数, 関数, PCB, 入出力域, SSA...)`。PCB は POINTER (の指す PCB) でも BASED 構造体そのものでもよい。メッセージの ISRT はフルワードの LL (セグメント長 - 2) を受け付ける。主手続きの POINTER パラメーターには PCB のアドレスが入る。DFSRRC00 の DLI・BMP・MSG 領域で動く |

## 4. CardDemo

`tools/carddemo-ims.sh test` (`tools/carddemo.sh all` の後) で、元の JCL とプログラムのまま次を確認する。

- **再ロード**: 付属のアンロード (`AWS.M2.CARDDEMO.IMSDATA.DBPAUTP0`) から DFSURGL0 で 224 セグメントを読み込む。
- **UNLDPADB** (PAUDBUNL、CBLTDLI GN/GNP、DLI): 要約 21 件と明細 202 件を順次ファイルに書き出す。キーが空白の要約 1 件は、プログラムが除外する。
- **UNLDGSAM** (DBUNLDGS、GSAM ISRT): 同じ内容を GSAM の 2 ファイルに書き出す。
- **CBPAUP0J** (CBPAUP0C、EXEC DLI、BMP、CHKP): 期限切れの承認を削除する。要約 21 件と明細 202 件を削除する。
- **LOADPADB** (PAUDBLOD、CBLTDLI ISRT): アンロードから再び挿入し、224 セグメントに戻す。
- **オンライン** (`tools/carddemo-ims/authorizations.txt`):
  - メニュー 11 から COPAUS0C で口座 1 の承認の要約と一覧を表示する (SCHD、WHERE 付きの GU、GNP)。ページ送りもできる。
  - COPAUS1C で明細を表示する (WHERE 付きの GNP)。F8 で次の承認に進む。

## 5. 未対応 (今後)

- **IMS TM の残り**:
  - MFS (3270 端末の画面形式)。Bank of Z の IMS トランザクションは z/OS Connect (IMS Connect) から呼ばれるので MFS を使わない。
  - 常駐する領域 (WFI=Y、複数の領域の並行実行)、会話型トランザクション (SPA)、高速機能。
  - プログラム間通信 (代替 PCB で他のトランザクションへ ISRT したメッセージは、キューには入るが、同じ領域の実行が終わってから次の領域で処理される)。
- **PL/I の EXEC DLI**。
- **COPAUS2C の不正マーク (F5)**: Db2 を使う。ZEUS_DB2 の環境で別途確認する。
- **アンロードユーティリティ DFSURGU0**: 書き出しは未対応。再ロード DFSURGL0 は対応済み。
- **DL/I の機能の範囲**: ISRT のパス呼出し (D コマンドコードで複数のセグメントを挿入する)。

## 6. Bank of Z の IMS 経路

`tools/bankofz-ims.sh` が環境を作る。
- IMS 資材: DBD・PSB をライブラリに入れる。
- データ: LoadData の CSV を LOAD* プログラム (DLI 領域) でロードする。
- トランザクション: Ims-spoc ジョブで定義する。

`tools/bankofz-ims/session.txt` は次の流れを確かめる。
- ログイン: PL/I の IBLOGIN。パスワード違い、成功、二重ログインの 3 通り。
- 口座の要約 (IBACSUM): 存在しない顧客も含む。
- 顧客情報の取得 (IBGCUDAT) と更新 (IBSCUDAT): 2 進の顧客番号を含む入力。
- ログアウト (IBLOGOUT)。

IBTRAN はオブジェクト指向 COBOL から Java を呼ぶので対象外 (0007 の 2 章)。

## 7. テスト

- `crates/zeus-ims/tests`: 流用元のテスト。
  - 単体テスト (`core.rs`、`call_adapter.rs`、`gsam.rs`、`tm.rs`、`cli.rs`)。
  - `reference.rs` (`--ignored`、`.cache` の CardDemo と Bank of Z の実資材)。
- `crates/zeus-jcl/tests/jobs.rs` の `ims_batch_regions`: 小さな DBD/PSB と COBOL 3 本を使う。
  - **DLI** (CBLTDLI): ISRT と重複 (II)、GHU/REPL、GE、DLET、GN の巡回 (GA/GB)。
  - **BMP** (EXEC DLI): WHERE 付きの GU、GNP、ISRT、異常終了。
  - **DBB**: 異常終了したステップの挿入が取り消されていることを確認する。
- `tools/carddemo-ims.sh test`: 4 章の内容。

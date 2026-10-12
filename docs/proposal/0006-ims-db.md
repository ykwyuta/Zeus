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
| データベース | 全 DBD を 1 つの WAL に保持する。zjob は `ZEUS_IMS_WAL`、なければカタログの隣の `ims/ims.wal`。zcics は `--ims-wal` (既定は同じ場所)。WAL は OS のファイルロックで排他するので、zcics とバッチジョブは同時には開けない |
| `EXEC PGM=DFSRRC00` | `PARM='DLI\|DBB\|BMP,プログラム,PSB'`。ライブラリは IMS DD の連結。プログラムには PCB マスクを USING で渡す (BMP と CMPAT=YES では I/O PCB が先頭)。正常終了でコミット、異常終了でバックアウト。`PARM=(ULR,DFSURGL0,DBD)` は DD DFSUINPT のアンロード (DFSURGU0 形式) から再ロードする |
| CALL 'CBLTDLI' | 引数の個数 (省略可)、関数、PCB、入出力域、SSA。DB PCB: GU/GN/GNP と GHU/GHN/GHNP、ISRT/REPL/DLET (可変長セグメントは先頭の長さで書く)。GSAM PCB: OPEN/CLSE/GN/GU/ISRT と RSA。I/O PCB: CHKP (ID)、SYNC、ROLB。バッチでは GU/GN は QC |
| EXEC DLI | SEGMENT、WHERE (フィールド・比較演算子・COBOL のデータ項目かリテラル)、INTO/FROM、PCB(n)、CHKP ID、SCHD PSB/TERM。結果は DIB (DIBSTAT、DIBSEGM、DIBSEGLV、DIBKFBL、DIBDBDNM、DIBDBORG) に返す。取得コマンドは保持付きの取得として扱う。SEGMENT で現在のパス上の親を指定した DLET/REPL は、キーフィードバックからその親を保持し直してから実行する (子を GNP で読んだ後に親を削除する CardDemo の書き方) |
| CICS (zcics) | `--ims-lib` (カタログの PDS 名かディレクトリ、複数可) と `--ims-wal`。タスクの EXEC DLI SCHD で PSB をスケジュールし (二重のスケジュールは TC)、TERM・SYNCPOINT・タスク終了で作業単位を確定する |
| コンパイラ | EXEC DLI の WHERE を解析し、比較の右辺の COBOL 項目を実行時に渡す。DLZDIB (DIB) を EXEC DLI の結果の受け取り先として渡す |

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

- **PL/I の PLITDLI**: Bank of Z の IBLOGIN が使う。
- **IMS TM**:
  - メッセージ処理領域 (MPP) と、I/O PCB の GU/ISRT による端末入出力。Bank of Z の IMS 経路が使う。
  - キュー (`zeus_ims::tm`) は流用済みだが、実行時にはまだつないでいない。
- **COPAUA0C の起動経路**: MQ のトリガーで起動される承認処理 (CICS のトリガーモニターが必要)。
- **COPAUS2C の不正マーク (F5)**: Db2 を使う。ZEUS_DB2 の環境で別途確認する。
- **アンロードユーティリティ DFSURGU0**: 書き出しは未対応。再ロード DFSURGL0 は対応済み。
- **DL/I の機能の範囲**:
  - コマンドコード、ブール演算子 (AND/OR) で結んだ修飾。
  - 等号以外の比較 (流用元の制約)。
  - パス呼出し (D コマンドコード)。
  - XRST、記号チェックポイント。
- **WAL の共有**: zcics とバッチの同時アクセス。流用元の提案にある常駐サーバー (imsdbd) が必要。

## 6. テスト

- `crates/zeus-ims/tests`: 流用元のテスト。
  - 単体テスト (`core.rs`、`call_adapter.rs`、`gsam.rs`、`tm.rs`、`cli.rs`)。
  - `reference.rs` (`--ignored`、`.cache` の CardDemo と Bank of Z の実資材)。
- `crates/zeus-jcl/tests/jobs.rs` の `ims_batch_regions`: 小さな DBD/PSB と COBOL 3 本を使う。
  - **DLI** (CBLTDLI): ISRT と重複 (II)、GHU/REPL、GE、DLET、GN の巡回 (GA/GB)。
  - **BMP** (EXEC DLI): WHERE 付きの GU、GNP、ISRT、異常終了。
  - **DBB**: 異常終了したステップの挿入が取り消されていることを確認する。
- `tools/carddemo-ims.sh test`: 4 章の内容。

# Proposal 0002: z/OS 互換アプリケーション実行環境 (ソース互換) 作業計画

- 状態: 提案 (Draft)
- 置き換え対象: [0001 z/Architecture エミュレータ作業計画](0001-zos-emulator-work-plan.md) の主目標
- ライセンス: GPL-3.0 (リポジトリ既定)

## 1. 背景と目標

要求は「2010 年頃に z/OS 上で作られた COBOL / PL/I / CICS / JCL アプリケーションの
実行環境」であり、検証用アプリケーションとして以下の **ソースコード** を使う。

- AWS CardDemo (`aws-samples/aws-mainframe-modernization-carddemo`)
- GenApp — CICS General Insurance Application (`cicsdev/cics-genapp`)
- Bank of Z (`IBM/Bank-of-Z`, CICS Banking Sample Application (CBSA) の後継)

### 目標

> **CardDemo・GenApp・Bank of Z を、ソースを書き換えずに Zeus 上でビルドし、
> JCL バッチと CICS オンライン (3270 画面) の両方を z/OS 上と同等に実行できること。**

言語・ミドルウェアの互換水準は z/OS 1.12 / Enterprise COBOL 4.2 / CICS TS 4.1 /
DB2 9 世代を基準とし、サンプルが実際に使う機能を優先して実装する。

## 2. 対象アプリケーションが使う技術

| 技術 | CardDemo (基本) | CardDemo (オプション) | GenApp | Bank of Z |
|---|---|---|---|---|
| COBOL | ○ | ○ | ○ | ○ |
| JCL / PROC / GDG | ○ | ○ | ○ | ○ (batch) |
| VSAM (KSDS/AIX/ESDS/RRDS) | ○ | ○ | ○ | ? |
| CICS + BMS (3270) | ○ | ○ | ○ | ○ |
| Db2 (EXEC SQL) | – | ○ | ○ | ○ |
| IMS DB / IMS TM | – | ○ (DB) | – | ○ (TM 経路) |
| MQ | – | ○ | – | – |
| Assembler | ○ (MVSWAIT, COBDATFT の 2 本) | – | – | – |
| PL/I | – | – | – | – |

- 最初の作業 (Phase 0) で全ソースを走査し、`EXEC CICS` / `EXEC SQL` / `EXEC DLI` / `CALL 'CBLTDLI'` /
  MQI 呼び出し / JCL の `PGM=` / ユーティリティ制御文 を自動集計した **API 使用一覧表** を作り、
  それを実装範囲 (スコープ) の正とする。上表の「?」もここで確定させる。
- **PL/I は 3 アプリとも使っていない**ため、本計画では対象外とし、後続提案に回す。
- z/OS Connect、Web フロントエンド、CICSPlex SM、Workload Simulator はスコープ外。

## 3. 方式の選定

| 案 | 内容 | 判定 |
|---|---|---|
| A. マシンエミュレータ上で z/OS 本体を動かす (0001 案) | Hercules 同等のハードウェアエミュレータを作り z/OS + CICS + Db2 を載せる | **不採用**。z/OS・CICS・Db2 は IBM 実機または IBM 公認エミュレータ (ZD&T) 以外での利用が許諾されない |
| B. CPU エミュレータ + z/OS API 再実装でロードモジュールを実行 | COBOL をロードモジュールにし、Zeus がその機械語を実行 | **不採用**。手元にあるのはソースのみで、ロードモジュールを生成できる Enterprise COBOL は z/OS 上でしか動かない |
| C. **ソース互換ランタイム** | COBOL ソースをホスト向けにコンパイルし、JCL・データセット・VSAM・CICS・Db2・IMS・MQ の各サブシステムを Rust で再実装 | **採用** |

案 C は、商用のリホスト製品と同じ考え方をとる。Zeus はハードウェアではなく
**z/OS の「サブシステム」をエミュレートする**ことになる。0001 の CPU エミュレータは
将来の選択肢として残す (§8)。

## 4. アーキテクチャ

```
           ┌──────────── COBOL ソース (+ copybook, BMS, CSD, JCL, DBD/PSB, DDL) ───────────┐
           │                                                                                  │
   zeus-xlat: EXEC CICS / EXEC SQL / EXEC DLI トランスレータ                     zeus-bms: BMS マクロ → 物理/シンボリックマップ
           │                                                                                  │
   COBOL コンパイラ (Phase 1〜: GnuCOBOL, IBM 方言モード)                                     │
           │  共有ライブラリ                                                                  │
           ▼                                                                                  ▼
 ┌────────────────────────────── Zeus ランタイム (Rust) ──────────────────────────────────────────┐
 │ zeus-jcl   JCL 解析・実行, PROC/INCLUDE/SET, COND/IF, DISP, GDG, SYSOUT, 内部リーダ, ジョブキュー │
 │ zeus-ds    カタログ, 順次 (FB/VB/FBA), PDS/PDSE, VSAM KSDS/ESDS/RRDS/AIX, GDG, EBCDIC 保存       │
 │ zeus-utils IEFBR14, IEBGENER, IEBCOPY, IDCAMS, SORT (DFSORT サブセット), DSNTEP2/DSNTIAUL 相当  │
 │ zeus-cics  リージョン, CSD 定義, タスク制御, COMMAREA/チャネル, ファイル制御, TS/TD キュー,       │
 │            BMS, START/RETRIEVE, SYNCPOINT, HANDLE CONDITION/ABEND, 名前付きカウンタ              │
 │ zeus-3270  TN3270(E) サーバ                                                                    │
 │ zeus-db2   SQLCA/SQLCODE 互換層 + SQL 方言変換 + バックエンド                                    │
 │ zeus-ims   DL/I (DBD/PSB, GU/GN/GNP/ISRT/REPL/DLET), IMS TM メッセージ処理                      │
 │ zeus-mq    MQI サブセット (MQCONN/MQOPEN/MQPUT/MQGET), ローカルキュー                           │
 │ zeus-txn   VSAM・TS キュー・Db2・MQ をまたぐ作業単位 (コミット/ロールバック)                      │
 └───────────────────────────────────────────────────────────────────────────────────────────────┘
```

設計方針:

1. **データは EBCDIC のまま保持**する。z/OS で作ったデータファイル (CardDemo の `app/data/EBCDIC` 等) を
   無変換で使え、VSAM のキー順・SORT の照合順序・COMP-3 / ゾーン 10 進の挙動が z/OS と一致する。
   ASCII への変換は 3270 端末・ログ表示・外部連携の境界でのみ行う。
2. **COBOL コンパイラ**: 最初は GnuCOBOL (IBM 方言モード、EBCDIC 照合) をフロントエンドに使い、
   Rust ランタイムは C ABI で呼び出す。Enterprise COBOL 4.2 との挙動差は互換オプション
   (TRUNC / NUMPROC / ARITH 相当) と差分テストで管理する。Rust 製の COBOL コンパイラは §8 の任意トラックとする。
3. **トランスレータは IBM の変換規約に合わせる**: `DFHEIBLK` / `DFHCOMMAREA` / `EIBRESP` / `SQLCA` など、
   アプリが直接参照する制御ブロックのレイアウトは z/OS と同じにする。
4. **Db2 バックエンド**: PostgreSQL を標準とし、Db2 方言 (FETCH FIRST, 日付時刻関数, SQLCODE 対応表等) を
   変換層で吸収する。Db2 for LUW Community Edition を接続可能な第 2 バックエンド兼オラクルとする。
5. **Assembler の 2 本**: CardDemo の MVSWAIT と COBDATFT は、同じ呼び出し仕様の Rust 実装で置き換える。

## 5. 比較テスト対象 (オラクル) の位置づけ

z/OS 本体を検証に使えないため、機能ごとに最も信頼できる比較対象を使い分ける。

| 検証対象 | オラクル | 方法 |
|---|---|---|
| JCL の解釈・ステップ実行・COND・DISP・GDG・SYSOUT | **Hercules + MVS 3.8j (TK4-/TK5) の JES2** | 同じ JCL を両方で実行し、ステップ完了コード・データセット状態・SYSOUT を比較 |
| VSAM / IDCAMS (DEFINE, REPRO, PRINT, AIX, キー順) | **MVS 3.8j の VSAM / IDCAMS** | 同一データでの REPRO・PRINT 出力、キー順の読み出し結果を比較 |
| IEBGENER / IEBCOPY / SORT、EBCDIC 照合順序 | **MVS 3.8j のユーティリティ** | 出力データセットのバイト単位の比較 |
| 10 進演算 (COMP-3・ゾーン 10 進)、COBOL 基本文の結果 | **MVS 3.8j の COBOL** (旧規格) | 両方で書ける範囲の検査プログラムで演算結果を比較 |
| CICS API の基本動作、BMS 画面 | **KICKS for TSO** (MVS 3.8j 上の CICS 互換。ライセンス要確認) と IBM CICS 文書 | 3270 画面スナップショット・EIBRESP の比較 |
| SQL の意味 (結果集合・SQLCODE) | **Db2 for LUW Community Edition** | 同じ SQL を Zeus と Db2 CE で実行し比較 |
| アプリ全体の動作 | 各アプリの文書と同梱データ、(任意) ZD&T・商用リホスト環境 | シナリオテストのゴールデン出力 |

- 新しいテスト用 JCL・データを作るたびに、まず MVS 3.8j で実行して「正解」を記録し、Zeus の回帰テストにする。
- Db2 / IMS / MQ / 新しい CICS API には無償のオラクルが少ない。そのため IBM マニュアルに準拠した
  仕様テストを手書きし、ライセンス保持者が ZD&T で取った結果があれば随時取り込む。

## 6. テスト戦略

1. **単体テスト**: データセット・VSAM・JCL 解析・SQL 変換・DL/I などの各クレートごと。
2. **オラクル差分テスト**: §5 の表に基づき、MVS 3.8j と Db2 CE を CI コンテナで起動して自動比較 (夜間)。
3. **アプリシナリオテスト**:
   - バッチ: 各アプリのジョブを順に流し、出力データセット・レポート・完了コードをゴールデンと比較。
   - オンライン: x3270 系のスクリプト端末 (`s3270`) で TN3270 に接続し、画面操作シナリオを再生する。
     各画面のスナップショットと、実行後のファイル・テーブル内容を検証する。
4. **進捗指標**: API 使用一覧表 (Phase 0) に対する実装率、アプリごとのシナリオ合格数。

## 7. フェーズ別作業計画

1〜2 名 + AI 支援体制を想定した目安。各フェーズの完了条件を満たしてから次に進む。

### Phase 0: 基盤と棚卸し (3 週)
- Cargo ワークスペース、CI、`CONTRIBUTING.md` (クリーンルーム方針、Hercules・IBM コード非参照)。
- 3 アプリのソースを取得するスクリプト (各ライセンス確認の上、サブモジュールまたは取得スクリプト)。
- **API 使用一覧表の自動生成ツール** (EXEC CICS/SQL/DLI、MQI、JCL の PGM・DD・ユーティリティ制御文)。
- Hercules + TK4- (または TK5) を CI で起動し、JCL を投入して SYSOUT を回収する自動化。
- **完了条件**: 一覧表が生成され、MVS 3.8j で JCL を投げて結果を取るテストが CI で動く。

### Phase 1: データ層と JCL バッチ (8〜10 週)
- データセットとカタログ、順次ファイル、PDS、VSAM (KSDS/AIX/ESDS/RRDS)、GDG。
- JCL エンジン (PROC, 記号置換, COND/IF, DISP, 一時データセット, SYSOUT, 内部リーダ)。
- IEFBR14 / IEBGENER / IDCAMS / SORT のサブセット (CardDemo が使う範囲)。
- GnuCOBOL 連携 (EBCDIC、COBOL ファイル I/O を Zeus のデータセットに接続)。
- MVSWAIT・COBDATFT の Rust 置き換え。
- **完了条件**: CardDemo の基本バッチ (データ初期ロード、取引記帳、利息計算、明細作成など) が全て完了し、
  出力がゴールデンと一致。JCL・IDCAMS・SORT の MVS 3.8j 差分テストが合格。

### Phase 2: CICS コア + BMS + TN3270 → CardDemo オンライン (10〜12 週)
- EXEC CICS トランスレータ、CSD 定義の読み込み (DFHCSDUP 入力形式)。
- タスク制御、擬似会話型 (RETURN TRANSID COMMAREA)、LINK/XCTL、HANDLE 系、EIB。
- ファイル制御 (READ/WRITE/REWRITE/DELETE/STARTBR/READNEXT/READPREV)、TS/TD キュー、
  ASKTIME/FORMATTIME、SYNCPOINT/ROLLBACK。
- BMS マップ生成と SEND/RECEIVE MAP、TN3270 サーバ。
- **完了条件**: CardDemo の全オンライン取引 (サインオン、メニュー、口座・カード照会/更新、取引一覧、
  請求支払、管理者機能) が 3270 シナリオテストで合格。

### Phase 3: Db2 → GenApp (8〜10 週)
- EXEC SQL トランスレータ、SQLCA、ホスト変数・標識変数、カーソル、DCLGEN copybook。
- PostgreSQL バックエンドと Db2 方言変換、Db2 CE オラクル比較。
- CICS と Db2 の作業単位の統合 (SYNCPOINT で VSAM と Db2 を同時にコミット)。
- バッチ用 Db2 ユーティリティ相当 (DDL 実行、DSNTEP2/DSNTIAUL 相当、ロード)。
- **完了条件**: GenApp の顧客・保険契約の登録/照会/更新/削除の全シナリオが合格。
  CardDemo の取引種別管理モジュール (Db2) も合格。

### Phase 4: Bank of Z (CICS 経路) (6〜8 週)
- Phase 0 の一覧表で判明した追加 CICS API (チャネル/コンテナ、名前付きカウンタ等) の実装。
- Bank of Z のバッチ部品。
- **完了条件**: Bank of Z の CICS 経路 (顧客・口座の作成/照会、入出金、振替) のシナリオが合格。

### Phase 5: IMS と MQ (12〜14 週)
- DL/I (DBD/PSB の解析、階層データストア、GU/GN/GNP/ISRT/REPL/DLET、SSA)、IMS バッチ (DFSRRC00 相当)。
- IMS TM (メッセージ処理プログラム、入出力 PCB、トランザクションのルーティング)。
- MQI サブセットとローカルキュー、CICS/IMS との作業単位統合。
- **完了条件**: Bank of Z の IMS 経路、CardDemo のオプション (承認: IMS+Db2+MQ、口座抽出: VSAM+MQ) が合格。

### Phase 6: 運用性・性能 (継続)
- 複数端末の同時実行、ロック・エンキュー、異常終了時の回復、ジョブスケジューラ連携
  (CardDemo の `scheduler` 定義の取り込み)、監視・ログ、ドキュメント。

## 8. 任意トラック (将来)

- **Rust 製 COBOL コンパイラ**: GnuCOBOL の互換性差が大きい場合、または Enterprise COBOL 固有挙動の
  完全再現が必要になった場合に着手する。
- **PL/I**: PL/I 資産を扱う必要が出た時点で別提案にする (フロントエンドの選定から行う)。
- **z/Architecture CPU エミュレータ (0001 案)**: Assembler 資産やロードモジュールしか残っていない
  プログラムを扱う必要が出た場合、命令実行部分だけを (アプリ用の命令に絞って) 0001 の Phase 1/5 の
  方針で実装し、Zeus ランタイムの SVC・マクロ層と接続する。

## 9. マイルストーン

| マイルストーン | 到達内容 | 目安 (累積) |
|---|---|---|
| M0 | CI・MVS 3.8j オラクル自動化・API 使用一覧表 | 〜3 週 |
| M1 | **CardDemo バッチ稼働** | 〜3 ヶ月 |
| M2 | **CardDemo オンライン稼働** | 〜6 ヶ月 |
| M3 | GenApp 稼働 (CICS + Db2 + VSAM) | 〜8 ヶ月 |
| M4 | Bank of Z (CICS 経路) 稼働 | 〜10 ヶ月 |
| M5 | IMS・MQ (Bank of Z IMS 経路、CardDemo オプション) | 〜13 ヶ月 |

## 10. リスクと対策

| リスク | 対策 |
|---|---|
| GnuCOBOL と Enterprise COBOL の挙動差 (2 進項目の切り捨て、符号処理、照合順序など) | 互換オプションを固定し、差分を一覧化。必要なら §8 のコンパイラに移行 |
| サンプルが 2010 年以降の新しい API (CICS TS 5.x/6.x、新しい COBOL 構文) を使う | Phase 0 の一覧表で使用箇所を洗い出し、個別に対応可否を判断する |
| CICS・Db2・IMS・MQ に無償の完全なオラクルがない | IBM マニュアル準拠の仕様テスト、KICKS・Db2 CE の活用、ZD&T 結果の取り込み |
| Db2 方言を PostgreSQL に変換しきれない | Db2 CE バックエンドに切り替えられる設計にする |
| 作業単位 (VSAM + Db2 + MQ) の整合 | Phase 2 から `zeus-txn` を設けて 2 相コミット相当の枠組みを用意 |
| IBM の商標・ライセンス (BMS マクロ定義、copybook 等) | IBM 提供物は取り込まず、仕様に基づいて再作成する |

## 11. 決定が必要な事項

1. COBOL コンパイラを当面 GnuCOBOL で進めてよいか (推奨: はい)。
2. データを EBCDIC のまま保持する方針でよいか (推奨: はい)。
3. Db2 バックエンドの第一候補 (推奨: PostgreSQL、Db2 CE は比較・切替用)。
4. IMS (Phase 5) を本計画に含めるか、CICS + Db2 の完成後に別提案とするか。
5. PL/I を本計画の対象外としてよいか。

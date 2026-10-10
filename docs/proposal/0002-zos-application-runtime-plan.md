# Proposal 0002: z/OS 互換アプリケーション実行環境 (ソース互換) 作業計画

- 状態: 提案 (Draft)
- 置き換え対象: [0001 z/Architecture エミュレータ作業計画](0001-zos-emulator-work-plan.md) の主目標
- 関連: [0003 Rust 製 COBOL コンパイラ](0003-cobol-compiler.md) (COBOL のコンパイルは外部処理系に依存せず自作する)
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
- **PL/I は 3 アプリとも使っていない**ため本計画には含めず、**次のフェーズ** (別提案) で対応する。
- **IMS (DB / TM) も次のフェーズ** (別提案) で対応する。本計画では Bank of Z は CICS 経路のみ、
  CardDemo は IMS を使う承認モジュールを除いた範囲を対象とする。
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
   zeus-cobol: Rust 製 COBOL コンパイラ (0003)                                    zeus-bms: BMS マクロ → 物理/シンボリックマップ
     EXEC CICS / EXEC SQL / EXEC DLI を統合処理                                               │
           │  プログラムオブジェクト (ZIR)                                                    │
           ▼                                                                                  ▼
 ┌────────────────────────────── Zeus ランタイム (Rust) ──────────────────────────────────────────┐
 │ zeus-jcl   JCL 解析・実行, PROC/INCLUDE/SET, COND/IF, DISP, GDG, SYSOUT, 内部リーダ, ジョブキュー │
 │ zeus-ds    カタログ, 順次 (FB/VB/FBA), PDS/PDSE, VSAM KSDS/ESDS/RRDS/AIX, GDG, EBCDIC 保存       │
 │ zeus-utils IEFBR14, IEBGENER, IEBCOPY, IDCAMS, SORT (DFSORT サブセット), DSNTEP2/DSNTIAUL 相当  │
 │ zeus-cics  リージョン, CSD 定義, タスク制御, COMMAREA/チャネル, ファイル制御, TS/TD キュー,       │
 │            BMS, START/RETRIEVE, SYNCPOINT, HANDLE CONDITION/ABEND, 名前付きカウンタ              │
 │ zeus-3270  TN3270(E) サーバ                                                                    │
 │ zeus-db2   SQLCA/SQLCODE 互換層 + バックエンド (第 1: Db2 CE, 第 2: PostgreSQL)                 │
 │ zeus-ims   (次フェーズ) DL/I, IMS TM メッセージ処理                                             │
 │ zeus-mq    MQI サブセット (MQCONN/MQOPEN/MQPUT/MQGET), ローカルキュー                           │
 │ zeus-txn   VSAM・TS キュー・Db2・MQ をまたぐ作業単位 (コミット/ロールバック)                      │
 └───────────────────────────────────────────────────────────────────────────────────────────────┘
```

設計方針:

1. **データは EBCDIC のまま保持**する。z/OS で作ったデータファイル (CardDemo の `app/data/EBCDIC` 等) を
   無変換で使え、VSAM のキー順・SORT の照合順序・COMP-3 / ゾーン 10 進の挙動が z/OS と一致する。
   ASCII への変換は 3270 端末・ログ表示・外部連携の境界でのみ行う。
2. **COBOL コンパイラは自作 (zeus-cobol, 0003)**: 外部の COBOL 処理系に依存しない。
   記憶域を z/OS と同じバイト配置で扱い、Enterprise COBOL 4.2 の TRUNC / NUMPROC / ARITH や
   中間結果の桁数規則を仕様どおりに実装する。JCL の `PGM=IGYCRCTL` を zeus-cobol に対応させ、
   アプリ同梱のコンパイル用 JCL をそのまま使えるようにする。
3. **埋め込み文はコンパイラに統合し、IBM の変換規約に合わせる**: `DFHEIBLK` / `DFHCOMMAREA` / `EIBRESP` /
   `SQLCA` など、アプリが直接参照する制御ブロックのレイアウトは z/OS と同じにする。
4. **Db2 バックエンド**: **Db2 for LUW Community Edition を第 1 バックエンド**として先に開発する。
   SQL は原則そのまま Db2 CE に渡し、Db2 for z/OS と LUW の方言差だけを変換層で吸収する。
   EBCDIC のホスト変数は境界で変換し、データベースは EBCDIC 照合 (例: `COLLATE USING EBCDIC_819_037`,
   要検証) で作成して z/OS と同じ並び順を得る。Db2 CE は再配布できないため、利用者が各自導入し、
   CI では公式コンテナイメージを使う。
   **PostgreSQL は第 2 バックエンド**として次に対応し、その際は Db2 CE をオラクルとして結果を比較する。
5. **Assembler の 2 本**: CardDemo の MVSWAIT と COBDATFT は、同じ呼び出し仕様の Rust 実装で置き換える。

## 5. 比較テスト対象 (オラクル) の位置づけ

z/OS 本体を検証に使えないため、機能ごとに最も信頼できる比較対象を使い分ける。

| 検証対象 | オラクル | 方法 |
|---|---|---|
| JCL の解釈・ステップ実行・COND・DISP・GDG・SYSOUT | **Hercules + MVS 3.8j (TK4-/TK5) の JES2** | 同じ JCL を両方で実行し、ステップ完了コード・データセット状態・SYSOUT を比較 |
| VSAM / IDCAMS (DEFINE, REPRO, PRINT, AIX, キー順) | **MVS 3.8j の VSAM / IDCAMS** | 同一データでの REPRO・PRINT 出力、キー順の読み出し結果を比較 |
| IEBGENER / IEBCOPY / SORT、EBCDIC 照合順序 | **MVS 3.8j のユーティリティ** | 出力データセットのバイト単位の比較 |
| 10 進演算 (COMP-3・ゾーン 10 進)、編集 | **Hercules** の 10 進命令 (AP/MP/DP/ED 等) | zeus-decimal と命令単位で差分テスト (0003 §5) |
| COBOL 基本文の結果 | **MVS 3.8j の COBOL** (旧規格) | 両方で書ける範囲の検査プログラムで演算結果を比較 |
| COBOL-85 規格の適合性 | **NIST CCVS85 テストスイート** | モジュール別の合格率を CI で追跡 (0003 §5) |
| CICS API の基本動作、BMS 画面 | **KICKS for TSO** (MVS 3.8j 上の CICS 互換。ライセンス要確認) と IBM CICS 文書 | 3270 画面スナップショット・EIBRESP の比較 |
| SQL の意味 (結果集合・SQLCODE) | Db2 for z/OS の SQL リファレンス、Db2 CE | 第 1 バックエンドは Db2 CE そのもの。z/OS との方言差は仕様テストで確認。PostgreSQL 対応時は Db2 CE の結果をオラクルにする |
| COBOL の実行結果全般 | **GnuCOBOL** (IBM 方言モード) | 同じプログラムを zeus-cobol と GnuCOBOL で実行して比較 (0003 §5.1)。製品には組み込まない |
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
COBOL コンパイラ側の細目 (C0〜C6) は [0003](0003-cobol-compiler.md) §7 を参照。

### Phase 0: 基盤と棚卸し (3 週) — コンパイラ C0 を含む
- Cargo ワークスペース、CI、`CONTRIBUTING.md` (クリーンルーム方針、Hercules・IBM コード非参照)。
- 3 アプリのソースを取得するスクリプト (各ライセンス確認の上、サブモジュールまたは取得スクリプト)。
- **API 使用一覧表と COBOL 構文使用一覧表の自動生成ツール**
  (EXEC CICS/SQL/DLI、MQI、JCL の PGM・DD・ユーティリティ制御文、COBOL の文・句・PIC・USAGE)。
- Hercules + TK4- (または TK5) を CI で起動し、JCL を投入して SYSOUT を回収する自動化。
- NIST CCVS85 テストスイートを CI で実行するハーネス。
- **完了条件**: 2 つの一覧表が生成され、MVS 3.8j と CCVS85 のテストが CI で動く。

### Phase 1: COBOL コンパイラ中核 + データ層・JCL (14〜18 週, 2 本並行)
- **コンパイラ (0003 C1〜C2)**: ソース管理・構文解析・データ配置計算、zeus-decimal、
  MOVE/算術/条件/制御文、ZIR インタプリタ。
- **データ層・JCL (並行)**: データセットとカタログ、順次ファイル、PDS、VSAM (KSDS/AIX/ESDS/RRDS)、GDG。
  JCL エンジン (PROC, 記号置換, COND/IF, DISP, 一時データセット, SYSOUT, 内部リーダ)。
  IEFBR14 / IEBGENER / IDCAMS / SORT のサブセット。
- **完了条件**: 3 アプリの全 COBOL ソースが構文解析でき、CCVS85 の NC モジュールの大部分に合格、
  10 進命令の Hercules 差分でも不一致ゼロ。JCL・IDCAMS・SORT の MVS 3.8j 差分テストが合格。

### Phase 2: CardDemo バッチ (6〜8 週) — コンパイラ C3
- COBOL ファイル I/O をデータ層に接続、CALL・入れ子プログラム・組み込み関数。
- `PGM=IGYCRCTL` を zeus-cobol に対応させ、アプリ同梱のコンパイル JCL で全プログラムをビルド。
- MVSWAIT・COBDATFT の Rust 置き換え。
- **完了条件**: CardDemo の基本バッチ (データ初期ロード、取引記帳、利息計算、明細作成など) が全て完了し、
  出力がゴールデンと一致。CCVS85 の SQ/IX/RL/IC に合格。

### Phase 3: CICS コア + BMS + TN3270 → CardDemo オンライン (10〜12 週) — コンパイラ C4
- EXEC CICS のコンパイラ統合、CSD 定義の読み込み (DFHCSDUP 入力形式)。
- タスク制御、擬似会話型 (RETURN TRANSID COMMAREA)、LINK/XCTL、HANDLE 系、EIB。
- ファイル制御 (READ/WRITE/REWRITE/DELETE/STARTBR/READNEXT/READPREV)、TS/TD キュー、
  ASKTIME/FORMATTIME、SYNCPOINT/ROLLBACK。
- BMS マップ生成と SEND/RECEIVE MAP、TN3270 サーバ。
- **完了条件**: CardDemo の全オンライン取引 (サインオン、メニュー、口座・カード照会/更新、取引一覧、
  請求支払、管理者機能) が 3270 シナリオテストで合格。

### Phase 4: Db2 (Db2 CE) → GenApp (8〜10 週) — コンパイラ C5 (SQL)
- EXEC SQL のコンパイラ統合、SQLCA、ホスト変数・標識変数、カーソル、DCLGEN copybook。
- Db2 CE バックエンド (Db2 for z/OS との方言差の変換、EBCDIC 変換、EBCDIC 照合のデータベース)。
- CICS と Db2 の作業単位の統合 (SYNCPOINT で VSAM と Db2 を同時にコミット)。
- バッチ用 Db2 ユーティリティ相当 (DDL 実行、DSNTEP2/DSNTIAUL 相当、ロード)。
- **完了条件**: GenApp の顧客・保険契約の登録/照会/更新/削除の全シナリオが合格。
  CardDemo の取引種別管理モジュール (Db2) も合格。

### Phase 5: Bank of Z (CICS 経路) (6〜8 週)
- Phase 0 の一覧表で判明した追加 CICS API (チャネル/コンテナ、名前付きカウンタ等) の実装。
- Bank of Z のバッチ部品。
- **完了条件**: Bank of Z の CICS 経路 (顧客・口座の作成/照会、入出金、振替) のシナリオが合格。

### Phase 6: MQ と PostgreSQL バックエンド (8〜10 週)
- MQI サブセット (MQCONN/MQOPEN/MQPUT/MQGET) とローカルキュー、CICS との作業単位統合。
- PostgreSQL バックエンド (Db2 方言変換、SQLCODE 対応表)。Db2 CE で全シナリオを流した結果をオラクルとして比較。
- **完了条件**: CardDemo の口座抽出モジュール (VSAM+MQ) が合格。GenApp・Bank of Z・CardDemo の Db2 シナリオが
  PostgreSQL バックエンドでも Db2 CE と同じ結果になる。

### Phase 7: 運用性・性能 (継続) — コンパイラ C6
- Cranelift ネイティブコード生成、ソースレベルデバッガ。
- 複数端末の同時実行、ロック・エンキュー、異常終了時の回復、ジョブスケジューラ連携
  (CardDemo の `scheduler` 定義の取り込み)、監視・ログ、ドキュメント。

## 8. 次のフェーズ (別提案) と任意トラック

次のフェーズ (本計画の完了後に別提案):
- **IMS**: EXEC DLI / CALL 'CBLTDLI' のコンパイラ統合、DL/I (DBD/PSB、階層データストア、GU/GN/GNP/ISRT/REPL/DLET、SSA)、
  IMS バッチ (DFSRRC00 相当)、IMS TM。対象は Bank of Z の IMS 経路と CardDemo の承認モジュール (IMS+Db2+MQ)。
- **PL/I**: zeus-cobol の中間表現 (ZIR)・zeus-decimal・ランタイムを共用し、PL/I フロントエンドを追加する。

任意トラック:
- **z/Architecture CPU エミュレータ (0001 案)**: Assembler 資産やロードモジュールしか残っていない
  プログラムを扱う必要が出た場合、命令実行部分だけを (アプリ用の命令に絞って) 0001 の Phase 1/5 の
  方針で実装し、Zeus ランタイムの SVC・マクロ層と接続する。10 進演算は zeus-decimal を共用する。

## 9. マイルストーン

| マイルストーン | 到達内容 | 目安 (累積) |
|---|---|---|
| M0 | CI・MVS 3.8j / CCVS85 オラクル自動化・2 つの一覧表 | 〜3 週 |
| M1 | COBOL コンパイラ中核 (3 アプリ全ソースの解析, CCVS85 NC), データ層・JCL | 〜4.5 ヶ月 |
| M2 | **CardDemo バッチ稼働 (自作コンパイラで)** | 〜6 ヶ月 |
| M3 | **CardDemo オンライン稼働** | 〜9 ヶ月 |
| M4 | GenApp 稼働 (CICS + Db2 CE + VSAM) | 〜11 ヶ月 |
| M5 | Bank of Z (CICS 経路) 稼働 | 〜13 ヶ月 |
| M6 | MQ (CardDemo 口座抽出) と PostgreSQL バックエンド | 〜15 ヶ月 |

## 10. リスクと対策

| リスク | 対策 |
|---|---|
| COBOL コンパイラの工数超過 (文法の巨大さ、算術規則の再現) | 構文使用一覧表で範囲を限定。CCVS85・Hercules 10 進差分で早期に誤りを検出 (0003 §9) |
| サンプルが 2010 年以降の新しい API (CICS TS 5.x/6.x、新しい COBOL 構文) を使う | Phase 0 の一覧表で使用箇所を洗い出し、個別に対応可否を判断する |
| CICS・Db2 for z/OS・MQ に無償の完全なオラクルがない | IBM マニュアル準拠の仕様テスト、KICKS・Db2 CE の活用、ZD&T 結果の取り込み |
| Db2 CE の制約 (再配布不可・リソース上限・LUW と z/OS の方言差) | 利用者導入の前提を明記。方言差は変換層と仕様テストで管理し、PostgreSQL を第 2 バックエンドとして用意 |
| Db2 方言を PostgreSQL に変換しきれない | Db2 CE の結果をオラクルに差分を一覧化し、変換できない構文は文書化する |
| 作業単位 (VSAM + Db2 + MQ) の整合 | Phase 3 から `zeus-txn` を設けて 2 相コミット相当の枠組みを用意 |
| IBM の商標・ライセンス (BMS マクロ定義、copybook 等) | IBM 提供物は取り込まず、仕様に基づいて再作成する |

## 11. 決定事項

1. データは EBCDIC のまま保持する。
2. Db2 バックエンドは Db2 CE を先に開発し、PostgreSQL を次に対応する。
3. IMS は次のフェーズ (別提案) とする。
4. PL/I は次のフェーズ (別提案) とする。
5. GnuCOBOL は製品には組み込まず、比較用のオラクルとして積極的に活用する。

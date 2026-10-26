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

- Phase 3 の冒頭で全ソースを走査し、`EXEC CICS` / `EXEC SQL` / `EXEC DLI` / `CALL 'CBLTDLI'` /
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
 │ zeus-ims   DL/I (DBD/PSB, 階層ストア, WAL), CBLTDLI/EXEC DLI, DFSRRC00 (0006)。IMS TM は次段階  │
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
4. **進捗指標**: CCVS85 のモジュール別合格率、API 使用一覧表 (Phase 3) に対する実装率、アプリごとのシナリオ合格数。

## 7. フェーズ別作業計画

1〜2 名 + AI 支援体制を想定した目安。各フェーズの完了条件を満たしてから次に進む。
COBOL コンパイラ側の細目 (C0〜C7) は [0003](0003-cobol-compiler.md) §7 を参照。
**最初の目標は CCVS85 の必須モジュールへの全件合格** (0003 §5.2) とし、アプリケーション対応はその後に行う。

### Phase 0: 基盤と CCVS85 ハーネス (3 週) — コンパイラ C0
- Cargo ワークスペース、CI、`CONTRIBUTING.md` (クリーンルーム方針、Hercules・IBM コード非参照)。
- **CCVS85 の取り出し・実行・結果集計ハーネス**と、GnuCOBOL による基準線の取得・CI 表示。
- Hercules の 10 進命令差分ハーネス (スタンドアロン実行。MVS 3.8j は不要)。
- **完了条件**: CCVS85 全モジュールの GnuCOBOL 基準線と zeus-cobol の結果 (全件不合格) が CI に並んで表示される。

### Phase 1: COBOL コンパイラ中核 (14〜18 週) — コンパイラ C1〜C2
- ソース管理 (固定形式, COPY/REPLACE)・構文解析・データ配置計算、zeus-decimal、
  MOVE/算術/条件/制御文/PERFORM、ZIR インタプリタ、DISPLAY/ACCEPT。
- **完了条件**: CCVS85 の全プログラムが構文解析でき、**NC・SM に合格**。10 進命令の Hercules 差分で不一致ゼロ。

### Phase 2: CCVS85 合格 (8〜10 週) — コンパイラ C3
- ファイル I/O (順次・相対・索引)。データセット層 (zeus-ds) の最小版として EBCDIC レコードで実装し、
  後の VSAM・JCL 対応にそのまま発展させる。ファイルの割り当ては当面、設定ファイルによる DD 名対応で行う。
  - 実装済み: `crates/zeus-ds` (COBOL 非依存)。DD 割り当て (明示指定 / `ZEUS_DD_CONFIG` の対応表 /
    `DD_<名前>` 環境変数)、格納形式 (固定長 / RDW 付き可変長 / テキスト行)、順次・相対 (RRDS 相当)・
    キー付き (KSDS + AIX 相当: キーはレコード内のオフセットと長さ、代替キーは索引を持ち重複は書き込み順)。
    FILE STATUS・レコード領域・印刷ファイルなど COBOL 固有の部分は zcobc の実行系に残す。
- SORT/MERGE、CALL・入れ子プログラム、組み込み関数。
- (並行で着手可) Hercules + MVS 3.8j の JCL 投入自動化、JCL エンジンの設計。
- **完了条件**: **CCVS85 の必須モジュール (NC/SM/IC/SQ/RL/IX/ST/IF) で FAIL ゼロ** (= 第 1 目標達成)。

### Phase 3: JCL・データ層 → CardDemo バッチ (6〜8 週) — コンパイラ C4
- 3 アプリのソース取得と **API 使用一覧表・COBOL 構文使用一覧表**の自動生成。
- データセットとカタログ、PDS、VSAM (KSDS/AIX/ESDS/RRDS)、GDG。
- JCL エンジン (PROC, 記号置換, COND/IF, DISP, 一時データセット, SYSOUT, 内部リーダ)。
  IEFBR14 / IEBGENER / IDCAMS / SORT のサブセット。JCL・IDCAMS・SORT の MVS 3.8j 差分テスト。
- Enterprise COBOL 拡張と `PGM=IGYCRCTL` 互換。MVSWAIT・COBDATFT の Rust 置き換え。
- **完了条件**: CardDemo の基本バッチ (データ初期ロード、取引記帳、利息計算、明細作成など) が全て完了し、
  出力がゴールデンと一致。JCL・IDCAMS・SORT の MVS 3.8j 差分テストが合格。

### Phase 4: CICS コア + BMS + TN3270 → CardDemo オンライン (10〜12 週) — コンパイラ C5
- EXEC CICS のコンパイラ統合、CSD 定義の読み込み (DFHCSDUP 入力形式)。
- タスク制御、擬似会話型 (RETURN TRANSID COMMAREA)、LINK/XCTL、HANDLE 系、EIB。
- ファイル制御 (READ/WRITE/REWRITE/DELETE/STARTBR/READNEXT/READPREV)、TS/TD キュー、
  ASKTIME/FORMATTIME、SYNCPOINT/ROLLBACK。
- BMS マップ生成と SEND/RECEIVE MAP、TN3270 サーバ。
- **完了条件**: CardDemo の全オンライン取引 (サインオン、メニュー、口座・カード照会/更新、取引一覧、
  請求支払、管理者機能) が 3270 シナリオテストで合格。
- 進捗 (2026-10): `zeus-cics` (CSD 読み込み、タスク・EIB、LINK/XCTL/RETURN TRANSID、HANDLE/IGNORE
  CONDITION・HANDLE AID/ABEND と既定の異常終了、カタログ上の KSDS/PATH に対するファイル制御と
  ブラウズ、TS/TD キュー、ASSIGN/ASKTIME/FORMATTIME/BIF DEEDIT、3270 画面モデル上の SEND/RECEIVE
  MAP・SEND TEXT) と、端末操作をスクリプトで流す `zcics` を作成。CardDemo のオンライン取引
  (サインオン、管理者メニューとユーザ一覧・追加・更新・削除、メインメニューの全項目、口座照会・
  更新、カード一覧・照会・更新、取引一覧・照会・追加、請求支払、月次レポートの投入) が動き、
  `tools/carddemo-online` の 11 シナリオを CI で実行している。レポート投入は TD キュー JOBS
  (内部リーダー) に書かれた JCL を `zeus-jcl` で実行し、帳票の GDG 世代まで作られる。
  TN3270 サーバ (`zcics --serve`) も作成し、3270 エミュレータ s3270 からのサインオン・管理
  メニュー・ユーザ一覧の操作を CI で確認している。TN3270 サーバは TN3270E (RFC 2355: 装置タイプ、
  LU 名の指定と重複拒否、FUNCTIONS の合意、5 バイトのヘッダ) と従来の TN3270 の両方に対応し、
  拡張属性を持つ端末には Read Partition Query を送って Query Reply (色・強調表示・代替画面サイズ)
  を読み、色と強調表示はそれを持つ端末にだけ送る。出力は RA で圧縮し、X'40' 未満のデータは
  オーダーと誤解されないよう空白にする。CLEAR・Telnet の TIMING-MARK・未知のオプションの拒否・
  ATTN (IP/BREAK) を扱う。GenApp の会話型画面 (Db2 の照会・更新) を s3270 で TN3270E と TN3270
  の両方から操作する試験を CI で実行している。
  さらに、プリンタセッション (TN3270E の ASSOCIATE と CONNECT、CSD の TERMINAL 定義の
  NETNAME・PRINTER による LU 名と端末 ID の対応、SCS-CTL-CODES 合意時は SCS データと
  PRINT-EOJ、それ以外は印刷開始付きの 3270 書き込み)、BIND-IMAGE (LU 2 / 1 / 3 の BIND に
  既定・代替画面サイズ) と UNBIND、SYSREQ (SSCP-LU データ、LOGOFF でセッション終了)、
  代替画面サイズ (ERASE ALTERNATE で Erase/Write Alternate、CLEAR で既定サイズに戻る、
  ASSIGN SCRNHT/ALTSCRNHT/COLOR など)、構造化フィールド (SEND STRFIELD を Write Structured
  Field で送り、AID X'88' の応答を RECEIVE に渡す)、RECEIVE BUFFER、ERASEAUP、
  START (TERMID・FROM・INTERVAL/AFTER/AT/TIME・REQID) / RETRIEVE / CANCEL と開始コード、
  を実装した。タスクはコルーチンとして動き (CICS の QR TCB と同じく 1 スレッドで切り替える)、
  端末の入力・DELAY・ENQ の資源・READ UPDATE のレコードを待つ間や、タイムスライス (50 ms) を
  使い切ったときに他のタスクに譲る。動いているタスクの端末宛ての入力は保留する。各タスクは自分の
  Db2 接続を使う。`tools/tn3270-test.sh` が s3270 (3279 モデル 5、TN3270E) と pr3287 (ASSOCIATE)
  でこれらと、長いタスクの間に他の端末の取引が応答することを確認し、GenApp の TN3270 試験は
  2 端末の同時操作も確認する (いずれも CI)。
  未着手:   SNA 応答 (RESPONSES は合意するが要求しない)、プリンタの BIND 以外の SNA 制御。

### Phase 5: Db2 (Db2 CE) → GenApp (8〜10 週) — コンパイラ C6
- EXEC SQL のコンパイラ統合、SQLCA、ホスト変数・標識変数、カーソル、DCLGEN copybook。
- Db2 CE バックエンド (Db2 for z/OS との方言差の変換、EBCDIC 変換、EBCDIC 照合のデータベース)。
- CICS と Db2 の作業単位の統合 (SYNCPOINT で VSAM と Db2 を同時にコミット)。
- バッチ用 Db2 ユーティリティ相当 (DDL 実行、DSNTEP2/DSNTIAUL 相当、ロード)。
- **完了条件**: GenApp の顧客・保険契約の登録/照会/更新/削除の全シナリオが合格。
  CardDemo の取引種別管理モジュール (Db2) も合格。
- 進捗 (2026-10): DRDA クライアント (旧 `zeus-drda`。独立した MIT ライセンスの
  [`rust-db2-driver`](https://github.com/ykwyuta/rust-db2-driver) に移植し、Zeus は Git リビジョンを固定して利用。
  Open Group の DRDA 仕様から実装。EXCSAT/ACCSEC/
  SECCHK/ACCRDB による接続、NULLID.SYSSH200 パッケージでの動的 SQL 実行・問い合わせ・コミット) と
  EXEC SQL 実行時 `zeus-db2` (ホスト変数を EBCDIC から変換してリテラルとして束縛、VARCHAR の
  level-49 対、標識変数、SELECT INTO・SET・カーソル、プログラムの SQLCA への SQLCODE/SQLSTATE 書き
  込み、Db2 for z/OS の DDL のうち LUW にない句・文の除去、動的 SQL (EXECUTE IMMEDIATE・PREPARE・EXECUTE・
  DESCRIBE と SQLDA・USING DESCRIPTOR)、COMP-1/COMP-2 のホスト変数) を作成。`zjob` は IKJEFT01 の
  DSN RUN PROGRAM を実行し、DSNTIAD/DSNTEP2/DSNTEP4 を SQL スクリプトの実行として扱う。`zcics` は
  タスクに Db2 接続を与え、SYNCPOINT とタスク終了でコミットする。CardDemo の取引種別管理モジュール
  (表の作成 CREADB21、一括保守 MNTTRDB2、一覧・更新・削除・追加画面) が `tools/carddemo-db2.sh test`
  で合格し、CI で Db2 CE コンテナを起動して実行している。
  GenApp: `tools/genapp.sh` が導入手順のカスタマイズ (CNTL の `<名前>` 置換) を行って ADEF121
  (VSAM)・DB2CRE (Db2 の表と見本データ) を実行し、プログラム・マップをコンパイルし、CDEF121 の
  DFHCSDUP 入力を CSD として使う。会話型タスク (SEND の後の RECEIVE が端末の次の入力を待つ。
  スクリプトと TN3270 の両方)、名前付きカウンター (DEFINE/DELETE/GET/QUERY/UPDATE/REWIND
  COUNTER)、FROM と LENGTH でデータ項目の先まで送る SEND を実装し、顧客の照会・追加・更新と、
  自動車・養老・住宅保険の照会・追加・更新・削除、商業保険の照会・追加・削除が Db2 と VSAM の
  両方で動く (`tools/genapp/` の 3 シナリオを CI で実行)。
  JUSTIFY=RIGHT の入力フィールドは、端末から来たデータの後続ブランクを除いてから右寄せする
  (FSET で送り返されたフィールドの値が保たれる。自動車保険の更新で CC が変わらないことを
  `tools/genapp/motor.txt` で確かめる)。JUSTIFY のない NUM のフィールドは RIGHT・ZERO とする。
  作業単位: 回復可能なファイル (VSAM クラスタの LOG(UNDO/ALL)、なければ FILE 定義の
  RECOVERY) への変更は SYNCPOINT ROLLBACK と異常終了で Db2 と一緒に取り消され、SYNCPOINT と
  タスク終了で Db2 と一緒に確定する (ログは領域内のメモリ。領域の障害からの回復はない)。
  イベント処理: zcics の `--event-binding` が CICS Explorer のイベントバインディング (.evbind) を読み、
  EXEC CICS LINK の前後の捕捉仕様 (現在のプログラム・取引・LINK 先のフィルター、COMMAREA か
  コンテナーのデータ) に合うとイベントを作り、取引開始アダプターの取引をチャネル DFHEP.EVENT
  (DFHEP.CCECONTEXT、DFHEP.NAME.nnnnn、DFHEP.DATA.nnnnn) で START する (START CHANNEL も実装)。
  GenApp のバンドル GENAEV01 (CDEF125 の LGST・SSST) で LGASTAT1 が要求ごとの名前付きカウンターを
  数える (`tools/genapp/events.txt`)。
  Db2 のユーティリティ: DSNTIAUL (TSO の DSN RUN。PARMS('SQL') の問い合わせか表名ごとに SYSRECnn へ
  Db2 の内部形式の固定長レコード (NULL 可の列の後に標識バイト、NULL は '?')、SYSPUNCH に LOAD 文)。
  DSNUTILB の LOAD (区切り形式 FORMAT DELIMITED と位置指定形式 POSITION・CHAR・VARCHAR・INTEGER・
  SMALLINT・BIGINT・DECIMAL PACKED/ZONED/EXTERNAL・FLOAT・DATE/TIME/TIMESTAMP EXTERNAL・NULLIF、
  REPLACE・RESUME)。CardDemo の TRANEXTR (`tools/carddemo-db2.sh test`) と Bank of Z の IMS 経路の
  Db2-insert (`tools/bankofz-ims.sh db2`) が動く。
  EBCDIC 照合: PostgreSQL では `tools/pg.sh start-ebcdic` が ICU の規則 (コードページ 037 の文字を
  符号順に並べたもの、`tools/pg-ebcdic037.rules`) で照合するデータベース zeusebcdic を作る。
  Db2 CE (Linux) は EBCDIC_819_037 などの照合を受け付けない (SQL1083N 理由 4) ため、Db2 では
  バイナリ照合のまま。
  未着手: 静的 SQL のパッケージ (BIND は何もしない)、GenApp の Web サービス経路。

- 日本語 EBCDIC (2026-10): CCSID 930・939・1390・1399 の変換を、ICU の対応表 (unicode-org/icu-data
  の UCM を `tools/icu-codepages.sh` で取得・変換し、ライセンスとともに同梱) から Zeus 自身のコードで
  行う (iconv や ICU には依存しない)。混在データの SO/SI、2 バイト部分の共有表、往復・片方向の
  代替対応、補助面の文字と結合文字の並び、代替文字に対応。プロセスの使用コードページは
  `ZEUS_CCSID` (zcobc・zjob は `--ccsid`、モジュールはコンパイル時の CCSID を記録)。
  コンパイラは混在文字定数、DBCS 定数 G"..." / N"..."、PIC G / PIC N (DISPLAY-1、NSYMBOL(DBCS)
  相当) を扱い、DISPLAY は DBCS 項目を SO/SI で囲む。DISPLAY・ACCEPT・テキストファイル・
  データセットの取り込みと印刷・zcics の画面・Db2 のホスト変数・TN3270 (DBCS 対応端末の
  Query Reply、SOSI 項目の SO/SI 入力許可、DBCS 部分を RA で圧縮しない) がこのコードページで
  変換する。s3270 (cp939) から日本語の BMS 画面を操作し、漢字の入力と表示を CI で確認する。
  NATIONAL (2026-10): PIC N USAGE NATIONAL・N"..."・NX"..." を UTF-16 で扱う (既定は
  NSYMBOL(NATIONAL) 相当、`--nsymbol dbcs` で PIC N・N"..." を DBCS に)。MOVE (英数字・DBCS との
  変換)、比較、文字単位の部分参照、LENGTH / BYTE-LENGTH、NATIONAL-OF / DISPLAY-OF (CCSID 指定可)、
  DISPLAY・ACCEPT・Db2 ホスト変数に対応。
  DBCS の利用者語: ASCII 以外の文字 (漢字・かな・全角英数字) を含む語をデータ名・段落名などに使える
  (`crates/zeus-cobol/tests/japanese.cbl`)。中国語・韓国語のコードページは対象外とする。

### Phase 6: Bank of Z (CICS 経路) (6〜8 週)
- Phase 3 の一覧表で判明した追加 CICS API (チャネル/コンテナ、名前付きカウンタ等) の実装。
- Bank of Z のバッチ部品。
- **完了条件**: Bank of Z の CICS 経路 (顧客・口座の作成/照会、入出金、振替) のシナリオが合格。
  - 達成 (2026-10): `tools/bankofz.sh test`。詳細は 0007 の #1。

### Phase 7: MQ と PostgreSQL バックエンド (8〜10 週)
- MQI サブセット (MQCONN/MQOPEN/MQPUT/MQGET) とローカルキュー、CICS との作業単位統合。
  - 実装済み (0005): ローカルキューを自作せず、IBM MQ の C クライアントライブラリ経由で実際のキューマネージャーに TCP で接続する
    (MQCONN/MQCONNX/MQDISC/MQOPEN/MQCLOSE/MQPUT/MQPUT1/MQGET/MQINQ/MQCMIT/MQBACK)。CICS の SYNCPOINT・タスク終了、
    バッチのステップ終了で作業単位を確定する。トリガーモニター (CKTI) は zcics の `--mq-initq`。
- PostgreSQL バックエンド (Db2 方言変換、SQLCODE 対応表)。Db2 CE で全シナリオを流した結果をオラクルとして比較。
  - 実装済み (0007 #8): `ZEUS_DB2=postgres://...` で zeus-pg (PostgreSQL のプロトコルのクライアント) を使う。
    CardDemo の Db2 モジュール・GenApp・Bank of Z の Db2 シナリオが PostgreSQL でも Db2 CE と同じ期待値で合格する (CI)。
- **完了条件**: CardDemo の口座抽出モジュール (VSAM+MQ) が合格。GenApp・Bank of Z・CardDemo の Db2 シナリオが
  PostgreSQL バックエンドでも Db2 CE と同じ結果になる。

### Phase 8: 運用性・性能 (継続) — コンパイラ C7
- Cranelift ネイティブコード生成、ソースレベルデバッガ。
- 複数端末の同時実行、ロック・エンキュー、異常終了時の回復、ジョブスケジューラ連携
  (CardDemo の `scheduler` 定義の取り込み)、監視・ログ、ドキュメント。

## 8. 次のフェーズ (別提案) と任意トラック

次のフェーズ (本計画の完了後に別提案):
- **IMS**: EXEC DLI / CALL 'CBLTDLI' のコンパイラ統合、DL/I (DBD/PSB、階層データストア、GU/GN/GNP/ISRT/REPL/DLET、SSA)、
  IMS バッチ (DFSRRC00 相当)、IMS TM。対象は Bank of Z の IMS 経路と CardDemo の承認モジュール (IMS+Db2+MQ)。
- **PL/I**: 着手済み (2026-10)。zeus-decimal・zeus-ds・コードページを共用する独立したコンパイラと
  インタプリタ (zeus-pli、`zpli`) として実装し、JCL の IBMZPLI / IEWL / EXEC PGM で動く。
  再帰・AUTOMATIC・ON ユニットの動的探索のため ZIR は使わない。範囲と未対応は 0004 を参照。

任意トラック:
- **z/Architecture CPU エミュレータ (0001 案)**: Assembler 資産やロードモジュールしか残っていない
  プログラムを扱う必要が出た場合、命令実行部分だけを (アプリ用の命令に絞って) 0001 の Phase 1/5 の
  方針で実装し、Zeus ランタイムの SVC・マクロ層と接続する。10 進演算は zeus-decimal を共用する。

## 9. マイルストーン

| マイルストーン | 到達内容 | 目安 (累積) |
|---|---|---|
| M0 | CI・CCVS85 ハーネス・GnuCOBOL 基準線 | 〜3 週 |
| M1 | COBOL コンパイラ中核 (CCVS85 NC・SM 合格) | 〜4.5 ヶ月 |
| M2 | **CCVS85 必須モジュール全件合格** | 〜6.5 ヶ月 |
| M3 | **CardDemo バッチ稼働** (JCL・VSAM・自作コンパイラ) | 〜8 ヶ月 |
| M4 | **CardDemo オンライン稼働** | 〜11 ヶ月 |
| M5 | GenApp 稼働 (CICS + Db2 CE + VSAM) | 〜13 ヶ月 |
| M6 | Bank of Z (CICS 経路) 稼働 | 〜15 ヶ月 |
| M7 | MQ (CardDemo 口座抽出) と PostgreSQL バックエンド | 〜17 ヶ月 |

## 10. リスクと対策

| リスク | 対策 |
|---|---|
| COBOL コンパイラの工数超過 (文法の巨大さ、算術規則の再現) | 構文使用一覧表で範囲を限定。CCVS85・Hercules 10 進差分で早期に誤りを検出 (0003 §9) |
| サンプルが 2010 年以降の新しい API (CICS TS 5.x/6.x、新しい COBOL 構文) を使う | Phase 3 の一覧表で使用箇所を洗い出し、個別に対応可否を判断する |
| CICS・Db2 for z/OS・MQ に無償の完全なオラクルがない | IBM マニュアル準拠の仕様テスト、KICKS・Db2 CE の活用、ZD&T 結果の取り込み |
| Db2 CE の制約 (再配布不可・リソース上限・LUW と z/OS の方言差) | 利用者導入の前提を明記。方言差は変換層と仕様テストで管理し、PostgreSQL を第 2 バックエンドとして用意 |
| Db2 方言を PostgreSQL に変換しきれない | Db2 CE の結果をオラクルに差分を一覧化し、変換できない構文は文書化する |
| 作業単位 (VSAM + Db2 + MQ) の整合 | Phase 4 から `zeus-txn` を設けて 2 相コミット相当の枠組みを用意 |
| IBM の商標・ライセンス (BMS マクロ定義、copybook 等) | IBM 提供物は取り込まず、仕様に基づいて再作成する |

## 11. 決定事項

1. データは EBCDIC のまま保持する。
2. Db2 バックエンドは Db2 CE を先に開発し、PostgreSQL を次に対応する。
3. IMS は次のフェーズ (別提案) とする。
4. PL/I は次のフェーズ (別提案) とする。
5. GnuCOBOL は製品には組み込まず、比較用のオラクルとして積極的に活用する。
6. 最初の目標を CCVS85 の必須モジュール全件合格とし、アプリケーション対応はその後に行う。

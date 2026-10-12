# 0007: 残課題と未対応機能の一覧 (2026-10)

0002〜0006 の「未対応」「未着手」「制約」と、対象アプリ (CardDemo / GenApp / Bank of Z) の
未稼働部分を一つにまとめ、修正規模の大きい順に並べた。この順に解消していく。
規模の目安: XL = 新しいサブシステム (数千行)、L = 既存サブシステムの大きな拡張 (千行前後)、
M = 数百行、S = 百行前後。状態は解消したら更新する。

## 1. 一覧 (規模の大きい順)

| # | 規模 | 項目 | 出典 | 状態 |
|---|---|---|---|---|
| 1 | XL | **Bank of Z の CICS 経路**: チャネルとコンテナ (PUT/GET CONTAINER)、非同期 API (RUN TRANSID CHANNEL・FETCH ANY / CHILD)、INQUIRE ASSOCIATION・INQUIRE/SET TERMINAL、Db2 の表 (CUSTOMER・ACCOUNT・PROCTRAN・CONTROL) の作成と初期データ、BMS 画面 (BNK1*) のシナリオ (顧客・口座の作成/照会/更新/削除、入出金、振替) | 0002 Phase 6 | **済** (`tools/bankofz.sh test`) |
| 2 | XL | **IMS TM** (メッセージ処理領域): トランザクションの定義と入力メッセージのキュー、I/O PCB の GU/GN/ISRT、MPP として COBOL / PL/I プログラムを実行、端末 (または API) からの入出力。Bank of Z の IMS 経路 (IBLOGIN・IBACSUM・IBGCUDAT・IBSCUDAT・IBLOGOUT) | 0006 | **済** (MPP 領域・I/O PCB。MFS などの残りは 0006) |
| 3 | L | **PL/I の DL/I 呼出し** (PLITDLI と EXEC DLI): PCB ポインタ、引数の数、SYSTEM(IMS) の主手続き。Bank of Z の IBLOGIN が使う (#2 の前提) | 0004・0006 | **済** (PLITDLI。PL/I の EXEC DLI は未対応) |
| 4 | L | **z/OS Connect 相当の REST 経路**: HTTP の JSON 要求を COMMAREA / コンテナに写像して CICS / IMS のプログラムを呼ぶ (Bank of Z の API、GenApp の Web サービス経路) | 0002 Phase 5・6 | **済** (Bank of Z の API。GenApp は Web サービス定義がない。0008) |
| 5 | L | **CICS のトリガーモニター (CKTI)**: 開始キューのトリガーメッセージから取引を START し、RETRIEVE で MQTM を渡す。CardDemo の CODATE01・COACCT01・COPAUA0C | 0005・0006 | **済** (`tools/carddemo-mq.sh test`) |
| 6 | L | zcics のタスクの並行実行 (入力待ちでない処理が他の端末を待たせない) | 0002 Phase 4 | **済** (`tools/tn3270-test.sh`) |
| 7 | L | IMS の WAL の共有 (zcics とバッチの同時アクセス、常駐サーバー) | 0006 | **済** (常駐サーバーなしで共有。`tools/carddemo-ims.sh test`) |
| 8 | L | PostgreSQL バックエンド (Db2 方言の変換、SQLCODE の対応表) | 0002 Phase 7 | **済** (`ZEUS_DB2=postgres://...`、CI) |
| 9 | M | DL/I の機能: コマンドコード、ブール演算子の修飾、等号以外の比較、パス呼出し、XRST・記号チェックポイント | 0006 | **済** (ISRT のパス呼出しを除く) |
| 10 | M | PL/I: ENTRY 変数による呼出し、FETCH/RELEASE | 0004 | **済** |
| 11 | M | PL/I: AREA と OFFSET、iSUB 定義、構造体の途中の REFER、CONTROLLED の引数 | 0004 | **済** |
| 12 | M | PL/I: 型の追加 (COMPLEX、GRAPHIC/WIDECHAR/UCHAR、浮動小数点ピクチャ、ORDINAL)、DEFAULT 文、GENERIC、HANDLE と DEFINE STRUCTURE | 0004 | 未着手 |
| 13 | M | PL/I の EXEC SQL: FLOAT・PICTURE・BIT のホスト変数、構造体のホスト変数の展開、動的 SQL と SQLDA | 0004 | 未着手 |
| 14 | M | MQ: MQSET、パブリッシュ/サブスクライブ、メッセージハンドル、MQCB/MQCTL、MQOD・MQPMO 2 以降のフィールド | 0005 | 未着手 |
| 15 | M | GenApp のイベント処理 (LGASTAT1) | 0002 Phase 5 | 未着手 |
| 16 | M | Db2 のユーティリティ (DSNTIAUL・LOAD)、EBCDIC 照合のデータベース | 0002 Phase 5 | 未着手 |
| 17 | M | PL/I: LOCATE、REGIONAL・相対データセット | 0004 | 未着手 |
| 18 | S | PL/I からの MQI 呼出し | 0005 | 未着手 |
| 19 | S | PL/I の CICS の制約 (COBOL から CALL された手続きの RETURN・HANDLE) | 0004 | 未着手 |
| 20 | S | PL/I のプリプロセッサ: %GO TO、キーワード引数、STATEMENT 形式 | 0004 | 未着手 |
| 21 | S | IMS のアンロード (DFSURGU0) | 0006 | 未着手 |
| 22 | S | BMS: JUSTIFY=RIGHT の入力フィールドの右寄せの確認、DBCS の利用者語 | 0002 | 未着手 |

## 2. この一覧の対象外

- **Cranelift によるネイティブコード生成とソースレベルデバッガ** (0002 Phase 8): 性能と運用性のための
  別提案相当の作業で、機能の欠けを埋めるものではない。インタプリタで対象アプリが動くことを優先する。
- **Java との連携**: Bank of Z の IBTRAN (オブジェクト指向 COBOL の REPOSITORY で Java クラスを呼ぶ) と
  IMS の Java (JMP) プログラム。JVM を必要とするため対象外。
- z/Architecture CPU エミュレータ (0001、任意トラック)、中国語・韓国語のコードページ (0002 で対象外と決定)。
- PL/I のマルチスレッド (ATTACH): 対象アプリが使わない。

## 3. 解消の記録

- **#1 Bank of Z の CICS 経路** (2026-10):
  - `tools/bankofz.sh` が環境を作る。Db2 の表を作り、BANKDATA で顧客・口座を生成し、ABNDFILE と BMS マップ・プログラムを用意する。
  - `tools/bankofz/customer.txt` と `account.txt` で、メニューから次の取引を確かめる:
    顧客の照会・作成・更新・削除、口座の一覧・照会・作成・更新・削除、入出金、振替。
  - 足りなかった機能として次を追加した。
    - zcics:
      - チャネルとコンテナ (PUT / GET / DELETE / MOVE、ブラウズ、ASSIGN CHANNEL、LINK・XCTL の CHANNEL)。
      - 非同期 API (RUN TRANSID CHILD・FETCH CHILD / ANY・FREE CHILD)。子タスクは RUN の時点で最後まで実行する。
      - INQUIRE ASSOCIATION、INQUIRE / SET TERMINAL UCTRANST、RETURN TRANSID IMMEDIATE。
    - zcobc: COMP-1 / COMP-2 (IBM 16 進浮動小数点)、FUNCTION RANDOM の系列 (種なしの呼出しが前の値に続く)。
    - LE の日付・時刻サービス: CEEGMT・CEELOCT・CEEDATM・CEEDATE・CEESECS。
    - Db2:
      - z/OS の表オプション (VOLATILE・AUDIT・DATA CAPTURE) を落とす。
      - DSNTEP2 が問合せの行を出力する。
      - TSO の RUN が PARM(...) を受け付ける。
  - 補足: CRECUST は子タスクに 399 バイトのコンテナを渡すが、子 (CRDTAGY1〜5) は 397 バイトの領域で GET するため、
    CICS の仕様どおり LENGERR になる。信用スコアは 0 のまま、顧客の作成は成功する。
- **#2 IMS TM、#3 PL/I の DL/I** (2026-10):
  - トランザクションの定義 (CSLUSPOC)、メッセージキュー (`zjob ims send / receive`)、メッセージ処理領域 (DFSRRC00 PARM='MSG')、I/O PCB と代替 PCB のメッセージ呼出し。
  - PL/I: CALL PLITDLI と、IMS の主手続き (POINTER パラメーターに PCB)。
  - COBOL: ENTRY 'DLITCBL' USING をパラメーターとして扱う。
  - Bank of Z の IMS 経路 (IBLOGIN・IBACSUM・IBGCUDAT・IBSCUDAT・IBLOGOUT) が `tools/bankofz-ims.sh test` で動く。
- **#4 REST 経路** (2026-10):
  - zeus-zconnect (`zconn`) が z/OS Connect の API プロジェクトを読み、CICS プログラム (端末のない LINK) と IMS トランザクション (メッセージ処理領域) を呼ぶ (0008)。
  - Bank of Z の CICS の API (顧客・口座・取引) と IMS の API が `tools/bankofz.sh test`・`tools/bankofz-ims.sh test` で動く。
  - 合わせて直したもの:
    - DRDA: 問合せの行が次のブロックに続く場合。
    - IMS: 異常終了したトランザクションだけを止め、ほかは続ける。
    - zeus-db2: `ZEUS_DB2_TRACE` を追加した。
- **#5 CICS のトリガーモニター** (2026-10):
  - zcics の `--mq-initq` が開始キューを 0.5 秒ごとに読み、トリガーメッセージ (MQTM) の ApplId の取引を START する。
    プログラムは RETRIEVE で MQTM を受け取る。
  - CardDemo の CODATE01・COACCT01 と、承認モジュールの COPAUA0C (MQ と IMS DB) が `tools/carddemo-mq.sh test` で動く。
  - 要求と応答のテスト用に `zmq` (put / get) を追加した。
- **#6 タスクの並行実行** (2026-10):
  - zcics のサーバーは各タスクをコルーチン (corosensei、スタックはタスクごと) で動かす。待ちの間は他のタスクが動く。
    - 待ち: 端末の入力 (会話型の RECEIVE)、DELAY (INTERVAL・FOR・TIME・UNTIL)、ENQ の資源、READ UPDATE のレコード、
      MQGET の MQGMO_WAIT (待ち時間の間 0.1 秒ごとにキューを見る)。
    - タイムスライス: COBOL・PL/I の解釈実行が一定命令ごとに時刻を見て、50 ms を超えたら譲る。
  - 以前は会話型の待ちの中で他の端末のタスクを入れ子に実行していた。待ちの解ける順序が入れ子の順に縛られ、計算の長いタスクは
    他を待たせた。
  - ENQ / DEQ を実装した。資源ごとの所有タスク、NOSUSPEND・HANDLE CONDITION ENQBUSY、作業単位またはタスクの終わりの解放。
    READ UPDATE はレコードをロックし、回復可能なファイルでは作業単位の終わりまで保持する。
  - unsafe コードはディスパッチャーのモジュール (`crates/zeus-cics/src/dispatch.rs`) だけ。動いているコルーチンの yielder を
    ポインターで参照する。
  - スクリプトモードは従来どおり 1 タスクずつ実行する (DELAY は眠る)。
  - 制約: MQ の既定の接続はリージョンで 1 つなので、タスクの作業単位 (同期点つきの MQPUT / MQGET) は混ざる。
- **#7 IMS の WAL の共有** (2026-10):
  - 常駐サーバーを置かず、WAL ファイルを複数のプロセスで共有する。
    - 読む側: データベースを読むたびにファイルの長さを見て、他のプロセスが追記したフレームを適用する。書きかけのフレームは次に回す。
    - 更新する側: 作業単位が最初のルートをロックするときに WAL の排他ロックを取り (待ち時間を過ぎるとタイムアウト)、他のプロセスのフレームを
      適用してから更新する。コミットで追記し、作業単位の終わりにロックを放す。レコードの番号はロックの中で振るので、プロセス間で重ならない。
    - 開くとき: 排他ロックが取れれば、失敗した書き込みの残り (末尾の不完全なフレーム) を切り詰める。
  - `tools/carddemo-ims.sh test` は、データベースを開いた zcics のリージョンが待つ間にバッチジョブ (アンロード・削除・再ロード) を流し、
    その後のオンライン取引がバッチの結果を表示することを確かめる。
- **#8 PostgreSQL バックエンド** (2026-10):
  - zeus-pg: PostgreSQL のプロトコル (版 3) のクライアント。起動、パスワード・SCRAM-SHA-256 の認証、単純問合せ (結果はテキスト)。
    SCRAM は RFC 7677 の例で確かめた。
  - zeus-db2 は ZEUS_DB2 の URL で Db2 (DRDA) と PostgreSQL を選ぶ (`postgres://user:pw@host:port/db`、`tools/pg.sh start`)。
    - 作業単位: 最初の文で BEGIN し、文ごとにセーブポイントを置く。失敗した文だけを取り消すので、Db2 と同じく作業単位は続く。
    - SQLCODE: SQLSTATE から Db2 の SQLCODE に対応付ける (23505 → -803、23503 → -530 / -532、42P01 → -204 など)。
      変更行のない INSERT・UPDATE・DELETE は +100。SQLERRD(3) は変更行数。
    - 値: TIMESTAMP は `yyyy-mm-dd-hh.mm.ss.nnnnnn`、TIME は `hh.mm.ss` で返す。Db2 の形式の日付・時刻・タイムスタンプの文字列定数は
      PostgreSQL の形式にする。NUL 文字 (LOW-VALUE) は空白にする。
    - 方言の変換: 特殊レジスター (CURRENT TIMESTAMP など)、SYSIBM.SYSDUMMY1、VALUE・IDENTITY_VAL_LOCAL・CHAR(日付, ISO)・YEAR などの関数、
      FOR UPDATE OF、WITH UR、NEXT VALUE FOR、DDL (NOT NULL WITH DEFAULT、識別列の指定、FOR BIT DATA、修飾した索引名、スキーマの自動作成、
      ALTER TABLE の ADD、DROP TABLE の CASCADE)。
    - FOR UPDATE のカーソルはサーバーのカーソルにして、WHERE CURRENT OF の位置付け更新を PostgreSQL で行う。
  - 合わせて直したもの: DSNTIAD の PARMS('RC0') はエラーでも戻りコード 0 (IBM の仕様どおり)。
  - CI の `postgres` ジョブが、CardDemo の Db2 モジュール・GenApp・Bank of Z を PostgreSQL で流す。
- **#9 DL/I の機能** (2026-10):
  - SSA の解析を作り直した。コマンドコード (D・F・L・C・P、U・V・Q・N・- は受け付けるだけ)、AND / OR で結んだ複数の修飾文、
    EQ 以外の比較演算子。パック 10 進のフィールドは数値で比べる。
  - GU・GN・GNP は複数階層の SSA を受け付ける。上位の SSA は親の条件で、GU は条件を満たす最初のパスを階層順に探す
    (すべてが固有キーの等号なら従来どおりキーでたどる)。GNP は親の下の全階層を順にたどる。親子関係は GU・GN で取り出したセグメントに置く。
  - パス呼出し (D): 親のセグメントを上から順に入出力域に並べる。
  - 記号チェックポイント (CHKP に領域を続ける、EXEC DLI SYMCHKP) は領域を WAL のチェックポイントのフレームに保存し、
    XRST (CALL と EXEC DLI) が ID か LAST で戻す。
  - `crates/zeus-jcl/tests/jobs.rs` の `ims_batch_regions` と `crates/zeus-ims/tests/core.rs` で確かめる。
- **#10 PL/I の ENTRY 変数と FETCH/RELEASE** (2026-10):
  - VARIABLE 属性を構文解析で保持し、ENTRY 変数 (仮引数・配列を含む) に手続きを代入・比較し、関数参照と CALL で呼び出す。
    引数の記述子と RETURNS は変数ごとに覚える。手続き名は ENTRY を期待する文脈 (代入先・仮引数・比較の相手) では ENTRY 定数になる。
  - FETCH / RELEASE: リンカーにモジュールの読込みと解放を加えた。TITLE で別名のモジュール、読み込めなければ ERROR。
  - `crates/zeus-pli/tests/programs.rs` の `entry_variables` と `fetch_and_release`。
- **#11 PL/I の記憶域の機能** (2026-10):
  - AREA と OFFSET: AREA(n) は見出し (大きさ・使用量) と割当て域。ALLOCATE IN で割り当て、OFFSET は領域内の相対位置。
    POINTER・OFFSET・EMPTY、ポインタとの変換、オフセットの位置指定子、領域の代入、AREA 条件。
  - iSUB 定義: 定義された配列の参照の添字を iSUB として基底の添字を求める。
  - 構造体の途中の REFER: REFER のメンバーが今の値で大きさを変え、後のメンバーの位置と構造体の大きさに反映する。
  - CONTROLLED の仮引数: 引数として呼出し側の変数の世代の起点を渡し、呼ばれた手続きの ALLOCATE / FREE がそれに作用する。
  - `crates/zeus-pli/tests/programs.rs` の `areas_isub_refer_and_controlled_parameters`。

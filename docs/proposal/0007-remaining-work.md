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
| 4 | L | **z/OS Connect 相当の REST 経路**: HTTP の JSON 要求を COMMAREA / コンテナに写像して CICS / IMS のプログラムを呼ぶ (Bank of Z の API、GenApp の Web サービス経路) | 0002 Phase 5・6 | 未着手 |
| 5 | L | **CICS のトリガーモニター (CKTI)**: 開始キューのトリガーメッセージから取引を START し、RETRIEVE で MQTM を渡す。CardDemo の CODATE01・COACCT01・COPAUA0C | 0005・0006 | 未着手 |
| 6 | L | zcics のタスクの並行実行 (入力待ちでない処理が他の端末を待たせない) | 0002 Phase 4 | 未着手 |
| 7 | L | IMS の WAL の共有 (zcics とバッチの同時アクセス、常駐サーバー) | 0006 | 未着手 |
| 8 | L | PostgreSQL バックエンド (Db2 方言の変換、SQLCODE の対応表) | 0002 Phase 7 | 未着手 |
| 9 | M | DL/I の機能: コマンドコード、ブール演算子の修飾、等号以外の比較、パス呼出し、XRST・記号チェックポイント | 0006 | 未着手 |
| 10 | M | PL/I: ENTRY 変数による呼出し、FETCH/RELEASE | 0004 | 未着手 |
| 11 | M | PL/I: AREA と OFFSET、iSUB 定義、構造体の途中の REFER、CONTROLLED の引数 | 0004 | 未着手 |
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

# 0005: IBM MQ (zeus-mq)

## 1. 目的

COBOL プログラムの MQI 呼出し (`CALL 'MQOPEN' USING ...`) を、実際の IBM MQ のキューマネージャーで動かす。
CardDemo の口座抽出モジュール (app-vsam-mq の CODATE01・COACCT01) と、Bank of Z の外部連携 (Bank of Q) が対象。

## 2. 方式

- キューマネージャーは自作しない。IBM MQ の C クライアントライブラリ (`libmqic_r.so`、IBM が無償で再配布を認めている
  Redistributable Client) を実行時に読み込み (`libloading`)、その MQI 関数を呼ぶ。
  - ライブラリはキューマネージャーとクライアントチャネル (TCP) で通信する。
  - ライブラリは Zeus に含めない。`tools/mq.sh client` が IBM の配布サイトから取得する。
- プログラムから見える MQI は z/OS のまま。
  - 構造体はビッグエンディアンの整数と EBCDIC の文字、4 バイトのポインター (31 ビット)。呼出しのたびに C の構造体
    (この機械のエンディアン、ASCII、8 バイト境界の 8 バイトのポインター) に変換し、戻ったら書き戻す。C 側のオフセットは
    z/OS 側から求める (ポインター・MQCHARV・MQINT64 を 8 バイト境界に置き、ポインターは 8 バイト)。
    - MQOD 1〜4 (168・200・336・400 バイト。C では 168・208・344・424)、MQMD 1・2 (324・364)、MQPMO 1〜3
      (128・152・176)、MQGMO 1〜4 (72・80・100・112)、MQSD 1 (312)、MQSRO、MQCMHO、MQDMHO、MQDMPO、MQSMPO、
      MQPD、MQIMPO (60)。
    - MQCHARV (トピック文字列、サブスクリプション名、プロパティー名など) は、VSPtr か VSOffset が指すプログラムの文字列を
      UTF-8 (CCSID 1208) の別のバッファーに写して渡し、出力 (ResObjectString、ReturnedName) は VSBufSize までプログラムの
      バッファーに EBCDIC で書き戻す。配布リストのポインター (ObjectRecPtr など) は渡さない。
    - これより新しい版の構造体は、上の版の部分だけを渡す。
  - メッセージのデータは変換しない。MQMD が「MQENC_NATIVE (z/OS の 785)」と「プログラムのコードページの CCSID」
    (MQCCSI_Q_MGR のときは Zeus のコードページ、既定 037) を示すので、他のプラットフォームのアプリケーションとの間の
    変換はキューマネージャーが行う (MQGMO_CONVERT)。
  - ハンドルはライブラリのハンドルをそのまま使う。接続ハンドル 0 (MQHC_DEF_HCONN) は既定の接続で、最初の呼出しで
    ZEUS_MQ のキューマネージャーにつなぐ (CICS アダプター経由の呼出しと同じ書き方ができる)。
- unsafe コードは FFI のモジュール (`crates/zeus-mq/src/ffi.rs`) だけ。ほかはワークスペースの規則どおり禁止。
- コピーメンバー (CMQV、CMQODV/L、CMQMDV/L、CMQPMOV/L、CMQGMOV/L、CMQTMV/L、CMQSDV/L、CMQSROV/L、CMQCMHOV/L、
  CMQDMHOV/L、CMQDMPOV/L、CMQSMPOV/L、CMQIMPOV/L、CMQPDV/L、CMQCBDV/L、CMQCBCV/L、CMQCTLOV/L) は、MQI の文書にある構造体の
  レイアウトと定数値から Zeus 用に書いた (IBM のコピーブックは使わない)。
  - 定数値は公式の C ヘッダー (cmqc.h) とコンパイルして照合した。
  - z/OS 固有の値 (MQENC-NATIVE 785 など) は z/OS の値。
- PL/I の %INCLUDE メンバーは同じ定義から作る: CMQP (定数。`-` を `_` にした名前付き定数)、CMQEPP (MQI の ENTRY の宣言)、
  構造体ごとのメンバー (CMQODP、CMQMDP、CMQPMOP、CMQGMOP、CMQTMP、CMQSDP、CMQSROP、CMQCMHOP、CMQDMHOP、CMQDMPOP、
  CMQSMPOP、CMQIMPOP、CMQPDP、CMQCBDP、CMQCBCP、CMQCTLOP)。構造体は初期値付きのレベル 1 の構造体 (MQMD など) で、
  メンバー名は COBOL の名前から接頭語とハイフンを除いたもの (MQMD.FORMAT)。別の変数は `DCL 1 MD LIKE MQMD;` で宣言する。

## 3. 接続

| 環境変数 | 内容 |
|---|---|
| `ZEUS_MQ` | `mq://ユーザー:パスワード@ホスト:ポート/キューマネージャー/チャネル` (チャネルの既定は SYSTEM.DEF.SVRCONN)。MQSERVER を設定し、MQCONNX に MQCSP (ユーザーとパスワード) を渡す |
| `MQSERVER` | 設定済みならそれを使う (通常の MQ クライアントと同じ) |
| `ZEUS_MQ_LIB` | ライブラリ (またはそのディレクトリ)。ほかに `MQ_INSTALLATION_PATH`、`/opt/mqm/lib64`、ローダーの検索パス |

- ライブラリが見つからないときは、各呼出しが CC=2、RC=2012 (MQRC_ENVIRONMENT_ERROR) を返す。
- `tools/mq.sh start` は IBM MQ Advanced for Developers のコンテナ (キューマネージャー QM1、チャネル DEV.APP.SVRCONN、
  キュー DEV.QUEUE.1〜3) を起動し、ZEUS_MQ の値を表示する。

## 4. 対応範囲

| 項目 | 内容 |
|---|---|
| 呼出し | MQCONN、MQCONNX (接続オプションは ZEUS_MQ のもの)、MQDISC、MQOPEN、MQCLOSE、MQPUT、MQPUT1、MQGET、MQINQ、MQSET、MQCMIT、MQBACK |
| パブリッシュ/サブスクライブ | MQSUB (MQSD のトピック文字列、管理されたキュー)、MQSUBRQ、MQOPEN の MQOT_TOPIC (MQOD 4 の ObjectString) と MQPUT |
| メッセージプロパティー | MQCRTMH、MQDLTMH、MQSETMP、MQINQMP、MQDLTMP。MQPMO 3 の OriginalMsgHandle、MQGMO 4 の MsgHandle (MQGMO_PROPERTIES_IN_HANDLE)。値は文字列 (EBCDIC と UTF-8 の変換)、整数 (エンディアン)、浮動小数点 (z/OS の 16 進浮動小数点と IEEE) を変換する |
| 非同期の受信 | MQCB (メッセージコンシューマーとイベントハンドラーの登録・削除・中断・再開、CallbackName のプログラム) と MQCTL (START_WAIT・START・STOP・SUSPEND・RESUME)。Zeus がコンシューマーのキューから受け取り、プログラムを (Hconn、MQMD、MQGMO、Buffer、MQCBC) で呼ぶ。待ち時間を過ぎてもメッセージがなければ MQCBCT_EVENT_CALL (MQRC_NO_MSG_AVAILABLE)。START_CALL・STOP_CALL・REGISTER_CALL・DEREGISTER_CALL。実行単位は 1 スレッドなので、START は今あるメッセージを渡して戻る |
| 作業単位 | バッチ: ステップの終わりにコミット (異常終了ならバックアウト) して切断する。CICS: SYNCPOINT (ROLLBACK) とタスクの終わりに既定の接続の作業単位を確定する。接続はリージョン (スレッド) で使い回す |
| 実行環境 | zjob (COBOL と PL/I のステップ)、zcics のタスク。PL/I は CALL MQCONN(...) のように呼ぶ (引数は参照渡し)。MQCB の CallbackName は COBOL のプログラムでも PL/I の外部手続き (ロードライブラリーのモジュール) でもよい |
| トリガーモニター | zcics の `--mq-initq Q` (複数可): CKTI と同じく開始キューのトリガーメッセージ (MQTM) を読み、その ApplId の取引を START する。プログラムは EXEC CICS RETRIEVE で MQTM (z/OS の形、684 バイト) を受け取る |
| `zmq` | テスト用のコマンド: `zmq put [--reply-to Q] QUEUE TEXT...`、`zmq get [--wait MS] QUEUE` (テキストはコードページで変換) |

## 5. 未対応 (今後)

- 配布リスト (MQOD・MQPMO の ObjectRecPtr / PutMsgRecPtr などのレコード)。
- MQCB の CallbackFunction (関数ポインター) による登録 (CallbackName のプログラムだけ)。MQCTL START の非同期実行。

## 6. テスト

- `crates/zeus-mq`: 構造体の変換 (MQMD の往復、C 側の長さ、MQCHARV の写しと書き戻し)、ZEUS_MQ の解析。
- `crates/zeus-jcl/tests/jobs.rs` の `mq_pubsub_properties_and_callbacks`: トピック `dev/` の購読と発行、
  メッセージプロパティーの設定と照会、MQCB/MQCTL のコンシューマー、MQSET (ZEUS_MQ があるとき)。
- `crates/zeus-jcl/tests/jobs.rs` の `mq_calls_from_pli`: PL/I のプログラムが PUT・コミット・MQINQ・GET をし、
  PL/I の手続きをコンシューマーとして MQCB / MQCTL で呼び戻す (ZEUS_MQ があるとき)。
- `crates/zeus-cobol` の front: コピーメンバー (と PL/I のメンバー) が 72 桁に収まること。
- `crates/zeus-jcl/tests/jobs.rs` の `mq_calls` (ZEUS_MQ があるときだけ実行): ジョブで COBOL をコンパイルし、キューを空にして
  から、3 件 PUT して MQBACK、2 件 PUT して MQCMIT、CONVERT 付きで GET すると 2 件だけ、最後は 2033 になることを確かめる。
- `tools/carddemo-mq.sh test`: CardDemo の MQ の取引を、トリガーモニターで起動して確かめる。
  - CODATE01 (CDRD): 日付と時刻を応答する。
  - COACCT01 (CDRA): 口座の内容を応答する。
  - COPAUA0C (CP00): 承認要求を IMS DB の保留中の承認に記録し、承認の応答を返す。
- 手で確かめたこと: C のアプリケーションが ASCII (CCSID 819) で書いたメッセージを COBOL が EBCDIC で受け取り、COBOL が
  書いた EBCDIC (037) のメッセージを C が ASCII で受け取る。

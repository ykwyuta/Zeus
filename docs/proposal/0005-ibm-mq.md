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
  - 構造体はビッグエンディアンの整数と EBCDIC の文字。呼出しのたびに C の構造体 (この機械のエンディアン、ASCII) に変換し、
    戻ったら書き戻す。対応する版の範囲では、z/OS と C でフィールドのオフセットが同じなので、変換はフィールド単位で済む。
    - MQOD 1 (168 バイト)、MQMD 1・2 (324・364)、MQPMO 1 (128)、MQGMO 1〜3 (72・80・100)。
    - これより新しい版の構造体は、上の版の部分だけを渡す。
  - メッセージのデータは変換しない。MQMD が「MQENC_NATIVE (z/OS の 785)」と「プログラムのコードページの CCSID」
    (MQCCSI_Q_MGR のときは Zeus のコードページ、既定 037) を示すので、他のプラットフォームのアプリケーションとの間の
    変換はキューマネージャーが行う (MQGMO_CONVERT)。
  - ハンドルはライブラリのハンドルをそのまま使う。接続ハンドル 0 (MQHC_DEF_HCONN) は既定の接続で、最初の呼出しで
    ZEUS_MQ のキューマネージャーにつなぐ (CICS アダプター経由の呼出しと同じ書き方ができる)。
- unsafe コードは FFI のモジュール (`crates/zeus-mq/src/ffi.rs`) だけ。ほかはワークスペースの規則どおり禁止。
- コピーメンバー (CMQV、CMQODV/L、CMQMDV/L、CMQPMOV/L、CMQGMOV/L、CMQTMV/L) は、MQI の文書にある構造体の
  レイアウトと定数値から Zeus 用に書いた (IBM のコピーブックは使わない)。
  - 定数値は公式の C ヘッダー (cmqc.h) とコンパイルして照合した。
  - z/OS 固有の値 (MQENC-NATIVE 785 など) は z/OS の値。

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
| 呼出し | MQCONN、MQCONNX (接続オプションは ZEUS_MQ のもの)、MQDISC、MQOPEN、MQCLOSE、MQPUT、MQPUT1、MQGET、MQINQ、MQCMIT、MQBACK |
| 作業単位 | バッチ: ステップの終わりにコミット (異常終了ならバックアウト) して切断する。CICS: SYNCPOINT (ROLLBACK) とタスクの終わりに既定の接続の作業単位を確定する。接続はリージョン (スレッド) で使い回す |
| 実行環境 | zjob (COBOL と PL/I のステップ)、zcics のタスク |

## 5. 未対応 (今後)

- CICS のトリガーモニター (CKTI): 開始キューのトリガーメッセージから取引を START する。CardDemo の CODATE01・COACCT01 は
  これで起動され、EXEC CICS RETRIEVE で MQTM を受け取る。プログラムのコンパイルと MQI 呼出しはできる。
- MQSET、MQSUB などのパブリッシュ/サブスクライブ、メッセージハンドル (MQCRTMH)、MQCB/MQCTL。
- MQOD 2 以降・MQPMO 2 以降のフィールド (配布リスト、ResolvedQName を返す MQOD 3 など)。
- PL/I からの MQI 呼出し。

## 6. テスト

- `crates/zeus-mq`: 構造体の変換 (MQMD の往復)、ZEUS_MQ の解析。
- `crates/zeus-cobol` の front: コピーメンバーが 72 桁に収まること。
- `crates/zeus-jcl/tests/jobs.rs` の `mq_calls` (ZEUS_MQ があるときだけ実行): ジョブで COBOL をコンパイルし、キューを空にして
  から、3 件 PUT して MQBACK、2 件 PUT して MQCMIT、CONVERT 付きで GET すると 2 件だけ、最後は 2033 になることを確かめる。
- 手で確かめたこと: C のアプリケーションが ASCII (CCSID 819) で書いたメッセージを COBOL が EBCDIC で受け取り、COBOL が
  書いた EBCDIC (037) のメッセージを C が ASCII で受け取る。

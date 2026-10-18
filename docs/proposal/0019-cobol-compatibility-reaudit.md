# COBOL互換対応の残件再監査

確認日: 2026-10-18。0018の段階別記録と現在のコードを再照合した。対象外は従来合意したXML、OO/Java、DATEPROCのみ。CIの成功を言語全体の互換性保証には使わない。

## 現在の残件

| ID | 現状 | 次の実装・確認 |
|---|---|---|
| K01 | ARCH指定とIGY-ARCHは未対応。IBM CPU世代をZeusのホストCPUへ置き換えていない | CPU世代指定による生成・実行の契約を決める。値の受理や定義済み変数の追加だけで完了にしない |
| K02 | SQLIMSの動的SQL、SQLIMSCA/DA、日時、感応性、DL/Iとの作業単位共有は実装済み | IFP専用スケジューラー、未確認のSQLIMSCODE、現在対応していない日時PATTERNを個別に監査する。IBMの全IMS機能・全診断との一致は未保証 |
| K03 | RERUNのassignment-name、接頭辞、環境変数、保存・再開は実装済み | ON file-name分岐のチェックポイントDD割当規則を確定する。データファイルへの埋込みや、根拠のない既定DD名の追加はしない |
| K04-A | 動的PATH、既存/NEW/MODのDSNと終了時DISP処理は実装済み | SPACE、UNIT、VOL、SMSクラス等の動的割当属性は`zeus-ds/src/dynamic.rs`で未対応として診断。仮想データセット上の具体的な意味を決めて実装する |
| K04-B | テープのLBI、複数巻、ラベル、保持期限、認可は実装済み | `vm/files.rs`のEXTEND終了処理は既存HDR1の作成日・保持期限・保護属性を再利用せずラベルを再生成する。既存データへの追記で属性を失わないよう公式仕様を確認し、保持と認可の回帰を追加する |
| K05-A | Db2 WHENEVERを読み飛ばしていた | 下記の追加対応で解消。SQLIMSとは独立した指令状態を使う |
| K05-B | Db2の文字取得時の切り詰め警告と、BINARY取得時の全記憶域幅の扱いが不足 | 下記の追加対応で解消。数値オーバーフロー・無効数値の診断全体は別途監査する |
| K05-C | SQLIMSのCOMP-1/2受け渡しはIEEE doubleを経由しない | Db2側の`host.rs`の浮動小数点入力・出力にはf64経路が残る。HFP下位ビットと範囲を保つ変換へ接続する |
| K05-D | 複合ODO、SAME、EXTERNAL/GLOBAL、ENTRY等は追加実装済み | 各カテゴリー・句・文の合法/不合法条件と数値境界の全組合せは未監査。抽象的な「検証中」の各行を、公式規則に対応する具体的な試験へ分解する |
| K06 | `98f555a`ではpush/PR双方の全12ジョブ成功。ローカル全テスト1,199件成功 | 今回の追加実装後に改めて全体検査・結合検証を行う。前コミットの成功を新変更の検証結果として扱わない |

Zeusの仮想アドレス、仮想テープ、チェックポイント形式と、IBMのロードモジュール・物理装置・チェックポイントイメージとの相互運用は、既存の実行モデルの境界として利用者へ明示する。対応済みと書かれた個別機能についても、IBM実機との比較を完了したという意味ではない。

## 今回の追加対応

Db2のWHENEVERを実行時の登録命令からではなく、翻訳時の指令として扱う。NOT FOUNDはSQLCODE +100、SQLERRORは負数、SQLWARNINGはSQLWARN0=Wまたは+100以外の正数で分岐する。GO TO/GOTO、コロン付きの手続き名、CONTINUEによる解除に対応する。実行されない段落や宣言部に書かれていてもソース上の順序で効力を持ち、SQLCAを格納してから分岐する。SQLIMSとの状態共有はしない。

文字取得の切り詰めではSQLWARN0/1と元の長さの標識を返す。SELECT INTOでも取得側の警告をサーバーSQLCAで消さずに保持し、結果列が多い場合はSQLWARN3を返す。BINARYへの取得はTRUNC指定やPICTURE桁数ではなく記憶域の全幅を使う。NULLの標識と、NULL時に受信領域を保持する動作も回帰確認する。

サーバー不要の回帰に加え、`ZEUS_DB2`が設定された結合試験では、実際のVALUESからの文字切り詰めと、SELECTの空結果によるWHENEVER分岐をDb2/PostgreSQL双方で実行する。環境変数のないローカル実行では、この結合試験部分は実行されない。

根拠: [Db2 for z/OS WHENEVER](https://www.ibm.com/docs/en/db2-for-zos/12.0.0?topic=statements-whenever)、[SQLCAの警告フィールド](https://www.ibm.com/docs/en/db2-for-zos/13.0.0?topic=sqlca-description-fields)、[COBOLホスト変数とBINARYの幅](https://www.ibm.com/docs/en/db2-for-zos/13.0.0?topic=cobol-host-variables-in)、[文字代入と元の長さ](https://public.dhe.ibm.com/ps/products/db2/info/vr7/html/db2s0/charas.htm)。IBMのサンプルソースや同梱宣言は転用していない。

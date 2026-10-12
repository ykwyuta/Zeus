# 0008: REST ゲートウェイ (zeus-zconnect、`zconn`)

## 1. 目的

z/OS Connect の API プロジェクト (OpenAPI 文書と、操作ごとの対応付け、z/OS アセット) をそのまま読み込む。
HTTP の JSON 要求を CICS プログラムの COMMAREA や IMS トランザクションの入力メッセージに変換して、
Zeus 上のプログラムを呼ぶ。対象は Bank of Z の API (CICS 経路 10 アセット、IMS 経路 4 アセット)。

実装は公開されているファイル形式 (YAML の対応付け、JSONata の式、`.dai` のデータ配置) から行った。
IBM のコードは使っていない。

## 2. 構成

| 部品 | 内容 |
|---|---|
| `api` | `api/openapi.yaml` のパスとメソッド。`operations/<URL エンコードしたパス>/<メソッド>` の `operation.yaml` (アセット名)、`request.yaml`、`response_mapping.yaml` (条件と HTTP 状態の順序付きの一覧) と応答ごとの対応付け。`zosAssets/<名前>/zosAsset.yaml` (`cicsCommarea-1.0`: プログラムと EIB のトランザクション ID、`imsTransaction-1.0`: トランザクションコード) と `providerFiles/request.dai`・`response.dai` |
| `expr` | 対応付けの式 (JSONata の部分集合)。パス (引用符付きの名前、配列への写像、述語とインデックス)、文字列・数値・正規表現・配列・オブジェクトのリテラル、比較、`and`/`or`、`&`、四則、`?:`、ラムダ。関数: `$exists $not $boolean $number $string $length $count $sum $abs $floor $ceil $round $trim $uppercase $lowercase $substring $substringBefore $substringAfter $contains $replace $match $pad $formatNumber $now $join $filter $map`。`{{式}}` のテンプレート |
| `dai` | `.dai` のフィールド (STRUCT、ARRAY (OCCURS と DEPENDING ON)、CHAR、DECIMAL のゾーン・パック、SHORT/INT/LONG と符号なし)。位置は startPos、REDEFINES は再定義される項目の位置、配列要素の中は順に置く。JSON への変換では末尾の空白を落とし、`included="N"` の項目は出さない。データへの変換では、まず全項目に初期値 (空白・ゼロ) を入れ、そのあと JSON にある項目だけを書く (JSON にない REDEFINES が元の項目を壊さない)。DEPENDING ON の計数項目は配列の要素数にする |
| `gateway` | 要求の振り分けと、変数 `$pathParameters $queryParameters $headers $body $requestBody`。COMMAREA は要求と応答の配置の大きい方の長さで渡す。IMS の入力の LL はセグメント長。応答は `$zosAssetResponse` (`commarea` / `message` / 異常終了なら `abendCode`) と `$apiRequest` を使い、条件が真になった最初の応答の対応付けで本体を作る |
| `zconn` | `serve PORT` (HTTP/1.1、1 要求ずつ、CORS ヘッダー付き)、`call METHOD PATH [JSON]`、`script FILE` (`call`・`status`・`expect`)、`--trace` (渡したデータ、返ったデータ、プログラムの表示) |

## 3. Zeus 側の呼出し

- CICS: `zeus_cics::task::link`。端末のないタスクとして、アセットのトランザクション ID の下でプログラムを LINK し、
  COMMAREA を返す (IPIC による分散プログラムリンクに相当)。異常終了コードは `abendCode` になる。
- IMS: 入力メッセージを、要求ごとの LTERM (`ZCONnnnn`) からのメッセージとしてキューに入れる。
  `--ims-region` のジョブ (DFSRRC00 PARM='MSG') を流し、その LTERM への出力を受け取る (0006)。

## 4. Bank of Z

`tools/bankofz.sh test` は 3270 の台本のあとで `tools/bankofz/api.zconn` を流す。確かめるもの:
- 顧客: 照会、作成、更新、削除。存在しない顧客は 404。
- 口座: 照会、一覧、残高、入金。
- 取引: 一覧 (DEPENDING ON のない 100 要素の配列で、REDEFINES を含む)。

`tools/bankofz-ims.sh test` は `tools/bankofz-ims/api.zconn` を流す。確かめるもの:
- IMS の顧客の照会と更新。
- 口座の要約 (DEPENDING ON の配列)。

`tools/bankofz.sh api` と `tools/bankofz-ims.sh api` はサーバーを起動する。

IMS の入金 (`/ims/accounts/{customerId}/{accountId}/deposit`) は IBTRAN (Java) を呼ぶので 500 になる (0007 の 2 章)。

## 5. 未対応

- HTTPS、OAuth2 などの認証 (`security` は無視する)、要求の JSON スキーマ検査、`required` / `nullable` の検査。
- チャネル型のアセット (`cicsChannel`)、複数セグメントの IMS メッセージ、MQ のアセット。
- 並行した要求の処理 (1 要求ずつ処理する)。

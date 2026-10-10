# Proposal 0003: Rust 製 COBOL コンパイラ (zeus-cobol)

- 状態: 提案 (Draft)
- 関連: [0002 z/OS 互換アプリケーション実行環境](0002-zos-application-runtime-plan.md)
  (0002 の COBOL コンパイラを、GnuCOBOL から本提案の自作コンパイラに置き換える)

## 1. 目的

Enterprise COBOL 4.2 (z/OS 1.12 世代) の方言で書かれたソースを、外部の COBOL 処理系に
依存せずにコンパイル・実行できる **Rust 製コンパイラ zeus-cobol** を作る。
最初の到達目標は、CardDemo・GenApp・Bank of Z の全 COBOL プログラムを
ソース無修正でコンパイルし、z/OS と同じ結果で実行できること。

## 2. 自作する理由

| 観点 | GnuCOBOL を使う場合 | 自作する場合 |
|---|---|---|
| データの内部表現 | 独自表現。EBCDIC・照合順序は後付けの設定 | **z/OS と同じバイト配置** (EBCDIC, パック 10 進, ビッグエンディアン 2 進) をそのまま記憶域として扱える |
| EXEC CICS / SQL / DLI | 別のトランスレータでソースを書き換えてからコンパイル | コンパイラに**統合** (Enterprise COBOL の統合トランスレータと同じ形) |
| Enterprise COBOL 固有の挙動 | 互換オプションで近づけるだけ。差が残ると直せない | TRUNC / NUMPROC / ARITH や中間結果の桁数規則を仕様どおりに実装できる |
| ランタイムとの結合 | C ABI 越し。CICS・Db2 層との受け渡しが二重になる | Rust ランタイムと直結。デバッガ・トレースも自前で持てる |
| ライセンス・依存 | 外部プロジェクトの GPL / LGPL コードと保守サイクルに依存 | 依存なし |
| 工数 | 少ない | **大きい** (最初の稼働が約 3 ヶ月遅れる。§8) |

## 3. 方式

```
COBOL ソース + copybook
   │
   ├─ ソース管理: 固定形式 (列 1-6 / 7 / 8-72 / 73-80), 継続行, COPY ... REPLACING, REPLACE
   ├─ 字句解析 → 構文解析 (手書き再帰下降, 誤り回復あり)
   │     └─ EXEC CICS / EXEC SQL / EXEC DLI を同じ構文木に取り込む
   ├─ 意味解析: データ部の配置計算 (USAGE, SYNC, OCCURS, OCCURS DEPENDING ON, REDEFINES,
   │            RENAMES, 88 レベル), 名前解決 (修飾名・添字), 型と移送規則の決定
   ├─ 中間表現 ZIR (記憶域へのオフセットと長さで表した命令列)
   └─ バックエンド
         ├─ (1) ZIR インタプリタ … 最初に作る。正しさとデバッグ容易性を優先
         └─ (2) Cranelift ネイティブコード生成 … 性能が必要になってから
```

設計方針:

1. **記憶域は z/OS と同じバイト列**: WORKING-STORAGE・LINKAGE SECTION・COMMAREA は
   z/OS と同一レイアウトのバイト配列として確保する。REDEFINES、グループ項目の移送、
   CICS や CALL での受け渡しは、バイト列をそのまま渡すだけで正しく動く。
2. **数値演算ランタイム (zeus-decimal)**: パック 10 進・ゾーン 10 進・2 進の演算を、
   Enterprise COBOL の「中間結果と演算精度」の規則 (ARITH(COMPAT) 18 桁 / ARITH(EXTEND) 31 桁) で実装する。
   ROUNDED、ON SIZE ERROR、符号処理 (NUMPROC)、2 進項目の切り捨て (TRUNC) を含む。
   このライブラリは z/Architecture の 10 進命令 (AP/SP/MP/DP/ZAP/CP/ED/EDMK 等) と同じ意味を持たせ、
   **Hercules 上で同じ 10 進命令を実行した結果と突き合わせて検証**する
   (0001 の CPU エミュレータを作る場合も共用する)。
3. **IBM コンパイラ互換の外形**: JCL の `PGM=IGYCRCTL` を zeus-cobol に対応させ、
   コンパイラオプション、メッセージ (重大度 I/W/E/S/U)、戻りコード (0/4/8/12/16)、
   ソースリストを IBM と同じ形にする。アプリ同梱のコンパイル用 JCL を書き換えずに使える。
4. **プログラムの形**: コンパイル結果は「プログラムオブジェクト」として PDS 相当のライブラリに格納し、
   静的 CALL・動的 CALL・CICS の LINK/XCTL がすべて同じ仕組みでロードできるようにする。
5. **コードページは設定可能**: 既定は CCSID 1140 (037 系)。ソース中の英数字定数はコンパイル時に変換する。

## 4. 対応する言語範囲

Phase 0 の API 使用一覧表 (0002 §2) と同時に、3 アプリの **COBOL 構文使用一覧表** を作り、
使われている構文から実装する。想定される主な範囲:

- **見出し部・環境部**: PROGRAM-ID, 入れ子プログラム, SPECIAL-NAMES, FILE-CONTROL (SELECT/ASSIGN, ORGANIZATION, ACCESS, RECORD KEY, ALTERNATE KEY, FILE STATUS)。
- **データ部**: FD, WORKING-STORAGE, LOCAL-STORAGE, LINKAGE, レベル 01-49/66/77/88,
  PIC 全般 (編集 PIC を含む), USAGE DISPLAY/COMP/COMP-3/COMP-4/COMP-5/BINARY/PACKED-DECIMAL/POINTER/INDEX,
  VALUE, OCCURS (ASCENDING KEY, INDEXED BY, DEPENDING ON), REDEFINES, SIGN LEADING/TRAILING SEPARATE, JUSTIFIED, BLANK WHEN ZERO。
- **手続き部**: MOVE (CORRESPONDING 含む), 算術文 (ADD/SUBTRACT/MULTIPLY/DIVIDE/COMPUTE),
  IF/EVALUATE, PERFORM 全形式, GO TO (DEPENDING ON), CALL/CANCEL/GOBACK/EXIT PROGRAM/STOP RUN,
  INITIALIZE, INSPECT, STRING/UNSTRING, SEARCH/SEARCH ALL, SET, ACCEPT (DATE/TIME 等), DISPLAY,
  ファイル I/O 文 (OPEN/CLOSE/READ/WRITE/REWRITE/DELETE/START), SORT/MERGE (使用されていれば),
  組み込み関数 (CURRENT-DATE, NUMVAL, UPPER-CASE, INTEGER-OF-DATE 等, 使用分から)。
- **埋め込み文**: EXEC CICS, EXEC SQL (INCLUDE SQLCA / DCLGEN を含む), EXEC DLI。
- **範囲外 (当初)**: オブジェクト指向 COBOL, XML/JSON 文, Report Writer, COBOL 2002 以降の新構文。

## 5. 検証方法 (オラクル)

Enterprise COBOL そのものは使えないため、次を組み合わせる。

| 検証対象 | オラクル |
|---|---|
| COBOL-85 規格の適合性 | **NIST COBOL-85 テストスイート (CCVS85)**。モジュール (NC: 核, SQ/IX/RL: ファイル, IC: 呼び出し, ST: 整列 等) ごとの合格率を CI で追跡 |
| 10 進演算・EBCDIC・編集 | **Hercules** 上で AP/MP/DP/ED 等の命令を実行した結果 (命令単位の差分テスト) |
| 旧規格でも書ける範囲の実行結果 | **Hercules + MVS 3.8j の COBOL** で同じ検査プログラムを実行し比較 |
| 方言差の確認 (補助) | GnuCOBOL を **比較用にだけ** 使う (製品・ビルドには一切組み込まない) |
| Enterprise COBOL 固有規則 | IBM の Language Reference / Programming Guide に基づく仕様テストを手書き |
| アプリ単位 | 0002 のシナリオテスト (バッチ出力・3270 画面) |

## 6. クレート構成

```
crates/
├─ zeus-cobol-front   # ソース管理 (COPY/REPLACE), 字句解析, 構文解析, 構文木
├─ zeus-cobol-sema    # 名前解決, データ配置計算, 型・移送規則, 診断メッセージ
├─ zeus-cobol-zir     # 中間表現と最適化
├─ zeus-cobol-vm      # ZIR インタプリタ
├─ zeus-cobol-cl      # Cranelift バックエンド (後期)
├─ zeus-decimal       # パック/ゾーン 10 進・2 進演算, 編集, EBCDIC 変換 (CPU エミュレータと共用可)
└─ zeus-igy           # IGYCRCTL 互換のコマンド/オプション/リスト出力
```

## 7. 作業ステップ

| ステップ | 内容 | 完了条件 |
|---|---|---|
| C0 (3 週) | 構文使用一覧表の自動生成、CCVS85 と MVS 3.8j COBOL の比較用ハーネス | 3 アプリの構文使用一覧表、CCVS85 が CI で実行される (全件不合格でよい) |
| C1 (6〜8 週) | ソース管理・字句/構文解析・データ部の配置計算 | 3 アプリの全ソースが構文エラーなしで解析でき、各データ項目のオフセット・長さが一覧出力できる |
| C2 (8〜10 週) | zeus-decimal、MOVE/算術/条件/制御文、ZIR とインタプリタ、DISPLAY | CCVS85 の NC モジュールの大部分に合格、Hercules との 10 進命令差分テストで不一致ゼロ |
| C3 (4〜6 週) | ファイル I/O (0002 のデータセット層と接続)、CALL/入れ子プログラム、組み込み関数 | CCVS85 の SQ/IX/RL/IC に合格。CardDemo のバッチプログラムが全てコンパイル・実行できる |
| C4 (4〜6 週) | EXEC CICS の統合、EIB/COMMAREA、BMS シンボリックマップ | CardDemo のオンラインプログラムが全てコンパイルできる (実行は 0002 の CICS 層と並行) |
| C5 (4〜6 週) | EXEC SQL・EXEC DLI の統合 | GenApp・Bank of Z の全プログラムがコンパイルできる |
| C6 (継続) | Cranelift バックエンド、ソースレベルデバッガ、性能改善 | インタプリタ比で十分な高速化 |

## 8. 0002 計画への影響

- CardDemo バッチ稼働 (M1) が約 3 ヶ月 → 約 6 ヶ月に後ろ倒しになる。
  その間、データ層・JCL エンジン・ユーティリティは並行して開発できる。
- 以降のマイルストーンも約 3 ヶ月ずつ後ろにずれる (0002 §9 で更新済み)。
- 代わりに、EBCDIC・10 進演算・CICS 連携の互換性を自分たちで保証できるようになり、
  後半フェーズでの「互換性差の調査と回避策」の工数が減る見込み。

## 9. リスク

| リスク | 対策 |
|---|---|
| COBOL の文法が巨大で文脈依存 | 3 アプリの構文使用一覧表から実装し、範囲を限定。手書きパーサで誤り回復を持たせる |
| 算術の中間結果規則の再現誤り | 規則を表形式でテスト化。zeus-decimal を Hercules の 10 進命令と突き合わせる |
| インタプリタの性能 | バッチ・オンラインとも当面は問題ない規模。必要になれば Cranelift へ |
| Enterprise COBOL を直接比較に使えない | CCVS85・MVS 3.8j・IBM マニュアル準拠テストの三本立て。ZD&T 保持者の結果を随時取り込む |

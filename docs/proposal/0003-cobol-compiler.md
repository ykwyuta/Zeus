# Proposal 0003: Rust 製 COBOL コンパイラ (zeus-cobol)

- 状態: 提案 (Draft)
- 関連: [0002 z/OS 互換アプリケーション実行環境](0002-zos-application-runtime-plan.md)
  (0002 の COBOL コンパイラを、GnuCOBOL から本提案の自作コンパイラに置き換える)

## 1. 目的

Enterprise COBOL 4.2 (z/OS 1.12 世代) の方言で書かれたソースを、外部の COBOL 処理系に
依存せずにコンパイル・実行できる **Rust 製コンパイラ zeus-cobol** を作る。

- **第 1 目標: NIST COBOL-85 テストスイート (CCVS85) の対象モジュールに全件合格すること** (§5.2)。
  言語処理系としての土台を規格テストで固めてから、アプリケーション対応に進む。
- 第 2 目標: CardDemo・GenApp・Bank of Z の全 COBOL プログラムをソース無修正でコンパイルし、
  z/OS と同じ結果で実行できること。

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

まず **CCVS85 の必須モジュール (§5.2) が要求する COBOL-85 の範囲** を実装し、その後、
3 アプリの **COBOL 構文使用一覧表** (C4 で作成) に基づいて Enterprise COBOL 拡張を追加する。想定される主な範囲:

- **見出し部・環境部**: PROGRAM-ID, 入れ子プログラム, SPECIAL-NAMES, FILE-CONTROL (SELECT/ASSIGN, ORGANIZATION, ACCESS, RECORD KEY, ALTERNATE KEY, FILE STATUS)。
- **データ部**: FD, WORKING-STORAGE, LOCAL-STORAGE, LINKAGE, レベル 01-49/66/77/88,
  PIC 全般 (編集 PIC を含む), USAGE DISPLAY/COMP/COMP-3/COMP-4/COMP-5/BINARY/PACKED-DECIMAL/POINTER/INDEX,
  VALUE, OCCURS (ASCENDING KEY, INDEXED BY, DEPENDING ON), REDEFINES, SIGN LEADING/TRAILING SEPARATE, JUSTIFIED, BLANK WHEN ZERO。
- **手続き部**: MOVE (CORRESPONDING 含む), 算術文 (ADD/SUBTRACT/MULTIPLY/DIVIDE/COMPUTE),
  IF/EVALUATE, PERFORM 全形式, GO TO (DEPENDING ON), CALL/CANCEL/GOBACK/EXIT PROGRAM/STOP RUN,
  INITIALIZE, INSPECT, STRING/UNSTRING, SEARCH/SEARCH ALL, SET, ACCEPT (DATE/TIME 等), DISPLAY,
  ファイル I/O 文 (OPEN/CLOSE/READ/WRITE/REWRITE/DELETE/START), SORT/MERGE (RELEASE/RETURN 含む),
  組み込み関数 (COBOL-85 追補の全関数 + Enterprise COBOL で追加されたものの使用分)。
- **埋め込み文**: EXEC CICS, EXEC SQL (INCLUDE SQLCA / DCLGEN を含む)。EXEC DLI は次のフェーズ (IMS 対応時)。
- **範囲外 (当初)**: オブジェクト指向 COBOL, XML/JSON 文, Report Writer, COBOL 2002 以降の新構文。

## 5. 検証方法 (オラクル)

Enterprise COBOL そのものは使えないため、次を組み合わせる。

| 検証対象 | オラクル |
|---|---|
| COBOL-85 規格の適合性 | **NIST COBOL-85 テストスイート (CCVS85)**。モジュール (NC: 核, SQ/IX/RL: ファイル, IC: 呼び出し, ST: 整列 等) ごとの合格率を CI で追跡 |
| 10 進演算・EBCDIC・編集 | **Hercules** 上で AP/MP/DP/ED 等の命令を実行した結果 (命令単位の差分テスト) |
| 旧規格でも書ける範囲の実行結果 | **Hercules + MVS 3.8j の COBOL** で同じ検査プログラムを実行し比較 |
| COBOL プログラムの実行結果全般 | **GnuCOBOL を比較用オラクルとして積極活用** (製品・ビルドには組み込まない。§5.1) |
| Enterprise COBOL 固有規則 | IBM の Language Reference / Programming Guide に基づく仕様テストを手書き |
| アプリ単位 | 0002 のシナリオテスト (バッチ出力・3270 画面) |

### 5.1 GnuCOBOL による差分テスト

GnuCOBOL は最も手軽に何度でも回せるオラクルなので、次の 3 通りで積極的に使う。

1. **アプリ・テストプログラムの差分実行**: 同じ COBOL ソースを zeus-cobol と GnuCOBOL (IBM 方言モード,
   `-std=ibm` 系) でコンパイル・実行し、DISPLAY 出力と出力ファイルを比較する。
   CICS / SQL を含まないバッチプログラムと CCVS85 が対象。
2. **ランダム生成プログラムによる差分ファジング**: PIC・USAGE・符号・桁数を組み合わせた
   データ定義と MOVE / 算術 / COMPUTE / 編集 / INSPECT / STRING を自動生成し、両者で実行して比較する。
   不一致は自動で最小化してから判定する。
3. **既知の差異の台帳**: GnuCOBOL は ASCII を内部表現とし、Enterprise COBOL と挙動の異なる箇所がある。
   比較は文字コードを正規化した上で行い、不一致は「zeus-cobol の誤り」「GnuCOBOL と Enterprise COBOL の
   差」に仕分けて `docs/oracle/gnucobol-differences.md` に記録する。後者は IBM マニュアル・Hercules・
   MVS 3.8j で裁定し、Enterprise COBOL 側の挙動を正とする。

GnuCOBOL のバージョンは CI で固定し、ライセンス (GPL/LGPL) 上もテスト実行にのみ使うため問題ない。

### 5.2 CCVS85 の合格基準

CCVS85 はモジュールごとに多数のテストプログラムから成り、各プログラムが自分で PASS / FAIL /
DELETED / INSPECT を報告する。以下を「合格」と定義する。

| モジュール | 内容 | 扱い |
|---|---|---|
| NC | 核 (データ定義, MOVE, 算術, 条件, PERFORM 等) | **必須** |
| SM | 原始文操作 (COPY, REPLACE) | **必須** |
| IC | プログラム間連絡 (CALL, 入れ子プログラム) | **必須** |
| SQ | 順次ファイル | **必須** |
| RL | 相対ファイル | **必須** |
| IX | 索引ファイル | **必須** |
| ST | 整列併合 (SORT / MERGE) | **必須** |
| IF | 組み込み関数 (1989 追補) | **必須** |
| SG | セグメンテーション | **合格対象** |
| OB | 廃要素 (ALTER 等) | **合格対象** |
| DB | デバッグ (USE FOR DEBUGGING) | **合格対象** |
| RW | 報告書作成 | **合格対象** (Enterprise COBOL は前処理系で対応) |
| CM | 通信 | 対象外 (唯一の例外。MCS が前提で Enterprise COBOL も非対応) |

- 方針: GnuCOBOL が合格できないものも含め、CM 以外のすべてのモジュールに合格させる。

- **合格 = 必須モジュールで FAIL ゼロ・INSPECT はすべて目視確認済み**。DELETED (意図的な除外) は
  Enterprise COBOL も非対応の機能に限り、理由を `docs/oracle/ccvs85-exclusions.md` に 1 件ずつ記録する。
- **GnuCOBOL を基準線**にする: 同じスイートを GnuCOBOL で流した結果をモジュール・プログラム単位で
  保存し、zeus-cobol の結果と並べて CI で表示する。
- テストプログラムの実行は EBCDIC (既定のコードページ) で行う。照合順序に依存するテストは
  IBM の挙動を正として判定する。
- スイートは 1 本の原本ファイル (newcob.val) から各プログラムを取り出して使う。取り出しツールは Rust で書き、
  原本に付属する取り出し用 COBOL プログラム (EXEC85) 自体も、後で zeus-cobol のテストとしてコンパイル・実行する。

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

CCVS85 合格 (C0〜C3) を先に完了させ、その後でアプリケーション向けの拡張 (C4 以降) に進む。

| ステップ | 内容 | 完了条件 |
|---|---|---|
| C0 (3 週) | CCVS85 の取り出し・実行・結果集計ハーネス、GnuCOBOL での基準線取得、Hercules 10 進命令の差分ハーネス | CCVS85 全モジュールの GnuCOBOL 基準線が CI に表示され、zeus-cobol 側の枠 (全件不合格) も並ぶ |
| C1 (6〜8 週) | ソース管理 (固定形式, COPY/REPLACE)・字句/構文解析・データ部の配置計算 | CCVS85 の全プログラムが構文エラーなしで解析でき、データ項目の配置を一覧出力できる |
| C2 (8〜10 週) | zeus-decimal、MOVE/算術/条件/制御文/PERFORM、ZIR とインタプリタ、DISPLAY/ACCEPT | **NC・SM に合格**。Hercules との 10 進命令差分で不一致ゼロ |
| C3 (8〜10 週) | ファイル I/O (順次・相対・索引。0002 のデータセット層の最小版に接続)、SORT/MERGE、CALL/入れ子プログラム、組み込み関数 | **CCVS85 の必須モジュールすべてに合格 (= 第 1 目標達成)** |
| C4 (4〜6 週) | Enterprise COBOL 拡張 (COMP-5, POINTER, LOCAL-STORAGE, 追加の組み込み関数, コンパイラオプション)、`IGYCRCTL` 互換、アプリ構文使用一覧表に基づく不足分 | CardDemo のバッチプログラムが全てコンパイル・実行できる |
| C5 (4〜6 週) | EXEC CICS の統合、EIB/COMMAREA、BMS シンボリックマップ | CardDemo のオンラインプログラムが全てコンパイルできる |
| C6 (4〜6 週) | EXEC SQL の統合 (EXEC DLI は次のフェーズ) | GenApp と Bank of Z (CICS 経路) の全プログラムがコンパイルできる |
| C7 (継続) | Cranelift バックエンド、ソースレベルデバッガ、性能改善 | インタプリタ比で十分な高速化 |

### 進捗 (2026-10)

- C0〜C3 完了: CCVS85 の必須 8 モジュール (NC, SM, IC, SQ, RL, IX, ST, IF) すべてに合格。
  テスト合格数は各モジュールとも GnuCOBOL 基準線と同数 (IF は GnuCOBOL がコンパイルできない
  IF119A 等も合格)。除外 (DELETED) としたテストはない。
- 文字コード依存の実装者定義値は `ccvs85/exec85-zeus.conf` で EBCDIC の値にしている
  (X-63/X-64 のネイティブ照合順序のみ)。
- 残り: ファイル I/O はまだ zcobc の VM 内にあり、0002 のデータセット層 (zeus-ds) への切り出しは
  C4 と並行して行う。
- 任意モジュール SG・OB・DB にも合格 (テスト合格数は GnuCOBOL と同数)。DB のうち通信節 (CD) を
  使う 2 本 (DB205A, DB304M) のため、CD 記述と ENABLE/DISABLE/SEND/RECEIVE/PURGE 文は
  構文だけ受け付ける (実行するとエラー)。
- RW にも合格 (40/40)。報告書記述 (RD)・報告書集団 (RH/PH/CH/DE/CF/PF/RF)・LINE/NEXT GROUP/COLUMN/
  SOURCE/VALUE/SUM (UPON, RESET, 繰り上げ)/GROUP INDICATE・CONTROL による改段・頁管理・
  USE BEFORE REPORTING/SUPPRESS PRINTING を実装。CCVS85 にない CONTROL と SUM は単体テストで確認。
- 対象外は CM のみ。CCVS85 自身が無条件に DELETED にしているテスト (ソース中の「DELETE THE NEXT LINE
  TO DELETE THE TEST」等) は GnuCOBOL と同じく実行されない。

## 8. 0002 計画への影響

- 最初のマイルストーンが「CCVS85 合格」(約 6.5 ヶ月) になり、CardDemo バッチ稼働は約 8 ヶ月に後ろ倒しになる。
- C3 で必要な順次・相対・索引ファイルの実装は、0002 のデータセット層 (zeus-ds) の最小版として作り、
  そのまま VSAM・JCL 対応に発展させる。JCL エンジンとユーティリティは C2〜C3 の間に並行して着手できる。
- 規格テストで言語処理系の土台を先に固めるため、アプリ対応の段階で
  「コンパイラの誤りかランタイムの誤りか」の切り分けが容易になる。

## 9. リスク

| リスク | 対策 |
|---|---|
| COBOL の文法が巨大で文脈依存 | 3 アプリの構文使用一覧表から実装し、範囲を限定。手書きパーサで誤り回復を持たせる |
| 算術の中間結果規則の再現誤り | 規則を表形式でテスト化。zeus-decimal を Hercules の 10 進命令と突き合わせる |
| インタプリタの性能 | バッチ・オンラインとも当面は問題ない規模。必要になれば Cranelift へ |
| Enterprise COBOL を直接比較に使えない | CCVS85・MVS 3.8j・IBM マニュアル準拠テストの三本立て。ZD&T 保持者の結果を随時取り込む |

# Proposal 0022: z/OS COBOL の追加コーパス

- 状態: 提案 (Draft)
- 作成日: 2026-10-19
- 関連:
  - [0003 Rust 製 COBOL コンパイラ (zeus-cobol)](0003-cobol-compiler.md)
  - [0012 C1 網羅テストケースの自動生成](0012-c1-test-generation.md)
  - [0012 HLASM](0012-hlasm.md)
  - [0019 z/OS COBOL 互換性テストスイート (ZCVS)](0019-zos-cobol-test-suite.md)
  - [0020 z/OS PL/I 互換性テストスイート (ZPVS)](0020-zos-pli-test-suite.md)
  - [アプリケーションの構文使用状況](../inventory/apps.md)

## 1. 背景と目的

Zeus が今使っている COBOL のコーパスは次の 5 つである。

| コーパス | 取得 | 性格 |
|---|---|---|
| CCVS85 (NIST) | `tools/fetch-ccvs85.sh` | COBOL-85 規格適合性 (処理系に依存しない) |
| ZCVS | リポジトリーに同梱 (`zcvs/`) | z/OS 固有の言語・実行互換性 ([0019](0019-zos-cobol-test-suite.md)) |
| AWS CardDemo | `tools/fetch-carddemo.sh` | CICS / VSAM / バッチ / Db2 / IMS / MQ のサンプル業務アプリ |
| CICS GenApp | `tools/fetch-genapp.sh` | CICS / Db2 / VSAM の保険サンプル |
| Bank of Z | `tools/fetch-bankofz.sh` | CICS / IMS / MQ / Db2 の銀行サンプル (CBSA の後継) |

3 つの業務アプリはどれもベンダーが**デモ用に書き下ろしたサンプル**であり、次の点が手薄である。

1. **実運用されていたコード**: 実際の z/OS で長年保守されたソースに特有の書き方 (ライブラリアン管理用の埋め込み変数、
   `CBL`/`PROCESS` の多様なオプション、一連番号・識別領域付きの行、`;` 区切り、古い COPY REPLACING の慣用) がない。
2. **CICS の Web/HTTP 系 API**: `WEB SEND/RECEIVE/CONVERSE`、`DOCUMENT`、`INQUIRE URIMAP` などはサンプル 3 本にほぼ出てこない
   ([apps.md](../inventory/apps.md) の EXEC CICS 表に `WEB` がない)。
3. **COBOL と HLASM の混在**: [0012-hlasm](0012-hlasm.md) の実ソースは CardDemo の数本しかない。
4. **バッチコンパイル (1 ファイルに複数の `PROCESS` + プログラム)**、DBCS オプション、`CODEPAGE` 指定などのコンパイル単位の設定。
5. **IBM コンパイラーの実出力 (オラクル)**: CCVS85 を IBM Enterprise COBOL で実際にコンパイルした結果 (診断メッセージ・
   オプション一覧・XREF) が手元にない。比較オラクルは GnuCOBOL と Hercules に限られる。

本提案では、ライセンス上取得して使えるもののうち、上の不足を埋める公開コーパスを選び、取得スクリプト・インベントリー・
回帰テストへの組み込み方を決める。

## 2. 選定基準

- **ライセンス**: OSI 承認ライセンスまたは同等 (Apache-2.0, MIT, EPL-2.0, CC-BY-4.0, CeCILL-2.1) であること。
  ライセンスのない個人リポジトリー、利用目的を制限するもの (「自社製品のデモ目的に限る」等) は対象外。
- **取得のみ・同梱しない**: [CONTRIBUTING.md](../../CONTRIBUTING.md) の規則に従い、既存の `tools/fetch-*.sh` と同じく
  固定コミットで `.cache/` に取得し、リポジトリーにはコミットしない。IBM 製のサンプルも取得のみとし、Zeus の実装・テストに
  転載しない (Bank of Z / GenApp と同じ扱い)。
- **z/OS の方言**: IBM Enterprise COBOL (または z/OS 上の旧 COBOL) で書かれていること。GnuCOBOL 専用の自由形式コードや
  LLM 評価用に他言語から機械変換したコードは対象外。
- **既存コーパスとの重複が少ないこと**: CCVS85・CardDemo・GenApp のコピーを含むリポジトリーは、重複しない部分だけを使う。

## 3. 事前調査の結果

候補を固定コミットで取得し、`zcobc inventory` (2026-10-19 時点の `phase1-compiler`) でコンパイルした結果を示す。
「コンパイル可」はプログラム数に対する件数。失敗理由の詳細は §5 に示す。

| # | 候補 | ライセンス | 内容 | 規模 | コンパイル可 |
|---|---|---|---|---|---|
| A1 | [walmartlabs/zFAM](https://github.com/walmartlabs/zFAM) | Apache-2.0 | Walmart の z/OS File Access Manager。CICS Web (HTTP) 上の KVS。COBOL + HLASM + RDO + IDCAMS | COBOL 14 本 (14,560 行)、HLASM 11、マクロ 4、JCL 12 | 0 / 14 |
| A2 | [walmartlabs/zECS](https://github.com/walmartlabs/zECS) | Apache-2.0 | Walmart の Enterprise Cache Service。zFAM と同系統 | COBOL 5 本 (4,055 行)、HLASM 2、JCL 10 | 0 / 5 |
| A3 | [walmartlabs/zUID](https://github.com/walmartlabs/zUID) | Apache-2.0 | 一意 ID 発行サービス。**ソースが EBCDIC のまま**格納されている | COBOL 2、HLASM 3、JCL 6 | 1 / 2 (EBCDIC 指定時) |
| A4 | [etalab/taxe-fonciere](https://github.com/etalab/taxe-fonciere) | CeCILL-2.1 | 仏財務総局 (DGFiP) が公開した固定資産税の計算モジュール。実運用のバッチ用サブプログラム (CA Librarian の埋め込み変数あり) | COBOL 6 本 (2,306 行)、COPY 17 (2,171 行) | 2 / 6 |
| A5 | [IBM/IBM-Z-zOS](https://github.com/IBM/IBM-Z-zOS) の `zOS-Print/ACIF-User-Exit-samples` | Apache-2.0 | ACIF (Print Services) の利用者出口。一連番号付きの長年保守されたソース | COBOL 18 本 (6,106 行) | 0 / 18 |
| B1 | [IBM/dbb](https://github.com/IBM/dbb) の `zBuilder/MortgageApplication` | Apache-2.0 | CICS + Db2 + BMS の小さな住宅ローン計算アプリ。DBB のビルド定義付き | COBOL 6 本 (4,925 行)、COPY 6、BMS 2 | 3 / 6 |
| B2 | [IBM/zopeneditor-sample](https://github.com/IBM/zopeneditor-sample) | Apache-2.0 | SAM1/SAM2 (顧客ファイル更新バッチ) と JCL | COBOL 3 本 (1,149 行)、JCL 8 | 2 / 3 |
| B3 | [openmainframeproject/cobol-programming-course](https://github.com/openmainframeproject/cobol-programming-course) | CC-BY-4.0 (コードは教材) | Open Mainframe Project の Enterprise COBOL 教材。ラボの COBOL と実行用 JCL | COBOL 30 本 (3,261 行)、JCL 37 | 27 / 30 |
| B4 | [openmainframeproject/cobol-check](https://github.com/openmainframeproject/cobol-check) | Apache-2.0 | COBOL 単体テストフレームワーク。テスト対象の COBOL とテスト用の生成コード | COBOL 15 本 (1,024 行) | 10 / 15 |
| C1 | [eclipse-che4z/che-che4z-lsp-for-cobol](https://github.com/eclipse-che4z/che-che4z-lsp-for-cobol) の `tests/test_files/Cobol85PositiveTestsSuite/compileListing` | EPL-2.0 | **CCVS85 の 233 本を IBM Enterprise COBOL 6.3 (5655-EC6) で実際にコンパイルしたリスト**。オプション一覧・IGY 診断・XREF を含む (MAP なし) | リスト 233 本 | (オラクル) |
| C2 | [cschneid-the-elder/mapa](https://github.com/cschneid-the-elder/mapa) の `cobol/testdata/local`, `jcl/testdata` | MIT | 構文解析器のテスト用に書かれた COBOL 断片と z/OS JCL | COBOL 355、JCL 216 | 未計測 |
| D1 | [navikt/DSF](https://github.com/navikt/DSF) | MIT | ノルウェー労働福祉局 (NAV) の年金基幹システム「Det Sentrale Folketrygdsystemet」。実運用コード。**大半は PL/I** | PL/I 1,473 本 (580,678 行)、COBOL 7 本 (OS/VS COBOL) | 4 / 7 |

## 4. 候補ごとの用途

### 4.1 A 群: 実運用コード (最優先)

**A1–A3 Walmart zFAM / zECS / zUID** — 今のコーパスにない次の領域を、実運用されていたコードで一度に検証できる。

- CICS Web 系: `WEB SEND` (46 回)、`WEB WRITE` (29)、`WEB READ` (21)、`WEB CONVERSE` (19)、`DOCUMENT CREATE/RETRIEVE`、
  `WEB PARSE`、`WEB OPEN/CLOSE`、`INQUIRE URIMAP`、`PUT/GET CONTAINER`、`ENTER TRACENUM`、`GETMAIN/FREEMAIN`、
  `START TRANSID`、`WRITEQ TD`。zeus-cics の HTTP (CICS Web Support) 対応の受け入れ基準になる。
- COBOL から HLASM の CICS プログラム (`ZFAM001` など) を `LINK` / `XCTL` する混在構成。[0012-hlasm](0012-hlasm.md) の
  実ソースコーパスを CardDemo 以外に広げる。
- RDO (CSD 定義)、IDCAMS (VSAM KSDS 定義) を含むので、`zcics` の資源定義の取り込み経路も試せる。
- zUID はソースが EBCDIC のバイト列 (行区切りは EBCDIC の LF) で格納されており、`--source-encoding ebcdic` の
  実データでの検証にもなる。

**A4 DGFiP 固定資産税 (taxe-fonciere)** — LINKAGE で入出力レコードを受け取り計算結果を返す純粋な計算サブプログラムで、
画面もファイルもない。[0012](0012-c1-test-generation.md) の C1 テスト生成 (`ztestgen`) の対象として理想的である
(入力は LINKAGE のみ、分岐は税率・控除の判定)。`OCCURS` 65、`REDEFINES` 48 を使う。年度ごとのモジュール分岐
(`CALL 'EFITA3B8'` 等) は動的 CALL の解決 ([0011](0011-dependency-analysis.md)) の例にもなる。

**A5 ACIF 利用者出口** — 一連番号 (列 1–6) と識別領域 (列 73–80) が全行にあり、`PROCESS` 行にも付いている。
`;` を文の区切りに使うなど、古い保守スタイルの字句規則を検証できる。

### 4.2 B 群: 小規模の z/OS サンプル・教材

- **B1 MortgageApplication**: CICS + Db2 (`EXEC SQL`) + BMS の最小構成。1 ファイルに `PROCESS` + プログラムを
  複数並べた**バッチコンパイル**形式 (`epscsmrd.cbl`) と `PROCESS CODEPAGE(1140),NSYMBOL(NATIONAL),ARITH(EXTEND)` を含む。
- **B2 zopeneditor-sample**: 顧客ファイル更新バッチ。JCL ごと `zjob` で流せる大きさ。
- **B3 cobol-programming-course**: ラボの JCL (`//COBOL EXEC IGYWCL` 系) と期待される実行結果が教材の本文に書かれている。
  `zjob` でのコンパイル・実行の E2E 回帰に使える。CBL0002/CBL0009/CBL0012 のコンパイルエラーは教材が意図した
  「直す練習」の誤りとみられ (要確認)、負系テスト (ZCVS `ZE`) として扱える。
- **B4 cobol-check**: COBOL の単体テストフレームワークが生成する COBOL を Zeus で実行できれば、利用者が Zeus 上で
  cobol-check を使える。テスト用ファイルには意図的な不正コードが含まれるので、使うのは `src/main/cobol` の
  正常系のみとする。

### 4.3 C 群: オラクル・構文解析用

- **C1 IBM Enterprise COBOL 6.3 のコンパイルリスト (Che4z)**: CCVS85 を実際の IBM コンパイラーに通した結果であり、
  現在の比較オラクル (GnuCOBOL、Hercules) にはない **IBM 自身の判定**である。次の用途がある。
  1. IGY 診断 (メッセージ番号・重大度・行) と Zeus の診断の突き合わせ。例えば `IGYGR1216-I`
     (`RECORDING MODE F` を仮定) のような情報メッセージを Zeus が出すかどうか。
  2. 233 本それぞれで IBM が E/S/U を出したかどうかを [ccvs85-exclusions.md](../oracle/ccvs85-exclusions.md) の
     除外判断の根拠にする。
  3. リスト冒頭の「Options in effect」は 6.3 の既定オプション一覧の実例で、[0015](0015-cobol-ignored-clauses.md) の
     オプション監査に使える。
  リストは IBM のコンパイラーの出力であり、IBM のコードではない。ただし CONTRIBUTING の趣旨に沿って取得のみとし、
  リポジトリーには抜粋も含めない (差異の記録は `docs/oracle/` に自分の言葉で書く)。
- **C2 mapa**: 構文解析器向けの COBOL 断片 355 本と JCL 216 本。JCL は zeus-jcl の構文解析の回帰
  (`zjob --check` 相当) に使う。`cobol/testdata/nist` は CCVS85 の改変コピーなので使わない。

### 4.4 D 群: PL/I (参考)

**D1 NAV DSF** は COBOL としては 7 本の OS/VS COBOL (`EXHIBIT`、`REMARKS` 段落など COBOL-85 以前の構文) しかなく、
COBOL コーパスとしての価値は低い。一方で PL/I は 1,473 本・58 万行の実運用コードであり、[0004](0004-pli-compiler.md) /
[0020](0020-zos-pli-test-suite.md) の PL/I コーパスとして別に検討する価値が大きい (本提案の範囲外)。

## 5. 事前調査で見つかった Zeus の非互換

§3 の失敗のうち、コーパス側の問題 (欠けた COPY メンバー、教材の意図的な誤り) を除くと、次の Zeus 側の課題が残る。
各項目は ZCVS ([0019](0019-zos-cobol-test-suite.md)) に回帰テストを追加して直す。

| # | 現象 | 再現元 | 推定原因・方針 |
|---|---|---|---|
| G1 | `CBL CICS(SP)` を拒否 (`unsupported or invalid CBL/PROCESS option`) | zFAM/zECS 全 19 本 | 統合 CICS 翻訳系の `CICS('SP')` サブオプション (SP 系コマンドの許可) を受け付ける |
| G2 | `CBL DBCS` を拒否 | zFAM 3 本 | `DBCS` / `NODBCS` オプションを受け付ける (既定は `DBCS`。C1 のリストでも `DBCS` が有効) |
| G3 | `PROCESS ... NOSEQ` / `NOD` を拒否 | ACIF 16 本 | `SEQUENCE`/`NOSEQUENCE` と、短縮形 (`NOD` = `NODECK` 等) の表を IBM の短縮形一覧に合わせる |
| G4 | (取り下げ) 一連番号付き `PROCESS` 行の列 73–80 をオプションとして読む | — | 調査時にソースを編集して列がずれたための誤認。Zeus は `CBL`/`PROCESS` 行を列 72 で打ち切っている |
| G5 | 1 ファイル中の 2 つ目の `PROCESS` (バッチコンパイル) を拒否 (`IDENTIFICATION DIVISION expected (found PROCESS)`) | MortgageApplication `epscsmrd.cbl:331` | `END PROGRAM` の後の `CBL`/`PROCESS` を次のコンパイル単位の開始として扱う |
| G6 | `DFHVALUE(GET/POST/DELETE/HTTP/HTTPS/IMMEDIATE)` が未定義 | zFAM/zECS/zUID 全本 (G1/G2 を外した後) | CICS Web 系・`SYNCPOINT`/`ENQ` 系の CVDA 値を表に追加 |
| G7 | `OPEN OUTPUT F;` の直後の `ELSE` をファイル名として読む (`file ELSE is not defined`) | ACIF `APKPTXOP.COB:251` | `;` を区切り文字として扱ったうえで、OPEN のファイル名の並びを予約語で終える |
| G8 | `COPY X REPLACING 'X' BY BASENB.` で、コピー内の `'X'-CLE` が置換されず構文エラー | taxe-fonciere `XBASNB.cpy:4` 他 (3 本) | 英数字定数を置換対象にした REPLACING で、直後に `-` が続く語の扱いを Enterprise COBOL の仕様で確認して対応 (実運用で使われていた書き方) |
| G9 | `REPLACE ==:PROGRAM:== BY ...` で、小文字の `:program:-param` が置換されない | cobol-check `REPLAC.CBL:32` | 擬似テキストの照合を大文字小文字無視にする。`:` を区切りとする 6.x の規則も確認する |

コーパス側の問題で、取得スクリプトでの対処が必要なもの:

- zUID は EBCDIC (CCSID 037) のまま格納されている → `--source-encoding ebcdic --ccsid 37` で読む (取得時に変換はしない)。
- taxe-fonciere の `EFITAUX2.cob` は Latin-1 (UTF-8 でない) → 取得スクリプトで UTF-8 に変換する。
  `FMSTAU2` が使う `XB30`/`XB35` は公開物に含まれていない → インベントリーの対象外とする。
- MortgageApplication の `EPSMORT`/`EPSMLIS` は BMS から生成する記号マップ → 既存の `zbms --copy` で生成する。
- ACIF の `AFPAGOT` は IBM 製品の COPY メンバー、`STRFLDS` はプログラムではなく COPY メンバー → 対象外。
- cobol-check の `MOCK`/`MOCKTEST` の `BY VALUE PIC X(80)` は、英数字の BY VALUE を長さ 1 に限る IBM の規則どおりの診断とみられる (仕様で要確認) → 対象外。

## 5.1 実装 (2026-10-19)

§5 の G1–G3、G5–G9 を実装し、A・B 群は `XML PARSE` を使う 1 本を除いて全プログラムがコンパイルできる
([corpora.md](../inventory/corpora.md))。

- 取得とインベントリー: `tools/fetch-corpora.sh` (§7.1 の固定コミット。ACIF と MortgageApplication は sparse checkout)、
  `tools/corpora-inventory.sh` (コーパス側の理由で除くプログラムとその理由を出力に列挙する)。
- G1: `CICS(SP)` のように区切り記号なしのサブオプションも受け付ける。
- G2/G3: `DBCS`、`NOSEQUENCE` (`NOSEQ`)、`NODECK` (`NOD`) は Zeus の動作そのもの (シフトコードで DBCS 文字を区切る・
  一連番号を検査しない・オブジェクトデッキを書かない) なので受け付ける。逆の `NODBCS`・`SEQUENCE`・`DECK` は
  実装がないため従来どおり診断する。
- G5: `END PROGRAM` の後の `CBL`/`PROCESS` から次のコンパイル単位とし、単位ごとにそのオプションでコンパイルして
  1 つのモジュールに結合する (`link::modules`)。ほかの単位の行は空行にして行番号を保つ。
- G6: CVDA を CICS TS 5.5 の一覧 (「CVDAs and numeric values in alphabetic sequence」) の値に置き換えた
  (`front/cvda.rs`)。従来の 12 個は Zeus 独自の値だったが、ランタイムは名前で引いているので影響はない。
- G7: OPEN と CLOSE のファイル名の並びを `ELSE`・`WHEN`・`END-` で始まる語で終える。
- G8/G9: 英数字定数・`)`・`:` の直後に区切りなしで書いた `-語` を字句解析で印付けし、COPY REPLACING / REPLACE の
  後に、置換で前に語が来たときだけその語につなげる (`'X'-KEY` → `WS-LIT-KEY`、`:TAG:-VALUE` → `WS-TAG-VALUE`)。
  置換されなければ従来どおり `-` と語になる。G9 の大文字小文字は原因ではなかった。
- 調査中に見つかった追加の非互換: `INSPECT FUNCTION REVERSE(x) TALLYING ...` (zFAM 8 本)。関数の結果を TALLYING
  で調べる命令 `InspectValue` を ZIR に追加した (REPLACING / CONVERTING は受け取り側になるので診断する)。
- 回帰テスト: ZCVS `ZS104A` (COPY REPLACING・REPLACE・OPEN/CLOSE・INSPECT・バッチコンパイルの単位ごとの `TRUNC`)、
  `crates/zeus-cobol/tests/compiler_control.rs` (オプション、`DFHVALUE` の値、バッチ境界)。

残り:

- MortgageApplication `EPSCSMRD` の 3 番目の単位が `XMLPARSE(COMPAT)` と `XML PARSE` 文を使う。Zeus には
  XML PARSE がまだないため、オプションだけを受け付けることはせず未対応のままとする。
- G8 の `'X'-KEY` の置換が Enterprise COBOL の仕様でどう定められているかは未確認 (実運用のソースの書き方に合わせた)。

## 6. 採用しないもの

| 候補 | 理由 |
|---|---|
| [RocketSoftwareCOBOLandMainframe/BankDemo](https://github.com/RocketSoftwareCOBOLandMainframe/BankDemo) | 規模 (COBOL 82、BMS 14、SQL 21) は魅力だが、ライセンスが「Rocket 製品とのデモ目的に限る」と利用目的を制限している |
| [cicsdev/cics-banking-sample-application-cbsa](https://github.com/cicsdev/cics-banking-sample-application-cbsa) | アーカイブ済みで HEAD は README のみ。内容は Bank of Z に引き継がれている |
| sentientsergio/COBOL-Legacy-Benchmark-Suite、ibm-wsc/zCONNEE-Wildfire-Workshop、各種個人の COBOL/CICS/DB2 学習リポジトリー | ライセンスの記載がない |
| uwol/proleap-cobol-parser のテスト資源 | 大半 (510 本) が CCVS85 のコピー。残りは構文解析器向けの断片で C2 と重複する |
| Che4z の `aws-mainframe-modernization-carddemo` / `cicsGenApp` | 既存コーパスのコピー |
| Che4z の `daco-dialect-support` | Broadcom DaCo の方言で、Enterprise COBOL ではない |
| [zorse-project/COBOLEval](https://github.com/zorse-project/COBOLEval)、IBM Project CodeNet の COBOL、X-COBOL データセット | GnuCOBOL 向けの自由形式・競技プログラミング・LLM 評価用。z/OS 方言ではなく、X-COBOL は収録リポジトリーごとにライセンスが異なる |
| Db2 (`DSN8*`)、IMS (`DFSIVP*`)、MQ (`CSQ4*`) の製品同梱サンプル | IBM 製品の付属物で公開ライセンスがない |

保留: NIST SQL Test Suite 6.0 (埋め込み SQL の COBOL 約 300 本、米国政府著作物) は Db2 プリコンパイラのコーパスとして
有力だが、現在の配布元が見つかっていない。入手経路が見つかれば別提案とする。

## 7. 導入計画

### 7.1 取得スクリプト

既存の `tools/fetch-genapp.sh` と同じ形式 (固定コミット、`git fetch --depth 1`、`.cache/<名前>` へ取得) で追加する。
2026-10-19 時点の固定コミットは次のとおり。

| スクリプト | リポジトリー | コミット |
|---|---|---|
| `tools/fetch-walmart.sh` | walmartlabs/zFAM, zECS, zUID | `f57e36ad…`, `6d6bcbbc…`, `4bf25a2e…` |
| `tools/fetch-taxe-fonciere.sh` | etalab/taxe-fonciere | `6bd40b2d…` (EFITAUX2 を UTF-8 に変換) |
| `tools/fetch-ibm-samples.sh` | IBM/dbb, IBM/zopeneditor-sample, IBM/IBM-Z-zOS (sparse checkout) | `ce2c1be5…`, `8f983530…`, `083802e0…` |
| `tools/fetch-omp.sh` | openmainframeproject/cobol-programming-course, cobol-check | `780b1c96…`, `c79624bd…` |
| `tools/fetch-che4z-listings.sh` | eclipse-che4z/che-che4z-lsp-for-cobol (sparse checkout: `compileListing` のみ) | 導入時に固定 |
| `tools/fetch-mapa.sh` | cschneid-the-elder/mapa (sparse checkout: `jcl/testdata`) | `6ebe2d40…` |

### 7.2 段階

| 段階 | 内容 | 完了条件 |
|---|---|---|
| **K1 インベントリー** | 取得スクリプトを追加し、`tools/apps-inventory.sh` を A 群・B 群に広げて [apps.md](../inventory/apps.md) に列を足す | A・B 群の全プログラムが表に載り、コンパイル不可の理由が一覧される |
| **K2 字句・オプション (G1–G5, G7–G9)** | §5 の非互換を ZCVS に回帰テストを追加して直す | A1/A2/A4/A5/B1/B4 の (コーパス側の問題を除く) 全プログラムが `zcobc` でコンパイルできる |
| **K3 CICS Web (G6 以降)** | `DFHVALUE` の追加、zeus-cics の `WEB`/`DOCUMENT`/`URIMAP` 対応、zFAM の RDO・IDCAMS 取り込み | zFAM に HTTP で PUT/GET/DELETE してデータが往復するスクリプト (`tools/zfam.sh`) が CI で通る |
| **K4 バッチ E2E** | B2/B3 の JCL を `zjob` で実行し、教材記載の出力と比較 | `tools/omp-course.sh` が全ラボで RC 0 (意図的な誤りのラボを除く) |
| **K5 IBM リストのオラクル化** | C1 のリストから IGY 診断を抽出するツールを `zeus-ccvs` に追加し、Zeus の診断と比較した表を `docs/oracle/ibm-listing-differences.md` に出す | 233 本の比較表ができ、差異ごとに「Zeus を直す」「記録して許容」を決める |
| **K6 C1 テスト生成** | 全コーパスに `ztestgen` を適用 (`tools/corpora-testgen.sh`) | 実施済み (2026-10-20): 91 本で 43% から 78%、全ケースが `ztest` で再現。結果と拡張は [0012](0012-c1-test-generation.md) の「追加コーパスでの検証」 |

## 8. 未確認事項

- G8 (`'X'-CLE` の置換) が Enterprise COBOL の仕様上どう扱われるか (仕様の該当箇所を確認してから実装する)。
- B3 のコンパイルエラー 3 本 (CBL0002、CBL0009、CBL0012) が教材の意図した誤りかどうか。
- C1 のリストを `docs/oracle/` での比較に使うことが、EPL-2.0 と CONTRIBUTING の規則の両方に照らして問題ないか
  (リストの内容は転載せず、差異の要約だけを書く想定)。
- D1 の PL/I を [0020](0020-zos-pli-test-suite.md) のコーパスに加えるかどうか (別途判断)。

# 0011: 依存関係の分析 (zdeps)

COBOL・PL/I・JCL などのソースを読み、呼出しと取込みの関係をたどってグラフにし、CSV に書き出す。ノードはファイル
(見つからない資産や Zeus が提供する資産は仮想のノード)、エッジは呼出し関係で、エッジごとにソースの中の位置を持つ。
あわせて、Zeus のコンパイラで通らないプログラムや Zeus が対応しない構文、見つからない資産を報告する。

## 使い方

```text
zdeps analyze [--lib DIR]... [--out DIR] [--no-compile] [--bom] ROOT...
zdeps masters [--out DIR] [--bom]
```

- `ROOT` はファイルかディレクトリー。ディレクトリーなら下にあるソースをすべて起点にする。ファイルなら、そのファイルから
  たどれるものだけを分析する。
- `--lib DIR` (`-I`・`-L` でも同じ): COPY・%INCLUDE のライブラリー、カタログ式プロシージャー、呼出し先を探す
  ディレクトリー。何度でも指定できる。ここにあるファイルは起点にはならず、参照されたときだけノードになる。
- `--out DIR`: 出力先 (既定は `deps`)。マスタと結果の CSV を書く。
- `--no-compile`: COBOL と PL/I のプログラムを Zeus のコンパイラにかけない (コンパイル結果は `NOT_CHECKED`)。
- `--bom`: CSV の先頭に UTF-8 の BOM を付ける (表計算ソフトで開くとき)。
- 終了コードは、見つからない資産 (status `MISSING`) があれば 4、なければ 0 (動的な呼出し先だけなら 0)。
  引数の誤りは 2、起点がないときや書き出せないときは 8。
- `zdeps masters` はマスタだけを書く。

CSV は RFC 4180 の形式 (区切りはカンマ、必要なときは二重引用符で囲む、行末は CRLF)、文字コードは UTF-8。
複数の値を持つ属性 (データ型 `LIST`) は空白で区切る。パスは実行したディレクトリーからの相対パス。

## マスタ

分析の前提になる定義。プログラムに組み込まれ (`crates/zeus-deps/src/master.rs`)、`analyze` と `masters` が書き出す。

| ファイル | 内容 | 列 |
|---|---|---|
| `node_types.csv` | ノードマスタ: ノードの種類 | node_type, name, is_file, extensions, description |
| `edge_types.csv` | エッジマスタ: エッジの種類と、つなげるノードの種類 | edge_type, name, from_node_types, to_node_types, description |
| `node_attribute_types.csv` | ノード種ごとの属性マスタ (`*` はすべての種類) | node_type, attribute, name, data_type, description |
| `edge_attribute_types.csv` | エッジ種ごとの属性マスタ (`*` はすべての種類) | edge_type, attribute, name, data_type, description |
| `system_assets.csv` | 提供資産マスタ: Zeus が提供するプログラムとコピーメンバー | asset_name, node_type, provider, description |

データ型は `TEXT`・`INTEGER`・`LIST`・`FLAG` (Y/N)。

### ノードの種類

| 種類 | 内容 | ファイル |
|---|---|---|
| JCL_JOB | JOB 文で始まるジョブ | .jcl .job |
| JCL_PROC | カタログ式プロシージャー | .proc .prc |
| JCL_INCLUDE | INCLUDE MEMBER= で取り込まれる JCL | |
| COBOL_PROGRAM | COBOL のプログラム | .cbl .cob |
| COBOL_COPYBOOK | COBOL のコピーメンバー | .cpy .dcl |
| PLI_PROGRAM | PL/I のプログラム | .pli .pl1 |
| PLI_INCLUDE | PL/I のインクルードメンバー | .inc |
| BMS_MAPSET | BMS のマップセット | .bms |
| IMS_DBD / IMS_PSB | IMS の DBD・PSB のソース | .dbd .psb |
| CICS_CSD | DFHCSDUP の DEFINE 文 | .csd |
| PROGRAM | 見つからないプログラム (言語は分からない) | 仮想 |
| CICS_TRANSACTION | CSD に定義のない取引コード | 仮想 |
| SYSTEM_PROGRAM / SYSTEM_COPYBOOK | Zeus が提供する資産 (提供資産マスタ) | 仮想 |
| DYNAMIC_TARGET | 変数で指定され、ソースから値の決まらない呼出し先 | 仮想 |

拡張子が .cbl・.pli のファイルはプログラムかメンバーかを中身で、拡張子がないか .txt・.src のファイルは種類を中身で判断する (`//` で始まる JCL、DFHMSD、DBD・PSBGEN、DEFINE、
PROCEDURE DIVISION など)。それ以外のファイル (文書・データなど) は分析しない。

### エッジの種類

| 種類 | 元 | 読み取るもの |
|---|---|---|
| COPY | COBOL | `COPY name [OF lib] [REPLACING ...]` |
| SQL_INCLUDE | COBOL・PL/I | `EXEC SQL INCLUDE name` |
| PLI_INCLUDE | PL/I | `%INCLUDE name`・`%INCLUDE DD(name)`・`%XINCLUDE` |
| CALL / PLI_FETCH | COBOL・PL/I | `CALL 'name'`・`CALL 変数`、PL/I の外部手続きの CALL と FETCH |
| CICS_LINK / CICS_XCTL / CICS_LOAD | COBOL・PL/I | `EXEC CICS LINK/XCTL/LOAD PROGRAM(...)` |
| CICS_TRANSID | COBOL・PL/I | `EXEC CICS START/RETURN TRANSID(...)` (CSD の取引定義でプログラムに解決する) |
| CICS_MAP | COBOL・PL/I | `EXEC CICS SEND/RECEIVE MAP(...) MAPSET(...)` |
| DLI_PSB | COBOL・PL/I | `EXEC DLI SCHD PSB(...)` |
| JCL_EXEC_PGM / JCL_EXEC_PROC / JCL_INCLUDE | JCL | `EXEC PGM=`・`EXEC proc`・`INCLUDE MEMBER=` |
| JCL_TSO_RUN | JCL | IKJEFT01 の SYSTSIN の `RUN PROGRAM(...) PLAN(...)` |
| JCL_IMS_PGM / JCL_IMS_PSB | JCL | DFSRRC00 の `PARM='region,program,psb'` |
| PSB_DBD | PSB | `PCB TYPE=DB,DBDNAME=...` |
| CSD_PROGRAM / CSD_TRANSACTION / CSD_MAPSET | CSD・JCL | `DEFINE PROGRAM/TRANSACTION/MAPSET` (DFHCSDUP の SYSIN に書かれたものも読む) |

コピーメンバーの中の呼出しは、そのメンバーのノードから出るエッジになる。

## 結果

| ファイル | 内容 | 列 |
|---|---|---|
| `nodes.csv` | ノードの一覧 | node_id, node_type, name, status (FILE・SYSTEM・MISSING・DYNAMIC), path |
| `edges.csv` | エッジの一覧 (元・先・種類が同じ参照は 1 本) | edge_id, edge_type, from_node_id, from_name, to_node_id, to_name, resolution, occurrences |
| `node_attributes.csv` | ノード属性一覧 (1 行 1 属性) | node_id, node_type, attribute, value |
| `edge_attributes.csv` | エッジ属性一覧 (1 行 1 属性) | edge_id, edge_type, attribute, value |
| `edge_locations.csv` | エッジ読み取り位置情報一覧 (参照箇所ごと) | edge_id, seq, node_id, path, line, column, statement |
| `missing_assets.csv` | 不足資産一覧 (見つからない資産と動的な呼出し先の参照箇所ごと) | asset_name, node_type, reason (NOT_FOUND・DYNAMIC), node_id, referenced_by_node_id, referenced_by_name, referenced_by_path, edge_id, edge_type, line, column, occurrences |

- 行と桁は 1 から数える。桁は参照名 (リテラルなら引用符) の位置。`statement` はその行の文字列 (前後の空白を除く)。
- ノード属性の主なもの:
  - すべてのファイル: file_name, size_bytes, line_count, referenced_by。
  - COBOL: program_id, entries, exec_cics・exec_sql・exec_dli の数、compile_status (OK・ERROR・NOT_CHECKED)、
    compile_message (最初のエラー)、unsupported (Zeus が実行できない文: コンパイラが `Unsupported` にした命令)。
  - PL/I: procedure, main, procedures, compile_status (OK・WARNING・ERROR・NOT_CHECKED), compile_message。
  - JCL: job_name・proc_name, steps, step_names, instream_procs, parse_status (Zeus の JCL 解析), parse_message。
  - BMS: mapset_name, maps, parse_status。DBD・PSB・CSD: 名前、アクセス方式、PCB の数、グループ、取引。
- エッジ属性: target_name (ソースに書かれた名前), occurrences, resolution, via (下記), identifier (変数名)、
  種類ごとの library・replacing・steps・parm・plan・region・commands・transid・procopt など。

## 名前の解決

- COPY・%INCLUDE・SQL INCLUDE: コンパイラと同じ順で探す (`OF lib` のライブラリー、`--lib` のディレクトリー、
  ソースのディレクトリー。拡張子はなし・.cpy・.cbl・.cob・.dcl、PL/I はなし・.pli・.inc・.cpy)。見つからなければ
  分析したファイルの中から同じ名前のメンバーを探し、BMS のマップセットがあればそのシンボリック・マップとして
  マップセットにつなぐ (via `BMS`)。次に提供資産 (SQLCA・SQLDA・DFHAID・DFHBMSCA・CMQ* など)、なければ不足資産。
  ソースの隣にあるのがプログラムで、同じ名前のコピーメンバーが別にあるときはコピーメンバーを選ぶ。
- プログラム (CALL・LINK・XCTL・EXEC PGM など): 分析したファイルのプログラム ID・入口名・ファイル名、次に提供資産
  (zjob のユーティリティ、LE の呼出し可能サービス、CBLTDLI・PLITDLI、MQI など)、なければ `PROGRAM` の不足資産。
  CSD の DEFINE PROGRAM がマップセットを指すとき (古い CSD) はマップセットにつなぐ。
- 取引コード: CSD (ファイルか JCL の DFHCSDUP) の取引定義のプログラムにつなぐ (via `CSD`)。
- プロシージャー・INCLUDE メンバー: `--lib` とジョブのディレクトリーの .proc・.prc・.jcl、次に分析したファイル。
  ジョブの中で定義されたプロシージャーは参照にしない。
- 変数で指定された呼出し先: プログラムと、それをコピーするソース・それがコピーするメンバー (COPY のまとまり) の中で
  変数に入る定数を集める (COBOL の VALUE と `MOVE 'X' TO 変数`、`MOVE 変数 TO 変数` の伝播、PL/I の INITIAL と
  代入)。値ごとにエッジを作り (via `VALUE`)、値が見つからなければ DYNAMIC_TARGET につなぐ。
- 起点からたどれる限り再帰的に分析し、新しく見つかったファイル (`--lib` にあるものを含む) もたどる。

## コンパイルの検査

COBOL と PL/I のプログラムを Zeus のコンパイラ (zcobc・zpli と同じ) で翻訳する。COPY・%INCLUDE は `--lib` と、
分析で見つかったメンバーのディレクトリーから探す。BMS のマップセットからは COBOL と PL/I のシンボリック・マップを
作って使う。コンパイラの誤りは compile_message、実行できない文は unsupported に出る。

## 制限

- ソースの文字列から読むので、実行時に決まる名前 (ファイルから読んだ値、計算した名前、OCCURS の表から選ぶ名前) は
  DYNAMIC になる。
- JCL の記号パラメーター (`&PGM`) は置き換えない (DYNAMIC になる)。
- YAML などで書かれた CICS の資源定義 (CICS TS の CICS resource builder など) は読まない。
- アセンブラー・C・Java のソースは分析しない (呼ばれていれば不足資産になる)。

## 確かめたこと (2026-10)

| アプリケーション | ノード (ファイル) | エッジ | 不足 | 動的 | コンパイルできないもの |
|---|---|---|---|---|---|
| CardDemo | 269 (203) | 671 | 31 | 2 | なし |
| GenApp | 96 (74) | 269 | 14 | 0 | なし |
| Bank of Z | 168 (141) | 270 | 10 | 5 | IBTRAN (REPOSITORY 段落) |

- CardDemo の不足資産は、CSD に定義があってソースのないプログラムとマップセット (COACT00C・COTSTP1S など)、
  IBM の製品のプログラム (DFHCSDUP・ASMA90・DSNHPC・DSNTIAC)。動的な 2 件は OCCURS の表 (FILLER の VALUE) から
  選ぶプログラム名。
- GenApp の不足は CICS・CPSM の製品のプログラムとプロシージャー、テストツールのプログラム (ITP*)。
- Bank of Z の不足は YAML で定義された取引 (OMEN など)、動的な 5 件は Java の JNI の関数ポインター。

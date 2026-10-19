# C1 網羅テストケースの生成と確認 (ztestgen / ztest)

`ztestgen` は COBOL プログラムのすべての分岐を両方向とも通すこと (C1 網羅) を目標に、入力データを自動で作り、
テストケース (JSON) として書き出します。`ztest` はそのケースを再実行し、結果が同じかを確かめます。
ケースと一緒に、入力・出力データのレイアウト定義 (zeus-layout 形式のコピーブック) も書き出すので、
16 進のデータをレイアウトと照らし合わせて確認できます。

方式と検証の結果は [0012](../proposal/0012-c1-test-generation.md)、レイアウトの形式は
[0013](../proposal/0013-fixed-length-layouts.md) にあります。

## 1. 準備

```bash
cargo build --release -p zeus-testgen
```

`target/release` に `ztestgen` と `ztest` ができます。

- **Z3 (任意)**: 見つかれば、組込みのソルバで解けない条件に使います。環境変数 `ZTESTGEN_Z3` にライブラリの
  パスを指定するか、`libz3.dll` / `libz3.so` を検索パスに置きます (Python の `z3-solver` パッケージに含まれる
  ライブラリも探します)。使われたかどうかは `report.txt` の `solver:` の行に出ます。
- **コピーブック**: `-I DIR` で COPY の検索先を指定します (複数可)。
- **BMS**: CICS の画面を使うプログラムは、シンボリック・マップのコピーブックが要ります。BMS のソースがあれば
  `zbms` で作るか、`--bms DIR` で BMS のディレクトリを渡すとレイアウトの作成にも使われます。
- **呼び出し先**: CALL する別プログラムがあれば `zcobc compile` で `.zo` にし、`-L DIR` で渡します。
  見つからないプログラムはスタブ (出力がソルバの入力になる) で実行します。

## 2. ケースの生成 (ztestgen)

```text
ztestgen [-I DIR]... [-L DIR]... [--program NAME] [--runs N] [--time SECONDS]
         [--limit INSTRUCTIONS] [--no-smt] [--threads N] [--stub PROGRAM]...
         [--seeds DIR]... [--bms DIR]... [--no-layouts] [--out DIR] SOURCE.cbl
```

| オプション | 意味 | 既定 |
|---|---|---|
| `-I DIR` | COPY の検索先 | なし |
| `-L DIR` | 呼び出し先プログラム (`.zo`) の検索先 | なし |
| `--program NAME` | ソースに複数のプログラムがあるとき、対象のプログラム | 最初のプログラム |
| `--time SECONDS` | 探索の時間の上限 | 120 |
| `--runs N` | 実行回数の上限 | 2000 |
| `--limit N` | 1 回の実行の命令数の上限 (無限ループの打ち切り) | 2000000 |
| `--threads N` | 条件を解くスレッド数 | CPU 数 |
| `--no-smt` | Z3 を使わない | 使う |
| `--stub PROGRAM` | あっても常にスタブにするプログラム (複数可) | なし |
| `--seeds DIR` | 初期入力にするケースファイルのディレクトリ (後述) | なし |
| `--bms DIR` | BMS マップセットのディレクトリ (レイアウトのシンボリック・マップ) | なし |
| `--no-layouts` | レイアウト定義を書き出さない | 書き出す |
| `--out DIR` | 出力先 | `tests` |

例:

```bash
ztestgen -I app/cpy --time 60 --out out/CBACT04C app/cbl/CBACT04C.cbl
```

```bash
ztestgen -I app/cpy -I app/cpy-bms --time 60 --out out/COSGN00C app/cbl/COSGN00C.cbl
```

実行するとレポートが表示され、出力先に書かれます。終了コードは、正常なら 0、引数の誤りは 2、ソースが
読めないなどのエラーは 8 です。

### 何が入力になるか

| 入力 | ケースファイルでの場所 |
|---|---|
| PROCEDURE DIVISION USING の引数 (CICS は EIB と COMMAREA) | `params` |
| ACCEPT で読む行 (SYSIN) | `accept` |
| 入力ファイルのレコード (順次・索引) | `files` |
| ファイル入出力の失敗 (OPEN・READ などの n 回目を失敗させる) | `faults` |
| 見つからないプログラム (スタブ) が引数に書く値 | `stubs` |
| EXEC CICS / SQL / DLI が返す値 (RESP、INTO の項目、SQLCODE、ホスト変数など) | `exec` |
| 現在時刻 (CURRENT-DATE、ASKTIME など) | `clock` |

## 3. 出力ディレクトリ

```text
out/FILEPGM/
  report.txt                        網羅率と、通せなかった分岐の一覧
  case-001.json ... case-005.json   テストケース
  layouts/
    FILEPGM.FILE.TRANIN.cpy         入力ファイル TRANIN のレイアウト
    FILEPGM.FILE.RPTOUT.RPT-REC.cpy 出力ファイル RPTOUT のレイアウト (FD のレコード)
    FILEPGM.FILE.RPTOUT.WS-LINE.cpy 同じファイルの WRITE FROM の作業域
    layouts.csv                     レイアウトの一覧 (zeus-layout の索引)
    data.csv                        ケースのどのデータがどのレイアウトか
```

## 4. report.txt の読み方

```text
program FILEPGM: 32 runs, 5 cases
C1: 16 of 16 branch directions (100.0%), 100.0% of the feasible (0 infeasible)
solver: built-in search, then Z3

layouts: 3 copybooks in out/FILEPGM/layouts (layouts.csv; data.csv: the data of the cases)
  files[TRAN-FILE] (input, TRAN-REC): FILEPGM.FILE.TRANIN.cpy
  expect.files[RPT-FILE] (output, RPT-REC): FILEPGM.FILE.RPTOUT.RPT-REC.cpy FILEPGM.FILE.RPTOUT.WS-LINE.cpy
```

- `C1: a of b branch directions`: 分岐 (IF、EVALUATE の WHEN、PERFORM UNTIL、AT END など) の向きの総数 b のうち、
  ケースが通した数 a。`of the feasible` は、起こり得ないと分かった向き (`infeasible`) を除いた割合です。
- `stubs`: スタブで実行した呼び出し先。そのプログラムの中は試験されていません。
- `not covered`: 通せなかった向きと理由。

| 理由 | 意味 | 対応の例 |
|---|---|---|
| `not reached` | その分岐まで実行が届かなかった | 手前の分岐の条件を見る。シードで届く入力を与える |
| `condition does not depend on the inputs followed` | 条件が入力から追えない値 (計算の結果、追えない命令) で決まる | ループの回数や内部の状態に依存していないか確認 |
| `no inputs found` | 条件は入力に依存するが、時間内に解けなかった | `--time` を延ばす、シードを与える |
| `infeasible (no inputs on the paths tried)` | 試した経路では満たせなかった (証明ではない) | 本当に起こり得ないかをソースで確認 |
| `impossible (static analysis: ...)` | 静的な解析で起こり得ないと証明した (分母から除く) | 不要な分岐の可能性 |

`line` はソースの行、`instruction` は内部の命令番号です。`jump` / `next` は条件分岐の命令が飛んだ側か、
次へ進んだ側かで、IF の真偽とは一致しないことがあります (行のソースを見て判断してください)。

## 5. テストケース (case-NNN.json) の読み方

FILEPGM のケースの例 (`covers` は一部省略):

```json
{
  "program": "FILEPGM",
  "params": [],
  "accept": [],
  "files": [
    { "name": "TRAN-FILE",
      "records": [
        { "hex": "C44040404040400000000C404040404040404040",
          "fields": { "FILLER": "         ", "TR-ACCT": "      ",
                      "TR-AMOUNT": "0.00", "TR-TYPE": "D" } } ] } ],
  "expect": {
    "return_code": 0,
    "display": [],
    "params": [],
    "error": null,
    "files": [ { "name": "RPT-FILE", "records": [] } ] },
  "covers": [ { "program": "FILEPGM", "line": 34, "pc": 6, "jumped": true }, ... ]
}
```

| キー | 内容 |
|---|---|
| `params[k]` | k 番目 (0 から) の USING の引数。`hex` は記憶域の 16 進 (EBCDIC)、`fields` は基本項目ごとの値 (読むための表示) |
| `accept` | ACCEPT で読む行 |
| `files[].records[]` | 入力ファイルのレコード。`hex` と `fields` は引数と同じ |
| `faults` | 失敗させた入出力 (`file`、`statement`、`occurrence` = 何回目か、`status` = VSAM の状態 `F910` など。なければ 35 / 30) |
| `stubs` | スタブにした呼び出し先が引数に書く値 (`program`、`call` = 何回目の呼出しか、`argument` = 何番目の引数か、`hex`) |
| `exec` | EXEC 文が返す値。キーは `プログラム.行.回目.オプション[.項目]` (`CICSPGM.43.1.RESP` は 43 行目の EXEC の 1 回目の RESP)。値は `{"num": 13}` (数値) か `{"hex": "..."}` (先頭のバイト) |
| `clock` | 実行時の現在時刻 (UTC、`YYYY-MM-DD HH:MM:SS`)。ないときは 2025-01-01 09:00:00 |
| `expect.return_code` | RETURN-CODE |
| `expect.display` | DISPLAY した行 |
| `expect.params` | 実行後の引数の記憶域 (16 進) |
| `expect.error` | 異常終了の内容 (ABEND、範囲外の参照など)。正常終了なら null |
| `expect.files` | 出力ファイルに書いたレコード (`records` は 16 進、`fields` はその項目の値) |
| `expect.exec` | 実行した EXEC 文と、送った内容 (RIDFLD、FROM、COMMAREA、SQL の入力ホスト変数など) |
| `expect.stubbed` | スタブで実行した呼び出し先 |
| `covers` | このケースが通した分岐の向き (`line` = 行、`jumped` = 飛んだ側か) |

`expect` は **ケースを作ったときにプログラムがした結果** です。仕様として正しいかどうかは保証しません。
人が確認する目的は、この期待値が仕様どおりかを判断することです (誤っていれば、それはプログラムの不具合の
候補です)。プログラムを変更した後に `ztest` で差分が出れば、振る舞いが変わったことを示します。

CICS の例 (CICSPGM。43 行目の READ が NOTFND (13) を返すケース):

```json
"exec": {
  "CICSPGM.25.1.INTO.IN-CMD": { "hex": "E2C8D6E6" },
  "CICSPGM.43.1.RESP": { "num": 13 }
},
"expect": { "display": [ "NOT FOUND" ], ... }
```

25 行目の `RECEIVE INTO(WS-INPUT)` が IN-CMD に `SHOW` (EBCDIC の E2 C8 D6 E6) を返し、43 行目の
`READ FILE('ACCTDAT')` が NOTFND を返したので `NOT FOUND` を表示した、と読みます。

## 6. レイアウト定義 (layouts/)

### コピーブック

`ztestgen` は zeus-layout (`zlayout` と同じ処理) で、プログラムが受け渡すデータのレイアウトを 1 レイアウト
1 ファイルのコピーブックとして書き出します。COPY は展開済み、REDEFINES は形ごとに別のファイルに分かれます。

```cobol
      *================================================================
      * PROGRAM   : FILEPGM
      * KIND      : FILE (INPUT)  DD: TRANIN  FILE: TRAN-FILE
      *             ORGANIZATION: SEQUENTIAL
      * LAYOUT    : 1 OF 1
      * LENGTH    : 20 BYTES
      * SOURCE    : FILEPGM.cbl:12
      *================================================================
       01  TRAN-REC.                                                    00001
           05  TR-TYPE                 PIC X.                           00001
               88  TR-CREDIT           VALUE 'C'.
               88  TR-DEBIT            VALUE 'D'.
           05  TR-ACCT                 PIC X(6).                        00002
           05  TR-AMOUNT               PIC S9(5)V99 COMP-3.             00008
           05  FILLER                  PIC X(9).                        00012
```

- 73〜80 桁目が各項目の **開始位置** (1 から数えたバイト位置) です。
- ヘッダーの `KIND` は種類と資源 (DD 名、CICS のファイル名、引数の番号など)、`SELECTED` はそのレイアウトが
  選ばれる条件 (レコード種別の値。複数のレイアウトがあるとき)、`SOURCE` は定義の場所です。
- ファイル名は `プログラム.種類.資源[.レイアウト名].cpy`。種類は FILE、CARD、PARM、CALL、COMMAREA、BMS、
  SQLRESULT、TABLE、MQ と、ケースのデータでほかに当てはまらないグループ項目の RECORD (端末からの入力の
  作業域、小さい COMMAREA など)。
- 各コピーブックはコンパイルし直して、位置と長さが元のプログラムと一致するかを確かめています
  (`layouts.csv` の `check` 列が `OK`)。

### layouts.csv

レイアウトの一覧です (zeus-layout の索引と同じ形式)。

```text
program,kind,resource,layout,index,count,selector,choices,length,file,source,check
FILEPGM,FILE,TRANIN,,1,1,,,20 BYTES,FILEPGM.FILE.TRANIN.cpy,FILEPGM.cbl:12,OK
FILEPGM,FILE,RPTOUT,RPT-REC,1,2,,,20 BYTES,FILEPGM.FILE.RPTOUT.RPT-REC.cpy,FILEPGM.cbl:20,OK
FILEPGM,FILE,RPTOUT,WS-LINE,2,2,,,20 BYTES,FILEPGM.FILE.RPTOUT.WS-LINE.cpy,FILEPGM.cbl:27,OK
```

### data.csv: ケースのデータとレイアウトの対応

```text
data,direction,record,layouts
files[TRAN-FILE],input,TRAN-REC,FILEPGM.FILE.TRANIN.cpy
expect.files[RPT-FILE],output,RPT-REC,FILEPGM.FILE.RPTOUT.RPT-REC.cpy FILEPGM.FILE.RPTOUT.WS-LINE.cpy
```

| 列 | 内容 |
|---|---|
| `data` | ケースファイルの場所 (下の表) |
| `direction` | `input` (ケースがプログラムに与える)、`output` (実行後に確かめる)、`input/output` (両方) |
| `record` | そのデータを持つ 01 レベルの項目 |
| `layouts` | そのレコードのレイアウト (空白区切り。REDEFINES の形や、READ INTO / WRITE FROM の作業域ごとに複数) |

| `data` の書き方 | 対応するケースのデータ |
|---|---|
| `params[k]` | `params[k]` と `expect.params[k]` |
| `files[NAME]` | 入力ファイル NAME のレコード (`files`) |
| `expect.files[NAME]` | 出力ファイル NAME のレコード (`expect.files`) |
| `exec[PGM.L.*.OPTION]` | L 行目の EXEC 文が返す値 (`exec` のキー `PGM.L.n.OPTION...`。`*` は何回目でも) |
| `expect.exec[L CICS CMD OPTION]` | L 行目の EXEC CICS が送るオプションの内容 (`expect.exec` の行) |
| `expect.exec[L SQL CMD Hk]` | L 行目の SQL 文の k 番目の入力ホスト変数 |
| `stubs[PGM.k]` | PGM を呼ぶ k 番目の引数 (`stubs` の `program`・`argument`) |
| `accept` | ACCEPT で読む行 (`accept`) |

数値だけの項目 (RESP、SQLCODE など) は、ケースに値がそのまま書かれるのでレイアウトはありません。

## 7. 確認の手順

1. `report.txt` で網羅率と `not covered` を確かめる。`impossible` / `infeasible` は不要な分岐の候補、ほかは
   テストが足りない箇所です。必要ならシード (後述) を足して再生成します。
2. `layouts/data.csv` で、入力と出力のそれぞれがどのレイアウトかを確かめる。
3. ケースごとに、入力 (`params`・`files`・`exec`・`accept`) の `fields` を読み、どの分岐を通るケースかを
   `covers` の行で確かめる。`fields` のないデータ (`exec` の `hex`、`expect.params` など) はレイアウトの
   開始位置と長さで 16 進を切り出します。
4. 期待値 (`expect.display`・`expect.files` の `fields`・`expect.exec`・`expect.return_code`・`expect.error`) が
   仕様どおりかを判断する。
5. `ztest` で再現することを確かめ、ケースをリポジトリに保存する (回帰テスト)。

### 16 進をレイアウトで読む

上の TRAN-FILE のレコード `C44040404040400000000C404040404040404040` (20 バイト) を TRAN-REC で読むと:

| 項目 | 位置 | 長さ | バイト | 値 |
|---|---|---|---|---|
| TR-TYPE | 1 | 1 | `C4` | `D` (TR-DEBIT) |
| TR-ACCT | 2 | 6 | `404040404040` | 空白 |
| TR-AMOUNT | 8 | 4 (COMP-3) | `0000000C` | +0.00 |
| FILLER | 12 | 9 | `40...` | 空白 |

位置 p の項目は、16 進の文字列の (p-1)×2 文字目から長さ×2 文字です。英数字は EBCDIC (`40` が空白、`F0`〜`F9`
が数字、`C1`〜`C9` が A〜I)、COMP-3 は 1 バイトに 2 桁で最後の半バイトが符号 (`C` 正、`D` 負、`F` 符号なし)、
COMP は 2 進です。

## 8. 再実行 (ztest)

```text
ztest [-I DIR]... [-L DIR]... [--coverage DIR] SOURCE.cbl CASES-DIR
```

```bash
ztest -I app/cpy app/cbl/CBACT04C.cbl out/CBACT04C
```

```text
case-001.json: ok
case-002.json: FAILED
  display ... instead of ...
ztest: 19 cases, 1 failed
```

ケースごとに入力を与えて実行し、`expect` と比べます (`expect.files` の `fields` は比べません。`records` を比べます)。
1 件でも違えば終了コード 1 です。`--coverage DIR` で網羅の記録を取り、`zcov report DIR` で行・分岐の網羅率を
表示できます (`zcov --help`)。

## 9. シード: 入力を手で与える

`--seeds DIR` に置いたケースファイルは、最初に実行する入力になります。前回のケースをそのまま渡すと、
そこから探索を続けます。手で書くときは `fields` に項目名と値だけを書けば、ほかは空白・0 になります
(`hex` と `expect` は省略できます)。

```json
{
  "params": [ { "fields": { "PARM-LENGTH": "9", "PARM": "1,100,1,5" } } ],
  "files": [ { "name": "IN-FILE", "records": [ { "fields": { "TR-TYPE": "C", "TR-AMOUNT": "150.00" } } ] } ],
  "exec": { "PGM.120.1.RESP": { "num": 13 } },
  "stubs": [ { "program": "SUBPGM", "call": 1, "argument": 1, "text": "OK" } ],
  "clock": "2025-12-31 23:59:59"
}
```

正しい画面の入力や、業務的に意味のあるレコードを 1 件与えると、自動では届かない分岐に届くことがあります。

## 10. 注意

- 期待値はプログラムの実際の振る舞いを記録したもので、正しさの基準 (オラクル) ではありません。
- EXEC CICS / SQL / DLI は実際のトランザクションやデータベースを動かさないスタブで実行します。応答は
  ソルバが選んだ値なので、実際には起こらない組合せ (存在しないレコードを READ して NORMAL など) も現れます。
- スタブにした呼び出し先の中は試験されません (`stubs`、`expect.stubbed`)。
- 浮動小数点の計算、ループで計算した値、長いメッセージの組立てなどは追えないことがあります
  ([0012](../proposal/0012-c1-test-generation.md) の「向いているもの・向いていないもの」)。

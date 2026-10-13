# 0010: COBOL と PL/I のカバレッジ

テストやジョブで COBOL・PL/I のプログラムのどこが実行されたかを報告する。インタプリタで動くときに数える
(ネイティブの実行ファイルの機械語は対象外。カバレッジを取るときは機械語を使わずインタプリタで動く)。

## 使い方

```text
ZEUS_COVERAGE=DIR <zcobc run / zpli / zjob / zcics / ネイティブの実行ファイル ...>
zcobc run --coverage DIR prog.zo
zpli --coverage DIR prog.pli          (zpli run --coverage DIR prog.zo)

zcov report [--detail] [--lcov FILE] [--html DIR] DIR...
```

- ZEUS_COVERAGE (または `--coverage`) で記録を始める。実行単位 (プログラムの実行、ジョブのステップ、CICS のタスク) が
  終わるたびに、プロセスの合計を `DIR/zeus-<pid>.cov.json` に書く (終わらないサーバーの zcics・zconn も残る)。
- `zcov report` は 1 つ以上のディレクトリーのファイルを合わせて報告する。
  - 標準出力: プログラムごとの行・分岐・段落 (PL/I は手続き) の網羅率と合計。`--detail` で実行されなかった行、
    片方にしか進まなかった分岐、入らなかった段落を並べる。
  - `--lcov FILE`: LCOV の形式 (`genhtml` や IDE の拡張機能で表示できる)。
  - `--html DIR`: 一覧とソースファイルごとのページ (実行回数を行の横に、実行された行は緑、されなかった行は赤、
    分岐の片方だけの行は黄色)。

## 数えるもの

| 項目 | COBOL | PL/I |
|---|---|---|
| 行 | 文のある行。文の最初の命令が実行された回数 (1 行に複数の文があれば最大) | 文 (DECLARE と手続きの定義を除く) のある行。文が実行された回数 |
| 分岐 | 条件付きの飛越し (IF、EVALUATE の WHEN、PERFORM UNTIL・VARYING の判定、AT END・INVALID KEY・ON SIZE ERROR など) ごとに、飛んだ回数と次へ進んだ回数 | IF の THEN と ELSE (同じ行の IF はまとめて数える) |
| 段落・手続き | 段落 (名前のない節は節) に入った回数 (先頭の命令の実行回数) | 手続きに入った回数 (最初の実行文の実行回数) |

- COBOL: コンパイラが文ごとに位置 (ファイル・行) を残す (ZIR の `stmt_pos`、命令ごとの `instr_stmt`、段落の `pos`。
  構文解析は文の前に位置の印 `Stmt::Line` を置く)。COPY メンバーの中の文はそのメンバーのファイルの行になる。
  古いコンパイラの `.zo` は位置を持たないので数えられない (コンパイルし直す)。
- PL/I: 文のノードの位置と、モジュールの `stmt_lines`・`source_paths` を使う。
- ソースのパスはコンパイルしたときの絶対パス。HTML はそのファイルを読む。

## 確かめたこと (2026-10)

- `crates/zeus-cobol/tests/coverage.rs`: 2 回の実行の合計、行・分岐・段落、`--detail`、LCOV。
- `crates/zeus-pli/tests/coverage.rs`: 行・IF の分岐・手続き。
- CCVS85 をカバレッジを取りながら流しても結果は同じ (434 本)。
- CardDemo のバッチ (`tools/carddemo.sh all`) とオンライン (`tools/carddemo-online.sh test`) で 29 本のプログラムの
  報告を作れる (行 58%、分岐 44%、段落 87%)。

## 制約

- ネイティブの実行ファイルの機械語の部分は数えない (ZEUS_COVERAGE があると機械語を使わない)。
- PL/I の分岐は IF だけ (SELECT の WHEN、DO WHILE の判定は数えない)。同じ行の複数の IF は 1 つにまとめる。
- 条件の中の各条件 (AND・OR の項) ごとの網羅 (MC/DC) は数えない。

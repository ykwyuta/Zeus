# 0009: 環境ごとのネイティブの実行ファイル

COBOL と PL/I のプログラムを、Zeus を入れていない環境でもそのまま動く実行ファイルにする。
2 段階で進める。

1. **単体の実行ファイル** (済): コンパイル済みのモジュールと Zeus の実行時を 1 つの実行ファイルにまとめる。
   実行は今のインタプリタで行う。
2. **機械語へのコンパイル** (これから): Cranelift でモジュールを機械語にし、実行時と同じ実行ファイルに入れる。
   1 と同じ `--native` の中身を差し替える。

## 1. 単体の実行ファイル

### 使い方

```text
zcobc compile --native [--target <triple>] [--with <module.zo>]... [options] -o PROG source.cbl
zpli  compile --native [--target <triple>] [--with <module.zo>]... [options] -o PROG source.pli

./PROG [--dd NAME=PATH]... [--parm TEXT | TEXT] [--ccsid N]
```

- `-o` を省くと、ソースの名前から拡張子を除いたものになる。
- `--with` は別にコンパイルしたサブプログラム (COBOL の `zcobc compile`、PL/I の `zpli compile` の `.zo`) を入れる。
  主プログラムが COBOL でも PL/I でもよく、言語をまたいで CALL できる。実行ファイルのディレクトリーにある `.zo` も探す。
- 実行時: `--dd` で DD を割り当てる (`zcobc run` / `zpli run` と同じく、ファイルはデータセットの形。
  `DD_<名前>` 環境変数と ZEUS_DD_CONFIG も使える)。位置引数か `--parm` が EXEC の PARM。
  DISPLAY と SYSPRINT は標準出力、ACCEPT と SYSIN は標準入力。EXEC SQL は ZEUS_DB2、MQI は ZEUS_MQ で動く。
- 終了コードは戻りコード。異常終了は 12 (PL/I は 16)、実行時エラーは 16。

### 仕組み

- 実行時 `zeus-rt` (クレート `zeus-native`) は、COBOL と PL/I の実行時、Db2 (DRDA・PostgreSQL)、MQ を含む普通の
  Rust のプログラム。`--native` はこれを複製し、末尾にモジュールを付ける (`zeus_cobol::image`)。
  付けたデータは、メンバーごとの名前とモジュール、メンバー数、全体の長さ、識別子 `ZEUSIMG1`。
- 起動すると自分の実行ファイルの末尾からモジュールを読み、一時ディレクトリーに書いて実行単位のロードライブラリーにし、
  最初のメンバーを主プログラムとして実行する (終わると消す)。
- 実行ファイルは対象の OS の普通の形式 (Linux は ELF、macOS は Mach-O、Windows は PE) で、末尾のデータは
  ローダーに無視される。

### 対象の環境 (`--target`)

`--target` は Rust のターゲットの三つ組。その環境の `zeus-rt` を探す。

- ZEUS_RUNTIME (ファイル)、`$ZEUS_RUNTIME_DIR/<triple>/zeus-rt`、コンパイラと同じ cargo の出力の
  `target/<triple>/release/zeus-rt` の順。`--target` がなければコンパイラの隣の `zeus-rt`。
- 実行時は `cargo build --release -p zeus-native --target <triple>` で作る。

確かめたもの (2026-10):

| ターゲット | 結果 |
|---|---|
| x86_64-unknown-linux-gnu | 作成・実行 (テスト `crates/zeus-native/tests/native.rs`) |
| x86_64-unknown-linux-musl | 静的リンクの実行ファイルを作成・実行 |
| aarch64-unknown-linux-musl | x86-64 の上でクロスビルド (リンカー rust-lld。libloading が求める `-ldl` には空の libdl.a を渡す) し、qemu-aarch64 で実行 |
| x86_64-pc-windows-gnu | `cargo check` が通る。リンクと実行は未確認 (MinGW か Windows の上でビルドする) |
| macOS | 未確認 |

### 制約

- プログラムはインタプリタで動くので、速さは `zcobc run` / `zpli run` と同じ。
- 静的リンク (musl) では MQ のクライアントライブラリーを動的に読めないので MQI は使えない。
- CICS・IMS の領域 (`zcics`) とジョブ (`zjob`) の中で動くものは対象外 (バッチの単体のプログラム)。

## 2. 機械語へのコンパイル (計画)

- Cranelift (`cranelift-codegen`・`cranelift-object`) で、COBOL の ZIR と PL/I の IR の手続きを機械語の関数にする。
  制御の流れ (IF・PERFORM・GO TO・ループ) は機械語の分岐にし、各命令は最初は実行時の関数の呼出しにする
  (サブルーチンスレッド化)。その後、2 進・パック 10 進の算術、MOVE、比較など頻度の高い命令から機械語で直接書く。
- 出力はオブジェクトファイルで、`zeus-rt` の静的ライブラリーとリンクして実行ファイルにする (対象の環境のリンカーが要る)。
  コンパイルできない命令を含むプログラムは、第 1 段階のインタプリタの形に戻す。
- 検証: CCVS85 と CardDemo・GenApp・Bank of Z のバッチを、インタプリタと機械語の両方で流して結果を比べる。

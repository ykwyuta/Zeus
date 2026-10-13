# 0009: 環境ごとのネイティブの実行ファイル

COBOL と PL/I のプログラムを、Zeus を入れていない環境でもそのまま動く実行ファイルにする。
2 段階で進める。

1. **単体の実行ファイル** (済): コンパイル済みのモジュールと Zeus の実行時を 1 つの実行ファイルにまとめる。
2. **機械語へのコンパイル** (COBOL は第 1 段を済): Cranelift で COBOL のプログラムを対象の環境の機械語にし、
   同じ実行ファイルに入れる。PL/I はインタプリタのまま。

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

- PL/I のプログラムはインタプリタで動く (速さは `zpli run` と同じ)。COBOL は 2 章。
- 静的リンク (musl) では MQ のクライアントライブラリーを動的に読めないので MQI は使えない。
- CICS・IMS の領域 (`zcics`) とジョブ (`zjob`) の中で動くものは対象外 (バッチの単体のプログラム)。

## 2. 機械語へのコンパイル

`zcobc compile --native` (と、`--with` の COBOL モジュール) は、COBOL のプログラムを対象の環境の機械語にする
(`crates/zeus-cobol/src/native.rs`)。`--no-machine-code` で付けない。実行時に ZEUS_INTERPRET=1 でインタプリタで動かす。

### 仕組み

- プログラムごとに 1 つの関数 `fn(ctx, pc) -> u32` を Cranelift (0.127) で作る。ZIR の命令ごとに基本ブロックを作り、
  GO TO は分岐、IF・PERFORM・段落の終わり・CALL の後は、次の命令か計算した番地のブロックへ (番地の表の分岐) 進む。
- 機械語にした命令: WORKING-STORAGE の項目について
  - 2 進 (COMP の 2・4・8 バイト)、パック 10 進 (9 バイトまで)、ゾーン 10 進 (DISPLAY、18 桁まで) の整数の COMPUTE / ADD / SUBTRACT / MULTIPLY、数値 MOVE、SET、
    PERFORM VARYING の更新 (加減乗算は 64 ビットの桁あふれ検査付き)
  - DISPLAY の符号なし・先頭/末尾の符号・先頭/末尾の分離符号。数値 MOVE はこれらと COMP・COMP-3 の間の変換も行う
  - 条件の数値比較 (AND・OR・NOT を含む)
  - 整数の IS POSITIVE / NEGATIVE / ZERO 条件
  - 英数字・英字・集団項目と文字定数の比較 (256 バイトまで)。短い側を空白で補完し、
    PROGRAM COLLATING SEQUENCE の重みで比較する (同順位の文字も扱う)
  - 英数字の MOVE (定数から・項目から。空白の詰め物、JUSTIFIED RIGHT の右寄せと左側の切捨て)
  - 英字項目への MOVE、SPACES / ZEROES / QUOTES / HIGH-VALUES / LOW-VALUES / ALL 定数からの MOVE。
    HIGH/LOW-VALUES はプログラムの照合順序に合わせた文字を使う
  - 数値テーブルの参照・更新: 整数の変数や加減乗算式を添字に使える (多次元も対応)。
    宣言した OCCURS 範囲を超える添字は、読み書きの前に `step` に戻す。
    1 未満の添字はインタプリタと同じく 1 として扱う
  - 文字 MOVE・比較の添字と部分参照 (`ITEM(I)(START:LEN)`、`ITEM(START:)` など、項目長 256 バイトまで)。
    整数の変数・加減乗算式を添字、開始位置、長さに使える。JUSTIFIED RIGHT と ALL 定数も対応。
    部分参照が項目の範囲を超える場合は読み書き前に `step` に戻す。
    数値項目の部分参照は英数字として扱う。重なりのある MOVE は `step` に任せる
- それ以外の命令は、実行時の `step` (インタプリタのその命令の処理) を呼ぶ。値が絵の桁数に収まらない (サイズエラー)、
  64 ビットを超える、などのときも、何も書き込まずに `step` に任せるので、結果はインタプリタと同じになる。
- 機械語は再配置のない位置独立のコード: 実行時の関数は文脈 (`step` の番地、記憶域の番地、プログラムの静的領域の位置) の
  表から呼ぶ。実行時 (`crates/zeus-native/src/machine_code.rs`、Zeus で唯一の機械語を扱う unsafe のコード) が
  実行可能なメモリーに写し、命令キャッシュを同期して呼ぶ。外部のリンカーは要らないので、`--target` でほかの環境の
  機械語も作れる。
- 実行ファイルのメンバー `NAME.zx`: 対象 (命令セットと呼出し規約: `x86_64-sysv`、`aarch64-apple` など)、プログラムごとの
  名前・命令数・コード。実行時は自分の環境のものだけを使い、違えば止まる。

### 確かめたこと (2026-10)

- CCVS85 (`zeus-ccvs run --toolchain zeus-native`): x86-64 と aarch64 (qemu) で、434 本すべての結果 (合否と件数) が
  インタプリタと同じ (必須モジュール 8/8、9572/9719 件)。
- `crates/zeus-native/tests/native.rs` の `machine_code_matches_the_interpreter`: 負の数、符号なし、パック 10 進の符号、
  サイズエラーの切捨て、64 ビットを超える値、JUSTIFIED の MOVE で、機械語とインタプリタの出力が同じ。
- `display_integers_and_justified_moves_match_the_interpreter`: Windows x86-64 で DISPLAY の各符号形式、18 桁の整数、
  COMP・COMP-3 との MOVE、ループ、サイズエラー、右寄せ MOVE の出力・終了コードがインタプリタと一致。
  実行時の単体テストでは、数値演算・比較・MOVE が機械語で動くことも確認 (サイズエラーフラグの初期化は実行時に任せる)。
- `text_comparisons_constants_and_signs_match_the_interpreter`: 標準 EBCDIC・逆順・同順位を含む照合順序で、
  空白補完・各比較演算・定数 MOVE・符号条件の出力と終了コードが一致。
  直接実行の単体テストと x86-64 / AArch64 のコード生成テストでも独自の照合順序を確認。
- `tables_and_fixed_reference_modification_match_the_interpreter`: 数値テーブルのループ・多次元・添字式、
  1 未満の添字、宣言範囲外から隣の項目への参照、定数添字・部分参照・重なる MOVE がインタプリタと一致。
  直接実行の単体テストでも添字付き演算と固定部分参照の機械語化を確認。
- `dynamic_text_references_match_the_interpreter`: 文字テーブルの変数添字、可変開始位置・長さ、長さ省略、
  空の部分参照・負の長さ、右寄せ、重なる MOVE、範囲外添字が標準・逆順照合でインタプリタと一致。
  直接実行の単体テストと 4 プラットフォームのコード生成テストでも可変参照を確認。
- 速さ: 2 進とパック 10 進の 2 重ループ (200 万回) が x86-64 で 0.88 秒 (インタプリタ) から 0.15 秒。
  起動は約 30 ミリ秒。

### これから

- 機械語にする命令を増やす: 小数点付きの値、数値項目を英数字として比較する場合、
  OCCURS DEPENDING ON の可変長集団項目、INITIALIZE・INSPECT など。
- PL/I の IR の機械語。
- `zjob` と `zcics` の中のプログラムにも機械語を使う。

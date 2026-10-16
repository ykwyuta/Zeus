# RESERVE・物理ブロック・仮想テープ・再帰 CALL

2026-10-16。0015 の I04/I11/I12/I13/I14/I19/I20/I25 の追加対応。

## Zeus での媒体の実体

ホスト OS 上のファイルを使用する。「物理ブロック」は z/OS の記録単位を表すバイト列であり、ホストディスクのセクターではない。
仮想テープは **非圧縮 AWSTAPE** イメージ。6 バイトのヘッダーでブロック長、直前のチャンク長、開始／終了／テープマークを記録する。
実テープ装置・ドライブ・チャネルを直接操作するものではない。

通常は `.aws` 拡張子でテープと認識する。拡張子に依存させない場合は、JCL `UNIT=TAPE/3420/3480/3490/3590`、
または VM の `set_dd_io` で `IoAttributes.tape=true` を指定する。

## 確認した仕様

- [IBM Enterprise COBOL V4.2 Language Reference](https://publibfp.dhe.ibm.com/epubs/pdf/igy3lr50.pdf): 冊子 pp.107–108 の RECURSIVE、p.141 の RESERVE、p.157 の APPLY WRITE-ONLY、pp.190–192 の記録形式、pp.407–409 の OPEN。
- [IBM: recursive program](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=program-identifying-as-recursive): 共有 WORKING-STORAGE と呼び出しごとの LOCAL-STORAGE。
- [IBM z/OS: segment descriptor word](https://www.ibm.com/docs/en/zos/3.1.0?topic=method-segment-descriptor-word): SDW の長さと 00/01/10/11 の完全／先頭／末尾／中間区分。
- [IBM z/OS: standard data set label 2](https://www.ibm.com/docs/en/zos/3.1.0?topic=format-standard-data-set-label-2-hdr2eov2eof2): F/V/U、ブロック長、レコード長、ブロック属性。
- [IBM z/OS: label definitions](https://www.ibm.com/docs/en/zos/3.2.0?topic=labels-label-definitions-organization): 80 文字の VOL1/HDR/EOF ラベル。
- [IBM: QSAM buffering performance](https://www.ibm.com/docs/en/SS6SG3_6.5/pdf/perfguide.pdf): BUFNO/RESERVE と AWO の背景。6.5 の追加言語仕様を V4.2 に取り込むものではない。
- AWSTAPE のホストコンテナーのレイアウトは [Hercules の公開ヘッダー](https://raw.githubusercontent.com/SDL-Hercules-390/hyperion/master/hetlib.h) のヘッダー／フラグ定義でも照合した。これは IBM COBOL の言語仕様とは別の媒体表現。

## 実装した動作

| 対象 | 動作 | 主な実装 |
|---|---|---|
| RESERVE | OPEN で指定個数のブロックサイズのバッファーを確保し CLOSE で解放。順次ファイルの読み書きで使用。RESERVE が DD BUFNO より優先、どちらもない場合の Zeus 既定は 2 | `zeus-ds/src/seq.rs::BufferPool`, COBOL `vm/files.rs` |
| RESERVE の制約 | LINE SEQUENTIAL をコンパイル診断。ロード済みプログラム間の同名 EXTERNAL ファイルの RESERVE 不一致を OPEN 時に診断 | `sema/codegen.rs`, `vm/files.rs` |
| F / FB | 固定長レコードを指定ブロックへまとめる。最終ブロックは短くなり得る。データに RDW/BDW を付けない | `zeus-ds/src/physical.rs` |
| V / VB | BDW と RDW を生成・検査し、ブロックから論理レコードを復元 | 同上 |
| S / VBS | 複数ブロックにまたがるレコードを SDW で分割・再結合。順序違反、未完了、長さ・予約バイト不正を検出 | 同上 |
| U | 1 レコードを 1 ブロックとして保存。RDW/BDW なし。可変のブロック境界が必要なため、Zeus ではテープイメージで使用 | 同上、`tape.rs` |
| BLOCK CONTAINS | CHARACTERS または RECORDS を実際のブロックサイズへ反映。0 はシステム決定サイズ。DD BLKSIZE があれば優先 | COBOL `sema/codegen.rs`, `vm/files.rs` |
| APPLY WRITE-ONLY | V 形式で、最大レコード長ではなく次のレコードの実長を基準にブロックを充填する | parser, ZIR, `physical.rs` |
| 仮想テープ | ブロック境界、テープマーク、複数の論理ファイルを保持。入力・出力・EXTEND は既存の OPEN モードに接続 | `zeus-ds/src/tape.rs`, COBOL `vm/files.rs` |
| 標準ラベル | VOL1、HDR1/HDR2、EOF1/EOF2 とテープマークを読み書き。LABEL RECORDS OMITTED はラベルなし。FD 省略時は STANDARD 扱い | 同上、parser data |
| NO REWIND | CLOSE 後に同一ホストパスのテープ位置を保持し、次の OPEN NO REWIND で次ファイルへ進む | `Machine.tape_positions` |
| REVERSED | 固定長の単一巻テープを末尾レコードから読み取る。非テープでは IBM の定義に従って修飾の効果なし | `vm/files.rs` |
| JCL | UNIT、DCB の BUFNO/BLKSIZE、LABEL のファイル番号を COBOL VM に引き渡す。DCB=*.ddname の I/O 属性参照も対応 | `zeus-jcl/src/job.rs`, `zeus-ds/src/dd.rs` |
| RECURSIVE | 同一プログラムへ再入可能。WORKING-STORAGE、ファイル状態、EXTERNAL は共有し、LOCAL-STORAGE、LINKAGE のアドレス束縛、PERFORM スタック、式の一時領域を分離 | COBOL `zir.rs`, `vm/mod.rs`, `vm/eval.rs` |
| 再帰の制約 | 入れ子プログラムと入れ子を含むプログラム、INITIAL との組み合わせを診断。RECURSIVE を宣言しない再入は従来どおりエラー | `sema/codegen.rs`, VM |

上記パスは `crates/` 配下。COBOL の省略パスは `crates/zeus-cobol/src/` 配下。

例:

```cobol
       SELECT F ASSIGN TO OUTFILE RESERVE 3 AREAS.
       ...
       FD F RECORDING MODE S
           BLOCK CONTAINS 1024 CHARACTERS.
```

```jcl
//OUTFILE DD DSN=APP.TAPE,DISP=(NEW,CATLG),UNIT=3480,
// DCB=(RECFM=FB,LRECL=80,BLKSIZE=800,BUFNO=5)
```

RESERVE がある場合は 3 バッファーになる。BLKSIZE の指定がある場合は DD が優先する。
RECFM と FD の記録形式も整合させる必要がある。上の COBOL と JCL はそれぞれ独立した指定例。

## ホスト表現と残る制約

- BLOCK CONTAINS、DD BLKSIZE、S/U、仮想テープを指定した場合に物理ブロック層を使う。それ以外の既存ホストファイルは、従来の F 連続レコード／V の RDW 列を維持する。既存 V ファイルに一律 BDW を挿入する変更ではない。
- F の通常ファイルには境界用の追加バイトを挿入しない。読取側にも同じ BLKSIZE/LRECL を指定する。AWS 内ではブロック境界自体が保存される。
- ブロックサイズは今回 1–32760 バイトの従来形式を対象とする。ラージブロック、圧縮 HET、暗号化、装置固有のエラー回復は未対応。
- システム決定ブロックサイズは Zeus の選択であり、IBM の装置別最適値や処理性能まで一致するものではない。
- バッファープールは OPEN 中に存在する I/O 用領域。論理レコードは従来どおり全件をメモリーに保持するため、RESERVE による総メモリー量の上限や QSAM の非同期先読みを実現したわけではない。VSAM のページキャッシュは別機能。
- 単一の仮想巻を対象とする。自動複数巻交換、EOV と次巻の連結、オペレーターのマウント、ユーザーラベルのコールバックは未対応。REEL/UNIT と FOR REMOVAL の経路は現在の仮想巻を閉じ、取り外し状態にする基本動作まで。IBM の複数巻継続を再現したという意味ではない。
- 標準ラベルの VOLSER は現在 ZEUS01、作成元は ZEUS。ラベルの全属性マージ・保護・満了日処理、HDR2 と DD/FD 間の属性不一致の全診断は未実装。標準ラベルとデータブロックの基本構造を対象とする。
- DD LABEL のラベル種別 SL/NL は未配線。今回ラベル有無は COBOL FD の LABEL RECORDS で指定する。DD LABEL の先頭のファイル番号は有効。
- JCL のカタログは従来の論理 RECFM/LRECL を維持する。今回の物理イメージは対応する COBOL VM と物理データセット API で扱う。他言語・全ユーティリティへ同じ媒体機能を展開する作業は残る。
- 再帰の非アクティブな起動領域は再利用する。CLOSE までのファイル共有は維持する。CALL 深さの Zeus 制限は 1000。再帰プログラムは現在インタープリターで実行し、既存ネイティブ最適化を迂回して LOCAL-STORAGE のアドレスを正しく分離する。
- RECURSIVE に対する ALTER/デバッグ等の細かい禁止条件の網羅的なコンパイル検査、スレッド並行実行、PROCEDURE DIVISION RETURNING の ABI は別途残る。
- RERUN のチェックポイント保存・再始動は未実装であり診断する。TRUNC/ARITH/NUMPROC 等の未対応設定も 0015 のとおり。

## 検証

- 物理ブロック単体試験: 既知の BDW/RDW/SDW バイト列、AWO の充填差、S の先頭／中間／末尾、破損・未完了の検出、F/U の境界。
- AWSTAPE 単体試験: 既知のヘッダー列、長さのエンディアン、マーク、前ブロック長、破損検出。
- `crates/zeus-cobol/tests/physical_recursive.rs`: RESERVE の優先順位、S のファイル読み書き、U の異長ブロック、SL の複数ファイルと番号、NO REWIND、REVERSED、再帰の WS/LS/引数/PERFORM、外部言語フレームを通る再入と ZIR 保存復元。
- `crates/zeus-jcl/tests/jobs.rs`: COBOL のコンパイル・リンクから UNIT=3480 と BLKSIZE による仮想テープ出力まで実行。

IBM 実機／コンパイラーとの差分実行および実テープ装置との接続試験は行っていない。

最終結果: `cargo test --workspace` は **970 passed / 0 failed**。
`cargo clippy -p zeus-cobol -p zeus-ds -p zeus-jcl -p zeus-layout --all-targets -- -D warnings`、
`cargo fmt --all --check`、`git diff --check` も通過した。
全体試験は既存の Windows IMS ファイル置換試験のためサンドボックス外で実行した。

# COBOL の既知の非互換に対する追加実装

確認日: 2026-10-16。基準は Enterprise COBOL 4.2。IBM の実行結果との比較を待たずに、仕様とコードから判明した不足を扱う。0014 の総合表、0015 の読み飛ばし監査、0016 の媒体・再帰の実装に対する追補。

## 今回実装した範囲

| ID | 項目 | 実装と確認 |
|---|---|---|
| R01 | WHEN-COMPILED | コンパイル時に日時をモジュールへ保存。FUNCTION は21文字、特殊レジスターは16文字の別形式。保存・読込後も実行日時に変わらない。UTC の日時とオフセット `+0000` を保存する |
| R02 | PROCEDURE DIVISION RETURNING / CALL RETURNING | LINKAGE の01/77項目に結果用領域を割り当て、呼出し元の受信項目へ転送。RETURNING付きCALLでは呼出し元のRETURN-CODEを保持し、呼出し解決の例外時は受信値を保持。文字・数値・外部言語インターフェースの結果に対応。ENTRYとの併用、入れ子、USINGとの重複、不一致の受信属性を診断 |
| R03 | TRUNC | CBL/PROCESS の STD/OPT/BIN。STDはPICTURE桁数、BINはCOMP/COMP-4/BINARYをCOMP-5と同様に格納幅で処理。VALUE、MOVE、算術のサイズエラー、DISPLAYを確認。OPTは有効なPICTURE範囲のデータに対してSTDと同じ経路を使い、IBMの最適化命令列は再現しない |
| R04 | NATIONAL数値・編集 | USAGE NATIONALの固定小数点数をUTF-16で格納。SIGN SEPARATE、演算、数値クラス判定、数値編集、逆編集に対応。PIC N/B/0/スラッシュのnational-editedは文字を保ったまま編集する |
| R05 | GROUP-USAGE NATIONAL | グループと下位グループをnationalカテゴリーとし、下位項目のNATIONALを暗黙指定。MOVE、比較、参照変更、INITIALIZEを確認。不正なUSAGE、JUSTIFIED、非独立符号などを診断 |
| R06 | PROCEDURE-POINTER / FUNCTION-POINTER | 4.2の配置に合わせて8/4バイトを確保。NULL、SET TO ENTRY、ポインター間のSET、NULL比較、間接CALL、CALL RETURNINGに対応。値は実行単位内の仮想エントリー識別子。IBMロードモジュールやホストC関数アドレスのABIではない |
| R07 | FILE STATUSの第2項目 | 固定6バイトのWORKING-STORAGE/LINKAGEグループを検査。仮想KSDS/RRDSの重複・未検出・終端に、2バイトずつのreturn/component/reasonをビッグエンディアンで格納。アクセス前の状態検査ではゼロ。基本クラスターはcomponent 0、代替索引は2 |
| R08 | RECURSIVEの禁止事項 | ALTER、宛先なしGO TO、SEGMENT-LIMIT、セグメント番号、USE FOR DEBUGGINGを診断。既存のINITIAL・入れ子との組合せ禁止も維持 |

COMP-5では従来、符号付き格納幅の境界で剰余の計算が誤っていた。格納時は下位ビットを残し、サイズエラーは符号付き・符号なしそれぞれの正確な範囲で判定するよう修正した。バイナリー／内部十進数のDISPLAYでは、正数に不要なCゾーンの符号を付けず、負数の下位桁にオーバーパンチを付ける。COMP-5/TRUNC(BIN)のDISPLAYは格納幅に応じた5/10/20桁とする。

## 利用例

先頭の固定形式ソース行で設定する。

```cobol
       CBL TRUNC(BIN)
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EXAMPLE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 BINARY-NUMBER PIC 9(4) BINARY VALUE 12345.
       01 NATURAL-GROUP GROUP-USAGE NATIONAL.
          05 LABEL-TEXT PIC N(8).
          05 ITEM-COUNT PIC 99.
       PROCEDURE DIVISION.
           DISPLAY BINARY-NUMBER
           DISPLAY FUNCTION WHEN-COMPILED
           STOP RUN.
```

この例のBINARY-NUMBERは2バイトで12345を保持する。TRUNC(STD)では2345となる。COMP-5はTRUNC指定によらず格納幅を使う。OPTでPICTURE範囲外の値を扱った結果は移植性の対象に含めない。

## 残る既知の不足

この追加実装でIBM言語全体の未実装がなくなったわけではない。

| 項目 | 残る実装・検証 |
|---|---|
| ARITH / NUMPROC | 設定の切替は未実装で明示指定を診断。COMPAT/EXTENDの中間精度・演算ごとの規則、128ビット浮動小数点、PFD/NOPFD/MIGの符号規則を実装する必要がある。現在の十進演算がIBMの既定と完全一致する保証はない |
| 指数表記・浮動小数点 | 指数リテラル、外部浮動小数点PICTURE、浮動小数点演算の全範囲・精度は不足。NATIONAL数値の追加は固定小数点を対象とする |
| エントリー・呼出しABI | 一般のENTRY別名・開始位置・エントリー別引数の処理は不足。ポインターによる呼出しは主エントリー等、既存の名前解決が扱う対象に限る。実機の関数記述子、OSリンケージ、ホストCの生アドレスへの接続は未対応 |
| データと文の制約 | SAMEグループ、EXTERNAL/GLOBAL、可変位置、各データカテゴリーの合法な句・文の全組合せは未検証。戻り値の集団項目の全下位属性照合も未対応 |
| テープ・ファイル | 自動複数巻交換、EOV連結、ユーザーラベル、RERUN再始動、FD/DD/媒体ラベル間の属性照合の残件は0016を参照。仮想VSAMの詳細状態は実装した論理アクセスの範囲で、実VSAMの全障害・共有・競合を再現するものではない |
| XML / OO・Java / DATEPROC | 各構文・実行モデル・特殊レジスター・型規則が未実装。OOではCLASS-ID/METHOD-ID/INVOKE、日付ではDATE FORMAT/YEARWINDOW等が残る |
| その他のコンパイラー制御 | 条件付きコンパイル指令、全CBL/PROCESS設定、リスト出力・診断の完全再現は未対応 |

古いモジュールには保存日時がないため、WHEN-COMPILEDを使用する場合は再コンパイルする。実行日時で代用せずエラーにする。新しい命令・型を含むモジュールは、追加実装後のランタイムで実行する。

## 公式資料と検証

- [Enterprise COBOL 4.2 Language Reference](https://publibfp.dhe.ibm.com/epubs/pdf/igy3lr50.pdf): PROGRAM-ID pp.107–108、FILE STATUS pp.150–151、GROUP-USAGE p.203、USAGE pp.246–249、RETURNING p.266、DISPLAY pp.346–348、SET pp.447–448。
- [TRUNC](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=options-trunc): STD/OPT/BINの意味、COMP-5との関係、VALUEとDISPLAYへの適用。
- [コンパイル日時](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=functions-finding-date-compilation): 特殊レジスターと組込み関数の形式。
- [GROUP-USAGE](https://www.ibm.com/docs/en/cobol-zos/6.3.0?topic=entry-group-usage-clause): NATIONALの継承・符号・グループの処理規則。
- [VSAM logical errors](https://www.ibm.com/docs/en/zos/2.5.0?topic=rplerrcd-reason-code-logical-errors) / [component codes](https://www.ibm.com/docs/en/zos/3.2.0?topic=codes-component-rplcmpon): return 8、reason 4/8/16とcomponent 0/2の根拠。

回帰試験は `crates/zeus-cobol/tests/remaining_compatibility.rs` の16件。日時とモジュール保存、文字・数値の結果、呼出し失敗とRETURN-CODE保持、TRUNC、NATIONAL、ポインター、VSAM詳細状態、不正定義、再帰の禁止事項を含む。`crates/zeus-native/tests/native.rs`にも複合試験を追加し、単独実行ファイルとインタープリターでTRUNC・NATIONAL・ポインター・戻り値・保存日時を比較する。既存のJCL・ネイティブ実行試験の期待出力も、正数のDISPLAY修正に合わせた。

IBMコンパイラー／z/OSとの差分実行は未実施。上記の実装試験と実機の互換性判定を区別する。

最終確認: `cargo test --workspace` は **987 passed / 0 failed**。WindowsのIMSファイル置換と単独実行ファイルの試験を含め、サンドボックス外で実行した。`cargo fmt --all --check` と `git diff --check` も通過。

`cargo clippy -p zeus-cobol -p zeus-layout -p zeus-jcl -p zeus-native --all-targets -- -D warnings` も通過。

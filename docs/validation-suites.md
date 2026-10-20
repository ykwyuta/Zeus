# 言語横断の互換性テスト

2026-10-20時点の追加スイート。NIST CCVS85と既存Rustテストに加えて実行します。

同日のローカル検証は `cargo test --all` が1,251成功・0失敗・6件除外、`cargo fmt --all --check` と `cargo clippy --all-targets -- -D warnings` が成功しました。新設の言語スイート統合テスト7件は除外なしで成功しています。CLIでも失敗判定が終了コード1、存在しないプログラム選択とワーカー数0が終了コード2になることを確認しました。

| スイート | 言語 | 主プログラム | 実行本数 / 判定数 | 負系 / 正常対照 |
|---|---|---:|---:|---:|
| [ZCVS](../zcvs/README.md) | COBOL | 158 | 93 / 1,431 | 45 / 20 |
| [ZPVS](../zpvs/README.md) | PL/I | 68 | 38 / 318 | 15 / 15 |
| [ZHVS](../zhvs/README.md) | HLASM | 125 | 85 / 537 | 20 / 20 |

PL/Iは通常/ネイティブ、HLASMはZeus/OBJ/GOFFを `crates/zeus-ccvs/tests/language_suites.rs`、COBOLは通常/ネイティブを `tests/zcvs.rs` で毎回検査します。`cargo test --all` に含まれるためCIも同じ試験を実行します。コンパイラー、アセンブラー、共通ランタイムをビルドしてから、全ソースと期待インベントリー、レポートID、件数、診断、正常対照、JCL出力を照合します。

混在言語の引数/共有記憶域はPL/I→COBOL/HLASM、HLASM→PL/I/COBOLの両方向を検査します。コンパイル拒否試験には対象規則を修復した正常対照を設け、単なるパニック・未実行・空レポートを成功扱いにしません。

仕様分野の網羅性と、全入力/全組合せの網羅性は別です。新スイートは全モジュールを実行可能にし、代表値、境界、内部バイト表現、状態遷移、負系を収録します。例外の全組合せ、CPU全世代/全命令形式、全OSサービスは検査していません。IBM実機との一致は実機差分実行後に別途記録する必要があります。

## 詳細回帰との対応

提案0020/0021の広い検査範囲は、単独実行用スイートと以下の既存試験を合わせて確認します。既存のRust試験を、新スイートの判定数に加算してはいません。

| 分野 | 単独実行スイート | 詳細な回帰試験 |
|---|---|---|
| PL/Iの条件、LOCATE/REGIONAL、AREA/iSUB/REFER、CONTROLLED引数、型定義 | PC / PF / PS / PD | [programs.rs](../crates/zeus-pli/tests/programs.rs) |
| LEハンドラーの寿命、条件トークン、LIFO/昇格/再開 | PM / HSの言語連携 | [PL/I LE](../crates/zeus-pli/tests/hlasm_le.rs)、[HLASM LE](../crates/zeus-cobol/tests/hlasm_le.rs) |
| 浮動小数点のNaN/無限大/非正規数、丸め、例外、PFPO | HF（54方向の正/負/ゼロを含む） | [hlasm_float.rs](../crates/zeus-cobol/tests/hlasm_float.rs) |
| QSAMの可変長/ロケートモード、DD優先、異常系 | HS103A | [hlasm_io.rs](../crates/zeus-cobol/tests/hlasm_io.rs) |
| 記憶域の再利用/断片化/所有権/サブプール寿命 | HS102A | [hlasm_storage_services.rs](../crates/zeus-cobol/tests/hlasm_storage_services.rs) |
| 機械語バイト、再配置、OBJ/GOFF再読込み | ZHVS全実行プログラムの3経路 | [OBJ出力](../crates/zeus-cobol/tests/hlasm_object_output.rs)、[GOFF出力](../crates/zeus-cobol/tests/hlasm_goff_output.rs) |

## Hercules（Docker / ローカル）による検証の再現手順

[tools/hercules/Dockerfile](../tools/hercules/Dockerfile) と [tools/hercules-suites.sh](../tools/hercules-suites.sh)（内部で `zeus-hercdiff zhvs` / `zeus-hercdiff zpvs` を呼び出し）により、[ZHVS](../zhvs/README.md)（ベアメタル `z/Arch`）および [ZPVS](../zpvs/README.md)（MVS 3.8j / OS/360 PL/I (F) V5.5 `PGM=IEMAA`）の Hercules 検証を再現できます。

```sh
# 1. Docker イメージ（rust + hercules）のビルド
tools/hercules-suites.sh build

# 2. ZHVS（HLASM）ベアメタル z/Arch 検証（28本・204判定）
#    -> work/zhvs-hercules.json に結果を出力
tools/hercules-suites.sh zhvs

# 3. ZPVS（PL/I）MVS 3.8j バッチ検証
#    ホスト側の MVS 3.8j DASD ディレクトリ（mvsres.140.cckd 等）を読み取り専用マウントし、
#    CCKD シャドウファイル上で自動 IPL・JCL 投入・REPORT 回収を行います
#    -> work/zpvs-hercules.json に結果を出力
tools/hercules-suites.sh zpvs --mvs-dasd /path/to/MVSR38/Dasd

# 4. ZHVS と ZPVS を一括実行
tools/hercules-suites.sh all --mvs-dasd /path/to/MVSR38/Dasd
```

ホストに `hercules` がインストール済みの場合は、`ZEUS_HERCULES_LOCAL=1` を指定するか `cargo run --release -p zeus-hercdiff -- zhvs|zpvs ...` を直接実行できます。

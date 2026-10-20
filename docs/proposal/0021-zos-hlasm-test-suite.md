# Proposal 0021: CCVS85 方式による z/OS HLASM 互換性テストスイート (ZHVS)

- 状態: スイート実装済み、実機差分検証は未実施
- 作成日: 2026-10-16
- 関連:
  - [0012 Application HLASM and shared storage (zas)](0012-hlasm.md)
  - [0019 CCVS85 補完 z/OS COBOL 互換性テストスイート (ZCVS)](0019-zos-cobol-test-suite.md)
  - [0020 CCVS85 方式による z/OS PL/I 互換性テストスイート (ZPVS)](0020-zos-pli-test-suite.md)

## 1. 背景と目的

現在 `zas` (Zeus HLASM アセンブラー・実行エンジン) の検証は、`crates/zeus-cobol/tests/hlasm*.rs` の 44 本の Rust 統合テストファイルによって行われている。これらは Rust 関数内でインライン文字列としてアセンブラーソースを組み立て、Rust 側の API (`Machine` のレジスターやメモリの直接参照) で結果を検査している。

この方式は開発中の単体検証には高速で有効だが、次の課題がある。

1. **実機 z/OS (`ASMA90` + `IEWL`) や Hercules 上での直接実行・オラクル照合ができない**
   - テストが Rust コードの中に閉じ込められているため、純粋な `.ASM` ソースおよびマクロライブラリーとして外部の z/OS 環境や Hercules へ持ち出し、「テスト自体の期待値が実機の z/Architecture および IBM HLASM V1R6 と完全一致するか」を直接検証できない。
2. **ソース実行 (`.zo`) とバイナリーオブジェクト実行 (`OBJ` / `GOFF`) の透過的な一括検証**
   - `zas` は Zeus 形式 (`.zo`) に加え、80 バイト固定長 `OBJ` および `GOFF` の入出力・再ロード実行をサポートしている ([0012](0012-hlasm.md))。独立したアセンブラーテストスイートがあれば、同じ `.ASM` スイートを `--toolchain zas` (`.zo`)、`--toolchain zas-obj` (`OBJ` 往復)、`--toolchain zas-goff` (`GOFF` 往復) の 3 経路でそのまま走らせることができる。
3. **CCVS85 / ZCVS / ZPVS と統一されたモジュール別レポート**
   - COBOL (`ZCVS`)・PL/I (`ZPVS`) と同じ `DD_REPORT` 集計形式を採用することで、Zeus がサポートする 3 言語すべての互換性状況を `zeus-ccvs` の単一フォーマットで管理できる。

そこで本提案では、CCVS85 と同じ自己検証レポート形式を持つ **z/OS HLASM 互換性テストスイート「ZHVS (z/OS HLASM Validation Suite)」** を `zhvs/` 以下に構築することを提案する。

---

## 2. 設計原則

### 2.1 標準マクロ (`DCB` / `OPEN` / `PUT` / `CLOSE`) による CCVS85 互換レポート出力

`zas` は順次レコード入出力マクロ (`DCB DDNAME=REPORT,DSORG=PS,MACRF=PM,RECFM=FB,LRECL=120`、`OPEN`、`PUT`、`CLOSE`) を既に実装している ([zas.txt]( ../help/zas.txt))。ZHVS では共通マクロライブラリー (`zhvs/copy/`) に次のテスト用マクロを用意し、すべてのアセンブラープログラムが自己検証レポートを `REPORT` DD に出力する。

- **`ZHVSHDR`**: 標準 OS リンケージのプロローグ (`STM 14,12,12(13)`、セーブエリア連鎖)、`REPORT` DCB の `OPEN`、ヘッダー行出力、テストカウンター初期化を展開する。
- **`ZHVSCHK`**: 汎用レジスター値・メモリバイト列・条件コード (`IPM` で取得) を期待値と比較し、`UNPK` + `TR` による 16 進展開を行って `PASS` または `FAIL` (期待値・実測値付き) の 120 バイト行を `PUT` する。
- **`ZHVSSUM`**: `CVD` + `ED` で合格件数・失敗件数・削除件数を編集し、CCVS85 パーサー (`crates/zeus-ccvs/src/report.rs`) が認識する集計行を出力して `CLOSE` し、`R15=0` で復帰する。

```text
ZHVS REPORT - PROGRAM HI101A - MODULE HI (GENERAL AND 64-BIT INSTRUCTIONS)
------------------------------------------------------------------------------------------------------------------------
PASS  HI101A-T001  POP-CH7        ALCG/ALCGR 128-BIT CARRY PROPAGATION AND CC
PASS  HI101A-T002  POP-CH7        RISBG WRAPAROUND BIT ROTATION AND ZEROING
FAIL  HI101A-T003  POP-CH7        DSGR 64-BIT SIGNED DIVIDE REMAINDER SIGN
      EXPECTED: FFFFFFFFFFFFFFFD0000000000000005
      ACTUAL:   00000000000000030000000000000005
------------------------------------------------------------------------------------------------------------------------
     2 OF      3  TESTS WERE EXECUTED SUCCESSFULLY
     1   TEST(S) FAILED
    NO   TEST(S) DELETED
```

### 2.2 3 つの実行経路 (`.zo` / `OBJ` / `GOFF`) と実機互換性

- すべての `.ASM` ソースは IBM HLASM V1R6 Language Reference (SC26-4940) および z/Architecture Principles of Operation (SA22-7832) に準拠した固定形式 (列 1–8 名前、列 10–14 命令、列 16–71 オペランド、列 72 継続標識、列 73–80 識別) で記述する。
- 機械命令・算術・浮動小数点・アドレス解決を検証するモジュール (`HA`, `HM`, `HI`, `HD`, `HF`) は、ランタイムサービス (`DCB`/`PUT`) を呼ぶレポート出力部を外出しの共通サブルーチン (`lib/ZHVSRPT.ASM`) に分離できる構造とする。これにより、**テスト対象の CSECT 自体を `--output-format obj` や `--output-format goff` で機械語オブジェクトカードに変換し、リンクして実行するラウンドトリップ検証** が可能になる。

---

## 3. モジュール体系 (`H*`)

CCVS85、ZCVS (`Z*`)、ZPVS (`P*`) と衝突しない `H` 始まりの 2 文字モジュールコードを採用する。

| モジュール | 名称 | 必須/任意 | 検証対象領域 |
|---|---|---|---|
| **`HA`** | Assembler Language, Expressions & Layout | **必須** | 固定形式・継続行・`ICTL`・`*PROCESS`、`CSECT`/`RSECT`/`DSECT`/`LOCTR`、`ORG`/`LTORG`/`CNOP`、`USING` (通常・ラベル付き・従属・範囲・`PUSH`/`POP`)、複合再配置 `EQU` と属性 (`L'`, `T'`, `S'`, `I'`, `D'`, プログラム/アセンブラー属性)、全 `DC`/`DS` 定数型と `L`/`S`/`E` 修飾子式 |
| **`HM`** | Macro & Conditional Assembly | **必須** | 位置/キーワード引数・部分リスト (`&SYSLIST`)、スカラー/配列 `SETA`/`SETB`/`SETC`、生成 `SET` 記号、`AIF`/`AGO`/`ACTR`/`ANOP`/`MEXIT`/`MNOTE`、組込みマクロ関数 (`C2A`, `D2X`, `FIND`, `SYSATTRP`, `SYSATTRA` 等)、`OPSYN`、先読み (Lookahead) |
| **`HI`** | General, 64-Bit, Relative & Atomic Instructions | **必須** | 32/64 ビット整数演算・論理・比較、キャリー/ボロー (`ALC`/`SLB`)、128 ビット乗除算 (`MLG`/`DLG`)、長変位 (`RXY`/`RSY`/`SIY`)、部分レジスター即値 (`IIHF`/`LLIHH` 等)、バイト反転 (`LRVG`/`STRVG`)、ビット回転・選択 (`RISBG`/`RNSBG`/`FLOGR`/`POPCNT`/`SELR`/`LOCG`)、相対分岐 (`BRC`/`BRASL`/`LARL`/`BRCTG`)、マスク操作 (`ICM`/`STCM`/`CLM`)、原子操作 (`CS`/`CSG`/`CDS`/`CDSG`/`CSST`/`LPDG`/`LAAG`) |
| **`HD`** | Character, String & Decimal Instructions | **必須** | 文字・ビット操作 (`MVC`/`CLC`/`MVN`/`MVZ`/`XC`/`NC`/`OC`/`TR`/`TRT`/`TRTR`/`MVCL`/`CLCL`/`MVCLE`/`CLCLE`/`MVST`/`CLST`/`SRST`/`EX`/`EXRL`)、パック/ゾーン 10 進命令 (`ZAP`/`AP`/`SP`/`CP`/`MP`/`DP`/`SRP`/`PACK`/`UNPK`/`PKU`/`PKA`/`UNPKU`/`UNPKA`/`CVB`/`CVBG`/`CVD`/`CVDG`/`TP`)、`ED`/`EDMK` 編集パターン |
| **`HF`** | Floating-Point Architecture (HFP / BFP / DFP) | **必須** | 16 進浮動小数点 (HFP 短・長・拡張、非正規化 `MYR`/`MAYR`、`SQDR`/`FIDR`)、2 進浮動小数点 (BFP 演算、積和 `MADBR`、`DIEBR`/`DIDBR`、丸め・NaN/Inf・`FPC`)、10 進浮動小数点 (DFP DPD 符号化、量子保持、`ADTR`/`MDTR`/`QADTR`/`RRDTR`/`CDPT`/`CPDT`/`CDZT`/`CZDT`)、`PFPO` 基数間変換 (54 種)、`THDR`/`TBDR` |
| **`HS`** | Linkage, Services, OBJ/GOFF & ADATA | **必須** | `AMODE 24/31/64` (`BASSM`/`BSM`, `SAM24/31/64`, `F4SA`)、`EXTRN`/`WXTRN` と `A`/`V` 解決、`GETMAIN`/`FREEMAIN`/`STORAGE`、`LOAD`/`DELETE`/`LINK`/`LINKX`/`XCTL`/`XCTLX`、`WTO`/`WAIT`/`POST`/`STIMER`、`DCB`/`OPEN`/`GET`/`PUT`/`CLOSE` (`EODAD`/`SYNAD`)、LE 条件サービス (`CEEHDLR`/`CEESGL`/`CEEDCOD` 等)、COBOL/PL/I 相互呼出し、`OBJ`/`GOFF`/`ADATA`/リスト出力検証 |
| **`HE`** | Assembler Diagnostics & Negative Suite | **必須** | アドレス解決不能・変位範囲外・再配置係数違反・`RSECT` 書込み・偶奇レジスター違反・前方属性参照違反・`OBJ`/`GOFF` 制約違反などが正しく診断されることを確認する負系テスト群 |

---

## 4. 各モジュールの詳細テスト計画

### 4.1 `HA`: アセンブラー言語・式・属性・セクション配置 (Assembler Language & Layout)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 (SC26-4940) |
|---|---|---|---|
| `HA101A` | 式・複合 `EQU`・再配置相殺 | 単項/二項演算、同一セクション差分の絶対値化、複合再配置 `EQU` チェーン (`TABLE+LIMIT-START`)、前方参照 `EQU` の解決 | Ch.2, Ch.5 |
| `HA102A` | `EQU` 明示属性と `L'`/`T'`/`S'`/`I'` | `EQU val,len,type,prog,asm`、命令ラベル (型 `'I'`)・`DC`/`DS` ラベルの長さ/型属性、リテラル (`=F'7'`) の属性参照、スケール `S'` と整数桁 `I'` | Ch.5, Ch.7 |
| `HA103A` | `DC`/`DS` 定数と `L`/`S`/`E` 修飾子式 | `C`/`X`/`B`/`H`/`F`/`FD`/`P`/`Z`/`A`/`V`/`Y`/`S`/`Q`/`J`、括弧付き式による長さ (`L(expr)`)・ビット長 (`L.n`)・スケール (`S(expr)`)・指数 (`E(expr)`)・重複因数 | Ch.5 Data definition |
| `HA104A` | セクション・`LOCTR`・`ORG`・`LTORG` | 複数 `CSECT`/`RSECT`、`LOCTR` によるインターリーブ配置と再開、`DSECT`、`CNOP`、`ORG` 巻き戻しと境界、`LTORG` リテラルプール重複排除 | Ch.4, Ch.5 |
| `HA105A` | `USING` / `DROP` / `PUSH` / `POP` | 通常 `USING`、ラベル付き `USING`、従属 `USING`、範囲上限付き `USING`、複数基底レジスターの最小変位・負変位選択規則、`PUSH`/`POP USING` | Ch.6 Controlling addressability |

### 4.2 `HM`: マクロ言語・条件付きアセンブリー (Macros & Conditional Assembly)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `HM101A` | マクロ引数・部分リスト・生成記号 | 位置引数・キーワード既定値、ラベル引数、多重部分リスト (`&SYSLIST(i,j)`)、`N'` / `K'` 属性、生成 `SET` 記号 (`&(&X)`) | Ch.8, Ch.9 |
| `HM102A` | `SETA`/`SETB`/`SETC` と分岐制御 | スカラー・配列 `SET` 変数、`SETC` 部分文字列・連結・重複、`AIF` / `AGO` / 計算 `AGO` / `ACTR` ループ上限 / `MEXIT` | Ch.9 |
| `HM103A` | 組込みマクロ関数と `SYSATTRP`/`SYSATTRA` | `A2B`/`B2A`/`C2X`/`X2C`/`D2X`/`X2D`/`FIND`/`INDEX`/`SIGNED`/`UPPER`/`LOWER`/`DOUBLE`/`DEQUOTE`/`BYTE`、`SYSATTRP`/`SYSATTRA` | Ch.9 |
| `HM104A` | `OPSYN`・入れ子マクロ・先読み (Lookahead) | `OPSYN` による命令別名定義と削除、遮蔽された入れ子マクロ定義、未定義記号に対するオープンコード/COPY 先読み (`L'`/`T'`/`S'`/`I'` と `D'` の非汚染) | Ch.5, Ch.9 |

### 4.3 `HI`: 汎用命令・64 ビット・相対・マスク・原子操作 (General & 64-Bit Instructions)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 (SA22-7832) |
|---|---|---|---|
| `HI101A` | 32/64 ビット整数算術・論理・比較 | `L`/`LG`/`LGF`/`LGFR`、`A`/`AG`/`AGF`、`AL`/`ALG`、キャリー/ボロー連鎖 (`ALC`/`ALCG`/`SLB`/`SLBG`)、128 ビット乗除算 (`MLG`/`DLG`)、符号付き除算 (`DR`/`DSG`/`DSGF`) | Ch.7 General Instructions |
| `HI102A` | 長変位 (`RXY`/`RSY`/`SIY`)・即値・バイト反転 | 20 ビット符号付き変位 (`LY`/`STY`/`AY`/`LMG`/`STMG`/`CLIY`/`MVIY`)、部分レジスター即値 (`IIHF`/`IILF`/`LLIHH`../`TMHH`..)、エンディアン反転 (`LRVH`/`LRV`/`LRVG`/`STRV`..)、`LB`/`LGB`/`LLC`/`LLH` | Ch.7 |
| `HI103A` | ビット回転・選択 (`RISBG` 系)・`FLOGR`・`POPCNT` | `RISBG`/`RISBGN`/`RISBHG`/`RISBLG` (ラップアラウンドとゼロ拡張)、`RNSBG`/`ROSBG`/`RXSBG` (T テスト形式含む)、`FLOGR`、`POPCNT` (バイト別・全幅マスク 8)、3 オペランド論理 (`NGRK`/`OGRK`/`XGRK`..)、`SELR`/`SELGR`/`LOCG`/`STOCG` | Ch.7 |
| `HI104A` | 相対分岐・カウンタ分岐・マスク操作 | `BRC`/`BRCL` (`J`/`JG` 全条件別名)、`BRAS`/`BRASL`、`LARL`、`BRCT`/`BRCTG`/`BRCTH`、`BCTR`/`BCTGR`、`ICM`/`ICMY`/`ICMH`、`STCM`/`STCMY`/`STCMH`、`CLM`/`CLMY`/`CLMH` | Ch.7 |
| `HI105A` | インターロック・原子操作 | `CS`/`CSY`/`CSG`、`CDS`/`CDSY`/`CDSG`、`CSST` (1〜16 バイト比較・追加ストア)、`LPD`/`LPDG`、アトミック算術/論理 (`LAA`/`LAAG`/`LAAL`/`LAN`/`LAO`/`LAX` と G 形式) | Ch.7 |

### 4.4 `HD`: 文字・文字列・10 進命令・編集 (Character & Decimal Instructions)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `HD101A` | 文字移動・比較・変換・検索 | `MVC`/`MVI`/`CLC`/`CLI`、`MVN`/`MVZ`、`XC`/`NC`/`OC`、`TR`、`TRT`/`TRTR` (R1/R2 更新と CC)、`MVCL`/`CLCL` (パディングと >256B)、`MVCLE`/`CLCLE`、`MVST`/`CLST`/`SRST`、`EX`/`EXRL` | Ch.7 |
| `HD102A` | パック/ゾーン 10 進算術・変換 | `ZAP`/`AP`/`SP`/`CP`/`MP`/`DP`、`SRP` (正負シフトと丸め)、`PACK`/`UNPK`、`PKU`/`PKA`/`UNPKU`/`UNPKA`、`CVB`/`CVBY`/`CVBG`、`CVD`/`CVDY`/`CVDG`、`TP` (`zeus-hercdiff` オラクル照合ケースを含む) | Ch.8 Decimal Instructions |
| `HD103A` | `ED` / `EDMK` 複合編集パターン | 桁選択 (`X'20'`)、有効開始 (`X'21'`)、フィールド分離 (`X'22'`)、負符号復帰、通貨記号の浮動挿入 (`EDMK` の R1 設定)、ゼロ抑制と条件コード | Ch.8 |

### 4.5 `HF`: 浮動小数点アーキテクチャ (HFP / BFP / DFP / PFPO)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `HF101A` | HFP (16 進浮動小数点) 定数と命令 | `E`/`D`/`L` および `EH`/`DH`/`LH`/`LQ` 定数 (スケール・指数修飾子)、短/長/拡張演算 (`AER`/`ADR`/`AXR`, `MDR`/`MXR`, `DDR`/`DXR`, `SQDR`/`SQXR`, `FIDR`)、非正規化積和 (`MYR`/`MAYR`)、整数変換 (`CDFR`/`CGDR`) | Ch.9, Ch.18 HFP |
| `HF102A` | BFP (IEEE 2 進浮動小数点) と `FPC` | `EB`/`DB`/`LB` 定数 (特殊値 `SNAN`/`QNAN`/`INF`)、四則演算・平方根、融合積和 (`MAEBR`/`MADBR`/`MSEBR`/`MSDBR`)、`DIEBR`/`DIDBR`、`FIEBR`/`FIDBR`、整数変換 (`CDFBR`/`CGDBR`/`CLGDBR`)、`TCEB`/`TCDB`/`TCXB`、`LFPC`/`SFPC`/`SRNMB` | Ch.9, Ch.19 BFP |
| `HF103A` | DFP (IEEE 10 進浮動小数点) とパック/ゾーン変換 | `ED`/`DD`/`LD` DPD 定数、量子 (quantum) 保持演算 (`ADTR`/`MDTR`/`DDTR`)、`QADTR`/`RRDTR`、指数/仮数抽出挿入 (`EEDTR`/`ESDTR`/`IEDTR`)、パック/ゾーン直接変換 (`CDPT`/`CPDT`/`CDZT`/`CZDT`/`CDSTR`/`CSDTR`)、`TDCDT`/`TDGDT`、`SLDT`/`SRDT` | Ch.9, Ch.20 DFP |
| `HF104A` | `PFPO` 基数間変換と `THDR`/`TBDR` | HFP ⇔ BFP (`THDER`/`THDR`/`TBEDR`/`TBDR`)、`PFPO` による HFP/BFP/DFP 全 54 組の相互変換・丸めモード・範囲トラップ時のスケーリング指数 (`R1`) | Ch.9 |

### 4.6 `HS`: リンケージ・サービス・言語間連携・OBJ/GOFF/ADATA (`HS`)

| プログラム | 対象項目 | 主な検証内容 | 仕様根拠 |
|---|---|---|---|
| `HS101A` | `AMODE 24/31/64` と外部結合 (`EXTRN`/`WXTRN`) | `BASSM`/`BSM`・`SAM24`/`31`/`64` モード切替、144B `F4SA` セーブエリア、`EXTRN`/`WXTRN`、`A`/`V` アドレス定数、外部相対命令 (`BRASL`) のリンク解決、`RSECT` 書込み保護 | [0012](0012-hlasm.md) |
| `HS102A` | 動的記憶域とモジュール管理サービス | `GETMAIN`/`FREEMAIN` (`R`/`RU`/`RC`) と `STORAGE OBTAIN`/`RELEASE` (サブプール寿命)、`LOAD`/`DELETE`、`LINK`/`LINKX` (`PLIST4`/`PLIST8`, `AMODE64OK`)、`XCTL`/`XCTLX`、`WTO`、`WAIT`/`POST` | [0012](0012-hlasm.md) |
| `HS103A` | 順次レコード入出力 (`DCB`/`OPEN`/`GET`/`PUT`/`CLOSE`) | `MACRF=GM/PM` (移動モード) と `GL/PL` (ロケートモード)、`RECFM=F/FB/V/VB`、`EODAD` 終端分岐、`SYNAD` 回復分岐、ヒープ複製 DCB のオープン | [0012](0012-hlasm.md) |
| `HS104A` | LE 条件管理サービスと言語間呼出し | `CEENCOD`/`CEEDCOD`、`CEEHDLR`/`CEEHDLU` (LIFO 順序・重複警告 256)、`CEESGL` (再開 10 / パーコレート 20 / プロモート 30/31/32/33 / 修正 60)、`CEEGQDT`、COBOL/PL/I との共有記憶域呼出し | [0012](0012-hlasm.md) |
| `HS105M` | `OBJ` / `GOFF` バイナリー出力・再ロード往復 | `HA`〜`HF` の演算・再配置・複数要素 (`RSECT` + 書込み域) を `--output-format obj` / `goff` で出力し、バイナリーから再ロード・リンクして同一のレポート結果になることを検証 | [0012](0012-hlasm.md) |

### 4.7 `HE`: アセンブル診断・負系テスト (Diagnostics & Negative Suite)

- `HE101N`〜`HE120N`: 12 ビット/20 ビット変位範囲超過、USING 未設定のベース解決失敗、再配置係数が 0/1 以外の相対分岐、前方参照を含む `EQU` 長さ/型属性、奇数レジスター指定のペア命令、`ORG` による命令上書き、不正な `*PROCESS` / `--pcontrol` / `--compat` 指定、`OBJ`/`GOFF` エンコード制約違反などを確実にエラー (`expect = "compile-error"`) として拒否することを検証する。

---

## 5. ディレクトリ構成と自己検証コードの形式

### 5.1 リポジトリー構成 (`zhvs/`)

```text
zhvs/
├─ README.md
├─ manifest.toml             # スイート定義 (toolchain = zas / zas-obj / zas-goff)
├─ copy/
│  ├─ ZHVSHDR.mac            # プロローグ・REPORT OPEN マクロ
│  ├─ ZHVSCHK.mac            # レジスター/メモリ/CC 検証・16進展開・PUT マクロ
│  └─ ZHVSSUM.mac            # CCVS85 互換集計出力・CLOSE・エピローグ マクロ
├─ HA/
│  ├─ HA101A.ASM
│  └─ ...
├─ HM/
├─ HI/
├─ HD/
├─ HF/
├─ HS/
│  ├─ lib/
│  │  └─ HSS01A.ASM          # LINK / LOAD / EXTRN 対象サブルーチン
│  └─ HS101A.ASM
└─ HE/
   ├─ HE101N.ASM
   └─ ...
```

### 5.2 自己検証 HLASM プログラムの具体例 (`HI101A.ASM`)

```hlasm
HI101A   CSECT
         ZHVSHDR PROG=HI101A,MOD=HI
*---------------------------------------------------------------------*
* TEST 1: 64-BIT ADD LOGICAL WITH CARRY (ALCG)                        *
*---------------------------------------------------------------------*
         LG    2,=XL8'FFFFFFFFFFFFFFFF'
         LG    3,=XL8'0000000000000001'
         ALGR  2,3                     PRODUCE CARRY (CC=2, R2=0)
         LG    4,=XL8'0000000000000005'
         LG    5,=XL8'0000000000000002'
         ALCGR 4,5                     4 = 5 + 2 + CARRY(1) = 8
         STG   4,ACTUAL8
         ZHVSCHK ID='HI101A-001',DESC='POP-CH7 ALCGR CARRY ADD',       X
               ACT=ACTUAL8,EXP==XL8'0000000000000008',LEN=8
*---------------------------------------------------------------------*
* TEST 2: RISBG BIT ROTATION AND ZEROING                              *
*---------------------------------------------------------------------*
         LG    2,=XL8'0000000012345678'
         RISBG 3,2,32,63,4,Z           ROTATE LEFT 4 BITS IN LOW WORD
         STG   3,ACTUAL8
         ZHVSCHK ID='HI101A-002',DESC='POP-CH7 RISBG ROTATE ZERO',     X
               ACT=ACTUAL8,EXP==XL8'0000000023456780',LEN=8
*
         ZHVSSUM
ACTUAL8  DS    D
         LTORG
         END   HI101A
```

---

## 6. 3 言語横断のハーネス統合と段階的実装計画

[0019 (ZCVS)](0019-zos-cobol-test-suite.md)、[0020 (ZPVS)](0020-zos-pli-test-suite.md)、および本提案 (ZHVS) により、Zeus の主要 3 言語すべてが同一の `zeus-ccvs` レポート基盤で検証可能になる。

| スイート | 言語 | ディレクトリ | モジュール | 対応ツールチェーン (`--toolchain`) |
|---|---|---|---|---|
| **CCVS85** | COBOL-85 (NIST) | `work/suite-zeus` | `NC`, `SM`, `IC`, `SQ`, `RL`, `IX`, `ST`, `IF`, `SG`, `OB`, `DB`, `RW` | `zeus`, `zeus-native`, `gnucobol` |
| **ZCVS** ([0019](0019-zos-cobol-test-suite.md)) | z/OS Enterprise COBOL | `zcvs/` | `ZD`, `ZA`, `ZN`, `ZP`, `ZF`, `ZS`, `ZX`, `ZE` | `zeus`, `zeus-native`, `gnucobol` |
| **ZPVS** ([0020](0020-zos-pli-test-suite.md)) | z/OS Enterprise PL/I | `zpvs/` | `PD`, `PA`, `PC`, `PS`, `PF`, `PM`, `PE` | `zpli`, `zpli-native`, `ironspring` |
| **ZHVS** (本提案) | z/OS HLASM | `zhvs/` | `HA`, `HM`, `HI`, `HD`, `HF`, `HS`, `HE` | `zas`, `zas-obj`, `zas-goff` |

### 段階的ロードマップ (`H0`〜`H3`)

| フェーズ | 内容 | 目標規模 (目安) | 完了条件 |
|---|---|---|---|
| **H0: 共通マクロとハーネス対応** | `zhvs/copy/` (`ZHVSHDR`, `ZHVSCHK`, `ZHVSSUM`) の作成、`zeus-ccvs` への `zas` / `zas-obj` / `zas-goff` ツールチェーン追加 | マクロ 3 本 + ハーネス拡張 | `zeus-ccvs run --suite zhvs --toolchain zas` が動作しレポート集計が出力される |
| **H1: 言語・マクロ・整数・10進 (`HA`, `HM`, `HI`, `HD`)** | 式・属性・`USING`・マクロ・32/64 ビット整数・相対・マスク・原子操作・文字・パック 10 進・`ED`/`EDMK` | 18 プログラム / 300+ テスト項目 | `HA`, `HM`, `HI`, `HD` が `zas` / `zas-obj` / `zas-goff` で FAIL 0 |
| **H2: 浮動小数点 (`HF`)** | HFP / BFP / DFP 全形式、量子保持、パック/ゾーン変換、`PFPO` 54 組、`FPC` 制御 | 8 プログラム / 200+ テスト項目 | `HF` が `.zo` および `OBJ`/`GOFF` ラウンドトリップで FAIL 0 |
| **H3: サービス・リンケージ・負系 (`HS`, `HE`)** | `AMODE 24/31/64`、`GETMAIN`、`LOAD`/`LINK`/`XCTL`、`DCB` I/O、LE 条件サービス、負系診断 (`HE`) | 20+ プログラム / 200+ テスト項目 | 全モジュール (`HA`, `HM`, `HI`, `HD`, `HF`, `HS`, `HE`) が CI で全件合格 |

## 実装状況 (2026-10-20)

`zhvs/` に全7モジュール、主プログラム125本（実行85本/537判定、負系20本、正常対照20本）を実装しました。

[実行方法・対応範囲](../../zhvs/README.md) / [全シナリオ台帳](../../zhvs/SCENARIOS.md) / [言語横断の検証](../validation-suites.md)。提案本文は設計範囲を示し、各小項目の完全網羅を意味しません。現時点の実測可能な範囲は台帳の判定IDで管理します。

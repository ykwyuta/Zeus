# Zeus

Zeus は、z/OS 向けアプリケーションのソースを Linux／Windows 上でコンパイル・実行する、Rust 製のアプリケーション実行環境です。COBOL、PL/I、HLASM、JCL、データセット、CICS、IMS、SQL、MQ を組み合わせたバッチ処理とオンライン処理を扱います。

**z/OS 全体を動かすエミュレーターではありません。** ソースを Zeus 用のモジュールへコンパイルする方式です。COBOL の主な対象は IBM Enterprise COBOL **4.2** で、一部の後続版の構文も追加しています。全構文・全設定・実行結果の完全互換は保証していません。

- 初めて使う: [導入](#導入) → [COBOL を動かす](#cobol-を動かす)
- 既存アプリを調べる: [依存関係・レイアウト・カバレッジ](#依存関係レイアウトカバレッジ)
- 移植可否を判断する: [対応機能](#何ができるか) → [互換性と制約](#互換性と制約)

## 何ができるか

| 用途 | 実装していること | コマンド・詳細 |
|---|---|---|
| COBOL | 固定形式ソース、COPY/REPLACE、データ配置、演算・制御文、順次／相対／索引ファイル、SORT/MERGE、LOCAL-STORAGE、再帰 CALL、国別・DBCS データ | `zcobc` / [対応一覧](docs/proposal/0014-cobol-ibm-conformance.md) |
| PL/I | 手続きと再帰、構造体・配列、BASED/DEFINED、PICTURE、ON-unit、ストリーム／レコード I/O、VSAM KSDS、COBOL との相互呼び出し | `zpli` / [対応範囲](docs/proposal/0004-pli-compiler.md) |
| HLASM | アプリケーション向けアセンブラー、条件付きマクロ、整数・文字・10進演算、再配置、モジュール間リンク、COBOL/PL/I からの呼び出し | `zas` / [対応範囲](docs/proposal/0012-hlasm.md) |
| JCL | JOB/EXEC/DD、PROC/INCLUDE、シンボル置換、COND/IF、DISP、GDG、一時データセット、連結、コンパイル・リンク・実行 | `zjob` / [ガイド](docs/help/zjob.txt) |
| バッチユーティリティ | IEFBR14、IEBGENER、IDCAMS、SORT、IKJEFT01、DSN RUN PROGRAM などのアプリケーション向け機能 | `zjob` / [実装と残件](docs/proposal/0007-remaining-work.md) |
| データセット・テープ | ホストファイルによる順次／相対／キー付きデータ、代替索引、カタログ。COBOL の物理ブロック、RESERVE、AWSTAPE 仮想テープ | `zjob catalog` / [媒体の形式と制約](docs/proposal/0016-cobol-physical-tape-recursion.md) |
| CICS・3270 | EXEC CICS、BMS、COMMAREA、チャネル／コンテナー、ファイル更新・回復、スクリプト駆動、TN3270/TN3270E、TLS、プリンター | `zcics`、`zbms` / [ガイド](docs/help/zcics.txt) |
| SQL | EXEC SQL、ホスト変数・標識変数、カーソル、SQLCA、コミット／ロールバック。Db2 または PostgreSQL を使用 | [外部サービス](#外部サービス) |
| IBM MQ | MQI 呼び出し、メッセージ送受信、コミット／バックアウト、CICS のトリガー監視 | `zmq` / [対応範囲](docs/proposal/0005-ibm-mq.md) |
| IMS | DBD/PSB、階層データベース、DL/I、GSAM、WAL、バッチ／メッセージ処理リージョン、TM キュー | `zims`、`zjob ims` / [対応範囲](docs/proposal/0006-ims-db.md) |
| REST API | z/OS Connect プロジェクトの OpenAPI と要求・応答マッピングを読み、CICS のプログラム／IMS トランザクションを呼び出す | `zconn` / [対応範囲](docs/proposal/0008-zconnect.md) |
| 調査・検証 | 依存関係と未対応構文の CSV、固定長レイアウトの COPY 句、文・分岐のカバレッジ | `zdeps`、`zlayout`、`zcov` |
| 単独実行ファイル | ランタイムとモジュールを同梱し、Zeus のコマンドを別途インストールせずに実行 | `zcobc compile --native`、`zpli compile --native` |

「実装」は、各製品・言語の全機能に対応するという意味ではありません。対応する形式・設定・制約は各リンク先で確認できます。

## 導入

### ビルド済みコマンド

[GitHub Actions の CI](https://github.com/ykwyuta/Zeus/actions/workflows/ci.yml) の成功した実行から、次の artifact を取得できます。

| artifact | 対象 |
|---|---|
| `zeus-linux-x86_64` | Linux x86-64。Ubuntu 22.04 上でビルド |
| `zeus-windows-x86_64` | Windows x86-64 |

展開した `bin` を PATH に追加してください。`zcobc`、`zpli`、`zas`、`zjob`、`zcics`、`zbms`、`zims`、`zmq`、`zconn`、`zdeps`、`zlayout`、`zcov`、`zeus-rt` が含まれます。単独実行ファイルを作る場合は、`zeus-rt` をコンパイラーと同じディレクトリーに置きます。

### ソースからビルド

Rust の stable ツールチェーンと Cargo を用意し、リポジトリーのルートで実行します。宣言上の最小 Rust バージョンは `Cargo.toml` を参照してください。

```sh
cargo build --release --workspace --bins
```

コマンドは `target/release` に生成されます。Linux の PATH 設定例:

```sh
export PATH="$PWD/target/release:$PATH"
```

Windows PowerShell の場合:

```powershell
$env:Path = "$(Resolve-Path .\target\release);$env:Path"
```

基本的な COBOL 実行に IBM コンパイラー、GnuCOBOL、z/OS は不要です。SQL や MQ を使う場合は、それぞれの外部サービスが必要です。

## COBOL を動かす

次の内容を UTF-8 の `hello.cbl` として保存します。固定形式のため、各行の先頭 **7 文字は空白** にしてください。

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
       PROCEDURE DIVISION.
           DISPLAY 'HELLO FROM ZEUS'
           STOP RUN.
```

```sh
zcobc check hello.cbl
zcobc compile -o hello.zo hello.cbl
zcobc run hello.zo
```

`HELLO FROM ZEUS` と表示されます。`.zo` は Zeus 用のコンパイル済みモジュールです。

COPY ライブラリー、DD 割り当て、実行パラメーターがある場合:

```sh
zcobc compile -I copybooks -o report.zo src/report.cbl
zcobc run --dd INPUT=input.dat --parm NIGHTLY report.zo
```

副プログラムは別途コンパイルし、主プログラムのモジュールと同じディレクトリーなど、検索対象の場所に置きます。EXEC CICS／SQL／DLI を実行するには、対応するリージョンやサービスの環境が必要です。

### 単独実行ファイル

```sh
zcobc compile --native -o hello hello.cbl
./hello
```

Windows では `-o hello.exe` とし、`.\hello.exe` で実行します。副プログラムは `--with helper.zo` で同梱できます。生成物にも、プログラムが使うデータファイルや SQL／MQ サービスは必要です。

COBOL の対応する処理は Cranelift でホスト CPU の機械語にし、その他はランタイムの実装を呼び出します。PL/I と再帰 COBOL はインタープリターで実行します。`--target` による別プラットフォーム向けの作成には、その対象用の `zeus-rt` が必要です。詳細は [単独実行ファイル](docs/proposal/0009-native-executables.md) を参照してください。

## JCL・データセットを使う

```sh
zjob --home demo catalog import APP.INPUT input.txt --recfm FB --lrecl 80 --text
zjob --home demo catalog define-pds APP.LOAD --recfm U
zjob --home demo catalog put "APP.LOAD(REPORT)" report.zo
zjob --home demo submit nightly.jcl
```

カタログは `demo/catalog`、ジョブログと SYSOUT は `demo/spool` に保存されます。`--home` を省略すると `ZEUS_HOME`、それもなければ `./zeus` を使います。JCL の `STEPLIB`／`JOBLIB`、または `--loadlib` でプログラムの検索先を指定します。

データは EBCDIC と z/OS の配置を保持します。**普通のテキストファイルをそのまま固定長データセットとして扱うことはできません。** 上の `--text` はホストテキストを EBCDIC レコードへ変換します。`catalog import` と `catalog put` は既存データ／メンバーを置き換えます。

### 物理ブロックと仮想テープ

COBOL では `RECORDING MODE F/V/S/U`、`BLOCK CONTAINS`、`APPLY WRITE-ONLY`、`RESERVE`／DD `BUFNO` に対応しています。V は BDW/RDW、S は SDW を使ってブロックにまとめ、読み取り時に論理レコードを復元します。

テープの実体はホスト上の非圧縮 AWSTAPE イメージです。`.aws` のパス、または JCL の `UNIT=TAPE/3480` などで選択し、標準ラベル、テープマーク、複数の論理ファイル、NO REWIND、固定長 REVERSED を扱います。実テープ装置を直接操作する機能ではありません。

テープでは32760バイトを超えるLBIブロックと拡張BDW、JCLの`BLKSZLIM`に対応しています。`BLOCK CONTAINS 0`の自動選択は仮想テープ上限256 KiBの範囲で行います。HDR2のないVSE形式の入力もFDの形式で読み取ります。`EXPDT`／`RETPD`は標準ラベルへ保存され、期限内媒体の上書きや保護付きデータへのアクセスにはホスト側の認可が必要です。組込み利用時は`Machine::set_tape_authorizer`で接続します。詳細は[追加互換性一覧](docs/proposal/0018-cobol-compatibility-completion.md)を参照してください。

ASSIGNの`label-S-DD名`等はDD名部分で割当を解決します。DDが未割当なら、同名の環境変数の`PATH(絶対パス)`、または`DSN(完全名)`をOPEN時に評価します。動的DSNはOLD／SHR、順次データセットのNEW／MOD、既存PDS内のメンバーを扱い、KEEP／DELETE／CATALOG／UNCATALOGを割当解除時に実行します。単独実行のカタログは`$ZEUS_HOME/catalog`（既定`./zeus/catalog`）です。SPACE等の追加割当属性は未対応で、状態98になります。環境変数によるRERUNのチェックポイント割当にも対応します。

仮想テープは複数巻に対応します。`IoAttributes.volume_capacity` で巻容量、`volumes` で巻のパスを指定できます。省略時の継続巻名は `名前.vol002.aws` などです。標準ラベルはEOV/HDRで接続し、Zeusで保存した巻集合は `名前.aws.volumes.json` も保持します。この情報でファイル終端と継続巻を区別し、複数ファイルで容量を共有します。`CLOSE REEL/UNIT` はファイルを開いたまま巻を切り替え、REEL/UNITのユーザーラベルを物理巻へ保存します。ラージブロック、全ラベル属性の照合などには残件があります。物理イメージはCOBOL VMと対応APIが対象です。詳細は [媒体の利用範囲](docs/proposal/0016-cobol-physical-tape-recursion.md) と [追加対応状況](docs/proposal/0018-cobol-compatibility-completion.md) を参照してください。

コンパイラー設定の `AWO/NOAWO` は可変長ブロックの充填規則、`BLOCK0/NOBLOCK0` は省略時のブロック化、`APOST/QUOTE` は定数QUOTE/QUOTESの文字を切り替えます。明示したBLOCK CONTAINSはBLOCK0より優先し、U形式にはBLOCK0を適用しません。

`CBL DYNAM`はCOBOLのリテラルCALLを動的ロードへ切り替えます。`CANCEL`すると内部ファイルを閉じ、ENTRY別名を含むロード状態を解除し、次のCALLで再ロードします。EXTERNAL領域は保持します。既定の`NODYNAM`では利用可能なCOBOLの静的依存先を実行前に結合し、CANCELで取り消しません。CALL identifierはどちらの設定でも動的です。SET TO ENTRYのリテラル指定も同じ設定に従います。CICSとDYNAMの組合せは診断します。
従来の実装でCANCELによりリテラルCALL先を初期化していたソースは、`DYNAM`を明示して再コンパイルしてください。

`>>CALLINTERFACE STATIC`／`DYNAMIC`／`DLL`（略記`>>CALLINT`）で、後続のCALLとSET TO ENTRYの結合方式を切り替えられます。引数なしの指令でCBL/PROCESSの設定へ戻ります。COPY内でも使用でき、REPLACEの対象にはなりません。同じコンパイル単位で同じリテラル呼出先に方式を混在させると、最初の方式へ統一して警告を出します。CLIのcompile/checkとJCLはRC=4を返し、成果物は生成します。CICSでも個別のDYNAMIC指定は使用できます。DLLでは公開されたPROGRAM-ID／ENTRYを解決します。`EXPORTALL`で公開し、保存時に生成する`.zo.x`（拡張子のないロードライブラリメンバーは`.x`）側ファイルをモジュールと一緒に配置します。JCLバインダーはSYSDEFSDにも側ファイルを出力できます。側ファイルはZeus独自形式です。DLL呼出先の複数ENTRYはWORKING-STORAGEを共有し、CANCELでは解放しません。

コンパイラーリストは`zcobc compile --options 'MAP,LIST' --listing main.lst main.cbl`で保存できます。SOURCEは元のソースとCOPY/REPLACE適用後のトークン、MAPはデータ配置・手続き、LISTはZIR命令と初期領域、OFFSETはソース行とZIR命令位置、XREFは解決済みデータ・手続き・ファイル参照とリテラル呼出し先、VBREFは動詞の参照行を出力します。各NO指定で抑止でき、コンパイル失敗時も診断を保存します。JCLのIGYCRCTLはPARMを同じ規則で解析し、SYSPRINTへ出力します。ソースのCBL/PROCESSは呼出し時の`--options`／PARMより優先します。リスト上のPCはZeusの命令番号です。

`OPTIMIZE(0/1/2)`はネイティブ生成の最適化方針に反映します。4.2形式の`NOOPTIMIZE`、`OPTIMIZE(STD/FULL)`も指定できます。条件付きコンパイルでは`IGY-CICS`、`IGY-DYNAM`、`IGY-SQL`、`IGY-OPTIMIZE`等の定義済み変数と旧名を使えます。`IGY-COMPILER-VRM`はZeus自身の版番号、`IGY-LP`は仮想COBOL ABIの32を返します。IBMのCPU世代に対応する`IGY-ARCH`は定義しません。

`FLAG(W)`はW以上の診断をリスト末尾に出力し、`FLAG(W,S)`はS以上を該当ソース行にも埋め込みます。既定は`FLAG(I,I)`です。`NOFLAG`はリストの診断表示を抑止しますが、コンパイルの成否や戻り値は変えません。コンパイラー設定自体のエラーは抑止しません。2段階指定にはSOURCEが必要です。

`RERUN ON CKPT EVERY 100 RECORDS OF F`、巻末、SORTのチェックポイントを保存し、`zcobc restart step.ckpt` で再開できます。通常実行時に `--dd CKPT=step.ckpt` を割り当てます。CALL/PERFORMの復帰先、データ領域、開いているファイルの内容と位置も保存します。形式はZeus独自で、外部トランザクション環境や対話コンソールを含む実行は診断します。既に出力した標準出力は取り消しません。

JCLでチェックポイントDDを`DISP=MOD`にすると複数の保存位置を保持します。`zcobc checkpoints step.ckpt`で識別番号を確認し、`zcobc restart --checkpoint 2 step.ckpt`で位置を選択できます。番号省略時は最新の位置から再開します。

`CBL SSRANGE`は表の実効アドレス、可変長項目、参照変更の範囲を実行時に検査します。既定は`NOSSRANGE`です。4.2相当の`CHECK(OFF)`は`zcobc run --check-range OFF main.zo`またはAPIの`Machine::set_range_checking(false)`で指定できます。NATIONALの参照変更はUTF-16単位で検査します。

## 文字コード

`THREAD`はRECURSIVEを必須とし、Rust APIの`vm::threaded::ThreadedRunUnit`から複数ホストスレッドで同じ実行単位を利用できます。呼出しは専用スレッドで直列化し、WORKING-STORAGE、EXTERNAL、ファイル状態を共有します。LOCAL-STORAGEと再帰の特殊レジスターは呼出しごとに分離します。ホストの呼出しをjoinした後に`finish()`を呼ぶと、開いたファイルの保存と実行単位の終了を待機できます。ホストのLE/POSIX ABIを提供するものではありません。

`LP(64)`は6.x仕様を補足したZeusの仮想ABIです。POINTER、FUNCTION-POINTER、INDEXを8バイト、POINTER-32を4バイトで配置します。PROCEDURE-POINTERはどちらのLPでも8バイトです。LENGTH OFの暗黙定義と引数幅も切り替え、DLLの異なるLP間の結合を診断します。LP(64)と再帰の呼出し別領域は現在、単独実行ファイル内でもインタープリターで実行します。

ファイルのSORT/MERGEは`SORT-CONTROL`、`SORT-CORE-SIZE`、`SORT-FILE-SIZE`、`SORT-MODE-SIZE`、`SORT-MESSAGE`と`SORT-RETURN`を使えます。制御DDの`OPTION MAINSIZE/RESINV/FILSZ/MSGDDN`と`SMS`で設定を上書きできます。小さい作業領域では一時ファイルを使う安定マージソートを行います。既定の作業領域予算は64MiBですが、入力データセットと最終結果はVM内に保持するため、プロセス全体のメモリー上限ではありません。`SORT-RETURN`を16にすると次のRELEASE/RETURNで打ち切ります。制御DDの`CKPT`／`CHKPT`と`SORTCKPT`による保存・再開にも対応します。制御キーワードの対応範囲とIBMとの差は[追加互換性一覧](docs/proposal/0018-cobol-compatibility-completion.md)に記載しています。

既定は EBCDIC CCSID 37 です。`ZEUS_CCSID`、または対応するコマンドの `--ccsid` で、欧州・アラビア語・ヘブライ語・タイ語等のSBCSと、日本語・韓国語・中国語の混在コードページを含む54種類を選択できます。対応番号は[コードページ一覧](crates/zeus-cobol/codepages/README.md)に記載しています。COBOLソースの`CBL CODEPAGE(939)`（短縮形`CP`）はコンパイル単位のリテラルと文字変換に反映され、別コンパイルしたCALL先ではそのプログラムの指定を使用します。実行時の明示的な`--ccsid`／`ZEUS_CCSID`はこれを上書きします。

ソース入力の既定はUTF-8です。EBCDICの行区切りファイルには`--source-encoding ebcdic`、改行を持たない固定80バイトレコードには`--source-encoding ebcdic80`を指定します。compile/check/inventoryに共通で、外部COPYも同じ形式として読みます。文字の対応はCODEPAGE設定に従い、DBCSの2バイト文字とSO/SIを含む元のバイト桁位置で固定形式を処理します。UTF-8のBOMも受理し、不正なUTF-8やプログラム領域の不正なEBCDICは診断します。

```sh
zcobc compile --source-encoding ebcdic80 --options 'CP(939)' --listing main.lst -o main.zo main.cbl
```

JCLのIGYCRCTLは、データセットのSYSINとSYSLIBの元のレコードを保持してCODEPAGEに従って読み込みます。インストリームとTEXT形式のSYSINはUTF-8として扱い、SYSLIB内のUTF-8ファイルとEBCDICメンバーも個別に扱います。コンパイラーリストには復号後のソースを表示します。

- PIC G／DISPLAY-1 と G リテラルは DBCS データを保持します。
- PIC N／USAGE NATIONAL と N/NX リテラルは、既定では UTF-16 の国別データです。
- `zcobc compile --nsymbol dbcs`、またはソースの `CBL NSYMBOL(DBCS)` で N の解釈を DBCS に切り替えます。
- `CODE-SET` による ASCII／EBCDIC ファイル変換は、コンパイラーの`CODEPAGE`と独立し、Zeusの既定のファイル変換表（ASCII／CCSID 37）を使います。全54種類のコンパイルCCSIDと併用できます。

日本語・国別データを使う場合も、対応するデータカテゴリーや文の全形式を保証するものではありません。[COBOL 対応一覧](docs/proposal/0014-cobol-ibm-conformance.md) で確認してください。

## 外部サービス

| 機能 | 必要なもの・設定 |
|---|---|
| Db2 | Db2 サーバー。`ZEUS_DB2=db2://user:password@host:port/DB` で指定。Zeus の DRDA クライアントを使用 |
| PostgreSQL | PostgreSQL サーバー。`ZEUS_DB2=postgres://user:password@host:port/db` で指定。対応 SQL を変換して実行 |
| IBM MQ | MQ キューマネージャーと IBM MQ C クライアントライブラリー。`ZEUS_MQ=mq://user:password@host:port/QMGR/CHANNEL` で指定 |
| 3270 端末 | スクリプト実行は端末エミュレーター不要。対話接続には x3270 などの TN3270 対応端末を使用 |
| IMS DB/TM | Zeus 内のデータベースとキューを使用。アプリケーションの DBD/PSB、プログラム、リージョン設定が必要 |

開発用サービスの起動スクリプトは `tools/db2ce.sh`、`tools/pg.sh`、`tools/mq.sh` です。コンテナー実行環境などを別途必要とし、サーバー製品を Zeus に同梱するものではありません。

## サンプルで試す

CardDemo、GenApp、Bank of Z の取得・構築・シナリオ実行用スクリプトを用意しています。以下の `.sh` はリポジトリーのルートから Linux のシェル環境で実行します。Windows のコマンド配布とは別に、スクリプト用の実行環境が必要です。

| サンプル | 入口 | 確認対象 |
|---|---|---|
| CardDemo バッチ | `tools/carddemo.sh all` | 初期化、日次処理、帳票。POSTTRAN はサンプル取引の拒否による RC 4 を期待し、他のジョブは RC 0 |
| CardDemo オンライン | バッチ構築後 `tools/carddemo-online.sh test` | サインオン、口座・カード・取引、支払い、ユーザー管理。`serve` で TN3270 接続 |
| CardDemo の追加機能 | `tools/carddemo-db2.sh test`、`tools/carddemo-ims.sh test`、`tools/carddemo-mq.sh test` | Db2、IMS、MQ のシナリオ。必要な接続先を先に設定 |
| GenApp | `tools/genapp.sh test` | 顧客・保険契約の CICS/Db2/VSAM 処理。`ZEUS_DB2` が必要 |
| Bank of Z | `tools/bankofz.sh test` | 顧客・口座、送金、CICS、REST API。`ZEUS_DB2` が必要 |
| Bank of Z の IMS 経路 | `tools/bankofz-ims.sh test` | DB ロード、メッセージ処理、COBOL/PL/I と DL/I |

構築スクリプトは作業用データを初期化します。例えば CardDemo の setup は既定の `work/carddemo` を作り直します。既存の業務データを置いたディレクトリーを `ZEUS_HOME` に指定しないでください。

記録済みの [構文使用・コンパイル一覧](docs/inventory/apps.md) では、CardDemo 41/41、GenApp 31/31、Bank of Z 42/43、計 **114/115 プログラム** がコンパイルできています。Bank of Z の IBTRAN は未対応です。固定した取得コミットでの記録であり、全シナリオの実行成功や、現在の作業ツリーでの再検証結果を示す数字ではありません。

## 依存関係・レイアウト・カバレッジ

```sh
zdeps analyze --lib copybooks --out deps src
zlayout -I copybooks --out layouts src/report.cbl
zcobc run --coverage coverage report.zo
zcov report --detail --lcov coverage.lcov --html coverage-html coverage
```

- **`zdeps`** は COPY/INCLUDE、呼び出し、JCL、CICS、IMS などの関係を追い、CSV を出力します。既定ではコンパイルも行い、未対応構文、位置、一般のコンパイル診断を分けて報告します。移植調査の入口であり、実行互換性の合格判定ではありません。
- **`zlayout`** はファイル、引数、COMMAREA、MQ、BMS、SQL などの固定長レイアウトを COPY 句として出力し、コンパイルして配置を検査します。
- **`zcov`** は実行時の文・分岐・段落／手続きの通過情報から、集計、LCOV、HTML を生成します。`ZEUS_COVERAGE` で `zjob`、`zcics` や単独実行ファイルからも記録できます。

詳細: [依存関係](docs/proposal/0011-dependency-analysis.md) / [レイアウト](docs/proposal/0013-fixed-length-layouts.md) / [カバレッジ](docs/proposal/0010-coverage.md)。

## 互換性と制約

COBOL-85 の主要な文と z/OS のデータ表現には広く対応しています。再帰 CALL、CALL RETURNING、手続き・関数ポインターによる間接 CALL、NATIONAL の固定小数点数・編集項目・グループ、VSAM 詳細状態に対応しています。WHEN-COMPILED はコンパイル日時をモジュールに保存します。

| 項目 | 現在の範囲 |
|---|---|
| IBM COBOL の言語全体 | OO/Java、XML、DATEPROC、特殊なデータカテゴリーなどに未対応・部分対応あり |
| コンパイラー設定 | NSYMBOL、CICS、SQL、TRUNC、ARITH、NUMPROC、APOST、AWO、BLOCK0、SSRANGE、CODEPAGE、DYNAM、OPTIMIZEと各NO指定に対応。条件付きコンパイル・定義済み変数、SOURCE/MAP/LIST/OFFSET/XREF/VBREFのZeus形式のリストを提供。DLL/EXPORTALLとDLL側ファイル、THREADの共有実行単位、LP(64)の仮想ABIにも対応。ARCH・SQLIMS等には残件があり、未対応指定を診断。EXEC SQLIMSをDb2へ転送する処理も廃止し、未実装として診断 |
| 呼び出し規約 | RECURSIVE、PROCEDURE DIVISION RETURNING と結果転送、仮想エントリーへの間接CALLに対応。IBMのロードモジュール・実アドレスABIの直接実行は対象外 |
| ファイル・テープ | 物理ブロック、標準ラベル／ラベルなしの複数巻、REEL/UNITラベル、RERUN再始動に対応。全媒体属性の照合などには残件あり |
| アセンブラー | 対応するアプリケーション命令を Zeus の実行エンジンで処理。IBM オブジェクトデック／ロードモジュールの直接実行、HLASM のホスト CPU 向けネイティブ生成は対象外 |
| システム環境 | OS、JES、CICS、Db2、IMS、各種ユーティリティの全機能を置き換えるものではなく、実装したアプリケーション向けの機能を提供 |

「構文を受理する」「実行できる」「IBM と同じ結果になる」は別の確認です。次の一覧では実装済み・部分対応・未対応・未検証を区別しています。

- [IBM COBOL の構文・実行対応一覧](docs/proposal/0014-cobol-ibm-conformance.md)
- [句・設定の読み飛ばし監査](docs/proposal/0015-cobol-ignored-clauses.md)
- [RESERVE・物理ブロック・仮想テープ・再帰 CALL](docs/proposal/0016-cobol-physical-tape-recursion.md)
- [戻り値・TRUNC・NATIONAL・ポインターなどの追加実装と残件](docs/proposal/0017-cobol-remaining-compatibility.md)

NIST CCVS85 の必須 8 モジュールと一部の任意モジュールの合格実績は [コンパイラーの進捗](docs/proposal/0003-cobol-compiler.md#進捗-2026-10) に記録しています。直近の追加対応を含むローカル全体試験は **970 件成功（2026-10-16）** でした。CCVS85 の再実行や IBM 実機との差分実行を示すものではなく、IBM 方言の完全互換の証明ではありません。

## 詳しい使い方と開発

引数・既定値・環境変数・終了コードは各コマンドの `--help` を参照してください。例えば `zcobc compile --help`、`zjob catalog --help`、`zcics --help` が使えます。主要コマンドの詳細ヘルプは実行ファイルに組み込まれ、オフラインでも読めます。元のガイドは [docs/help](docs/help) にあります。

| 読みたい内容 | 文書 |
|---|---|
| コンパイル・実行・DD | [zcobc](docs/help/zcobc.txt)、[zpli](docs/help/zpli.txt)、[zas](docs/help/zas.txt)、[zjob](docs/help/zjob.txt) |
| オンライン・API・メッセージ | [zcics](docs/help/zcics.txt)、[zbms](docs/help/zbms.txt)、[zconn](docs/help/zconn.txt)、[zims](docs/help/zims.txt)、[zmq](docs/help/zmq.txt) |
| 調査・実行結果の分析 | [zdeps](docs/help/zdeps.txt)、[zcov](docs/help/zcov.txt)、[レイアウト](docs/proposal/0013-fixed-length-layouts.md) |
| 設計判断・実装計画 | [docs/proposal](docs/proposal) |
| ビルド・試験・クリーンルーム方針 | [CONTRIBUTING.md](CONTRIBUTING.md)、[CCVS85](ccvs85/README.md) |

コードは `crates/zeus-cobol`、`zeus-pli`、`zeus-ds`、`zeus-jcl`、`zeus-cics` など、機能ごとの Rust crate に分かれています。開発時の基本検査:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

ライセンスは [GPL-3.0-or-later](LICENSE) です。

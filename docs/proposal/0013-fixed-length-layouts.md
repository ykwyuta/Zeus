# 0013: 入出力の固定長定義をコピーブックとして出力する (提案)

COBOL プログラムがやり取りするデータ (ファイル、カード、MQ のメッセージ、Db2 のテーブル、検索結果、サブルーチンの
引数、BMS の画面) について、その **固定長のレイアウト** を COBOL のコピーブックの形式で書き出す。COPY は展開した
状態で解析し、1 つのデータに複数のレイアウトがあるとき (マルチレイアウト) はレイアウトごとに別のファイルにする。

用途: 移行先でのデータ変換 (EBCDIC・パック 10 進の変換定義)、テストデータの作成 (0012)、インターフェース仕様の
棚卸し、プログラム間でのレイアウトの食い違いの検出。

この文書は提案で、実装はまだ行わない。

## 対象とする入出力

| 種類 | 何をレイアウトとするか | 見つけ方 |
|---|---|---|
| ファイル | FD のレコード記述 (01)、`READ ... INTO` / `WRITE ... FROM` / `REWRITE ... FROM` の作業域、CICS の `READ / WRITE / REWRITE FILE() INTO / FROM`、`READNEXT` など | SELECT ASSIGN (DD 名) と FD、ファイル入出力文の INTO / FROM、EXEC CICS のファイル命令 |
| カード | SYSIN などの 80 桁のカード入力: `ACCEPT 項目 [FROM SYSIN]` の受取り項目、カード形式のファイル (ASSIGN が SYSIN・カードの DD、FD が 80 バイト) | ACCEPT 文、ASSIGN 名、JCL の `DD *` (0011 の zdeps の情報) |
| MQ | MQPUT / MQPUT1 / MQGET のメッセージバッファ (MQI の呼出しの引数の位置で決まる)。MQMD・MQOD などの標準構造は対象外 (指定で含める) | `CALL 'MQGET' USING ... BUFFER-LENGTH BUFFER ...` の引数の位置 |
| テーブル | Db2 のテーブルの列 (`EXEC SQL DECLARE ... TABLE`) と、それに対応するホスト構造 (DCLGEN の 01) | DECLARE TABLE の列定義、`EXEC SQL INCLUDE` されたメンバー |
| 検索結果 | `SELECT ... INTO`、`FETCH ... INTO` で受け取る 1 行の形。ホスト変数が別々の項目でも、選択リストの順に並べた 1 つのレコードとして構成する。標識変数 (NULL) も含める | EXEC SQL の文と、そのホスト変数・標識変数 |
| サブルーチンの引数 | 呼ばれる側: `PROCEDURE DIVISION USING` の LINKAGE 項目 (引数ごと)。呼ぶ側: `CALL ... USING` の実引数 (呼出し先・位置ごと)。CICS の COMMAREA (`DFHCOMMAREA`、`LINK / XCTL COMMAREA()`) と、IMS の PCB マスクもここに含める | PROCEDURE DIVISION USING、CALL 文、EXEC CICS LINK / XCTL / RETURN の COMMAREA |
| BMS | マップごとのシンボリック・マップ (入力 `...I` と出力 `...O`)。プログラムが `SEND / RECEIVE MAP() INTO / FROM` で使うもの | BMS のソース (zeus-bms でシンボリック・マップを作る) と、EXEC CICS のマップ命令 |

## 解析の前提

- **COPY は展開済み**: Zeus の COBOL フロントエンド (`zeus_cobol::front`) で、COPY・COPY REPLACING・REPLACE・
  入れ子の COPY をすべて展開してから解析する。各項目が元はどのファイルの何行目かは `DataEntry.pos` で分かる
  ので、出力に「出どころのコピーブック」を注記できる。
- **項目の位置と長さは意味解析の結果を使う**: `sema` が計算する記述子 (`Desc`: オフセット、長さ、型、OCCURS、
  ODO) を使い、SYNC の詰め物や COMP・COMP-3 の長さを含めて正しい位置を出す。自前で数え直さない。
- 呼出し先や MQ のキュー名、ファイルの DD 名は、0011 の依存関係分析 (zdeps) と同じ定数伝播 (VALUE・MOVE) で
  分かる範囲で名前に使う。

## マルチレイアウトの判定と分割

1 つの入出力に複数の形があるとき、レイアウトごとに 1 ファイルにする。

1. **FD の下の複数の 01**: 暗黙の再定義なので、01 ごとに 1 レイアウト。
2. **REDEFINES**: 大きさにかかわらず、すべての REDEFINES をレイアウトの分岐とみなし、
   「共通部 + 選んだ形」で 1 つの完全なレコードを作る (REDEFINES 句のないレコードにする)。例 (CardDemo の CVEXPORT):

   ```text
   EXPORT-RECORD = 共通部 (REC-TYPE, SEQUENCE-NUM, BRANCH-ID, REGION-CODE)
                 x EXPORT-TIMESTAMP (26) の形: EXPORT-TIMESTAMP-R (日付・区切り・時刻) の 1 つ
                 x EXPORT-RECORD-DATA (460) の形: 顧客・口座・取引・カード相互参照・カードの 5 つ
   -> 1 x 5 = 5 レイアウト
   ```

   分け方の規則:
   - **選択点**: 同じ領域を共有する項目の組 (元の項目と、それを REDEFINES する項目すべて) を 1 つの選択点とし、
     その中の 1 つを選ぶ。FD の下の複数の 01 も同じ扱いの選択点。
   - **元の項目も候補**: 元の項目そのもの (例: `EXPORT-TIMESTAMP PIC X(26)`) も形の 1 つとする。ただし、元の項目が
     基本項目の英数字 (`PIC X(n)`) で、集団項目に再定義されているときは、内訳のない入れ物なので候補から外す
     (CVEXPORT の `EXPORT-RECORD-DATA X(460)` と `EXPORT-TIMESTAMP X(26)`)。`--keep-base` で残す。
   - **入れ子**: ある形の中にさらに REDEFINES があれば、その形を選んだときだけ、その中の選択点を展開する (木構造)。
   - **組合せ**: 互いに独立な選択点は直積でレイアウトを作る。判別子 (下の 4) が分かる選択点は、判別子の値ごとに
     1 つの形に対応づけ、ほかの選択点と掛け合わせる。
   - **上限**: レイアウトの数が上限 (`--max-layouts`、既定 256) を超えるときは、判別子のない選択点のうち領域の小さい
     ものから順に分割をやめ、元の REDEFINES のまま本文に残す。残した選択点はヘッダーに書き、黙って省かない。
3. **同じ領域を別の作業域で読む**: ファイルのレコード域を、場所によって別の 01 に `READ INTO` / `MOVE` している
   場合 (データの流れで追う)、それぞれを同じファイルのレイアウトとする。
4. **判別子 (レコード種別) の推定**: 各レイアウトがどの値で選ばれるかを、次から推定してヘッダーに書く。
   - 読む側: `EVALUATE 種別項目 WHEN 'C' ...` / `IF 種別項目 = 'C'` の分岐の先で、そのレイアウトの項目だけを使う。
   - 書く側: `MOVE 'C' TO 種別項目` と同じ段落で、そのレイアウトの項目に値を入れる (CBEXPORT は 'C' で顧客、
     'A' で口座)。
   - 88 レベル (`88 REC-CUSTOMER VALUE 'C'`)。
   - 推定できないときは「判別子不明」と書き、利用者が対応表を与えられるようにする (CSV)。

## 出力の形式

- **COBOL のコピーブック** (固定形式: 7 桁目が注記、8〜72 桁目が本文)。そのまま `COPY` してコンパイルできる。
- 1 レイアウト 1 ファイル。ファイル名は `<プログラム>.<種類>.<資源>[.<レイアウト>].cpy`:

  ```text
  CBEXPORT.FILE.EXPFILE.CUSTOMER.cpy
  CBEXPORT.FILE.EXPFILE.ACCOUNT.cpy
  COTRTLIC.SQLRESULT.C-TR-TYPE-FORWARD.cpy      (カーソルの FETCH の結果)
  COTRTLIC.TABLE.CARDDEMO.TRANSACTION_TYPE.cpy
  COSGN00C.BMS.COSGN00.COSGN0AI.cpy
  COSGN00C.BMS.COSGN00.COSGN0AO.cpy
  CBPAUP0C.CARD.SYSIN.cpy                       (ACCEPT PRM-INFO FROM SYSIN)
  COPAUA0C.MQ.W01-GET-BUFFER.cpy                 (MQGET の受取り域。キュー名は実行時に決まる)
  CSUTLDTC.PARM.1.cpy                            (呼ばれる側の第 1 引数)
  COTRN02C.CALL.CSUTLDTC.1.cpy                   (呼ぶ側の第 1 実引数)
  ```

  資源の名前は、ファイルは ASSIGN の DD 名、MQ は定数で分かるキュー名 (分からなければバッファの項目名)、
  テーブルは修飾付きの表名、検索結果はカーソル名 (SELECT INTO は段落名と行番号)。
  レイアウト名は、形が 2 つ以上ある選択点が 1 つだけなら、選んだ形の項目名から付ける (`EXPORT-CUSTOMER-DATA` →
  `CUSTOMER`)。複数の選択点の組合せになるときは連番 (`L001`、`L002`、…) とし、選んだ形の一覧はヘッダーと
  `layouts.csv` に書く (ファイル名が長くなりすぎないため)。
- **本文**:
  - 最上位を 01 にし、レベル番号は元のまま (相対関係を保つ)。各選択点では選んだ形だけを残し、REDEFINES 句を
    外して元の位置に並べる。出力されるレコードには REDEFINES が残らない (上限で分割をやめた選択点を除く)。
  - PIC・USAGE・SIGN・OCCURS (DEPENDING ON を含む)・SYNC・JUSTIFIED・BLANK WHEN ZERO・88 レベルを残す。
    VALUE はレイアウトに不要なので既定では外す (`--values` で残す)。
  - 項目名は元のまま (COPY REPLACING を適用した後の名前)。同じ名前の重複は修飾がなくても区別できるよう、
    必要なら接尾辞を付けて注記する。
  - 検索結果のように元が別々の項目のものは、新しい 01 (`<カーソル名>-ROW` など) の下に 05 で並べ直す。
    VARCHAR は 49 レベルの長さと本体、NULL 標識は直後の `PIC S9(4) COMP` として加える。
- **注記** (7 桁目の `*`):

  ```cobol
      *================================================================
      * PROGRAM   : CBEXPORT
      * KIND      : FILE (OUTPUT)  DD: EXPFILE  ORGANIZATION: SEQUENTIAL
      * LAYOUT    : 1 OF 5  CUSTOMER
      * SELECTED  : EXPORT-REC-TYPE = 'C'   (CBEXPORT 274: MOVE 'C' TO EXPORT-REC-TYPE)
      * CHOICES   : EXPORT-TIMESTAMP     -> EXPORT-TIMESTAMP-R
      *             EXPORT-RECORD-DATA   -> EXPORT-CUSTOMER-DATA
      * LENGTH    : 500 BYTES (FIXED)
      * SOURCE    : CVEXPORT.cpy (COPY IN CBEXPORT LINE 96)
      *================================================================
       01  EXPORT-RECORD.
           05  EXPORT-REC-TYPE                 PIC X(1).                00001
           05  EXPORT-TIMESTAMP-R.                                      00002
               10  EXPORT-DATE                 PIC X(10).               00002
               ...
           05  EXPORT-CUSTOMER-DATA.                                    00041
               10  EXP-CUST-ID                 PIC 9(09) COMP.          00041
               ...
  ```

  - 73〜80 桁目 (コンパイラが読まない一連番号の領域) に、各項目の **開始位置** (1 から数えたバイト位置) を入れる
    (`--no-offsets` で入れない)。注記の行を増やさずに位置が分かる。
  - 長さは可変長 (RECORD VARYING、ODO) なら最小と最大を書く。
- **索引**: 出力したファイルの一覧を `layouts.csv` (プログラム、種類、資源、レイアウト、判別子、長さ、ファイル名、
  出どころ) に書く。定義そのものではないので CSV でよい (0011 と同じ CSV の形式)。

## 正しさの確認

- 書き出したコピーブックを zcobc でコンパイルし、各項目のオフセット・長さ・型が元のプログラムの記述子と一致することを
  自動で確かめる (位置がずれたらエラー)。
- 同じファイル (DD・データセット) や同じテーブル、同じ呼出し先の引数を複数のプログラムが使うとき、レイアウトを
  比べて **食い違い** (長さ、項目の位置、型) を報告する。インターフェースの不一致の検出になる。
- BMS は、プログラムが COPY したシンボリック・マップと、BMS のソースから zeus-bms で作ったものを比べる。

## コマンドと組み込み (案)

```text
zlayout [-I DIR]... [--out DIR] [--kinds file,card,mq,table,sqlresult,parm,call,bms]
        [--values] [--no-offsets] [--selectors FILE.csv] PROGRAM.cbl...
```

- 新しいクレート `crates/zeus-layout` (`zlayout`)。COBOL フロントエンドと意味解析 (zeus-cobol) を使い、入出力の
  検出は ZIR の命令 (`Read`・`Write`・`Call`・`Exec`) と AST から行う。
- 0011 の zdeps から `--layouts` で呼び、依存関係のノード (ファイル、テーブル、プログラム) の属性に、レイアウトの
  ファイル名を加えられるようにする。

## 段階計画

| 段階 | 内容 | 確認 |
|---|---|---|
| A | ファイル (FD・INTO / FROM)、カード (ACCEPT・SYSIN)、サブルーチンの引数。COPY 展開、出力の形式、73〜80 桁の位置、再コンパイルでの確認 | CardDemo のバッチ (CBACT01C〜04C、CBTRN02C、CBPAUP0C)、CSUTLDTC とその呼出し元 |
| B | マルチレイアウト (FD の複数 01、すべての REDEFINES の選択点と組合せ、上限、判別子の推定) | CVEXPORT (CBEXPORT・CBIMPORT)、CardDemo の取引ファイル |
| C | テーブル・検索結果 (DECLARE TABLE、SELECT / FETCH INTO、VARCHAR、標識変数)、BMS、CICS の COMMAREA・ファイル | CardDemo の Db2 モジュール (COTRTLIC)、オンライン (COSGN00C)、GenApp |
| D | MQ のバッファ、IMS のセグメントと PCB、プログラム間の食い違いの報告、PL/I (同じ形式で COBOL のコピーブックとして出す) | CardDemo の MQ モジュール (COPAUA0C、COACCT01)、Bank of Z |

## 制限

- 動的に決まる長さ (可変長レコードの実際の長さ、`MQGET` の受取り長) は定義の最大長で出す。
- 判別子はソースの分岐と代入から推定するので、外部の仕様でしか分からない場合は利用者が対応表を与える必要がある。
- 同じ領域を参照修飾 (`REC(41:460)`) で直接切り出すような使い方は、項目の定義がないため、位置と長さの注記だけを出す。
- POINTER を通したレイアウト (SET ADDRESS OF で別の構造に重ねる) は、重ねた構造ごとに別レイアウトとして出す。

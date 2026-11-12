# ZHVS 網羅台帳 (HLASM と z/Architecture)

HLASM Language Reference の章立てと、Zeus の命令の解読表 (machine.rs ほか) が知る命令を列挙した。命令の一覧は Principles of Operation の全命令ではなく、Zeus が知る命令である。 要素 659、使用あり 594、対象外 16、未網羅 49。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |
| --- | --- | --- | --- | --- | --- |
| Assembler instructions | *PROCESS | 未網羅 |  | - |  |
| Assembler instructions | ACONTROL | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | ADATA | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | AINSERT | 未網羅 |  | - | 未網羅: Zeus に無い (2026-11-12 に確認) |
| Assembler instructions | ALIAS | 使用あり | HA102A | - |  |
| Assembler instructions | AMODE | 未網羅 |  | - | 未網羅: 結合時の属性。実行時に観測する検査 (AMODE なら LA の結果) をまだ書いていない |
| Assembler instructions | CATTR | 未網羅 |  | - | 未網羅: 結合時の属性。実行時に観測する検査 (AMODE なら LA の結果) をまだ書いていない |
| Assembler instructions | CCW | 使用あり | HA106A | - |  |
| Assembler instructions | CCW0 | 使用あり | HA107A | - |  |
| Assembler instructions | CCW1 | 使用あり | HA106A | - |  |
| Assembler instructions | CEJECT | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | CNOP | 使用あり | HA104A | - |  |
| Assembler instructions | COM | 未網羅 |  | - | 未網羅: Zeus に無い (2026-11-12 に確認) |
| Assembler instructions | COPY | 未網羅 |  | - |  |
| Assembler instructions | CSECT | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HA106A ほか 154 | - |  |
| Assembler instructions | CXD | 未網羅 |  | - | 未網羅: 外部ダミーセクション。Q/J 型の値は結合プログラムが決めるので、実機の値で確かめる |
| Assembler instructions | DC | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HA106A ほか 115 | - |  |
| Assembler instructions | DROP | 使用あり | HA105A, HE118N, HSS02A, HSS03A, HSS04A | - |  |
| Assembler instructions | DS | 使用あり | HA101A, HA102A, HA104A, HA105A, HA106A, HA107A ほか 151 | - |  |
| Assembler instructions | DSECT | 使用あり | HA104A, HM108A | - |  |
| Assembler instructions | DXD | 未網羅 |  | - | 未網羅: 外部ダミーセクション。Q/J 型の値は結合プログラムが決めるので、実機の値で確かめる |
| Assembler instructions | EJECT | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | END | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HA106A ほか 154 | - |  |
| Assembler instructions | ENTRY | 未網羅 |  | - |  |
| Assembler instructions | EQU | 使用あり | HA101A, HA102A, HA104A, HE120N, HE120P, HM108A | - |  |
| Assembler instructions | EXITCTL | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | EXTRN | 未網羅 |  | - |  |
| Assembler instructions | ICTL | 未網羅 |  | - |  |
| Assembler instructions | ISEQ | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | LOCTR | 使用あり | HA107A | - |  |
| Assembler instructions | LTORG | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HA106A ほか 113 | - |  |
| Assembler instructions | MNOTE | 使用あり | HE119N, HE119P, HM109A | - |  |
| Assembler instructions | OPSYN | 使用あり | HM104A | - |  |
| Assembler instructions | ORG | 使用あり | HA104A, HI110A, HI125A | - |  |
| Assembler instructions | POP | 使用あり | HA105A | - |  |
| Assembler instructions | PRINT | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | PUNCH | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | PUSH | 使用あり | HA105A | - |  |
| Assembler instructions | REPRO | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | RMODE | 未網羅 |  | - | 未網羅: 結合時の属性。実行時に観測する検査 (AMODE なら LA の結果) をまだ書いていない |
| Assembler instructions | RSECT | 未網羅 |  | - | 未網羅: Zeus は RSECT と CSECT を同じ OBJ に書けない (複数の制御セクションが要る) |
| Assembler instructions | SPACE | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | START | 使用あり | HA101A | - |  |
| Assembler instructions | TITLE | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Assembler instructions | USING | 使用あり | HA105A, HE101N, HE101P, HE102N, HE102P, HE103N ほか 38 | - |  |
| Assembler instructions | WXTRN | 使用あり | HA107A, HM108A | - |  |
| Assembler instructions | XATTR | 未網羅 |  | - | 未網羅: 結合時の属性。実行時に観測する検査 (AMODE なら LA の結果) をまだ書いていない |
| DC types | A | 使用あり | HA106A, HA107A, HS102A, HS103A, HS104A, HS105A | - |  |
| DC types | AD | 使用あり | HA106A | - |  |
| DC types | B | 使用あり | HA103A, HE111N, HE111P | - |  |
| DC types | C | 使用あり | HA103A, HA107A, HD101A, HI110A, HI111A, HI114A ほか 15 | - |  |
| DC types | CA | 使用あり | HA106A | - |  |
| DC types | CE | 使用あり | HA106A | - |  |
| DC types | CU | 使用あり | HA106A | - |  |
| DC types | D | 使用あり | HD107A | - |  |
| DC types | DB | 使用あり | HF102A, HF225A, HF226A, HF227A, HF228A, HF229A ほか 1 | - |  |
| DC types | DD | 使用あり | HF103A, HF243A, HF244A, HF245A, HF246A, HF247A ほか 2 | - |  |
| DC types | DH | 使用あり | HF207A, HF208A, HF209A, HF210A, HF211A, HF212A | - |  |
| DC types | E | 使用あり | HD107A | - |  |
| DC types | EB | 使用あり | HF219A, HF220A, HF221A, HF222A, HF223A, HF224A | - |  |
| DC types | ED | 使用あり | HF103A, HF237A, HF238A, HF239A, HF240A, HF241A ほか 2 | - |  |
| DC types | EH | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A | - |  |
| DC types | F | 使用あり | HA102A, HA103A, HA105A, HA106A, HD107A, HI106A ほか 17 | - |  |
| DC types | FD | 使用あり | HA103A, HI105A, HI106A, HI112A, HI113A, HI114A ほか 3 | - |  |
| DC types | G | 未網羅 |  | - |  |
| DC types | H | 使用あり | HA103A, HD107A, HI106A, HI112A, HI114A, HI115A ほか 1 | - |  |
| DC types | J | 未網羅 |  | - | 未網羅: CXD/DXD と同じ理由 |
| DC types | JD | 未網羅 |  | - |  |
| DC types | L | 使用あり | HD107A | - |  |
| DC types | LB | 使用あり | HF231A, HF232A, HF233A, HF234A, HF235A, HF236A | - |  |
| DC types | LD | 使用あり | HF249A, HF250A, HF251A, HF252A, HF253A, HF254A ほか 1 | - |  |
| DC types | LH | 使用あり | HF213A, HF214A, HF215A, HF216A, HF217A, HF218A | - |  |
| DC types | LQ | 未網羅 |  | - |  |
| DC types | P | 使用あり | HA103A, HD103A, HD104A, HD107A, HI125A, HS105A | - |  |
| DC types | Q | 未網羅 |  | - | 未網羅: CXD/DXD と同じ理由 |
| DC types | QD | 未網羅 |  | - | 未網羅: CXD/DXD と同じ理由 |
| DC types | QY | 未網羅 |  | - | 未網羅: CXD/DXD と同じ理由 |
| DC types | R | 未網羅 |  | - | 未網羅: PSECT のアドレス (XPLINK/GOFF)。Zeus の対応を確かめていない |
| DC types | RD | 未網羅 |  | - | 未網羅: PSECT のアドレス (XPLINK/GOFF)。Zeus の対応を確かめていない |
| DC types | S | 使用あり | HA107A | - |  |
| DC types | SY | 使用あり | HA107A | - |  |
| DC types | V | 使用あり | HA107A | - |  |
| DC types | VD | 使用あり | HA107A | - |  |
| DC types | X | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HA106A ほか 106 | - |  |
| DC types | Y | 使用あり | HA106A | - |  |
| DC types | Z | 使用あり | HA103A, HD107A | - |  |
| Conditional assembly | ACTR | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | AEJECT | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Conditional assembly | AGO | 使用あり | HM105A | - |  |
| Conditional assembly | AIF | 使用あり | HM102A, HM105A | - |  |
| Conditional assembly | ANOP | 使用あり | HM102A, HM105A | - |  |
| Conditional assembly | AREAD | 未網羅 |  | - | 未網羅: Zeus に無い (2026-11-12 に確認) |
| Conditional assembly | ASPACE | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Conditional assembly | GBLA | 使用あり | HM105A, HM106A, HM107A | - |  |
| Conditional assembly | GBLB | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | GBLC | 使用あり | HM105A, HM106A, HM107A, HM108A, HM109A | - |  |
| Conditional assembly | LCLA | 使用あり | HM102A, HM103A, HM104A, HM105A, HM106A, HM107A ほか 2 | - |  |
| Conditional assembly | LCLB | 使用あり | HM102A, HM105A, HM109A | - |  |
| Conditional assembly | LCLC | 使用あり | HM102A, HM103A, HM105A, HM106A, HM107A, HM108A ほか 1 | - |  |
| Conditional assembly | MACRO | 使用あり | HM101A, HM105A, HM106A, HM107A, HM108A, HM109A | - |  |
| Conditional assembly | MEND | 使用あり | HM101A, HM105A, HM106A, HM107A, HM108A, HM109A | - |  |
| Conditional assembly | MEXIT | 使用あり | HM105A | - |  |
| Conditional assembly | MHELP | 対象外 |  | - | 対象外: リスト・目的コードの付加レコード・出口・診断の制御で、実行時の値に出ない |
| Conditional assembly | SETA | 使用あり | HM102A, HM103A, HM104A, HM105A, HM106A, HM107A ほか 2 | - |  |
| Conditional assembly | SETB | 使用あり | HM102A, HM105A, HM107A, HM109A | - |  |
| Conditional assembly | SETC | 使用あり | HM102A, HM103A, HM105A, HM106A, HM107A, HM108A ほか 1 | - |  |
| Conditional assembly | SETAF | 対象外 |  | - | 対象外: 利用者が用意する外部関数モジュール (ASMA 出口) が要る |
| Conditional assembly | SETCF | 対象外 |  | - | 対象外: 利用者が用意する外部関数モジュール (ASMA 出口) が要る |
| Conditional assembly | T' | 使用あり | HM108A, HM109A | - |  |
| Conditional assembly | L' | 使用あり | HA102A, HA106A, HE120N, HE120P, HM104A | - |  |
| Conditional assembly | S' | 使用あり | HA102A | - |  |
| Conditional assembly | I' | 使用あり | HA102A | - |  |
| Conditional assembly | K' | 使用あり | HM101A, HM106A, HM107A, HM108A, HM109A | - |  |
| Conditional assembly | N' | 使用あり | HM101A, HM105A | - |  |
| Conditional assembly | D' | 使用あり | HM108A | - |  |
| Conditional assembly | O' | 使用あり | HM107A | - |  |
| Conditional assembly | A2B | 使用あり | HM106A | - |  |
| Conditional assembly | A2C | 使用あり | HM106A | - |  |
| Conditional assembly | A2D | 使用あり | HM106A | - |  |
| Conditional assembly | A2X | 使用あり | HM106A | - |  |
| Conditional assembly | B2A | 使用あり | HM106A | - |  |
| Conditional assembly | B2C | 使用あり | HM106A | - |  |
| Conditional assembly | B2D | 使用あり | HM106A | - |  |
| Conditional assembly | B2X | 使用あり | HM106A | - |  |
| Conditional assembly | BYTE | 使用あり | HM107A | - |  |
| Conditional assembly | C2A | 使用あり | HM106A | - |  |
| Conditional assembly | C2B | 使用あり | HM106A | - |  |
| Conditional assembly | C2D | 使用あり | HM106A | - |  |
| Conditional assembly | C2X | 使用あり | HM103A | - |  |
| Conditional assembly | D2A | 使用あり | HM105A, HM106A | - |  |
| Conditional assembly | D2B | 使用あり | HM106A | - |  |
| Conditional assembly | D2C | 使用あり | HM106A | - |  |
| Conditional assembly | D2X | 使用あり | HM103A, HM106A | - |  |
| Conditional assembly | DCLEN | 使用あり | HM107A | - |  |
| Conditional assembly | DCVAL | 使用あり | HM107A | - |  |
| Conditional assembly | DEQUOTE | 使用あり | HM107A | - |  |
| Conditional assembly | DOUBLE | 使用あり | HM107A | - |  |
| Conditional assembly | FIND | 使用あり | HM107A | - |  |
| Conditional assembly | INDEX | 使用あり | HM103A | - |  |
| Conditional assembly | ISBIN | 使用あり | HM107A | - |  |
| Conditional assembly | ISDEC | 使用あり | HM107A | - |  |
| Conditional assembly | ISHEX | 使用あり | HM107A | - |  |
| Conditional assembly | ISSYM | 使用あり | HM107A | - |  |
| Conditional assembly | LOWER | 使用あり | HM107A | - |  |
| Conditional assembly | SIGNED | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | SLA | 使用あり | HM107A | - |  |
| Conditional assembly | SLL | 使用あり | HM107A | - |  |
| Conditional assembly | SRA | 使用あり | HM107A | - |  |
| Conditional assembly | SRL | 使用あり | HM107A | - |  |
| Conditional assembly | UPPER | 使用あり | HM103A | - |  |
| Conditional assembly | X2A | 使用あり | HM106A | - |  |
| Conditional assembly | X2B | 使用あり | HM106A | - |  |
| Conditional assembly | X2C | 使用あり | HM106A | - |  |
| Conditional assembly | X2D | 使用あり | HM106A | - |  |
| Conditional assembly | AND | 使用あり | HM107A, HM109A | - |  |
| Conditional assembly | OR | 使用あり | HM107A | - |  |
| Conditional assembly | XOR | 使用あり | HM107A | - |  |
| Conditional assembly | NOT | 使用あり | HM107A | - |  |
| Conditional assembly | &SYSADATA_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSADATA_MEMBER | 未網羅 |  | - |  |
| Conditional assembly | &SYSADATA_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSASM | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSCLOCK | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSDATC | 使用あり | HM107A | - |  |
| Conditional assembly | &SYSDATE | 使用あり | HM107A | - |  |
| Conditional assembly | &SYSECT | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | &SYSIN_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSIN_MEMBER | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSIN_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSJOB | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSLIB_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIB_MEMBER | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIB_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIN_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIN_MEMBER | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIN_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIST | 使用あり | HM105A | - |  |
| Conditional assembly | &SYSLOC | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | &SYSMAC | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | &SYSM_HSEV | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSM_SEV | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSNDX | 使用あり | HM105A | - |  |
| Conditional assembly | &SYSNEST | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | &SYSOPT_DBCS | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSOPT_OPTABLE | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSOPT_RENT | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | &SYSOPT_XOBJECT | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSPARM | 使用あり | HM107A | - |  |
| Conditional assembly | &SYSPRINT_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSPRINT_MEMBER | 未網羅 |  | - |  |
| Conditional assembly | &SYSPRINT_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSPUNCH_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSPUNCH_MEMBER | 未網羅 |  | - |  |
| Conditional assembly | &SYSPUNCH_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSSEQF | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSSTEP | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSSTMT | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSSTYP | 使用あり | HM106A, HM107A | - |  |
| Conditional assembly | &SYSTEM_ID | 使用あり | HM109A | - |  |
| Conditional assembly | &SYSTERM_DSN | 未網羅 |  | - |  |
| Conditional assembly | &SYSTERM_MEMBER | 未網羅 |  | - |  |
| Conditional assembly | &SYSTERM_VOLUME | 未網羅 |  | - |  |
| Conditional assembly | &SYSTIME | 使用あり | HM107A | - |  |
| Conditional assembly | &SYSVER | 使用あり | HM109A | - |  |
| Conditional assembly | ESYM | 未網羅 |  | - |  |
| Conditional assembly | SYSATTRA | 使用あり | HM108A | - |  |
| Conditional assembly | SYSATTRP | 使用あり | HM108A | - |  |
| Machine instructions | A | 使用あり | HE101N, HE101P, HE102N, HE102P, HE103N, HE103P ほか 36 | - |  |
| Machine instructions | AD | 使用あり | HF101A, HM108A | - |  |
| Machine instructions | ADB | 使用あり | HI129A | - |  |
| Machine instructions | ADR | 使用あり | HI127A | - |  |
| Machine instructions | AE | 使用あり | HI126A | - |  |
| Machine instructions | AEB | 使用あり | HI129A | - |  |
| Machine instructions | AER | 使用あり | HI126A | - |  |
| Machine instructions | AG | 使用あり | HI106A | - |  |
| Machine instructions | AGF | 使用あり | HI112A | - |  |
| Machine instructions | AGFI | 使用あり | HI112A | - |  |
| Machine instructions | AGFR | 使用あり | HI106A | - |  |
| Machine instructions | AGHI | 使用あり | HI101A, HI107A, HI123A | - |  |
| Machine instructions | AGR | 使用あり | HI112A | - |  |
| Machine instructions | AH | 使用あり | HI112A | - |  |
| Machine instructions | AHI | 使用あり | HI111A, HM104A, HSS03A | - |  |
| Machine instructions | AHY | 使用あり | HI112A | - |  |
| Machine instructions | AL | 使用あり | HI113A | - |  |
| Machine instructions | ALC | 使用あり | HI113A | - |  |
| Machine instructions | ALCG | 使用あり | HI113A | - |  |
| Machine instructions | ALCGR | 使用あり | HI101A | - |  |
| Machine instructions | ALCR | 使用あり | HI113A | - |  |
| Machine instructions | ALG | 使用あり | HI113A | - |  |
| Machine instructions | ALGR | 使用あり | HI101A, HI107A | - |  |
| Machine instructions | ALR | 使用あり | HI113A | - |  |
| Machine instructions | ALY | 使用あり | HI113A | - |  |
| Machine instructions | AP | 使用あり | HD104A, HS105A | - |  |
| Machine instructions | AR | 使用あり | HSS03A | - |  |
| Machine instructions | AU | 使用あり | HI126A | - |  |
| Machine instructions | AUR | 使用あり | HI126A | - |  |
| Machine instructions | AW | 使用あり | HI127A | - |  |
| Machine instructions | AWR | 使用あり | HI127A | - |  |
| Machine instructions | AXR | 使用あり | HI128A | - |  |
| Machine instructions | AY | 使用あり | HI112A | - |  |
| Machine instructions | BAL | 使用あり | HI123A | - |  |
| Machine instructions | BALR | 使用あり | HI123A | - |  |
| Machine instructions | BAS | 使用あり | HI123A | - |  |
| Machine instructions | BASR | 使用あり | HS101A, HS102A, HS103A, HS104A, HS105A | - |  |
| Machine instructions | BASSM | 使用あり | HI125A | - |  |
| Machine instructions | BC | 使用あり | HI123A | - |  |
| Machine instructions | BCR | 使用あり | HI123A | - |  |
| Machine instructions | BCT | 使用あり | HI111A | - |  |
| Machine instructions | BCTG | 使用あり | HI123A | - |  |
| Machine instructions | BCTGR | 使用あり | HI123A | - |  |
| Machine instructions | BCTR | 使用あり | HI123A | - |  |
| Machine instructions | BR | 使用あり | HE101N, HE101P, HE102N, HE102P, HE103N, HE103P ほか 39 | - |  |
| Machine instructions | BRAS | 使用あり | HI123A | - |  |
| Machine instructions | BRASL | 使用あり | HI123A | - |  |
| Machine instructions | BRC | 使用あり | HI123A, HI125A | - |  |
| Machine instructions | BRCL | 使用あり | HI123A | - |  |
| Machine instructions | BRCT | 使用あり | HI123A | - |  |
| Machine instructions | BRCTG | 使用あり | HI123A | - |  |
| Machine instructions | BRCTH | 使用あり | HI123A | - |  |
| Machine instructions | BSM | 使用あり | HI125A | - |  |
| Machine instructions | BXH | 使用あり | HI111A | - |  |
| Machine instructions | BXLE | 使用あり | HI111A | - |  |
| Machine instructions | C | 使用あり | HI114A, HM103A | - |  |
| Machine instructions | CD | 使用あり | HI127A | - |  |
| Machine instructions | CDB | 使用あり | HI129A | - |  |
| Machine instructions | CDPT | 使用あり | HI131A | - |  |
| Machine instructions | CDR | 使用あり | HI127A | - |  |
| Machine instructions | CDS | 使用あり | HI124A | - |  |
| Machine instructions | CDSG | 使用あり | HI124A | - |  |
| Machine instructions | CDSY | 使用あり | HI124A | - |  |
| Machine instructions | CDZT | 使用あり | HI131A | - |  |
| Machine instructions | CE | 使用あり | HI126A | - |  |
| Machine instructions | CEB | 使用あり | HI129A | - |  |
| Machine instructions | CER | 使用あり | HI126A | - |  |
| Machine instructions | CG | 使用あり | HI114A | - |  |
| Machine instructions | CGF | 使用あり | HI114A | - |  |
| Machine instructions | CGFI | 使用あり | HI114A | - |  |
| Machine instructions | CGFR | 使用あり | HI114A | - |  |
| Machine instructions | CGHI | 使用あり | HI114A, HI118A, HI123A | - |  |
| Machine instructions | CGR | 使用あり | HI107A | - |  |
| Machine instructions | CH | 使用あり | HI114A, HM108A | - |  |
| Machine instructions | CHI | 使用あり | HI114A | - |  |
| Machine instructions | CHY | 使用あり | HI114A | - |  |
| Machine instructions | CL | 使用あり | HI114A | - |  |
| Machine instructions | CLC | 使用あり | HA107A, HD101A | - |  |
| Machine instructions | CLCL | 使用あり | HI110A | - |  |
| Machine instructions | CLCLE | 使用あり | HI125A | - |  |
| Machine instructions | CLG | 使用あり | HI114A | - |  |
| Machine instructions | CLGR | 使用あり | HI114A | - |  |
| Machine instructions | CLI | 使用あり | HI114A | - |  |
| Machine instructions | CLIY | 使用あり | HI114A | - |  |
| Machine instructions | CLM | 使用あり | HI111A | - |  |
| Machine instructions | CLMH | 使用あり | HI114A | - |  |
| Machine instructions | CLMY | 使用あり | HI114A | - |  |
| Machine instructions | CLR | 使用あり | HI114A | - |  |
| Machine instructions | CLST | 使用あり | HI110A | - |  |
| Machine instructions | CLY | 使用あり | HI114A | - |  |
| Machine instructions | CP | 使用あり | HD104A | - |  |
| Machine instructions | CPDT | 使用あり | HI131A | - |  |
| Machine instructions | CPXT | 使用あり | HI131A | - |  |
| Machine instructions | CR | 使用あり | HA107A, HI114A, HI117A, HI123A | - |  |
| Machine instructions | CS | 使用あり | HI105A | - |  |
| Machine instructions | CSG | 使用あり | HI124A | - |  |
| Machine instructions | CSST | 未網羅 |  | - | 未網羅: 機能コードと格納特性ごとのパラメーター表の配置を Principles of Operation で確かめてから書く (記憶だけで期待値を決めない) |
| Machine instructions | CSY | 使用あり | HI124A | - |  |
| Machine instructions | CVB | 使用あり | HD102A | - |  |
| Machine instructions | CVBG | 使用あり | HI125A | - |  |
| Machine instructions | CVBY | 使用あり | HI125A | - |  |
| Machine instructions | CVD | 使用あり | HD102A | - |  |
| Machine instructions | CVDG | 使用あり | HI125A | - |  |
| Machine instructions | CVDY | 使用あり | HI125A | - |  |
| Machine instructions | CXPT | 使用あり | HI131A | - |  |
| Machine instructions | CXZT | 使用あり | HI131A | - |  |
| Machine instructions | CY | 使用あり | HI114A | - |  |
| Machine instructions | CZDT | 使用あり | HI131A | - |  |
| Machine instructions | CZXT | 使用あり | HI131A | - |  |
| Machine instructions | D | 使用あり | HI115A | - |  |
| Machine instructions | DD | 使用あり | HF101A | - |  |
| Machine instructions | DDB | 使用あり | HI129A | - |  |
| Machine instructions | DDR | 使用あり | HI127A | - |  |
| Machine instructions | DE | 使用あり | HI126A | - |  |
| Machine instructions | DEB | 使用あり | HI129A | - |  |
| Machine instructions | DER | 使用あり | HI126A | - |  |
| Machine instructions | DL | 使用あり | HI115A | - |  |
| Machine instructions | DLG | 使用あり | HI115A | - |  |
| Machine instructions | DLGR | 使用あり | HI115A | - |  |
| Machine instructions | DLR | 使用あり | HI115A | - |  |
| Machine instructions | DP | 使用あり | HD104A | - |  |
| Machine instructions | DR | 使用あり | HE105N, HE105P | - |  |
| Machine instructions | DSG | 使用あり | HI115A | - |  |
| Machine instructions | DSGF | 使用あり | HI115A | - |  |
| Machine instructions | DSGFR | 使用あり | HI115A | - |  |
| Machine instructions | DSGR | 使用あり | HI115A | - |  |
| Machine instructions | ED | 使用あり | HD103A | - |  |
| Machine instructions | EDMK | 使用あり | HI110A | - |  |
| Machine instructions | EFPC | 使用あり | HI130A | - |  |
| Machine instructions | EX | 使用あり | HI110A | - |  |
| Machine instructions | EXRL | 使用あり | HI123A | - |  |
| Machine instructions | FLOGR | 使用あり | HI103A | - |  |
| Machine instructions | FREEMAIN | 使用あり | HSS02A | - |  |
| Machine instructions | GETMAIN | 使用あり | HSS02A | - |  |
| Machine instructions | HDR | 使用あり | HI127A | - |  |
| Machine instructions | HER | 使用あり | HI126A | - |  |
| Machine instructions | IC | 使用あり | HI111A, HSS03A | - |  |
| Machine instructions | ICM | 使用あり | HE117N, HE117P, HI104A | - |  |
| Machine instructions | ICMH | 使用あり | HI109A | - |  |
| Machine instructions | ICMY | 使用あり | HI117A | - |  |
| Machine instructions | ICY | 使用あり | HI117A | - |  |
| Machine instructions | IIHF | 使用あり | HI102A | - |  |
| Machine instructions | IIHH | 使用あり | HI117A | - |  |
| Machine instructions | IIHL | 使用あり | HI117A | - |  |
| Machine instructions | IILF | 使用あり | HI102A | - |  |
| Machine instructions | IILH | 使用あり | HI117A | - |  |
| Machine instructions | IILL | 使用あり | HI117A | - |  |
| Machine instructions | IPM | 使用あり | HA107A, HD101A, HD103A, HD104A, HD105A, HD106A ほか 77 | - |  |
| Machine instructions | KDB | 使用あり | HI129A | - |  |
| Machine instructions | KEB | 使用あり | HI129A | - |  |
| Machine instructions | L | 使用あり | HA105A, HA107A, HD102A, HE101N, HE101P, HE102N ほか 70 | - |  |
| Machine instructions | LA | 使用あり | HA104A, HA106A, HA107A, HI102A, HI110A, HI111A ほか 9 | - |  |
| Machine instructions | LAA | 使用あり | HI124A | - |  |
| Machine instructions | LAAG | 使用あり | HI105A | - |  |
| Machine instructions | LAAL | 使用あり | HI124A | - |  |
| Machine instructions | LAALG | 使用あり | HI124A | - |  |
| Machine instructions | LAM | 使用あり | HI125A | - |  |
| Machine instructions | LAN | 使用あり | HI124A | - |  |
| Machine instructions | LANG | 使用あり | HI124A | - |  |
| Machine instructions | LAO | 使用あり | HI124A | - |  |
| Machine instructions | LAOG | 使用あり | HI124A | - |  |
| Machine instructions | LARL | 使用あり | HI117A | - |  |
| Machine instructions | LAX | 使用あり | HI124A | - |  |
| Machine instructions | LAXG | 使用あり | HI124A | - |  |
| Machine instructions | LAY | 使用あり | HI117A | - |  |
| Machine instructions | LB | 使用あり | HI116A | - |  |
| Machine instructions | LBR | 使用あり | HI116A | - |  |
| Machine instructions | LCDR | 使用あり | HI127A | - |  |
| Machine instructions | LCER | 使用あり | HI126A | - |  |
| Machine instructions | LCGR | 使用あり | HI106A | - |  |
| Machine instructions | LCR | 使用あり | HI116A | - |  |
| Machine instructions | LD | 使用あり | HF101A, HF102A, HF103A, HF207A, HF208A, HF209A ほか 39 | - |  |
| Machine instructions | LDEB | 使用あり | HI130A | - |  |
| Machine instructions | LDR | 使用あり | HI127A | - |  |
| Machine instructions | LE | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 16 | - |  |
| Machine instructions | LEDR | 使用あり | HI127A | - |  |
| Machine instructions | LER | 使用あり | HI126A | - |  |
| Machine instructions | LFPC | 使用あり | HI129A, HI130A | - |  |
| Machine instructions | LG | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 71 | - |  |
| Machine instructions | LGB | 使用あり | HI116A | - |  |
| Machine instructions | LGBR | 使用あり | HI116A | - |  |
| Machine instructions | LGF | 使用あり | HI106A | - |  |
| Machine instructions | LGFI | 使用あり | HI116A | - |  |
| Machine instructions | LGFR | 使用あり | HD102A | - |  |
| Machine instructions | LGH | 使用あり | HI106A | - |  |
| Machine instructions | LGHI | 使用あり | HA101A, HA102A, HA104A, HE107N, HE107P, HI101A ほか 17 | - |  |
| Machine instructions | LGR | 使用あり | HS101A | - |  |
| Machine instructions | LH | 使用あり | HA107A, HI116A | - |  |
| Machine instructions | LHI | 使用あり | HI111A | - |  |
| Machine instructions | LHY | 使用あり | HI116A | - |  |
| Machine instructions | LINK | 使用あり | HSS04A | - |  |
| Machine instructions | LLC | 使用あり | HI116A | - |  |
| Machine instructions | LLGC | 使用あり | HI106A | - |  |
| Machine instructions | LLGF | 使用あり | HI106A | - |  |
| Machine instructions | LLGFR | 使用あり | HD101A, HD103A, HD104A, HD105A, HD106A, HI102A ほか 5 | - |  |
| Machine instructions | LLGH | 使用あり | HI106A | - |  |
| Machine instructions | LLH | 使用あり | HA107A, HI116A | - |  |
| Machine instructions | LLIHF | 使用あり | HI116A | - |  |
| Machine instructions | LLIHH | 使用あり | HI116A | - |  |
| Machine instructions | LLIHL | 使用あり | HI116A | - |  |
| Machine instructions | LLILF | 使用あり | HI116A | - |  |
| Machine instructions | LLILH | 使用あり | HI116A | - |  |
| Machine instructions | LLILL | 使用あり | HI116A | - |  |
| Machine instructions | LM | 使用あり | HSS02A, HSS03A, HSS04A | - |  |
| Machine instructions | LMG | 使用あり | HI117A | - |  |
| Machine instructions | LMY | 使用あり | HI117A | - |  |
| Machine instructions | LNDR | 使用あり | HI127A | - |  |
| Machine instructions | LNER | 使用あり | HI126A | - |  |
| Machine instructions | LNGR | 使用あり | HI106A | - |  |
| Machine instructions | LNR | 使用あり | HI116A | - |  |
| Machine instructions | LOC | 使用あり | HI118A | - |  |
| Machine instructions | LOCFH | 使用あり | HI118A | - |  |
| Machine instructions | LOCFHR | 使用あり | HI118A | - |  |
| Machine instructions | LOCG | 使用あり | HI118A | - |  |
| Machine instructions | LOCGHI | 使用あり | HI118A | - |  |
| Machine instructions | LOCGR | 使用あり | HI118A | - |  |
| Machine instructions | LOCHHI | 使用あり | HI118A | - |  |
| Machine instructions | LOCHI | 使用あり | HI118A | - |  |
| Machine instructions | LOCR | 使用あり | HI118A | - |  |
| Machine instructions | LPD | 使用あり | HI124A | - |  |
| Machine instructions | LPDG | 使用あり | HI124A | - |  |
| Machine instructions | LPDR | 使用あり | HI127A | - |  |
| Machine instructions | LPER | 使用あり | HI126A | - |  |
| Machine instructions | LPGR | 使用あり | HI106A | - |  |
| Machine instructions | LPQ | 使用あり | HI124A | - |  |
| Machine instructions | LPR | 使用あり | HI116A | - |  |
| Machine instructions | LR | 使用あり | HA105A, HE101N, HE101P, HE102N, HE102P, HE103N ほか 39 | - |  |
| Machine instructions | LRDR | 使用あり | HI127A | - |  |
| Machine instructions | LRV | 使用あり | HI118A | - |  |
| Machine instructions | LRVG | 使用あり | HI102A | - |  |
| Machine instructions | LRVGR | 使用あり | HI118A | - |  |
| Machine instructions | LRVH | 使用あり | HI118A | - |  |
| Machine instructions | LRVR | 使用あり | HI118A | - |  |
| Machine instructions | LTDR | 使用あり | HI127A | - |  |
| Machine instructions | LTER | 使用あり | HI126A | - |  |
| Machine instructions | LTGR | 使用あり | HI116A | - |  |
| Machine instructions | LTR | 使用あり | HI116A | - |  |
| Machine instructions | LXDB | 使用あり | HI130A | - |  |
| Machine instructions | LXEB | 使用あり | HI130A | - |  |
| Machine instructions | LXR | 使用あり | HI128A | - |  |
| Machine instructions | LY | 使用あり | HE103N, HE103P, HI102A | - |  |
| Machine instructions | M | 使用あり | HI115A | - |  |
| Machine instructions | MADB | 使用あり | HI129A | - |  |
| Machine instructions | MAEB | 使用あり | HI129A | - |  |
| Machine instructions | MD | 使用あり | HF101A | - |  |
| Machine instructions | MDB | 使用あり | HI129A | - |  |
| Machine instructions | MDEB | 使用あり | HI130A | - |  |
| Machine instructions | MDR | 使用あり | HI127A | - |  |
| Machine instructions | ME | 使用あり | HI126A | - |  |
| Machine instructions | MEEB | 使用あり | HI129A | - |  |
| Machine instructions | MER | 使用あり | HI126A | - |  |
| Machine instructions | MGHI | 使用あり | HI115A | - |  |
| Machine instructions | MH | 使用あり | HI115A | - |  |
| Machine instructions | MHI | 使用あり | HI115A | - |  |
| Machine instructions | ML | 使用あり | HI115A | - |  |
| Machine instructions | MLG | 使用あり | HI115A | - |  |
| Machine instructions | MLGR | 使用あり | HI115A | - |  |
| Machine instructions | MLR | 使用あり | HI115A | - |  |
| Machine instructions | MP | 使用あり | HD104A | - |  |
| Machine instructions | MR | 使用あり | HE104N, HE104P | - |  |
| Machine instructions | MS | 使用あり | HI125A | - |  |
| Machine instructions | MSDB | 使用あり | HI129A | - |  |
| Machine instructions | MSEB | 使用あり | HI129A | - |  |
| Machine instructions | MSG | 使用あり | HI115A | - |  |
| Machine instructions | MSGR | 使用あり | HI115A | - |  |
| Machine instructions | MSY | 使用あり | HI115A | - |  |
| Machine instructions | MVC | 使用あり | HA106A, HA107A, HD101A, HD103A, HD105A, HD106A ほか 15 | - |  |
| Machine instructions | MVCL | 使用あり | HI110A | - |  |
| Machine instructions | MVCLE | 使用あり | HI125A | - |  |
| Machine instructions | MVI | 使用あり | HE106N, HE106P, HI124A, HI125A | - |  |
| Machine instructions | MVIY | 使用あり | HI125A | - |  |
| Machine instructions | MVN | 使用あり | HD101A | - |  |
| Machine instructions | MVO | 使用あり | HI110A | - |  |
| Machine instructions | MVST | 使用あり | HI110A | - |  |
| Machine instructions | MVZ | 使用あり | HD101A | - |  |
| Machine instructions | MXD | 使用あり | HI128A | - |  |
| Machine instructions | MXDB | 使用あり | HI130A | - |  |
| Machine instructions | MXDR | 使用あり | HI128A | - |  |
| Machine instructions | MXR | 使用あり | HI128A | - |  |
| Machine instructions | N | 使用あり | HA104A, HA106A, HA107A, HD101A, HD103A, HD104A ほか 67 | - |  |
| Machine instructions | NC | 使用あり | HD101A | - |  |
| Machine instructions | NCGRK | 使用あり | HI121A | - |  |
| Machine instructions | NCRK | 使用あり | HI121A | - |  |
| Machine instructions | NG | 使用あり | HI119A | - |  |
| Machine instructions | NGR | 使用あり | HI116A, HI119A | - |  |
| Machine instructions | NGRK | 使用あり | HI121A | - |  |
| Machine instructions | NI | 使用あり | HI119A | - |  |
| Machine instructions | NIHF | 使用あり | HI119A | - |  |
| Machine instructions | NIHH | 使用あり | HI119A | - |  |
| Machine instructions | NIHL | 使用あり | HI119A | - |  |
| Machine instructions | NILF | 使用あり | HI119A | - |  |
| Machine instructions | NILH | 使用あり | HI119A | - |  |
| Machine instructions | NILL | 使用あり | HI119A, HI124A | - |  |
| Machine instructions | NIY | 使用あり | HI119A | - |  |
| Machine instructions | NNGRK | 使用あり | HI121A | - |  |
| Machine instructions | NNRK | 使用あり | HI121A | - |  |
| Machine instructions | NOGRK | 使用あり | HI121A | - |  |
| Machine instructions | NORK | 使用あり | HI121A | - |  |
| Machine instructions | NR | 使用あり | HI119A | - |  |
| Machine instructions | NRK | 使用あり | HI121A | - |  |
| Machine instructions | NXGRK | 使用あり | HI121A | - |  |
| Machine instructions | NXRK | 使用あり | HI121A | - |  |
| Machine instructions | NY | 使用あり | HI119A | - |  |
| Machine instructions | O | 使用あり | HI120A, HI125A | - |  |
| Machine instructions | OC | 使用あり | HD101A | - |  |
| Machine instructions | OCGRK | 使用あり | HI121A | - |  |
| Machine instructions | OCRK | 使用あり | HI121A | - |  |
| Machine instructions | OG | 使用あり | HI120A | - |  |
| Machine instructions | OGR | 使用あり | HI116A, HI120A | - |  |
| Machine instructions | OGRK | 使用あり | HI121A | - |  |
| Machine instructions | OI | 使用あり | HS105A | - |  |
| Machine instructions | OIHF | 使用あり | HI120A | - |  |
| Machine instructions | OIHH | 使用あり | HI120A | - |  |
| Machine instructions | OIHL | 使用あり | HI120A | - |  |
| Machine instructions | OILF | 使用あり | HI120A | - |  |
| Machine instructions | OILH | 使用あり | HI120A | - |  |
| Machine instructions | OILL | 使用あり | HI120A | - |  |
| Machine instructions | OIY | 使用あり | HI120A | - |  |
| Machine instructions | OR | 使用あり | HI120A | - |  |
| Machine instructions | ORK | 使用あり | HI121A | - |  |
| Machine instructions | OY | 使用あり | HI120A | - |  |
| Machine instructions | PACK | 使用あり | HD104A | - |  |
| Machine instructions | PFPO | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 48 | - |  |
| Machine instructions | POPCNT | 使用あり | HI103A | - |  |
| Machine instructions | RISBG | 使用あり | HI103A | - |  |
| Machine instructions | RISBGN | 使用あり | HI122A | - |  |
| Machine instructions | RISBHG | 使用あり | HI122A | - |  |
| Machine instructions | RISBLG | 使用あり | HI122A | - |  |
| Machine instructions | RLL | 使用あり | HI122A | - |  |
| Machine instructions | RLLG | 使用あり | HI103A | - |  |
| Machine instructions | RNSBG | 使用あり | HI122A | - |  |
| Machine instructions | ROSBG | 使用あり | HI122A | - |  |
| Machine instructions | RXSBG | 使用あり | HI122A | - |  |
| Machine instructions | S | 使用あり | HI112A | - |  |
| Machine instructions | SAM24 | 使用あり | HS101A | - |  |
| Machine instructions | SAM31 | 使用あり | HI125A, HS101A | - |  |
| Machine instructions | SAM64 | 使用あり | HI125A | - |  |
| Machine instructions | SD | 使用あり | HI127A | - |  |
| Machine instructions | SDB | 使用あり | HI129A | - |  |
| Machine instructions | SDR | 使用あり | HI127A | - |  |
| Machine instructions | SE | 使用あり | HI126A | - |  |
| Machine instructions | SEB | 使用あり | HI129A | - |  |
| Machine instructions | SELFHR | 使用あり | HI118A | - |  |
| Machine instructions | SELGR | 使用あり | HI118A | - |  |
| Machine instructions | SELR | 使用あり | HI118A | - |  |
| Machine instructions | SER | 使用あり | HI126A | - |  |
| Machine instructions | SFPC | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 48 | - |  |
| Machine instructions | SG | 使用あり | HI106A | - |  |
| Machine instructions | SGF | 使用あり | HI112A | - |  |
| Machine instructions | SGFR | 使用あり | HI106A | - |  |
| Machine instructions | SGR | 使用あり | HI112A | - |  |
| Machine instructions | SH | 使用あり | HI112A | - |  |
| Machine instructions | SHY | 使用あり | HI112A | - |  |
| Machine instructions | SL | 使用あり | HI113A | - |  |
| Machine instructions | SLA | 使用あり | HI122A | - |  |
| Machine instructions | SLAG | 使用あり | HI122A | - |  |
| Machine instructions | SLB | 使用あり | HI113A | - |  |
| Machine instructions | SLBG | 使用あり | HI113A | - |  |
| Machine instructions | SLBGR | 使用あり | HI113A | - |  |
| Machine instructions | SLBR | 使用あり | HI113A | - |  |
| Machine instructions | SLDA | 使用あり | HI111A | - |  |
| Machine instructions | SLDL | 使用あり | HI122A | - |  |
| Machine instructions | SLDT | 使用あり | HI131A | - |  |
| Machine instructions | SLG | 使用あり | HI113A | - |  |
| Machine instructions | SLGR | 使用あり | HI113A | - |  |
| Machine instructions | SLL | 使用あり | HE116N, HE116P | - |  |
| Machine instructions | SLLG | 使用あり | HI108A | - |  |
| Machine instructions | SLR | 使用あり | HI113A | - |  |
| Machine instructions | SLXT | 使用あり | HI131A | - |  |
| Machine instructions | SLY | 使用あり | HI113A | - |  |
| Machine instructions | SP | 使用あり | HD104A | - |  |
| Machine instructions | SPM | 使用あり | HI125A | - |  |
| Machine instructions | SQDB | 使用あり | HI130A | - |  |
| Machine instructions | SQEB | 使用あり | HI130A | - |  |
| Machine instructions | SR | 使用あり | HA107A, HI110A, HI125A | - |  |
| Machine instructions | SRA | 使用あり | HI122A | - |  |
| Machine instructions | SRAG | 使用あり | HI122A | - |  |
| Machine instructions | SRDA | 使用あり | HI111A | - |  |
| Machine instructions | SRDL | 使用あり | HI122A | - |  |
| Machine instructions | SRDT | 使用あり | HI131A | - |  |
| Machine instructions | SRL | 使用あり | HA107A, HD101A, HD103A, HD104A, HD105A, HD106A ほか 77 | - |  |
| Machine instructions | SRLG | 使用あり | HI108A | - |  |
| Machine instructions | SRNM | 使用あり | HI130A | - |  |
| Machine instructions | SRNMB | 使用あり | HI130A | - |  |
| Machine instructions | SRNMT | 使用あり | HI130A | - |  |
| Machine instructions | SRP | 使用あり | HE115N, HE115P | - |  |
| Machine instructions | SRST | 使用あり | HI110A | - |  |
| Machine instructions | SRXT | 使用あり | HI131A | - |  |
| Machine instructions | ST | 使用あり | HA106A, HA107A, HF201A, HF202A, HF203A, HF204A ほか 73 | - |  |
| Machine instructions | STAM | 使用あり | HI125A | - |  |
| Machine instructions | STC | 使用あり | HI117A, HI125A | - |  |
| Machine instructions | STCM | 使用あり | HI111A | - |  |
| Machine instructions | STCMH | 使用あり | HI117A | - |  |
| Machine instructions | STCMY | 使用あり | HI117A | - |  |
| Machine instructions | STCY | 使用あり | HI117A | - |  |
| Machine instructions | STD | 使用あり | HF101A, HF102A, HF103A, HF201A, HF202A, HF203A ほか 57 | - |  |
| Machine instructions | STE | 使用あり | HI126A, HI127A, HI129A, HI130A | - |  |
| Machine instructions | STFPC | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 48 | - |  |
| Machine instructions | STG | 使用あり | HA101A, HA102A, HA104A, HA105A, HD101A, HD102A ほか 89 | - |  |
| Machine instructions | STH | 使用あり | HI117A | - |  |
| Machine instructions | STHY | 使用あり | HI117A | - |  |
| Machine instructions | STM | 使用あり | HI111A, HSS02A, HSS03A, HSS04A | - |  |
| Machine instructions | STMG | 使用あり | HI117A | - |  |
| Machine instructions | STMY | 使用あり | HI117A | - |  |
| Machine instructions | STOC | 使用あり | HI118A | - |  |
| Machine instructions | STOCFH | 使用あり | HI118A | - |  |
| Machine instructions | STOCG | 使用あり | HI118A | - |  |
| Machine instructions | STPQ | 使用あり | HI124A | - |  |
| Machine instructions | STRV | 使用あり | HI118A | - |  |
| Machine instructions | STRVG | 使用あり | HI102A | - |  |
| Machine instructions | STRVH | 使用あり | HI118A | - |  |
| Machine instructions | STY | 使用あり | HI117A | - |  |
| Machine instructions | SU | 使用あり | HI126A | - |  |
| Machine instructions | SUR | 使用あり | HI126A | - |  |
| Machine instructions | SW | 使用あり | HI127A | - |  |
| Machine instructions | SWR | 使用あり | HI127A | - |  |
| Machine instructions | SXR | 使用あり | HI128A | - |  |
| Machine instructions | SY | 使用あり | HI112A | - |  |
| Machine instructions | TCDB | 使用あり | HI130A | - |  |
| Machine instructions | TCEB | 使用あり | HI130A | - |  |
| Machine instructions | TCXB | 使用あり | HI130A | - |  |
| Machine instructions | TDCDT | 使用あり | HI131A | - |  |
| Machine instructions | TDCET | 使用あり | HI131A | - |  |
| Machine instructions | TDCXT | 使用あり | HI131A | - |  |
| Machine instructions | TDGDT | 未網羅 |  | - | 未網羅: データグループのマスクのビット割り当て (安全なゼロ・極端な指数など) を Principles of Operation で確かめてから書く |
| Machine instructions | TDGET | 未網羅 |  | - | 未網羅: TDGDT と同じ理由 |
| Machine instructions | TDGXT | 未網羅 |  | - | 未網羅: TDGDT と同じ理由 |
| Machine instructions | TM | 使用あり | HI110A | - |  |
| Machine instructions | TMHH | 使用あり | HI119A | - |  |
| Machine instructions | TMHL | 使用あり | HI119A | - |  |
| Machine instructions | TMLH | 使用あり | HI119A | - |  |
| Machine instructions | TMLL | 使用あり | HI119A | - |  |
| Machine instructions | TMY | 使用あり | HI119A | - |  |
| Machine instructions | TP | 使用あり | HD104A, HD105A, HD106A, HE109N, HE109P, HS105A | - |  |
| Machine instructions | TR | 使用あり | HI110A | - |  |
| Machine instructions | TRT | 使用あり | HI110A | - |  |
| Machine instructions | TRTR | 使用あり | HI125A | - |  |
| Machine instructions | TS | 使用あり | HI124A | - |  |
| Machine instructions | UNPK | 使用あり | HD104A, HS105A | - |  |
| Machine instructions | X | 使用あり | HE120N, HE120P, HI120A | - |  |
| Machine instructions | XC | 使用あり | HA107A, HD101A, HI112A, HI113A, HI114A, HI115A ほか 16 | - |  |
| Machine instructions | XG | 使用あり | HI120A | - |  |
| Machine instructions | XGR | 使用あり | HI120A | - |  |
| Machine instructions | XGRK | 使用あり | HI103A, HI121A | - |  |
| Machine instructions | XI | 使用あり | HI120A | - |  |
| Machine instructions | XIHF | 使用あり | HI120A | - |  |
| Machine instructions | XILF | 使用あり | HI120A | - |  |
| Machine instructions | XIY | 使用あり | HI120A | - |  |
| Machine instructions | XR | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 52 | - |  |
| Machine instructions | XRK | 使用あり | HI121A | - |  |
| Machine instructions | XY | 使用あり | HI120A | - |  |
| Machine instructions | ZAP | 使用あり | HD104A, HE108N, HE108P | - |  |

# ZHVS 網羅台帳 (HLASM と z/Architecture)

HLASM Language Reference の章立てと、Zeus の命令の解読表 (machine.rs ほか) が知る命令を列挙した。命令の一覧は Principles of Operation の全命令ではなく、Zeus が知る命令である。 要素 623、使用あり 180、未網羅 443。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |
| --- | --- | --- | --- | --- | --- |
| Assembler instructions | ACONTROL | 未網羅 |  | - |  |
| Assembler instructions | ADATA | 未網羅 |  | - |  |
| Assembler instructions | AINSERT | 未網羅 |  | - |  |
| Assembler instructions | ALIAS | 使用あり | HA102A | - |  |
| Assembler instructions | AMODE | 未網羅 |  | - |  |
| Assembler instructions | CATTR | 未網羅 |  | - |  |
| Assembler instructions | CCW | 未網羅 |  | - |  |
| Assembler instructions | CCW0 | 未網羅 |  | - |  |
| Assembler instructions | CCW1 | 未網羅 |  | - |  |
| Assembler instructions | CEJECT | 未網羅 |  | - |  |
| Assembler instructions | CNOP | 使用あり | HA104A | - |  |
| Assembler instructions | COM | 未網羅 |  | - |  |
| Assembler instructions | COPY | 未網羅 |  | - |  |
| Assembler instructions | CSECT | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HD101A ほか 128 | - |  |
| Assembler instructions | CXD | 未網羅 |  | - |  |
| Assembler instructions | DC | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HD101A ほか 89 | - |  |
| Assembler instructions | DROP | 使用あり | HA105A, HE118N, HSS02A, HSS03A, HSS04A | - |  |
| Assembler instructions | DS | 使用あり | HA101A, HA102A, HA104A, HA105A, HD101A, HD102A ほか 125 | - |  |
| Assembler instructions | DSECT | 使用あり | HA104A | - |  |
| Assembler instructions | DXD | 未網羅 |  | - |  |
| Assembler instructions | EJECT | 未網羅 |  | - |  |
| Assembler instructions | END | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HD101A ほか 128 | - |  |
| Assembler instructions | ENTRY | 未網羅 |  | - |  |
| Assembler instructions | EQU | 使用あり | HA101A, HA102A, HA104A, HE120N, HE120P | - |  |
| Assembler instructions | EXITCTL | 未網羅 |  | - |  |
| Assembler instructions | EXTRN | 未網羅 |  | - |  |
| Assembler instructions | ICTL | 未網羅 |  | - |  |
| Assembler instructions | ISEQ | 未網羅 |  | - |  |
| Assembler instructions | LOCTR | 未網羅 |  | - |  |
| Assembler instructions | LTORG | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HD101A ほか 87 | - |  |
| Assembler instructions | MNOTE | 使用あり | HE119N, HE119P | - |  |
| Assembler instructions | OPSYN | 使用あり | HM104A | - |  |
| Assembler instructions | ORG | 使用あり | HA104A, HI110A | - |  |
| Assembler instructions | POP | 使用あり | HA105A | - |  |
| Assembler instructions | PRINT | 未網羅 |  | - |  |
| Assembler instructions | PUNCH | 未網羅 |  | - |  |
| Assembler instructions | PUSH | 使用あり | HA105A | - |  |
| Assembler instructions | REPRO | 未網羅 |  | - |  |
| Assembler instructions | RMODE | 未網羅 |  | - |  |
| Assembler instructions | RSECT | 未網羅 |  | - |  |
| Assembler instructions | SPACE | 未網羅 |  | - |  |
| Assembler instructions | START | 使用あり | HA101A | - |  |
| Assembler instructions | TITLE | 未網羅 |  | - |  |
| Assembler instructions | USING | 使用あり | HA105A, HE101N, HE101P, HE102N, HE102P, HE103N ほか 38 | - |  |
| Assembler instructions | WXTRN | 未網羅 |  | - |  |
| Assembler instructions | XATTR | 未網羅 |  | - |  |
| DC types | A | 未網羅 |  | - |  |
| DC types | AD | 未網羅 |  | - |  |
| DC types | B | 使用あり | HA103A, HE111N, HE111P | - |  |
| DC types | C | 使用あり | HA103A, HD101A, HI110A, HI111A, HM102A, HM103A ほか 5 | - |  |
| DC types | CA | 未網羅 |  | - |  |
| DC types | CE | 未網羅 |  | - |  |
| DC types | CU | 未網羅 |  | - |  |
| DC types | D | 使用あり | HD107A | - |  |
| DC types | DB | 使用あり | HF102A, HF225A, HF226A, HF227A, HF228A, HF229A ほか 1 | - |  |
| DC types | DD | 使用あり | HF103A, HF243A, HF244A, HF245A, HF246A, HF247A ほか 1 | - |  |
| DC types | DH | 使用あり | HF207A, HF208A, HF209A, HF210A, HF211A, HF212A | - |  |
| DC types | E | 使用あり | HD107A | - |  |
| DC types | EB | 使用あり | HF219A, HF220A, HF221A, HF222A, HF223A, HF224A | - |  |
| DC types | ED | 使用あり | HF103A, HF237A, HF238A, HF239A, HF240A, HF241A ほか 1 | - |  |
| DC types | EH | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A | - |  |
| DC types | F | 使用あり | HA102A, HA103A, HA105A, HD107A, HI106A, HS102A ほか 4 | - |  |
| DC types | FD | 使用あり | HA103A, HI105A, HI106A | - |  |
| DC types | G | 未網羅 |  | - |  |
| DC types | H | 使用あり | HA103A, HD107A, HI106A | - |  |
| DC types | HD | 未網羅 |  | - |  |
| DC types | J | 未網羅 |  | - |  |
| DC types | L | 使用あり | HD107A | - |  |
| DC types | LB | 使用あり | HF231A, HF232A, HF233A, HF234A, HF235A, HF236A | - |  |
| DC types | LD | 使用あり | HF249A, HF250A, HF251A, HF252A, HF253A, HF254A | - |  |
| DC types | LH | 使用あり | HF213A, HF214A, HF215A, HF216A, HF217A, HF218A | - |  |
| DC types | LQ | 未網羅 |  | - |  |
| DC types | P | 使用あり | HA103A, HD103A, HD104A, HD107A, HS105A | - |  |
| DC types | Q | 未網羅 |  | - |  |
| DC types | QD | 未網羅 |  | - |  |
| DC types | QY | 未網羅 |  | - |  |
| DC types | R | 未網羅 |  | - |  |
| DC types | RD | 未網羅 |  | - |  |
| DC types | S | 未網羅 |  | - |  |
| DC types | SY | 未網羅 |  | - |  |
| DC types | V | 未網羅 |  | - |  |
| DC types | VD | 未網羅 |  | - |  |
| DC types | X | 使用あり | HA101A, HA102A, HA103A, HA104A, HA105A, HD101A ほか 83 | - |  |
| DC types | Y | 未網羅 |  | - |  |
| DC types | Z | 使用あり | HA103A, HD107A | - |  |
| Conditional assembly | ACTR | 未網羅 |  | - |  |
| Conditional assembly | AEJECT | 未網羅 |  | - |  |
| Conditional assembly | AGO | 使用あり | HM105A | - |  |
| Conditional assembly | AIF | 使用あり | HM102A, HM105A | - |  |
| Conditional assembly | ANOP | 使用あり | HM102A, HM105A | - |  |
| Conditional assembly | AREAD | 未網羅 |  | - |  |
| Conditional assembly | ASPACE | 未網羅 |  | - |  |
| Conditional assembly | GBLA | 使用あり | HM105A | - |  |
| Conditional assembly | GBLB | 未網羅 |  | - |  |
| Conditional assembly | GBLC | 使用あり | HM105A | - |  |
| Conditional assembly | LCLA | 使用あり | HM102A, HM103A, HM104A, HM105A | - |  |
| Conditional assembly | LCLB | 使用あり | HM102A, HM105A | - |  |
| Conditional assembly | LCLC | 使用あり | HM102A, HM103A, HM105A | - |  |
| Conditional assembly | MACRO | 使用あり | HM101A, HM105A | - |  |
| Conditional assembly | MEND | 使用あり | HM101A, HM105A | - |  |
| Conditional assembly | MEXIT | 使用あり | HM105A | - |  |
| Conditional assembly | MHELP | 未網羅 |  | - |  |
| Conditional assembly | SETA | 使用あり | HM102A, HM103A, HM104A, HM105A | - |  |
| Conditional assembly | SETB | 使用あり | HM102A, HM105A | - |  |
| Conditional assembly | SETC | 使用あり | HM102A, HM103A, HM105A | - |  |
| Conditional assembly | SETAF | 未網羅 |  | - |  |
| Conditional assembly | SETCF | 未網羅 |  | - |  |
| Conditional assembly | &SYSNDX | 使用あり | HM105A | - |  |
| Conditional assembly | &SYSECT | 未網羅 |  | - |  |
| Conditional assembly | &SYSLIST | 使用あり | HM105A | - |  |
| Conditional assembly | &SYSDATE | 未網羅 |  | - |  |
| Conditional assembly | &SYSDATC | 未網羅 |  | - |  |
| Conditional assembly | &SYSTIME | 未網羅 |  | - |  |
| Conditional assembly | &SYSPARM | 未網羅 |  | - |  |
| Conditional assembly | &SYSLOC | 未網羅 |  | - |  |
| Conditional assembly | &SYSNEST | 未網羅 |  | - |  |
| Conditional assembly | &SYSMAC | 未網羅 |  | - |  |
| Conditional assembly | &SYSSTYP | 未網羅 |  | - |  |
| Conditional assembly | &SYSOPT_RENT | 未網羅 |  | - |  |
| Conditional assembly | T' | 使用あり | HM102A, HS104A | - |  |
| Conditional assembly | L' | 使用あり | HA102A, HD107A, HE120N, HE120P, HM104A, HS104A | - |  |
| Conditional assembly | S' | 使用あり | HA102A | - |  |
| Conditional assembly | I' | 使用あり | HA102A | - |  |
| Conditional assembly | K' | 使用あり | HE119P, HM101A | - |  |
| Conditional assembly | N' | 使用あり | HM101A, HM105A | - |  |
| Conditional assembly | D' | 使用あり | HA102A, HA103A, HD102A, HD104A, HD105A, HD106A ほか 32 | - |  |
| Conditional assembly | O' | 未網羅 |  | - |  |
| Conditional assembly | A2B | 未網羅 |  | - |  |
| Conditional assembly | A2C | 未網羅 |  | - |  |
| Conditional assembly | A2D | 未網羅 |  | - |  |
| Conditional assembly | A2X | 未網羅 |  | - |  |
| Conditional assembly | B2A | 未網羅 |  | - |  |
| Conditional assembly | B2C | 未網羅 |  | - |  |
| Conditional assembly | B2D | 未網羅 |  | - |  |
| Conditional assembly | B2X | 未網羅 |  | - |  |
| Conditional assembly | BYTE | 未網羅 |  | - |  |
| Conditional assembly | C2A | 未網羅 |  | - |  |
| Conditional assembly | C2B | 未網羅 |  | - |  |
| Conditional assembly | C2D | 未網羅 |  | - |  |
| Conditional assembly | C2X | 使用あり | HM103A | - |  |
| Conditional assembly | D2A | 使用あり | HM105A | - |  |
| Conditional assembly | D2B | 未網羅 |  | - |  |
| Conditional assembly | D2C | 未網羅 |  | - |  |
| Conditional assembly | D2X | 使用あり | HM103A | - |  |
| Conditional assembly | DCLEN | 未網羅 |  | - |  |
| Conditional assembly | DCVAL | 未網羅 |  | - |  |
| Conditional assembly | DEQUOTE | 未網羅 |  | - |  |
| Conditional assembly | DOUBLE | 未網羅 |  | - |  |
| Conditional assembly | FIND | 未網羅 |  | - |  |
| Conditional assembly | INDEX | 使用あり | HM103A | - |  |
| Conditional assembly | ISBIN | 未網羅 |  | - |  |
| Conditional assembly | ISDEC | 未網羅 |  | - |  |
| Conditional assembly | ISHEX | 未網羅 |  | - |  |
| Conditional assembly | ISSYM | 未網羅 |  | - |  |
| Conditional assembly | LOWER | 未網羅 |  | - |  |
| Conditional assembly | SIGNED | 未網羅 |  | - |  |
| Conditional assembly | SLA | 未網羅 |  | - |  |
| Conditional assembly | SLL | 未網羅 |  | - |  |
| Conditional assembly | SRA | 未網羅 |  | - |  |
| Conditional assembly | SRL | 未網羅 |  | - |  |
| Conditional assembly | UPPER | 使用あり | HM103A | - |  |
| Conditional assembly | X2A | 未網羅 |  | - |  |
| Conditional assembly | X2B | 未網羅 |  | - |  |
| Conditional assembly | X2C | 未網羅 |  | - |  |
| Conditional assembly | X2D | 未網羅 |  | - |  |
| Conditional assembly | AND | 未網羅 |  | - |  |
| Conditional assembly | OR | 未網羅 |  | - |  |
| Conditional assembly | XOR | 未網羅 |  | - |  |
| Conditional assembly | NOT | 未網羅 |  | - |  |
| Machine instructions | A | 使用あり | HE101N, HE101P, HE102N, HE102P, HE103N, HE103P ほか 35 | - |  |
| Machine instructions | AD | 使用あり | HF101A | - |  |
| Machine instructions | ADB | 未網羅 |  | - |  |
| Machine instructions | ADR | 未網羅 |  | - |  |
| Machine instructions | AE | 未網羅 |  | - |  |
| Machine instructions | AEB | 未網羅 |  | - |  |
| Machine instructions | AER | 未網羅 |  | - |  |
| Machine instructions | AG | 使用あり | HI106A | - |  |
| Machine instructions | AGF | 未網羅 |  | - |  |
| Machine instructions | AGFI | 未網羅 |  | - |  |
| Machine instructions | AGFR | 使用あり | HI106A | - |  |
| Machine instructions | AGHI | 使用あり | HI101A, HI107A | - |  |
| Machine instructions | AGR | 未網羅 |  | - |  |
| Machine instructions | AH | 未網羅 |  | - |  |
| Machine instructions | AHI | 使用あり | HI111A, HM104A, HSS03A | - |  |
| Machine instructions | AHY | 未網羅 |  | - |  |
| Machine instructions | AL | 未網羅 |  | - |  |
| Machine instructions | ALC | 未網羅 |  | - |  |
| Machine instructions | ALCG | 未網羅 |  | - |  |
| Machine instructions | ALCGR | 使用あり | HI101A | - |  |
| Machine instructions | ALCR | 未網羅 |  | - |  |
| Machine instructions | ALG | 未網羅 |  | - |  |
| Machine instructions | ALGR | 使用あり | HI101A, HI107A | - |  |
| Machine instructions | ALR | 未網羅 |  | - |  |
| Machine instructions | ALY | 未網羅 |  | - |  |
| Machine instructions | AP | 使用あり | HD104A, HS105A | - |  |
| Machine instructions | AR | 使用あり | HSS03A | - |  |
| Machine instructions | AU | 未網羅 |  | - |  |
| Machine instructions | AUR | 未網羅 |  | - |  |
| Machine instructions | AW | 未網羅 |  | - |  |
| Machine instructions | AWR | 未網羅 |  | - |  |
| Machine instructions | AXR | 未網羅 |  | - |  |
| Machine instructions | AY | 未網羅 |  | - |  |
| Machine instructions | BAL | 未網羅 |  | - |  |
| Machine instructions | BALR | 未網羅 |  | - |  |
| Machine instructions | BAS | 未網羅 |  | - |  |
| Machine instructions | BASR | 使用あり | HS101A, HS102A, HS103A, HS104A, HS105A | - |  |
| Machine instructions | BASSM | 未網羅 |  | - |  |
| Machine instructions | BC | 未網羅 |  | - |  |
| Machine instructions | BCR | 未網羅 |  | - |  |
| Machine instructions | BCT | 使用あり | HI111A | - |  |
| Machine instructions | BCTG | 未網羅 |  | - |  |
| Machine instructions | BCTGR | 未網羅 |  | - |  |
| Machine instructions | BCTR | 未網羅 |  | - |  |
| Machine instructions | BR | 使用あり | HE101N, HE101P, HE102N, HE102P, HE103N, HE103P ほか 39 | - |  |
| Machine instructions | BRAS | 未網羅 |  | - |  |
| Machine instructions | BRASL | 未網羅 |  | - |  |
| Machine instructions | BRC | 未網羅 |  | - |  |
| Machine instructions | BRCL | 未網羅 |  | - |  |
| Machine instructions | BRCT | 未網羅 |  | - |  |
| Machine instructions | BRCTG | 未網羅 |  | - |  |
| Machine instructions | BRCTH | 未網羅 |  | - |  |
| Machine instructions | BSM | 未網羅 |  | - |  |
| Machine instructions | BXH | 使用あり | HI111A | - |  |
| Machine instructions | BXLE | 使用あり | HI111A | - |  |
| Machine instructions | C | 使用あり | HM103A | - |  |
| Machine instructions | CD | 未網羅 |  | - |  |
| Machine instructions | CDB | 未網羅 |  | - |  |
| Machine instructions | CDPT | 未網羅 |  | - |  |
| Machine instructions | CDR | 未網羅 |  | - |  |
| Machine instructions | CDS | 未網羅 |  | - |  |
| Machine instructions | CDSG | 未網羅 |  | - |  |
| Machine instructions | CDSY | 未網羅 |  | - |  |
| Machine instructions | CDZT | 未網羅 |  | - |  |
| Machine instructions | CE | 未網羅 |  | - |  |
| Machine instructions | CEB | 未網羅 |  | - |  |
| Machine instructions | CER | 未網羅 |  | - |  |
| Machine instructions | CG | 未網羅 |  | - |  |
| Machine instructions | CGF | 未網羅 |  | - |  |
| Machine instructions | CGFI | 未網羅 |  | - |  |
| Machine instructions | CGFR | 未網羅 |  | - |  |
| Machine instructions | CGHI | 未網羅 |  | - |  |
| Machine instructions | CGR | 使用あり | HI107A | - |  |
| Machine instructions | CH | 未網羅 |  | - |  |
| Machine instructions | CHI | 未網羅 |  | - |  |
| Machine instructions | CHY | 未網羅 |  | - |  |
| Machine instructions | CL | 未網羅 |  | - |  |
| Machine instructions | CLC | 使用あり | HD101A | - |  |
| Machine instructions | CLCL | 使用あり | HI110A | - |  |
| Machine instructions | CLCLE | 未網羅 |  | - |  |
| Machine instructions | CLG | 未網羅 |  | - |  |
| Machine instructions | CLGR | 未網羅 |  | - |  |
| Machine instructions | CLI | 未網羅 |  | - |  |
| Machine instructions | CLIY | 未網羅 |  | - |  |
| Machine instructions | CLM | 使用あり | HI111A | - |  |
| Machine instructions | CLMH | 未網羅 |  | - |  |
| Machine instructions | CLMY | 未網羅 |  | - |  |
| Machine instructions | CLR | 未網羅 |  | - |  |
| Machine instructions | CLST | 使用あり | HI110A | - |  |
| Machine instructions | CLY | 未網羅 |  | - |  |
| Machine instructions | CP | 使用あり | HD104A | - |  |
| Machine instructions | CPDT | 未網羅 |  | - |  |
| Machine instructions | CPXT | 未網羅 |  | - |  |
| Machine instructions | CR | 未網羅 |  | - |  |
| Machine instructions | CS | 使用あり | HI105A | - |  |
| Machine instructions | CSG | 未網羅 |  | - |  |
| Machine instructions | CSST | 未網羅 |  | - |  |
| Machine instructions | CSY | 未網羅 |  | - |  |
| Machine instructions | CVB | 使用あり | HD102A | - |  |
| Machine instructions | CVBG | 未網羅 |  | - |  |
| Machine instructions | CVBY | 未網羅 |  | - |  |
| Machine instructions | CVD | 使用あり | HD102A | - |  |
| Machine instructions | CVDG | 未網羅 |  | - |  |
| Machine instructions | CVDY | 未網羅 |  | - |  |
| Machine instructions | CXPT | 未網羅 |  | - |  |
| Machine instructions | CXZT | 未網羅 |  | - |  |
| Machine instructions | CY | 未網羅 |  | - |  |
| Machine instructions | CZDT | 未網羅 |  | - |  |
| Machine instructions | CZXT | 未網羅 |  | - |  |
| Machine instructions | D | 未網羅 |  | - |  |
| Machine instructions | DD | 使用あり | HF101A | - |  |
| Machine instructions | DDB | 未網羅 |  | - |  |
| Machine instructions | DDR | 未網羅 |  | - |  |
| Machine instructions | DE | 未網羅 |  | - |  |
| Machine instructions | DEB | 未網羅 |  | - |  |
| Machine instructions | DER | 未網羅 |  | - |  |
| Machine instructions | DL | 未網羅 |  | - |  |
| Machine instructions | DLG | 未網羅 |  | - |  |
| Machine instructions | DLGR | 未網羅 |  | - |  |
| Machine instructions | DLR | 未網羅 |  | - |  |
| Machine instructions | DP | 使用あり | HD104A | - |  |
| Machine instructions | DR | 使用あり | HE105N, HE105P | - |  |
| Machine instructions | DSG | 未網羅 |  | - |  |
| Machine instructions | DSGF | 未網羅 |  | - |  |
| Machine instructions | DSGFR | 未網羅 |  | - |  |
| Machine instructions | DSGR | 未網羅 |  | - |  |
| Machine instructions | ED | 使用あり | HD103A | - |  |
| Machine instructions | EDMK | 使用あり | HI110A | - |  |
| Machine instructions | EFPC | 未網羅 |  | - |  |
| Machine instructions | EX | 使用あり | HI110A | - |  |
| Machine instructions | EXRL | 未網羅 |  | - |  |
| Machine instructions | FLOGR | 使用あり | HI103A | - |  |
| Machine instructions | FREEMAIN | 使用あり | HSS02A | - |  |
| Machine instructions | GETMAIN | 使用あり | HSS02A | - |  |
| Machine instructions | HDR | 未網羅 |  | - |  |
| Machine instructions | HER | 未網羅 |  | - |  |
| Machine instructions | IC | 使用あり | HI111A, HSS03A | - |  |
| Machine instructions | ICM | 使用あり | HE117N, HE117P, HI104A | - |  |
| Machine instructions | ICMH | 使用あり | HI109A | - |  |
| Machine instructions | ICMY | 未網羅 |  | - |  |
| Machine instructions | ICY | 未網羅 |  | - |  |
| Machine instructions | IIHF | 使用あり | HI102A | - |  |
| Machine instructions | IIHH | 未網羅 |  | - |  |
| Machine instructions | IIHL | 未網羅 |  | - |  |
| Machine instructions | IILF | 使用あり | HI102A | - |  |
| Machine instructions | IILH | 未網羅 |  | - |  |
| Machine instructions | IILL | 未網羅 |  | - |  |
| Machine instructions | IPM | 使用あり | HD101A, HD103A, HD104A, HD105A, HD106A, HF201A ほか 58 | - |  |
| Machine instructions | KDB | 未網羅 |  | - |  |
| Machine instructions | KEB | 未網羅 |  | - |  |
| Machine instructions | L | 使用あり | HA105A, HD102A, HE101N, HE101P, HE102N, HE102P ほか 69 | - |  |
| Machine instructions | LA | 使用あり | HA104A, HI102A, HI110A, HI111A, HS102A, HS103A ほか 3 | - |  |
| Machine instructions | LAA | 未網羅 |  | - |  |
| Machine instructions | LAAG | 使用あり | HI105A | - |  |
| Machine instructions | LAAL | 未網羅 |  | - |  |
| Machine instructions | LAALG | 未網羅 |  | - |  |
| Machine instructions | LAM | 未網羅 |  | - |  |
| Machine instructions | LAN | 未網羅 |  | - |  |
| Machine instructions | LANG | 未網羅 |  | - |  |
| Machine instructions | LAO | 未網羅 |  | - |  |
| Machine instructions | LAOG | 未網羅 |  | - |  |
| Machine instructions | LARL | 未網羅 |  | - |  |
| Machine instructions | LAX | 未網羅 |  | - |  |
| Machine instructions | LAXG | 未網羅 |  | - |  |
| Machine instructions | LAY | 未網羅 |  | - |  |
| Machine instructions | LB | 未網羅 |  | - |  |
| Machine instructions | LBR | 未網羅 |  | - |  |
| Machine instructions | LCDR | 未網羅 |  | - |  |
| Machine instructions | LCER | 未網羅 |  | - |  |
| Machine instructions | LCGR | 使用あり | HI106A | - |  |
| Machine instructions | LCR | 未網羅 |  | - |  |
| Machine instructions | LD | 使用あり | HF101A, HF102A, HF103A, HF207A, HF208A, HF209A ほか 33 | - |  |
| Machine instructions | LDEB | 未網羅 |  | - |  |
| Machine instructions | LDR | 未網羅 |  | - |  |
| Machine instructions | LE | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 12 | - |  |
| Machine instructions | LEDR | 未網羅 |  | - |  |
| Machine instructions | LER | 未網羅 |  | - |  |
| Machine instructions | LFPC | 未網羅 |  | - |  |
| Machine instructions | LG | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 56 | - |  |
| Machine instructions | LGB | 未網羅 |  | - |  |
| Machine instructions | LGBR | 未網羅 |  | - |  |
| Machine instructions | LGF | 使用あり | HI106A | - |  |
| Machine instructions | LGFI | 未網羅 |  | - |  |
| Machine instructions | LGFR | 使用あり | HD102A | - |  |
| Machine instructions | LGH | 使用あり | HI106A | - |  |
| Machine instructions | LGHI | 使用あり | HA101A, HA102A, HA104A, HE107N, HE107P, HI101A ほか 14 | - |  |
| Machine instructions | LGR | 使用あり | HS101A | - |  |
| Machine instructions | LH | 未網羅 |  | - |  |
| Machine instructions | LHI | 使用あり | HI111A | - |  |
| Machine instructions | LHY | 未網羅 |  | - |  |
| Machine instructions | LINK | 使用あり | HSS04A | - |  |
| Machine instructions | LLC | 未網羅 |  | - |  |
| Machine instructions | LLGC | 使用あり | HI106A | - |  |
| Machine instructions | LLGF | 使用あり | HI106A | - |  |
| Machine instructions | LLGFR | 使用あり | HD101A, HD103A, HD104A, HD105A, HD106A, HI102A ほか 5 | - |  |
| Machine instructions | LLGH | 使用あり | HI106A | - |  |
| Machine instructions | LLH | 未網羅 |  | - |  |
| Machine instructions | LLIHF | 未網羅 |  | - |  |
| Machine instructions | LLIHH | 未網羅 |  | - |  |
| Machine instructions | LLIHL | 未網羅 |  | - |  |
| Machine instructions | LLILF | 未網羅 |  | - |  |
| Machine instructions | LLILH | 未網羅 |  | - |  |
| Machine instructions | LLILL | 未網羅 |  | - |  |
| Machine instructions | LM | 使用あり | HSS02A, HSS03A, HSS04A | - |  |
| Machine instructions | LMG | 未網羅 |  | - |  |
| Machine instructions | LMY | 未網羅 |  | - |  |
| Machine instructions | LNDR | 未網羅 |  | - |  |
| Machine instructions | LNER | 未網羅 |  | - |  |
| Machine instructions | LNGR | 使用あり | HI106A | - |  |
| Machine instructions | LNR | 未網羅 |  | - |  |
| Machine instructions | LOC | 未網羅 |  | - |  |
| Machine instructions | LOCFH | 未網羅 |  | - |  |
| Machine instructions | LOCFHR | 未網羅 |  | - |  |
| Machine instructions | LOCG | 未網羅 |  | - |  |
| Machine instructions | LOCGHI | 未網羅 |  | - |  |
| Machine instructions | LOCGR | 未網羅 |  | - |  |
| Machine instructions | LOCHHI | 未網羅 |  | - |  |
| Machine instructions | LOCHI | 未網羅 |  | - |  |
| Machine instructions | LOCR | 未網羅 |  | - |  |
| Machine instructions | LPD | 未網羅 |  | - |  |
| Machine instructions | LPDG | 未網羅 |  | - |  |
| Machine instructions | LPDR | 未網羅 |  | - |  |
| Machine instructions | LPER | 未網羅 |  | - |  |
| Machine instructions | LPGR | 使用あり | HI106A | - |  |
| Machine instructions | LPQ | 未網羅 |  | - |  |
| Machine instructions | LPR | 未網羅 |  | - |  |
| Machine instructions | LR | 使用あり | HA105A, HE101N, HE101P, HE102N, HE102P, HE103N ほか 38 | - |  |
| Machine instructions | LRDR | 未網羅 |  | - |  |
| Machine instructions | LRV | 未網羅 |  | - |  |
| Machine instructions | LRVG | 使用あり | HI102A | - |  |
| Machine instructions | LRVGR | 未網羅 |  | - |  |
| Machine instructions | LRVH | 未網羅 |  | - |  |
| Machine instructions | LRVR | 未網羅 |  | - |  |
| Machine instructions | LTDR | 未網羅 |  | - |  |
| Machine instructions | LTER | 未網羅 |  | - |  |
| Machine instructions | LTGR | 未網羅 |  | - |  |
| Machine instructions | LTR | 未網羅 |  | - |  |
| Machine instructions | LXDB | 未網羅 |  | - |  |
| Machine instructions | LXEB | 未網羅 |  | - |  |
| Machine instructions | LXR | 未網羅 |  | - |  |
| Machine instructions | LY | 使用あり | HE103N, HE103P, HI102A | - |  |
| Machine instructions | M | 未網羅 |  | - |  |
| Machine instructions | MADB | 未網羅 |  | - |  |
| Machine instructions | MAEB | 未網羅 |  | - |  |
| Machine instructions | MD | 使用あり | HF101A | - |  |
| Machine instructions | MDB | 未網羅 |  | - |  |
| Machine instructions | MDEB | 未網羅 |  | - |  |
| Machine instructions | MDR | 未網羅 |  | - |  |
| Machine instructions | ME | 未網羅 |  | - |  |
| Machine instructions | MEEB | 未網羅 |  | - |  |
| Machine instructions | MER | 未網羅 |  | - |  |
| Machine instructions | MGHI | 未網羅 |  | - |  |
| Machine instructions | MH | 未網羅 |  | - |  |
| Machine instructions | MHI | 未網羅 |  | - |  |
| Machine instructions | ML | 未網羅 |  | - |  |
| Machine instructions | MLG | 未網羅 |  | - |  |
| Machine instructions | MLGR | 未網羅 |  | - |  |
| Machine instructions | MLR | 未網羅 |  | - |  |
| Machine instructions | MP | 使用あり | HD104A | - |  |
| Machine instructions | MR | 使用あり | HE104N, HE104P | - |  |
| Machine instructions | MSDB | 未網羅 |  | - |  |
| Machine instructions | MSEB | 未網羅 |  | - |  |
| Machine instructions | MSG | 未網羅 |  | - |  |
| Machine instructions | MSGR | 未網羅 |  | - |  |
| Machine instructions | MSY | 未網羅 |  | - |  |
| Machine instructions | MVC | 使用あり | HD101A, HD103A, HD105A, HD106A, HD107A, HI110A ほか 3 | - |  |
| Machine instructions | MVCL | 使用あり | HI110A | - |  |
| Machine instructions | MVCLE | 未網羅 |  | - |  |
| Machine instructions | MVI | 使用あり | HE106N, HE106P | - |  |
| Machine instructions | MVIY | 未網羅 |  | - |  |
| Machine instructions | MVN | 使用あり | HD101A | - |  |
| Machine instructions | MVO | 使用あり | HI110A | - |  |
| Machine instructions | MVST | 使用あり | HI110A | - |  |
| Machine instructions | MVZ | 使用あり | HD101A | - |  |
| Machine instructions | MXD | 未網羅 |  | - |  |
| Machine instructions | MXDB | 未網羅 |  | - |  |
| Machine instructions | MXDR | 未網羅 |  | - |  |
| Machine instructions | MXR | 未網羅 |  | - |  |
| Machine instructions | N | 使用あり | HA104A, HD101A, HD103A, HD104A, HD105A, HD106A ほか 62 | - |  |
| Machine instructions | NC | 使用あり | HD101A | - |  |
| Machine instructions | NCGRK | 未網羅 |  | - |  |
| Machine instructions | NCRK | 未網羅 |  | - |  |
| Machine instructions | NG | 未網羅 |  | - |  |
| Machine instructions | NGR | 未網羅 |  | - |  |
| Machine instructions | NGRK | 未網羅 |  | - |  |
| Machine instructions | NI | 未網羅 |  | - |  |
| Machine instructions | NIHF | 未網羅 |  | - |  |
| Machine instructions | NIHH | 未網羅 |  | - |  |
| Machine instructions | NIHL | 未網羅 |  | - |  |
| Machine instructions | NILF | 未網羅 |  | - |  |
| Machine instructions | NILH | 未網羅 |  | - |  |
| Machine instructions | NILL | 未網羅 |  | - |  |
| Machine instructions | NIY | 未網羅 |  | - |  |
| Machine instructions | NNGRK | 未網羅 |  | - |  |
| Machine instructions | NNRK | 未網羅 |  | - |  |
| Machine instructions | NOGRK | 未網羅 |  | - |  |
| Machine instructions | NORK | 未網羅 |  | - |  |
| Machine instructions | NR | 未網羅 |  | - |  |
| Machine instructions | NRK | 未網羅 |  | - |  |
| Machine instructions | NXGRK | 未網羅 |  | - |  |
| Machine instructions | NXRK | 未網羅 |  | - |  |
| Machine instructions | NY | 未網羅 |  | - |  |
| Machine instructions | O | 未網羅 |  | - |  |
| Machine instructions | OC | 使用あり | HD101A | - |  |
| Machine instructions | OCGRK | 未網羅 |  | - |  |
| Machine instructions | OCRK | 未網羅 |  | - |  |
| Machine instructions | OG | 未網羅 |  | - |  |
| Machine instructions | OGR | 未網羅 |  | - |  |
| Machine instructions | OGRK | 未網羅 |  | - |  |
| Machine instructions | OI | 使用あり | HS105A | - |  |
| Machine instructions | OIHF | 未網羅 |  | - |  |
| Machine instructions | OIHH | 未網羅 |  | - |  |
| Machine instructions | OIHL | 未網羅 |  | - |  |
| Machine instructions | OILF | 未網羅 |  | - |  |
| Machine instructions | OILH | 未網羅 |  | - |  |
| Machine instructions | OILL | 未網羅 |  | - |  |
| Machine instructions | OIY | 未網羅 |  | - |  |
| Machine instructions | OR | 未網羅 |  | - |  |
| Machine instructions | ORK | 未網羅 |  | - |  |
| Machine instructions | OY | 未網羅 |  | - |  |
| Machine instructions | PACK | 使用あり | HD104A | - |  |
| Machine instructions | PFPO | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 48 | - |  |
| Machine instructions | POPCNT | 使用あり | HI103A | - |  |
| Machine instructions | RISBG | 使用あり | HI103A | - |  |
| Machine instructions | RISBGN | 未網羅 |  | - |  |
| Machine instructions | RISBHG | 未網羅 |  | - |  |
| Machine instructions | RISBLG | 未網羅 |  | - |  |
| Machine instructions | RLL | 未網羅 |  | - |  |
| Machine instructions | RLLG | 使用あり | HI103A | - |  |
| Machine instructions | RNSBG | 未網羅 |  | - |  |
| Machine instructions | ROSBG | 未網羅 |  | - |  |
| Machine instructions | RXSBG | 未網羅 |  | - |  |
| Machine instructions | S | 未網羅 |  | - |  |
| Machine instructions | SAM24 | 使用あり | HS101A | - |  |
| Machine instructions | SAM31 | 使用あり | HS101A | - |  |
| Machine instructions | SAM64 | 未網羅 |  | - |  |
| Machine instructions | SD | 未網羅 |  | - |  |
| Machine instructions | SDB | 未網羅 |  | - |  |
| Machine instructions | SDR | 未網羅 |  | - |  |
| Machine instructions | SE | 未網羅 |  | - |  |
| Machine instructions | SEB | 未網羅 |  | - |  |
| Machine instructions | SELFHR | 未網羅 |  | - |  |
| Machine instructions | SELGR | 未網羅 |  | - |  |
| Machine instructions | SELR | 未網羅 |  | - |  |
| Machine instructions | SER | 未網羅 |  | - |  |
| Machine instructions | SFPC | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 48 | - |  |
| Machine instructions | SG | 使用あり | HI106A | - |  |
| Machine instructions | SGF | 未網羅 |  | - |  |
| Machine instructions | SGFR | 使用あり | HI106A | - |  |
| Machine instructions | SGR | 未網羅 |  | - |  |
| Machine instructions | SH | 未網羅 |  | - |  |
| Machine instructions | SHY | 未網羅 |  | - |  |
| Machine instructions | SL | 未網羅 |  | - |  |
| Machine instructions | SLA | 未網羅 |  | - |  |
| Machine instructions | SLAG | 未網羅 |  | - |  |
| Machine instructions | SLB | 未網羅 |  | - |  |
| Machine instructions | SLBG | 未網羅 |  | - |  |
| Machine instructions | SLBGR | 未網羅 |  | - |  |
| Machine instructions | SLBR | 未網羅 |  | - |  |
| Machine instructions | SLDA | 使用あり | HI111A | - |  |
| Machine instructions | SLDL | 未網羅 |  | - |  |
| Machine instructions | SLDT | 未網羅 |  | - |  |
| Machine instructions | SLG | 未網羅 |  | - |  |
| Machine instructions | SLGR | 未網羅 |  | - |  |
| Machine instructions | SLL | 使用あり | HE116N, HE116P | - |  |
| Machine instructions | SLLG | 使用あり | HI108A | - |  |
| Machine instructions | SLR | 未網羅 |  | - |  |
| Machine instructions | SLXT | 未網羅 |  | - |  |
| Machine instructions | SLY | 未網羅 |  | - |  |
| Machine instructions | SP | 使用あり | HD104A | - |  |
| Machine instructions | SPM | 未網羅 |  | - |  |
| Machine instructions | SQDB | 未網羅 |  | - |  |
| Machine instructions | SQEB | 未網羅 |  | - |  |
| Machine instructions | SR | 使用あり | HI110A | - |  |
| Machine instructions | SRA | 未網羅 |  | - |  |
| Machine instructions | SRAG | 未網羅 |  | - |  |
| Machine instructions | SRDA | 使用あり | HI111A | - |  |
| Machine instructions | SRDL | 未網羅 |  | - |  |
| Machine instructions | SRDT | 未網羅 |  | - |  |
| Machine instructions | SRL | 使用あり | HD101A, HD103A, HD104A, HD105A, HD106A, HF201A ほか 58 | - |  |
| Machine instructions | SRLG | 使用あり | HI108A | - |  |
| Machine instructions | SRNM | 未網羅 |  | - |  |
| Machine instructions | SRNMB | 未網羅 |  | - |  |
| Machine instructions | SRNMT | 未網羅 |  | - |  |
| Machine instructions | SRP | 使用あり | HE115N, HE115P | - |  |
| Machine instructions | SRST | 使用あり | HI110A | - |  |
| Machine instructions | SRXT | 未網羅 |  | - |  |
| Machine instructions | ST | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 52 | - |  |
| Machine instructions | STAM | 未網羅 |  | - |  |
| Machine instructions | STC | 未網羅 |  | - |  |
| Machine instructions | STCM | 使用あり | HI111A | - |  |
| Machine instructions | STCMH | 未網羅 |  | - |  |
| Machine instructions | STCMY | 未網羅 |  | - |  |
| Machine instructions | STCY | 未網羅 |  | - |  |
| Machine instructions | STD | 使用あり | HF101A, HF102A, HF103A, HF201A, HF202A, HF203A ほか 51 | - |  |
| Machine instructions | STE | 未網羅 |  | - |  |
| Machine instructions | STFPC | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 48 | - |  |
| Machine instructions | STG | 使用あり | HA101A, HA102A, HA104A, HA105A, HD101A, HD102A ほか 76 | - |  |
| Machine instructions | STH | 未網羅 |  | - |  |
| Machine instructions | STHY | 未網羅 |  | - |  |
| Machine instructions | STIMER | 未網羅 |  | - |  |
| Machine instructions | STM | 使用あり | HI111A, HSS02A, HSS03A, HSS04A | - |  |
| Machine instructions | STMG | 未網羅 |  | - |  |
| Machine instructions | STMY | 未網羅 |  | - |  |
| Machine instructions | STOC | 未網羅 |  | - |  |
| Machine instructions | STOCFH | 未網羅 |  | - |  |
| Machine instructions | STOCG | 未網羅 |  | - |  |
| Machine instructions | STORAGE | 未網羅 |  | - |  |
| Machine instructions | STPQ | 未網羅 |  | - |  |
| Machine instructions | STRV | 未網羅 |  | - |  |
| Machine instructions | STRVG | 使用あり | HI102A | - |  |
| Machine instructions | STRVH | 未網羅 |  | - |  |
| Machine instructions | STY | 未網羅 |  | - |  |
| Machine instructions | SU | 未網羅 |  | - |  |
| Machine instructions | SUR | 未網羅 |  | - |  |
| Machine instructions | SW | 未網羅 |  | - |  |
| Machine instructions | SWR | 未網羅 |  | - |  |
| Machine instructions | SXR | 未網羅 |  | - |  |
| Machine instructions | SY | 未網羅 |  | - |  |
| Machine instructions | TCDB | 未網羅 |  | - |  |
| Machine instructions | TCEB | 未網羅 |  | - |  |
| Machine instructions | TCXB | 未網羅 |  | - |  |
| Machine instructions | TDCDT | 未網羅 |  | - |  |
| Machine instructions | TDCET | 未網羅 |  | - |  |
| Machine instructions | TDCXT | 未網羅 |  | - |  |
| Machine instructions | TDGDT | 未網羅 |  | - |  |
| Machine instructions | TDGET | 未網羅 |  | - |  |
| Machine instructions | TDGXT | 未網羅 |  | - |  |
| Machine instructions | TM | 使用あり | HI110A | - |  |
| Machine instructions | TMHH | 未網羅 |  | - |  |
| Machine instructions | TMHL | 未網羅 |  | - |  |
| Machine instructions | TMLH | 未網羅 |  | - |  |
| Machine instructions | TMLL | 未網羅 |  | - |  |
| Machine instructions | TMY | 未網羅 |  | - |  |
| Machine instructions | TP | 使用あり | HD104A, HD105A, HD106A, HE109N, HE109P, HS105A | - |  |
| Machine instructions | TR | 使用あり | HI110A | - |  |
| Machine instructions | TRT | 使用あり | HI110A | - |  |
| Machine instructions | TRTR | 未網羅 |  | - |  |
| Machine instructions | TS | 未網羅 |  | - |  |
| Machine instructions | UNPK | 使用あり | HD104A, HS105A | - |  |
| Machine instructions | X | 使用あり | HE120N, HE120P | - |  |
| Machine instructions | XC | 使用あり | HD101A | - |  |
| Machine instructions | XCTL | 未網羅 |  | - |  |
| Machine instructions | XG | 未網羅 |  | - |  |
| Machine instructions | XGR | 未網羅 |  | - |  |
| Machine instructions | XGRK | 使用あり | HI103A | - |  |
| Machine instructions | XI | 未網羅 |  | - |  |
| Machine instructions | XIHF | 未網羅 |  | - |  |
| Machine instructions | XILF | 未網羅 |  | - |  |
| Machine instructions | XIY | 未網羅 |  | - |  |
| Machine instructions | XR | 使用あり | HF201A, HF202A, HF203A, HF204A, HF205A, HF206A ほか 52 | - |  |
| Machine instructions | XRK | 未網羅 |  | - |  |
| Machine instructions | XY | 未網羅 |  | - |  |
| Machine instructions | ZAP | 使用あり | HD104A, HE108N, HE108P | - |  |

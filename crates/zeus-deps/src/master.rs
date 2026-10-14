//! The masters the analysis is defined by: node types, edge types, the
//! attributes of each node type and of each edge type, and the assets
//! Zeus provides itself (utilities, run-time routines, system copy
//! members). They are written next to the results (`zdeps masters`) so
//! that the result files can be read with them.

/// A node type. File types are the source members of an application;
/// the others stand for what a reference names when it is not a file of
/// the application (a Zeus-provided asset, a program not found, a
/// dynamic target).
pub struct NodeType {
    pub code: &'static str,
    pub name: &'static str,
    pub is_file: bool,
    /// File name extensions (lower case) that identify the type.
    pub extensions: &'static [&'static str],
    pub description: &'static str,
}

pub const NODE_TYPES: &[NodeType] = &[
    NodeType {
        code: "HLASM_PROGRAM",
        name: "HLASM プログラム",
        is_file: true,
        extensions: &["asm", "hlasm"],
        description: "CSECT / START で始まる実行用アセンブラー",
    },
    NodeType {
        code: "HLASM_COPYBOOK",
        name: "HLASM COPY / マクロ",
        is_file: true,
        extensions: &["mac"],
        description: "アセンブラーのCOPY・DSECT・マクロ定義",
    },
    NodeType {
        code: "JCL_JOB",
        name: "JCL ジョブ",
        is_file: true,
        extensions: &["jcl", "job"],
        description: "JOB 文で始まるジョブ",
    },
    NodeType {
        code: "JCL_PROC",
        name: "JCL カタログ式プロシージャー",
        is_file: true,
        extensions: &["proc", "prc"],
        description: "EXEC で呼ばれるプロシージャーのメンバー",
    },
    NodeType {
        code: "JCL_INCLUDE",
        name: "JCL INCLUDE メンバー",
        is_file: true,
        extensions: &[],
        description: "INCLUDE MEMBER= で取り込まれる JCL",
    },
    NodeType {
        code: "COBOL_PROGRAM",
        name: "COBOL プログラム",
        is_file: true,
        extensions: &["cbl", "cob"],
        description: "PROCEDURE DIVISION を持つ COBOL のソース",
    },
    NodeType {
        code: "COBOL_COPYBOOK",
        name: "COBOL コピーメンバー",
        is_file: true,
        extensions: &["cpy", "dcl"],
        description: "COPY / EXEC SQL INCLUDE で取り込まれるメンバー",
    },
    NodeType {
        code: "PLI_PROGRAM",
        name: "PL/I プログラム",
        is_file: true,
        extensions: &["pli", "pl1"],
        description: "PROCEDURE を持つ PL/I のソース",
    },
    NodeType {
        code: "PLI_INCLUDE",
        name: "PL/I インクルードメンバー",
        is_file: true,
        extensions: &["inc"],
        description: "%INCLUDE / EXEC SQL INCLUDE で取り込まれるメンバー",
    },
    NodeType {
        code: "BMS_MAPSET",
        name: "BMS マップセット",
        is_file: true,
        extensions: &["bms"],
        description: "DFHMSD のマップセット定義",
    },
    NodeType {
        code: "IMS_DBD",
        name: "IMS DBD",
        is_file: true,
        extensions: &["dbd"],
        description: "IMS のデータベース定義",
    },
    NodeType {
        code: "IMS_PSB",
        name: "IMS PSB",
        is_file: true,
        extensions: &["psb"],
        description: "IMS のプログラム仕様ブロック",
    },
    NodeType {
        code: "CICS_CSD",
        name: "CICS CSD 定義",
        is_file: true,
        extensions: &["csd"],
        description: "DFHCSDUP の DEFINE 文 (取引・プログラム・マップセット)",
    },
    NodeType {
        code: "PROGRAM",
        name: "プログラム (言語不明)",
        is_file: false,
        extensions: &[],
        description: "見つからないプログラム: 呼ばれているがソースがない",
    },
    NodeType {
        code: "CICS_TRANSACTION",
        name: "CICS 取引",
        is_file: false,
        extensions: &[],
        description: "CSD に定義が見つからない取引コード",
    },
    NodeType {
        code: "SYSTEM_PROGRAM",
        name: "提供プログラム",
        is_file: false,
        extensions: &[],
        description: "Zeus が提供するユーティリティ・実行時ルーチン (提供資産マスタ)",
    },
    NodeType {
        code: "SYSTEM_COPYBOOK",
        name: "提供コピーメンバー",
        is_file: false,
        extensions: &[],
        description: "Zeus が提供するコピー・インクルードメンバー (提供資産マスタ)",
    },
    NodeType {
        code: "DYNAMIC_TARGET",
        name: "動的な呼出し先",
        is_file: false,
        extensions: &[],
        description: "変数で指定され、ソースから値を決められない呼出し先",
    },
];

/// An edge type: a reference from one file to another.
pub struct EdgeType {
    pub code: &'static str,
    pub name: &'static str,
    pub from: &'static [&'static str],
    pub to: &'static [&'static str],
    pub description: &'static str,
}

const PROGRAMS: &[&str] = &[
    "HLASM_PROGRAM",
    "COBOL_PROGRAM",
    "PLI_PROGRAM",
    "SYSTEM_PROGRAM",
    "PROGRAM",
    "DYNAMIC_TARGET",
];
const JCL: &[&str] = &["JCL_JOB", "JCL_PROC", "JCL_INCLUDE"];
const COBOL: &[&str] = &["COBOL_PROGRAM", "COBOL_COPYBOOK"];
const PLI: &[&str] = &["PLI_PROGRAM", "PLI_INCLUDE"];
const SOURCES: &[&str] = &[
    "COBOL_PROGRAM",
    "COBOL_COPYBOOK",
    "PLI_PROGRAM",
    "PLI_INCLUDE",
];

pub const EDGE_TYPES: &[EdgeType] = &[
    EdgeType {
        code: "HLASM_EXTERNAL",
        name: "HLASM 外部シンボル",
        from: &["HLASM_PROGRAM", "HLASM_COPYBOOK"],
        to: PROGRAMS,
        description: "EXTRN / WXTRN / V・Q・J・R型定数の参照",
    },
    EdgeType {
        code: "HLASM_COPY",
        name: "HLASM COPY / マクロ参照",
        from: &["HLASM_PROGRAM", "HLASM_COPYBOOK"],
        to: &["HLASM_COPYBOOK"],
        description: "COPYメンバーと外部マクロ定義",
    },
    EdgeType {
        code: "COPY",
        name: "COPY",
        from: COBOL,
        to: &["COBOL_COPYBOOK", "SYSTEM_COPYBOOK", "BMS_MAPSET"],
        description: "COPY 文 (BMS のシンボリック・マップはマップセットに解決する)",
    },
    EdgeType {
        code: "SQL_INCLUDE",
        name: "EXEC SQL INCLUDE",
        from: SOURCES,
        to: &["COBOL_COPYBOOK", "PLI_INCLUDE", "SYSTEM_COPYBOOK"],
        description: "EXEC SQL INCLUDE (SQLCA・SQLDA は提供)",
    },
    EdgeType {
        code: "PLI_INCLUDE",
        name: "%INCLUDE",
        from: PLI,
        to: &["PLI_INCLUDE", "SYSTEM_COPYBOOK", "BMS_MAPSET"],
        description: "%INCLUDE / %XINCLUDE (BMS のシンボリック・マップはマップセットに解決する)",
    },
    EdgeType {
        code: "CALL",
        name: "CALL",
        from: SOURCES,
        to: PROGRAMS,
        description: "CALL 文および HLASM LINK / XCTL / LOAD (定数名または動的な入口)",
    },
    EdgeType {
        code: "PLI_FETCH",
        name: "FETCH",
        from: PLI,
        to: PROGRAMS,
        description: "PL/I の FETCH (動的にロードする手続き)",
    },
    EdgeType {
        code: "CICS_LINK",
        name: "EXEC CICS LINK",
        from: SOURCES,
        to: PROGRAMS,
        description: "LINK PROGRAM()",
    },
    EdgeType {
        code: "CICS_XCTL",
        name: "EXEC CICS XCTL",
        from: SOURCES,
        to: PROGRAMS,
        description: "XCTL PROGRAM()",
    },
    EdgeType {
        code: "CICS_LOAD",
        name: "EXEC CICS LOAD",
        from: SOURCES,
        to: PROGRAMS,
        description: "LOAD PROGRAM()",
    },
    EdgeType {
        code: "CICS_TRANSID",
        name: "取引の開始",
        from: SOURCES,
        to: &[
            "COBOL_PROGRAM",
            "PLI_PROGRAM",
            "PROGRAM",
            "CICS_TRANSACTION",
            "DYNAMIC_TARGET",
        ],
        description: "START TRANSID() / RETURN TRANSID(): CSD の取引定義でプログラムに解決する",
    },
    EdgeType {
        code: "CICS_MAP",
        name: "BMS マップの使用",
        from: SOURCES,
        to: &["BMS_MAPSET", "DYNAMIC_TARGET"],
        description: "SEND / RECEIVE MAP() MAPSET()",
    },
    EdgeType {
        code: "DLI_PSB",
        name: "PSB のスケジュール",
        from: SOURCES,
        to: &["IMS_PSB", "DYNAMIC_TARGET"],
        description: "EXEC DLI SCHD PSB()",
    },
    EdgeType {
        code: "JCL_EXEC_PGM",
        name: "EXEC PGM=",
        from: JCL,
        to: PROGRAMS,
        description: "ステップで実行するプログラム",
    },
    EdgeType {
        code: "JCL_EXEC_PROC",
        name: "EXEC プロシージャー",
        from: JCL,
        to: &["JCL_PROC"],
        description: "カタログ式プロシージャーの呼出し (ジョブ内のプロシージャーは除く)",
    },
    EdgeType {
        code: "JCL_INCLUDE",
        name: "INCLUDE MEMBER=",
        from: JCL,
        to: &["JCL_INCLUDE"],
        description: "JCL の INCLUDE",
    },
    EdgeType {
        code: "JCL_TSO_RUN",
        name: "TSO RUN PROGRAM()",
        from: JCL,
        to: PROGRAMS,
        description: "IKJEFT01 の SYSTSIN の DSN RUN PROGRAM()",
    },
    EdgeType {
        code: "JCL_IMS_PGM",
        name: "IMS 領域のプログラム",
        from: JCL,
        to: PROGRAMS,
        description: "DFSRRC00 の PARM の 2 番目 (プログラム)",
    },
    EdgeType {
        code: "JCL_IMS_PSB",
        name: "IMS 領域の PSB",
        from: JCL,
        to: &["IMS_PSB", "IMS_DBD"],
        description: "DFSRRC00 の PARM の 3 番目 (PSB、ユーティリティでは DBD)",
    },
    EdgeType {
        code: "PSB_DBD",
        name: "PCB の DBD",
        from: &["IMS_PSB"],
        to: &["IMS_DBD"],
        description: "PCB DBDNAME=",
    },
    EdgeType {
        code: "CSD_PROGRAM",
        name: "CSD のプログラム定義",
        from: &["CICS_CSD", "JCL_JOB", "JCL_PROC"],
        to: &[
            "COBOL_PROGRAM",
            "PLI_PROGRAM",
            "PROGRAM",
            "SYSTEM_PROGRAM",
            "BMS_MAPSET",
        ],
        description: "DEFINE PROGRAM() (古い CSD でマップセットをプログラムとして定義したものはマップセットに解決する)",
    },
    EdgeType {
        code: "CSD_TRANSACTION",
        name: "CSD の取引定義",
        from: &["CICS_CSD", "JCL_JOB", "JCL_PROC"],
        to: PROGRAMS,
        description: "DEFINE TRANSACTION() PROGRAM()",
    },
    EdgeType {
        code: "CSD_MAPSET",
        name: "CSD のマップセット定義",
        from: &["CICS_CSD", "JCL_JOB", "JCL_PROC"],
        to: &["BMS_MAPSET"],
        description: "DEFINE MAPSET()",
    },
];

/// An attribute of a node type (`*`: of every node) or of an edge type.
pub struct AttrType {
    pub owner: &'static str,
    pub code: &'static str,
    pub name: &'static str,
    /// TEXT, INTEGER, FLAG (Y / N), LIST (blank-separated).
    pub data_type: &'static str,
    pub description: &'static str,
}

const fn a(
    owner: &'static str,
    code: &'static str,
    name: &'static str,
    data_type: &'static str,
    description: &'static str,
) -> AttrType {
    AttrType {
        owner,
        code,
        name,
        data_type,
        description,
    }
}

pub const NODE_ATTRS: &[AttrType] = &[
    a(
        "HLASM_PROGRAM",
        "compile_status",
        "コンパイル結果",
        "TEXT",
        "OK・WARNING・ERROR・NOT_CHECKED",
    ),
    a(
        "HLASM_PROGRAM",
        "compile_message",
        "コンパイルのメッセージ",
        "TEXT",
        "収集した診断メッセージ",
    ),
    a(
        "HLASM_PROGRAM",
        "compile_diagnostics_complete",
        "診断完了",
        "BOOLEAN",
        "安全に解析を完了できた場合にtrue",
    ),
    a(
        "HLASM_PROGRAM",
        "compile_return_code",
        "アセンブル戻り値",
        "INTEGER",
        "収集した診断の最大重大度",
    ),
    a(
        "*",
        "file_name",
        "ファイル名",
        "TEXT",
        "ファイルの名前 (ファイルのノード)",
    ),
    a("*", "size_bytes", "サイズ", "INTEGER", "バイト数"),
    a("*", "line_count", "行数", "INTEGER", "行数"),
    a(
        "*",
        "provider",
        "提供元",
        "TEXT",
        "提供資産の提供元 (SYSTEM_*)",
    ),
    a(
        "*",
        "referenced_by",
        "参照元の数",
        "INTEGER",
        "このノードを参照するエッジの数",
    ),
    a(
        "COBOL_PROGRAM",
        "program_id",
        "プログラム ID",
        "TEXT",
        "最初の PROGRAM-ID",
    ),
    a(
        "COBOL_PROGRAM",
        "programs",
        "プログラム ID の一覧",
        "LIST",
        "入れ子のプログラムを含む PROGRAM-ID",
    ),
    a(
        "COBOL_PROGRAM",
        "entries",
        "入口名",
        "LIST",
        "ENTRY 文の名前",
    ),
    a(
        "COBOL_PROGRAM",
        "exec_cics",
        "EXEC CICS の数",
        "INTEGER",
        "",
    ),
    a("COBOL_PROGRAM", "exec_sql", "EXEC SQL の数", "INTEGER", ""),
    a("COBOL_PROGRAM", "exec_dli", "EXEC DLI の数", "INTEGER", ""),
    a(
        "COBOL_PROGRAM",
        "compile_status",
        "コンパイル結果",
        "TEXT",
        "OK・ERROR・NOT_CHECKED (Zeus のコンパイラで検査)",
    ),
    a(
        "COBOL_PROGRAM",
        "compile_message",
        "コンパイルのメッセージ",
        "TEXT",
        "最初のエラー",
    ),
    a(
        "COBOL_PROGRAM",
        "unsupported",
        "未対応の構文",
        "LIST",
        "Zeus が実行できない文",
    ),
    a("COBOL_COPYBOOK", "exec_sql", "EXEC SQL の数", "INTEGER", ""),
    a(
        "PLI_PROGRAM",
        "procedure",
        "外部手続き名",
        "TEXT",
        "最初の手続きの名前",
    ),
    a("PLI_PROGRAM", "main", "主手続き", "FLAG", "OPTIONS(MAIN)"),
    a(
        "PLI_PROGRAM",
        "procedures",
        "手続きの一覧",
        "LIST",
        "内部手続きを含む",
    ),
    a("PLI_PROGRAM", "exec_cics", "EXEC CICS の数", "INTEGER", ""),
    a("PLI_PROGRAM", "exec_sql", "EXEC SQL の数", "INTEGER", ""),
    a(
        "PLI_PROGRAM",
        "compile_status",
        "コンパイル結果",
        "TEXT",
        "OK・WARNING・ERROR・NOT_CHECKED",
    ),
    a(
        "PLI_PROGRAM",
        "compile_message",
        "コンパイルのメッセージ",
        "TEXT",
        "最初のエラー",
    ),
    a("JCL_JOB", "job_name", "ジョブ名", "TEXT", ""),
    a("JCL_JOB", "steps", "ステップ数", "INTEGER", "EXEC 文の数"),
    a("JCL_JOB", "step_names", "ステップ名", "LIST", ""),
    a(
        "JCL_JOB",
        "instream_procs",
        "ジョブ内のプロシージャー",
        "LIST",
        "PROC 〜 PEND",
    ),
    a(
        "JCL_JOB",
        "parse_status",
        "構文検査",
        "TEXT",
        "OK・ERROR (Zeus の JCL 構文解析)",
    ),
    a(
        "JCL_JOB",
        "parse_message",
        "構文検査のメッセージ",
        "TEXT",
        "",
    ),
    a("JCL_PROC", "proc_name", "プロシージャー名", "TEXT", ""),
    a("JCL_PROC", "steps", "ステップ数", "INTEGER", ""),
    a("JCL_PROC", "step_names", "ステップ名", "LIST", ""),
    a("JCL_PROC", "parse_status", "構文検査", "TEXT", "OK・ERROR"),
    a(
        "JCL_PROC",
        "parse_message",
        "構文検査のメッセージ",
        "TEXT",
        "",
    ),
    a(
        "JCL_INCLUDE",
        "parse_status",
        "構文検査",
        "TEXT",
        "OK・ERROR",
    ),
    a("BMS_MAPSET", "mapset_name", "マップセット名", "TEXT", ""),
    a("BMS_MAPSET", "maps", "マップ", "LIST", "DFHMDI"),
    a(
        "BMS_MAPSET",
        "parse_status",
        "構文検査",
        "TEXT",
        "OK・ERROR (Zeus の BMS 処理)",
    ),
    a(
        "BMS_MAPSET",
        "parse_message",
        "構文検査のメッセージ",
        "TEXT",
        "",
    ),
    a("IMS_DBD", "dbd_name", "DBD 名", "TEXT", ""),
    a("IMS_DBD", "access", "アクセス方式", "TEXT", "ACCESS="),
    a("IMS_PSB", "psb_name", "PSB 名", "TEXT", "PSBGEN PSBNAME="),
    a("IMS_PSB", "language", "言語", "TEXT", "PSBGEN LANG="),
    a("IMS_PSB", "pcbs", "PCB の数", "INTEGER", ""),
    a("CICS_CSD", "groups", "グループ", "LIST", ""),
    a("CICS_CSD", "transactions", "取引", "LIST", ""),
    a("CICS_CSD", "programs", "プログラムの数", "INTEGER", ""),
    a(
        "DYNAMIC_TARGET",
        "identifier",
        "変数名",
        "TEXT",
        "呼出し先を持つ変数",
    ),
];

pub const EDGE_ATTRS: &[AttrType] = &[
    a(
        "HLASM_EXTERNAL",
        "constant_type",
        "定数型",
        "TEXT",
        "V / Q / J / R の参照元定数型",
    ),
    a(
        "CALL",
        "service",
        "呼出しサービス",
        "TEXT",
        "HLASM LINK / XCTL / LOAD",
    ),
    a(
        "HLASM_EXTERNAL",
        "weak",
        "弱い外部参照",
        "FLAG",
        "WXTRN の参照。未解決でも必須資産不足に含めない",
    ),
    a("*", "target_name", "参照名", "TEXT", "ソースに書かれた名前"),
    a(
        "*",
        "occurrences",
        "出現数",
        "INTEGER",
        "参照箇所の数 (エッジ読み取り位置情報一覧の行数)",
    ),
    a(
        "*",
        "resolution",
        "解決",
        "TEXT",
        "FILE (ファイル)・SYSTEM (提供資産)・MISSING (見つからない)・OPTIONAL (未解決の弱い参照)・DYNAMIC (変数で値が分からない)",
    ),
    a(
        "*",
        "via",
        "解決の経路",
        "LIST",
        "VALUE (変数に入れる定数: VALUE・MOVE・INITIAL・代入)・CSD (取引定義)・BMS (マップセットから生成するシンボリック・マップ)",
    ),
    a(
        "*",
        "identifier",
        "変数名",
        "TEXT",
        "呼出し先を変数で指定したときの変数",
    ),
    a("COPY", "library", "ライブラリー", "TEXT", "COPY ... OF/IN"),
    a("COPY", "replacing", "置換", "FLAG", "REPLACING の有無"),
    a(
        "CICS_TRANSID",
        "commands",
        "コマンド",
        "LIST",
        "START・RETURN",
    ),
    a("CICS_TRANSID", "transid", "取引コード", "TEXT", ""),
    a("CICS_MAP", "maps", "マップ", "LIST", "MAP()"),
    a("CICS_MAP", "commands", "コマンド", "LIST", "SEND・RECEIVE"),
    a("JCL_EXEC_PGM", "steps", "ステップ", "LIST", ""),
    a(
        "JCL_EXEC_PGM",
        "parm",
        "PARM",
        "TEXT",
        "最初のステップの PARM",
    ),
    a("JCL_EXEC_PROC", "steps", "ステップ", "LIST", ""),
    a("JCL_TSO_RUN", "steps", "ステップ", "LIST", ""),
    a("JCL_TSO_RUN", "plan", "プラン", "TEXT", "PLAN()"),
    a("JCL_IMS_PGM", "steps", "ステップ", "LIST", ""),
    a(
        "JCL_IMS_PGM",
        "region",
        "領域の種類",
        "TEXT",
        "DLI・DBB・BMP・MSG・ULU・ULR",
    ),
    a("JCL_IMS_PSB", "steps", "ステップ", "LIST", ""),
    a("PSB_DBD", "pcb_type", "PCB の種類", "TEXT", "TYPE="),
    a("PSB_DBD", "procopt", "処理オプション", "TEXT", "PROCOPT="),
    a("CSD_PROGRAM", "groups", "グループ", "LIST", ""),
    a("CSD_TRANSACTION", "transids", "取引コード", "LIST", ""),
    a("CSD_TRANSACTION", "groups", "グループ", "LIST", ""),
    a("CSD_MAPSET", "groups", "グループ", "LIST", ""),
    a(
        "CSD_PROGRAM",
        "steps",
        "ステップ",
        "LIST",
        "JCL の DFHCSDUP のステップ (JCL の SYSIN に書かれた定義)",
    ),
    a(
        "CSD_TRANSACTION",
        "steps",
        "ステップ",
        "LIST",
        "JCL の DFHCSDUP のステップ",
    ),
    a(
        "CSD_MAPSET",
        "steps",
        "ステップ",
        "LIST",
        "JCL の DFHCSDUP のステップ",
    ),
];

/// An asset Zeus provides: a reference to it is resolved without a file.
pub struct SystemAsset {
    pub name: &'static str,
    pub node_type: &'static str,
    pub provider: &'static str,
    pub description: &'static str,
}

const fn s(
    name: &'static str,
    node_type: &'static str,
    provider: &'static str,
    description: &'static str,
) -> SystemAsset {
    SystemAsset {
        name,
        node_type,
        provider,
        description,
    }
}

const P: &str = "SYSTEM_PROGRAM";
const C: &str = "SYSTEM_COPYBOOK";

pub const SYSTEM_ASSETS: &[SystemAsset] = &[
    s("IEFBR14", P, "zeus-jcl", "何もしないプログラム"),
    s("IEBGENER", P, "zeus-jcl", "順次データセットの複写"),
    s("ICEGENER", P, "zeus-jcl", "IEBGENER と同じ"),
    s("IDCAMS", P, "zeus-jcl", "VSAM・カタログの操作"),
    s("SORT", P, "zeus-jcl", "ソート・マージ"),
    s("ICEMAN", P, "zeus-jcl", "SORT と同じ"),
    s("DFSORT", P, "zeus-jcl", "SORT と同じ"),
    s("SYNCSORT", P, "zeus-jcl", "SORT と同じ"),
    s("IKJEFT01", P, "zeus-jcl", "TSO バッチ (DSN RUN など)"),
    s("IKJEFT1A", P, "zeus-jcl", "TSO バッチ"),
    s("IKJEFT1B", P, "zeus-jcl", "TSO バッチ"),
    s("IGYCRCTL", P, "zeus-jcl", "COBOL コンパイラ"),
    s("IBMZPLI", P, "zeus-jcl", "PL/I コンパイラ"),
    s("IEL1AA", P, "zeus-jcl", "PL/I コンパイラ"),
    s("IEL0AA", P, "zeus-jcl", "PL/I コンパイラ"),
    s("IEWL", P, "zeus-jcl", "バインダー"),
    s("IEWBLINK", P, "zeus-jcl", "バインダー"),
    s("HEWL", P, "zeus-jcl", "バインダー"),
    s("HEWLH096", P, "zeus-jcl", "バインダー"),
    s("IEWLF128", P, "zeus-jcl", "バインダー"),
    s("DSNUTILB", P, "zeus-jcl", "Db2 ユーティリティ (LOAD)"),
    s("DSNTIAUL", P, "zeus-jcl", "Db2 のアンロード (DSN RUN)"),
    s("DSNTIAD", P, "zeus-jcl", "Db2 の SQL 実行 (DSN RUN)"),
    s("DSNTEP2", P, "zeus-jcl", "Db2 の SQL 実行 (DSN RUN)"),
    s("DSNTEP4", P, "zeus-jcl", "Db2 の SQL 実行 (DSN RUN)"),
    s(
        "DFSRRC00",
        P,
        "zeus-jcl",
        "IMS の領域 (DLI・DBB・BMP・MSG・ULU・ULR)",
    ),
    s("DFSURGU0", P, "zeus-jcl", "IMS のアンロード"),
    s("DFSURGL0", P, "zeus-jcl", "IMS の再ロード"),
    s("DFSURPR0", P, "zeus-jcl", "IMS の再編成前処理 (何もしない)"),
    s("SDSF", P, "zeus-jcl", "受け付けて無視する"),
    s("FTP", P, "zeus-jcl", "受け付けて無視する"),
    s(
        "COBDATFT",
        P,
        "zeus-jcl",
        "CardDemo のアセンブラー日付変換 (再実装)",
    ),
    s(
        "MVSWAIT",
        P,
        "zeus-jcl",
        "CardDemo のアセンブラー待機 (再実装)",
    ),
    s("CEE3ABD", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CEEDAYS", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CEEGMT", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CEELOCT", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CEEDATM", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CEEDATE", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CEESECS", P, "zeus-cobol", "LE 呼出し可能サービス"),
    s("CBLTDLI", P, "zeus-ims", "DL/I 呼出し (COBOL)"),
    s("PLITDLI", P, "zeus-ims", "DL/I 呼出し (PL/I)"),
    s("PLIRETC", P, "zeus-pli", "PL/I の戻りコード"),
    s("PLIDUMP", P, "zeus-pli", "PL/I のダンプ"),
    s("MQCONN", P, "zeus-mq", "MQI"),
    s("MQCONNX", P, "zeus-mq", "MQI"),
    s("MQDISC", P, "zeus-mq", "MQI"),
    s("MQOPEN", P, "zeus-mq", "MQI"),
    s("MQCLOSE", P, "zeus-mq", "MQI"),
    s("MQPUT", P, "zeus-mq", "MQI"),
    s("MQPUT1", P, "zeus-mq", "MQI"),
    s("MQGET", P, "zeus-mq", "MQI"),
    s("MQCMIT", P, "zeus-mq", "MQI"),
    s("MQBACK", P, "zeus-mq", "MQI"),
    s("MQINQ", P, "zeus-mq", "MQI"),
    s("MQSET", P, "zeus-mq", "MQI"),
    s("MQSUB", P, "zeus-mq", "MQI"),
    s("MQSUBRQ", P, "zeus-mq", "MQI"),
    s("MQCRTMH", P, "zeus-mq", "MQI"),
    s("MQDLTMH", P, "zeus-mq", "MQI"),
    s("MQSETMP", P, "zeus-mq", "MQI"),
    s("MQINQMP", P, "zeus-mq", "MQI"),
    s("MQDLTMP", P, "zeus-mq", "MQI"),
    s("MQCB", P, "zeus-mq", "MQI"),
    s("MQCTL", P, "zeus-mq", "MQI"),
    s(
        "SQLCA",
        C,
        "zeus-cobol / zeus-pli",
        "SQL 連絡域 (EXEC SQL INCLUDE SQLCA)",
    ),
    s("SQLDA", C, "zeus-cobol / zeus-pli", "SQL 記述子域"),
    s("DFHAID", C, "zeus-cobol / zeus-pli", "CICS の AID 定数"),
    s("DFHBMSCA", C, "zeus-cobol / zeus-pli", "BMS の属性定数"),
    s("CEEIGZCT", C, "zeus-cobol", "LE の条件トークン"),
];

/// The CMQ* members of the MQ interface (COBOL ...V/...L, PL/I ...P).
pub fn system_copybook(name: &str, pli: bool) -> Option<&'static str> {
    let n = name.to_ascii_uppercase();
    if let Some(a) = SYSTEM_ASSETS
        .iter()
        .find(|a| a.node_type == C && a.name == n)
    {
        return Some(a.provider);
    }
    let mq = if pli {
        zeus_cobol::front::syslib::pli_member(&n).is_some()
    } else {
        zeus_cobol::front::syslib::member(&n).is_some()
    };
    mq.then_some(if pli { "zeus-pli" } else { "zeus-cobol" })
}

/// A program Zeus provides.
pub fn system_program(name: &str) -> Option<&'static SystemAsset> {
    let n = name.to_ascii_uppercase();
    SYSTEM_ASSETS
        .iter()
        .find(|a| a.node_type == P && a.name == n)
}

pub fn node_type(code: &str) -> Option<&'static NodeType> {
    NODE_TYPES.iter().find(|t| t.code == code)
}

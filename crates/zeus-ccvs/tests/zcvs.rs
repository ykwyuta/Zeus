use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use zeus_ccvs::{
    jcl::export_jcl,
    manifest::Manifest,
    runner::{Outcome, RunOptions, ZCVS_MODULES, run},
    summary,
    toolchain::Toolchain,
};

#[derive(Debug, serde::Deserialize)]
struct Coverage {
    expect: String,
    tests: Vec<String>,
}

fn coverage() -> BTreeMap<String, Coverage> {
    serde_json::from_str(include_str!("../../../zcvs/coverage.json"))
        .expect("parse reviewed ZCVS coverage inventory")
}

#[test]
fn zcvs_coverage_inventory_matches_sources_and_manifest() {
    let suite = workspace_root().join("zcvs");
    let manifest = Manifest::load(&suite).unwrap().unwrap();
    let expected = coverage();
    let mut sources = BTreeSet::new();
    for module in ZCVS_MODULES {
        for entry in fs::read_dir(suite.join(module)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "CBL") {
                continue;
            }
            let program = path.file_stem().unwrap().to_str().unwrap().to_string();
            sources.insert(program.clone());
            let spec = expected.get(&program).expect("unreviewed ZCVS program");
            let text = fs::read_to_string(&path).unwrap();
            let names: Vec<_> = text
                .lines()
                .filter(|line| line.contains("TO ZCVS-TEST-NAME"))
                .map(|line| {
                    line.split('\'')
                        .nth(1)
                        .expect("literal test name")
                        .to_string()
                })
                .collect();
            assert_eq!(names, spec.tests, "{program}: scenario inventory changed");
            assert_eq!(
                names.iter().collect::<BTreeSet<_>>().len(),
                names.len(),
                "{program}: duplicate scenario ID"
            );
            assert!(names.iter().all(|name| name.len() <= 20));
            let configured = manifest.programs.get(&program).map(|cfg| &cfg.expect);
            use zeus_ccvs::manifest::ExpectKind;
            let kind = match configured {
                Some(ExpectKind::CompileError) => "compile-error",
                Some(ExpectKind::CompileOnly) => "compile-only",
                _ => "run",
            };
            assert_eq!(spec.expect, kind, "{program}: execution contract changed");
            assert_eq!(names.is_empty(), kind != "run", "{program}");
            if kind == "compile-error" {
                assert!(manifest.programs[&program].diagnostic_contains.is_some());
            }
        }
    }
    assert_eq!(sources, expected.keys().cloned().collect());
    assert!(manifest.programs.keys().all(|name| sources.contains(name)));
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn collect_cbl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("read_dir");
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_cbl_files(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("CBL") || e.eq_ignore_ascii_case("JCL"))
        {
            out.push(path);
        }
    }
}

#[test]
fn zcvs_sources_are_fixed_80_columns() {
    let root = workspace_root();
    let zcvs = root.join("zcvs");
    let mut files = Vec::new();
    collect_cbl_files(&zcvs, &mut files);
    assert!(!files.is_empty(), "expected ZCVS source files");

    for file in files {
        let content = fs::read_to_string(&file).expect("read ZCVS file");
        for (idx, line) in content.lines().enumerate() {
            let char_len = line.chars().count();
            assert!(
                char_len <= 72,
                "{}:{}: line length {} exceeds Area B column 72: {:?}",
                file.display(),
                idx + 1,
                char_len,
                line
            );
        }
    }
}

#[test]
fn zcvs_manifest_and_export_jcl_are_valid() {
    let root = workspace_root();
    let zcvs = root.join("zcvs");
    let manifest = Manifest::load(&zcvs)
        .expect("load manifest")
        .expect("manifest exists");
    assert_eq!(manifest.name, "ZCVS");
    assert_eq!(
        manifest.required_modules,
        vec!["ZD", "ZA", "ZN", "ZP", "ZF", "ZS", "ZE"]
    );
    assert_eq!(manifest.optional_modules, vec!["ZX"]);

    let tmp = tempfile::tempdir().expect("tempdir");
    let modules: Vec<String> = ZCVS_MODULES.iter().map(|s| (*s).to_string()).collect();
    let report = export_jcl(&zcvs, tmp.path(), "IBMUSER.ZCVS", &modules).expect("export_jcl");

    assert!(report.copy_members >= 5);
    assert!(report.source_members >= 40);
    assert_eq!(report.jcl_members, 9);
    assert!(tmp.path().join("JCL/ZCVSALL.JCL").is_file());

    let mut exported = Vec::new();
    collect_cbl_files(tmp.path(), &mut exported);
    for file in exported {
        let content = fs::read_to_string(&file).expect("read exported file");
        for (idx, line) in content.lines().enumerate() {
            let cols = line.chars().count();
            assert!(
                cols <= 80,
                "{}:{}: exported line column length {} exceeds 80",
                file.display(),
                idx + 1,
                cols
            );
        }
    }
}

fn find_or_build_zcobc(root: &Path) -> PathBuf {
    static COMPILER: OnceLock<PathBuf> = OnceLock::new();
    COMPILER
        .get_or_init(|| {
            // An existing executable can predate the sources under test. Ask Cargo
            // to check freshness, and build the runtime used by native scenarios.
            let status = Command::new("cargo")
                .args([
                    "build",
                    "-p",
                    "zeus-cobol",
                    "--bin",
                    "zcobc",
                    "-p",
                    "zeus-native",
                    "--bin",
                    "zeus-rt",
                ])
                .current_dir(root)
                .status()
                .expect("cargo build zcobc");
            assert!(status.success(), "cargo build zcobc failed");
            let target = std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join("target"));
            root.join(target)
                .join("debug")
                .join(format!("zcobc{}", std::env::consts::EXE_SUFFIX))
        })
        .clone()
}

#[test]
fn zcvs_suite_passes_on_zeus_toolchain() {
    check_suite(false);
}

#[test]
fn zcvs_suite_passes_on_native_toolchain() {
    check_suite(true);
}

fn check_suite(native: bool) {
    let root = workspace_root();
    let zcobc = find_or_build_zcobc(&root);
    let zcvs = root.join("zcvs");
    let tmp = tempfile::tempdir().expect("tempdir");

    let tc = if native {
        Toolchain::zeus_native(zcobc)
    } else {
        Toolchain::zeus(zcobc)
    };
    let res = run(
        &tc,
        &RunOptions {
            suite: zcvs,
            work: tmp.path().to_path_buf(),
            modules: ZCVS_MODULES.iter().map(|s| (*s).to_string()).collect(),
            programs: Vec::new(),
            jobs: 4,
            timeout: Duration::from_secs(30),
        },
    )
    .expect("run ZCVS suite");

    let md = summary::markdown(&res, None);
    let expected = coverage();
    let mut actual_programs = BTreeSet::new();
    assert_eq!(res.modules.len(), ZCVS_MODULES.len());
    for module in &res.modules {
        assert!(
            !module.programs.is_empty(),
            "empty module {}",
            module.module
        );
        for prog in &module.programs {
            assert!(
                actual_programs.insert(prog.program.clone()),
                "duplicate program"
            );
            assert!(
                prog.outcome.ok(),
                "module {} program {} did not pass: {:?}\n{md}",
                module.module,
                prog.program,
                prog.outcome
            );
            let spec = &expected[&prog.program];
            match (&prog.outcome, spec.expect.as_str()) {
                (Outcome::Compiled, "compile-only") => {}
                (Outcome::Ran { counts }, kind @ ("compile-error" | "run")) => {
                    let total = if kind == "compile-error" {
                        1
                    } else {
                        spec.tests.len() as u32
                    };
                    assert_eq!(counts.total, total, "{}: missing assertions", prog.program);
                    assert_eq!(
                        counts.pass, total,
                        "{}: not all assertions passed",
                        prog.program
                    );
                    assert_eq!(counts.deleted, 0, "{}: deleted assertions", prog.program);
                    assert_eq!(counts.inspect, 0, "{}: manual assertions", prog.program);
                    if kind == "run" {
                        let report = fs::read_to_string(
                            tmp.path()
                                .join(&module.module)
                                .join(format!("{}.log", prog.program)),
                        )
                        .expect("read scenario report");
                        let names: Vec<_> = report
                            .lines()
                            .filter_map(|line| {
                                let mut words = line.split_whitespace();
                                (words.next() == Some("PASS"))
                                    .then(|| words.next().unwrap().to_string())
                            })
                            .collect();
                        assert_eq!(
                            names, spec.tests,
                            "{}: scenario sequence changed",
                            prog.program
                        );
                    }
                }
                _ => panic!("{}: unexpected outcome for {}", prog.program, spec.expect),
            }
        }
    }
    assert_eq!(
        actual_programs,
        expected.keys().cloned().collect(),
        "programs were omitted"
    );
    check_multireel_tape(&tmp.path().join("ZF"));
}

/// Inspect the fixture's physical image without using the production tape
/// decoder. COBOL readback alone could hide matching encoder/decoder mistakes.
fn check_multireel_tape(dir: &Path) {
    let payloads: [&[u8]; 4] = [
        &[0xc6, 0xc9, 0xd9, 0xe2, 0xe3, 0xf0, 0xf0, 0xf1],
        &[0xe2, 0xc5, 0xc3, 0xd6, 0xd5, 0xc4, 0xf0, 0xf2],
        &[],
        &[0xc6, 0xd6, 0xe4, 0xd9, 0xe3, 0xc8, 0xf0, 0xf4],
    ];
    for (index, payload) in payloads.iter().enumerate() {
        let name = if index == 0 {
            "TAPE202.aws".to_string()
        } else {
            format!("TAPE202.vol{:03}.aws", index + 1)
        };
        let bytes = fs::read(dir.join(&name)).expect("read ZF202M volume");
        let mut entries = Vec::new();
        let mut offset = 0;
        let mut previous = 0;
        while offset < bytes.len() {
            assert!(bytes.len() - offset >= 6, "{name}: truncated header");
            let header = &bytes[offset..offset + 6];
            let len = u16::from_le_bytes([header[0], header[1]]) as usize;
            assert_eq!(
                u16::from_le_bytes([header[2], header[3]]) as usize,
                previous,
                "{name}: previous block length"
            );
            assert_eq!(header[5], 0, "{name}: reserved flags");
            // These small records each occupy one complete AWS segment.
            assert_eq!(header[4], if len == 0 { 0x40 } else { 0xa0 });
            offset += 6;
            assert!(len <= bytes.len() - offset, "{name}: truncated block");
            entries.push(&bytes[offset..offset + len]);
            offset += len;
            previous = len;
        }
        let has_data = !payload.is_empty();
        assert_eq!(entries.len(), if has_data { 10 } else { 9 }, "{name}");
        let trailer = if has_data { 6 } else { 5 };
        let trailer_ids = if index == 3 {
            [[0xc5, 0xd6, 0xc6, 0xf1], [0xc5, 0xd6, 0xc6, 0xf2]]
        } else {
            [[0xc5, 0xd6, 0xe5, 0xf1], [0xc5, 0xd6, 0xe5, 0xf2]]
        };
        for (position, id) in [
            (0, [0xe5, 0xd6, 0xd3, 0xf1]), // VOL1
            (1, [0xc8, 0xc4, 0xd9, 0xf1]), // HDR1
            (2, [0xc8, 0xc4, 0xd9, 0xf2]), // HDR2
            (trailer, trailer_ids[0]),
            (trailer + 1, trailer_ids[1]),
        ] {
            assert_eq!(entries[position].len(), 80, "{name}: label length");
            assert_eq!(&entries[position][..4], &id, "{name}: label order");
        }
        for position in [3, trailer - 1, trailer + 2, trailer + 3] {
            assert!(entries[position].is_empty(), "{name}: tape mark");
        }
        if has_data {
            assert_eq!(entries[4], *payload, "{name}: partial block payload");
        }
        // EBCDIC F, BLKSIZE 00016, LRECL 00008 in HDR2/EOV2/EOF2.
        let format = [
            0xc6, 0xf0, 0xf0, 0xf0, 0xf1, 0xf6, 0xf0, 0xf0, 0xf0, 0xf0, 0xf8,
        ];
        assert_eq!(&entries[2][4..15], &format, "{name}: HDR2 format");
        assert_eq!(&entries[trailer + 1][4..15], &format);
        let mut count = [0xf0; 6];
        count[5] = if has_data { 0xf1 } else { 0xf0 };
        assert_eq!(&entries[trailer][54..60], &count, "{name}: block count");
        assert_eq!(&entries[trailer][76..80], &[0x40; 4]);
    }
    assert!(!dir.join("TAPE202.vol005.aws").exists());
}

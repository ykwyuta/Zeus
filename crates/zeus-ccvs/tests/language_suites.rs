use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use zeus_ccvs::{
    jcl,
    manifest::{ExpectKind, Manifest},
    report,
    runner::{self, Outcome, RunOptions},
    toolchain::{AsmFormat, Toolchain},
};

#[derive(serde::Deserialize)]
struct Scenario {
    expect: String,
    tests: Vec<String>,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn inventory(suite: &str) -> BTreeMap<String, Scenario> {
    serde_json::from_str(match suite {
        "zpvs" => include_str!("../../../zpvs/coverage.json"),
        _ => include_str!("../../../zhvs/coverage.json"),
    })
    .unwrap()
}
fn tools_dir() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let status = Command::new("cargo")
                .args([
                    "build",
                    "-p",
                    "zeus-cobol",
                    "--bin",
                    "zcobc",
                    "--bin",
                    "zas",
                    "-p",
                    "zeus-pli",
                    "--bin",
                    "zpli",
                    "-p",
                    "zeus-native",
                    "--bin",
                    "zeus-rt",
                ])
                .current_dir(root())
                .status()
                .unwrap();
            assert!(status.success(), "build validation tools");
            let target = std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| root().join("target"));
            root().join(target).join("debug")
        })
        .clone()
}

#[test]
fn inventories_pair_diagnostics_and_cover_every_source_in_report_order() {
    for suite in ["zpvs", "zhvs"] {
        let dir = root().join(suite);
        let manifest = Manifest::load(&dir).unwrap().unwrap();
        let specs = inventory(suite);
        let ext = if suite == "zpvs" { "PLI" } else { "ASM" };
        let mut found = BTreeSet::new();
        for module in manifest.all_modules() {
            let mut count = 0;
            for entry in fs::read_dir(dir.join(module)).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_none_or(|x| x != ext) {
                    continue;
                }
                count += 1;
                let name = path.file_stem().unwrap().to_str().unwrap();
                found.insert(name.to_string());
                let source = fs::read_to_string(&path).unwrap();
                for (index, line) in source.lines().enumerate() {
                    assert!(
                        line.len() <= 72,
                        "{}:{} exceeds margins",
                        path.display(),
                        index + 1
                    );
                }
                let ids: Vec<String> = source
                    .lines()
                    .filter_map(|line| {
                        if suite == "zpvs" {
                            line.trim()
                                .strip_prefix("CALL ZCHECK('")
                                .map(|s| s.split('\'').next().unwrap().to_string())
                        } else {
                            line.trim()
                                .strip_prefix("ZHVSCHK ")
                                .map(|s| s.split(',').next().unwrap().to_string())
                        }
                    })
                    .collect();
                let spec = &specs[name];
                assert_eq!(ids, spec.tests, "{name}: reviewed scenario IDs changed");
                assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
                let cfg = manifest.programs.get(name);
                let kind = cfg.map(|c| &c.expect).unwrap_or(&ExpectKind::Run);
                match spec.expect.as_str() {
                    "run" => {
                        assert_eq!(kind, &ExpectKind::Run);
                        assert!(!ids.is_empty());
                    }
                    "compile-error" => {
                        assert_eq!(kind, &ExpectKind::CompileError);
                        assert!(
                            cfg.unwrap()
                                .diagnostic_contains
                                .as_ref()
                                .is_some_and(|s| !s.is_empty())
                        );
                        let positive = format!("{}P", &name[..name.len() - 1]);
                        assert_eq!(specs[&positive].expect, "compile-only");
                    }
                    "compile-only" => assert_eq!(kind, &ExpectKind::CompileOnly),
                    other => panic!("unknown scenario kind {other}"),
                }
            }
            assert!(count > 0, "empty required module");
        }
        assert_eq!(found, specs.keys().cloned().collect());
        assert!(manifest.programs.keys().all(|p| specs.contains_key(p)));
        // All 54 cross-radix PFPO selectors are distinct compilation units.
        if suite == "zhvs" {
            assert_eq!(specs.keys().filter(|n| n.starts_with("HF2")).count(), 54);
        }
    }
}

#[test]
fn jcl_exports_the_right_compiler_sources_and_shared_adapters() {
    for (suite, compiler, extension) in [("zpvs", "IBMZPLI", "PLI"), ("zhvs", "ASMA90", "ASM")] {
        let out = tempfile::tempdir().unwrap();
        let result =
            jcl::export_jcl(&root().join(suite), out.path(), "USER.VALIDATE", &[]).unwrap();
        assert_eq!(result.jcl_members, 8);
        let all = fs::read_to_string(
            out.path()
                .join(format!("JCL/{}ALL.JCL", suite.to_uppercase())),
        )
        .unwrap();
        assert!(all.contains(&format!("PGM={compiler}")));
        assert!(all.contains("PGM=IEWL"));
        assert!(all.contains("PGM=IDCAMS"));
        assert!(all.contains("//XXXXX001 DD DSN="));
        if suite == "zpvs" {
            assert!(all.contains("//PFPRINT  DD DSN="));
            assert!(all.contains("//PFPRNEW  DD DSN="));
        }
        if suite == "zhvs" {
            assert!(all.contains("ZHVRPT"));
        }
        for directory in ["COPY", "SRCLIB", "JCL"] {
            for entry in fs::read_dir(out.path().join(directory)).unwrap() {
                let path = entry.unwrap().path();
                for line in fs::read_to_string(&path).unwrap().lines() {
                    assert!(line.len() <= 80, "{}: {line}", path.display());
                }
            }
        }
        assert!(
            out.path()
                .join(format!(
                    "SRCLIB/{}101A.{extension}",
                    if suite == "zpvs" { "PD" } else { "HA" }
                ))
                .is_file()
        );
    }
}

fn check(suite: &str, native: bool, format: AsmFormat) {
    let tools = tools_dir();
    let tc = if suite == "zpvs" {
        Toolchain::Zpli {
            compiler: tools.join(format!("zpli{}", std::env::consts::EXE_SUFFIX)),
            native,
        }
    } else {
        Toolchain::Zas {
            compiler: tools.join(format!("zas{}", std::env::consts::EXE_SUFFIX)),
            format,
        }
    };
    let dir = root().join(suite);
    let manifest = Manifest::load(&dir).unwrap().unwrap();
    let specs = inventory(suite);
    let temp = tempfile::tempdir().unwrap();
    let results = runner::run(
        &tc,
        &RunOptions {
            suite: dir,
            work: temp.path().to_path_buf(),
            modules: manifest.all_modules(),
            programs: vec![],
            jobs: 4,
            timeout: Duration::from_secs(30),
        },
    )
    .unwrap();
    let mut names = BTreeSet::new();
    for module in results.modules {
        for program in module.programs {
            names.insert(program.program.clone());
            let spec = &specs[&program.program];
            assert!(
                program.outcome.ok(),
                "{} {}: {:?}; inspect CLI run for logs",
                tc.name(),
                program.program,
                program.outcome
            );
            match (program.outcome, spec.expect.as_str()) {
                (Outcome::Compiled, "compile-only") => {}
                (Outcome::Ran { counts }, "compile-error") => {
                    assert_eq!((counts.total, counts.pass), (1, 1))
                }
                (Outcome::Ran { counts }, "run") => {
                    assert_eq!(
                        counts.total as usize,
                        spec.tests.len(),
                        "{}",
                        program.program
                    );
                    assert_eq!(counts.pass, counts.total);
                    assert_eq!((counts.deleted, counts.inspect), (0, 0));
                    let raw = fs::read(
                        temp.path()
                            .join(&module.module)
                            .join(format!("{}.log", program.program)),
                    )
                    .unwrap();
                    let text = String::from_utf8(report::normalize(&raw)).unwrap();
                    let ids: Vec<String> = text
                        .lines()
                        .filter_map(|line| {
                            line.trim()
                                .strip_prefix("PASS")
                                .map(|s| s.trim().to_string())
                        })
                        .collect();
                    assert_eq!(ids, spec.tests, "{}: report order/IDs", program.program);
                    assert!(text.contains("TESTS WERE EXECUTED SUCCESSFULLY"));
                    assert!(text.contains("NO TEST(S) DELETED"));
                }
                (outcome, kind) => panic!("unexpected {outcome:?} for {kind}"),
            }
        }
    }
    assert_eq!(names, specs.keys().cloned().collect());
}

#[test]
fn zpvs_interpreter() {
    check("zpvs", false, AsmFormat::Zeus);
}
#[test]
fn zpvs_native() {
    check("zpvs", true, AsmFormat::Zeus);
}
#[test]
fn zhvs_zeus() {
    check("zhvs", false, AsmFormat::Zeus);
}
#[test]
fn zhvs_obj() {
    check("zhvs", false, AsmFormat::Obj);
}
#[test]
fn zhvs_goff() {
    check("zhvs", false, AsmFormat::Goff);
}

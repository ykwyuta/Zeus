use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use zeus_ccvs::{
    jcl::export_jcl,
    manifest::Manifest,
    runner::{RunOptions, ZCVS_MODULES, run},
    summary,
    toolchain::Toolchain,
};

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
    let exe_name = format!("zcobc{}", std::env::consts::EXE_SUFFIX);
    let candidate = root.join("target").join("debug").join(&exe_name);
    if !candidate.is_file() {
        let status = Command::new("cargo")
            .args(["build", "--bin", "zcobc", "-p", "zeus-cobol"])
            .current_dir(root)
            .status()
            .expect("cargo build zcobc");
        assert!(status.success(), "cargo build zcobc failed");
    }
    candidate
}

#[test]
fn zcvs_suite_passes_on_zeus_toolchain() {
    let root = workspace_root();
    let zcobc = find_or_build_zcobc(&root);
    let zcvs = root.join("zcvs");
    let tmp = tempfile::tempdir().expect("tempdir");

    let tc = Toolchain::zeus(zcobc);
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
    for module in &res.modules {
        for prog in &module.programs {
            assert!(
                prog.outcome.ok(),
                "module {} program {} did not pass: {:?}\n{md}",
                module.module,
                prog.program,
                prog.outcome
            );
        }
    }
}

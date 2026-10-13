//! Coverage of a COBOL program: `zcobc run --coverage` twice (the counts
//! add up), then `zcov report` with the details and an LCOV file.

use std::path::Path;
use std::process::Command;

#[test]
fn cobol_coverage_report() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/coverage.cbl");
    let dir = tempfile::tempdir().unwrap();
    let zo = dir.path().join("COV1.zo");
    let data = dir.path().join("cov");
    let zcobc = env!("CARGO_BIN_EXE_zcobc");
    assert!(
        Command::new(zcobc)
            .args(["compile", "-o"])
            .arg(&zo)
            .arg(&src)
            .status()
            .unwrap()
            .success()
    );
    for _ in 0..2 {
        let out = Command::new(zcobc)
            .args(["run", "--coverage"])
            .arg(&data)
            .arg(&zo)
            .env_remove("ZEUS_COVERAGE")
            .output()
            .unwrap();
        assert!(out.status.success());
    }
    let lcov = dir.path().join("cov.info");
    let out = Command::new(env!("CARGO_BIN_EXE_zcov"))
        .args(["report", "--detail", "--lcov"])
        .arg(&lcov)
        .arg(&data)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let rows: Vec<String> = text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(
        rows[1..],
        [
            "COBOL COV1 69.2% 9/13 60.0% 6/10 50.0% 1/2",
            "not run coverage.cbl:19",
            "not run coverage.cbl:20",
            "not run coverage.cbl:23",
            "not run coverage.cbl:28",
            // EVALUATE: WHEN 'A' always matches; WHEN 'B' is not tested
            "branch never taken coverage.cbl:17",
            "branch never reached coverage.cbl:17",
            // IF N > 100 never true
            "branch always taken coverage.cbl:22",
            "not entered NEVER-PARA (coverage.cbl:27)",
            "TOTAL 69.2% 9/13 60.0% 6/10 50.0% 1/2",
        ],
        "{text}"
    );
    let info = std::fs::read_to_string(&lcov).unwrap();
    // two runs: the loop test ran 2 x 6 times, the IF in it 2 x 5
    for want in [
        "FNDA:2,MAIN-PARA\n",
        "FNDA:0,NEVER-PARA\n",
        "DA:11,10\n",
        "DA:12,4\n",
        "DA:14,6\n",
        "BRDA:10,1,0,2\n",
        "BRDA:10,1,1,10\n",
        "LF:13\n",
        "LH:9\n",
    ] {
        assert!(info.contains(want), "{want}{info}");
    }
    assert!(info.contains(&format!(
        "SF:{}\n",
        std::fs::canonicalize(&src).unwrap().display()
    )));
}

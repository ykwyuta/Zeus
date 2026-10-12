//! The symbolic maps generated for the CardDemo mapsets must match the
//! copybooks shipped with CardDemo (compared without comments, sequence
//! columns and spacing). Runs when CardDemo has been fetched with
//! tools/fetch-carddemo.sh.

use std::path::Path;

fn normalize(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('*') && l.chars().nth(6) != Some('*'))
        .map(|l| {
            let area: String = l.chars().skip(7).take(65).collect();
            area.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .filter(|l| !l.is_empty())
        .collect()
}

#[test]
fn carddemo_symbolic_maps() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cache/carddemo/app");
    let Ok(dir) = std::fs::read_dir(app.join("bms")) else {
        eprintln!("CardDemo not fetched; skipped");
        return;
    };
    let mut n = 0;
    for e in dir {
        let p = e.unwrap().path();
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let ms = zeus_bms::parse(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let expected = std::fs::read_to_string(app.join(format!("cpy-bms/{name}.CPY"))).unwrap();
        assert_eq!(
            normalize(&zeus_bms::symbolic::cobol(&ms)),
            normalize(&expected),
            "{name}"
        );
        n += 1;
    }
    assert!(n > 0);
}

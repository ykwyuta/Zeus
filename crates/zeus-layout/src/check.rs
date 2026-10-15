//! Checks a written copybook by compiling it: each item must get the
//! position and length it has in the program.

use zeus_cobol::front::{self, FileTable, FrontOptions};
use zeus_cobol::sema::data::{self, DataModel, Settings};

/// An item as the copybook must lay it out: name, position from 1,
/// length (of one occurrence).
#[derive(Debug, Clone, PartialEq)]
pub struct Expect {
    pub name: String,
    pub pos: u32,
    pub len: u32,
}

fn items(dm: &DataModel, id: usize, base: u32, out: &mut Vec<Expect>) {
    let it = &dm.items[id];
    out.push(Expect {
        name: it.name.clone().unwrap_or_else(|| "FILLER".into()),
        pos: it.offset - base + 1,
        len: it.len,
    });
    for &c in &it.children {
        items(dm, c, base, out);
    }
}

/// Compiles `copybook` (with 01 items for the names in `extra`, such as
/// OCCURS DEPENDING ON objects outside the record) and compares its
/// first record with `expect`.
pub fn check(copybook: &str, extra: &[String], expect: &[Expect]) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    std::fs::write(dir.path().join("ZLAYOUT.cpy"), copybook).map_err(|e| e.to_string())?;
    let mut src = String::from(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. ZLCHECK.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       COPY ZLAYOUT.\n",
    );
    for n in extra {
        src.push_str(&format!("       01  {n} PIC S9(9) COMP.\n"));
    }
    src.push_str("       PROCEDURE DIVISION.\n           GOBACK.\n");
    let path = dir.path().join("ZLCHECK.cbl");
    std::fs::write(&path, src).map_err(|e| e.to_string())?;
    let opts = FrontOptions {
        copy_dirs: vec![dir.path().to_path_buf()],
        ..Default::default()
    };
    let mut files = FileTable::default();
    let progs = front::parse_file(&path, &opts, &mut files).map_err(|d| d.to_string())?;
    let p = progs.first().ok_or("no program")?;
    let dm = data::build(
        p,
        &Settings {
            decimal_comma: false,
            currency: '$',
        },
    )
    .map_err(|d| d.to_string())?;
    let root = dm
        .items
        .iter()
        .position(|i| i.parent.is_none() && i.level == 1)
        .ok_or("no record")?;
    let mut got = Vec::new();
    items(&dm, root, dm.items[root].offset, &mut got);
    if got.len() != expect.len() {
        return Err(format!("{} items instead of {}", got.len(), expect.len()));
    }
    for (g, e) in got.iter().zip(expect) {
        if g != e {
            return Err(format!(
                "{}: position {} length {} instead of {} {} ({})",
                e.name, g.pos, g.len, e.pos, e.len, g.name
            ));
        }
    }
    Ok(())
}

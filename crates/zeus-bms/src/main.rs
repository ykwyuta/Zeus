//! `zbms` - processes a BMS mapset.
//!
//! ```text
//! zbms <mapset.bms> [--pli] [--copy <symbolic>] [--map <physical.json>]
//! ```
//!
//! Without options the symbolic map is written to standard output; with
//! --pli it is a PL/I include member instead of a COBOL copybook.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = None;
    let mut copy = None;
    let mut map = None;
    let mut pli = false;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--copy" => copy = it.next(),
            "--map" => map = it.next(),
            "--pli" => pli = true,
            _ => input = Some(a),
        }
    }
    let Some(input) = input else {
        eprintln!("usage: zbms <mapset.bms> [--pli] [--copy <symbolic>] [--map <physical.json>]");
        return ExitCode::from(2);
    };
    let text = match std::fs::read(&input) {
        Ok(t) => String::from_utf8_lossy(&t).into_owned(),
        Err(e) => {
            eprintln!("zbms: {input}: {e}");
            return ExitCode::from(16);
        }
    };
    let ms = match zeus_bms::parse(&text) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("zbms: {input}: {e}");
            return ExitCode::from(12);
        }
    };
    let sym = if pli {
        zeus_bms::symbolic::pli(&ms)
    } else {
        zeus_bms::symbolic::cobol(&ms)
    };
    let write = |path: &str, data: &[u8]| -> bool {
        match std::fs::write(path, data) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("zbms: {path}: {e}");
                false
            }
        }
    };
    let mut ok = true;
    match &copy {
        Some(p) => ok &= write(p, sym.as_bytes()),
        None if map.is_none() => print!("{sym}"),
        None => {}
    }
    if let Some(p) = &map {
        let json = serde_json::to_vec_pretty(&ms).expect("serializable");
        ok &= write(p, &json);
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(16)
    }
}

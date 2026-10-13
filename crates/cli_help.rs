//! Help for the small, manually parsed command-line drivers.
//! Manuals are embedded with include_str!: no documentation files are needed
//! at runtime. Recognize help before opening catalogs, files or connections.

pub fn show(manual: &str, value_options: &[&str]) -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if requested(&args, value_options) {
        print!("{manual}");
        true
    } else {
        false
    }
}

fn requested(args: &[String], value_options: &[&str]) -> bool {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--" {
            break;
        }
        if value_options.contains(&arg.as_str()) {
            it.next(); // A PARM, path or other option value can literally be --help.
        } else if matches!(arg.as_str(), "--help" | "-h") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::requested;

    #[test]
    fn help_does_not_consume_option_values() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(requested(
            &args(&["catalog", "import", "--help"]),
            &["--home"]
        ));
        assert!(!requested(&args(&["--parm", "--help"]), &["--parm"]));
        assert!(requested(&args(&["--parm", "--help", "-h"]), &["--parm"]));
        assert!(!requested(&args(&["--", "--help"]), &[]));
    }
}

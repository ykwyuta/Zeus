//! Programs of the suite that need non-default handling.
//!
//! Most programs compile, run and check themselves. The exceptions below are
//! described by what they *need*; each toolchain maps the needs to its own
//! options. The list follows the requirements stated in the programs'
//! comments and matches the practice of the GnuCOBOL test driver.

use crate::report::Style;

/// Compiler features a program needs beyond the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// A source listing must be produced (the listing is inspected).
    Listing,
    /// Obsolete elements must be flagged by the compiler.
    FlagObsolete,
    /// Debugging lines (`D` in column 7) must be compiled.
    DebuggingLines,
    /// GO TO into a section must not be diagnosed.
    AllowGotoSection,
    /// Run-time data-incompatibility checks must be disabled.
    NoDataIncompatibleCheck,
}

#[derive(Debug, Clone, Copy)]
pub struct Special {
    /// Only compiled; the program is not run (compile-time tests).
    pub compile_only: bool,
    /// The program writes no report; a clean run counts as one passed test.
    pub no_output: bool,
    /// Bytes fed to standard input.
    pub stdin: Option<&'static [u8]>,
    /// A non-zero exit status is expected (the program is interrupted).
    pub may_fail: bool,
    pub needs: &'static [Need],
    pub style: Style,
}

const DEFAULT: Special = Special {
    compile_only: false,
    no_output: false,
    stdin: None,
    may_fail: false,
    needs: &[],
    style: Style::Normal,
};

pub fn special(program: &str) -> Special {
    match program {
        "NC401M" | "RL301M" | "RL401M" | "IC401M" | "IX301M" | "IX401M" | "SQ303M" | "SQ401M"
        | "ST301M" | "RW301M" | "RW302M" | "DB205A" => Special {
            compile_only: true,
            ..DEFAULT
        },
        "NC110M" | "NC214M" | "OBSQ3A" | "ST102A" | "ST109A" | "ST110A" | "ST112M" | "ST113M"
        | "ST115A" | "ST116A" | "ST120A" | "ST122A" | "ST123A" | "DB301M" | "DB302M" | "DB303M"
        | "DB305M" | "IF402M" => Special {
            no_output: true,
            ..DEFAULT
        },
        "OBNC1M" => Special {
            // eight empty lines followed by an interrupt character
            stdin: Some(b"\n\n\n\n\n\n\n\n\x03\n"),
            may_fail: true,
            ..DEFAULT
        },
        "NC114M" => Special {
            needs: &[Need::Listing],
            ..DEFAULT
        },
        "NC302M" => Special {
            needs: &[Need::FlagObsolete],
            stdin: Some(b"\n"),
            ..DEFAULT
        },
        "DB304M" => Special {
            needs: &[Need::FlagObsolete],
            ..DEFAULT
        },
        "SM206A" => Special {
            needs: &[Need::DebuggingLines],
            ..DEFAULT
        },
        "IC115A" => Special {
            needs: &[Need::AllowGotoSection],
            ..DEFAULT
        },
        "DB201A" => Special {
            needs: &[Need::NoDataIncompatibleCheck],
            ..DEFAULT
        },
        "NC107A" => Special {
            style: Style::FigurativeConstants,
            ..DEFAULT
        },
        "NC113M" => Special {
            style: Style::MarginSequence,
            ..DEFAULT
        },
        "NC121M" | "NC220M" => Special {
            style: Style::DisplayMatch,
            ..DEFAULT
        },
        "NC135A" => Special {
            style: Style::TableSequence,
            ..DEFAULT
        },
        _ => DEFAULT,
    }
}

//! Calls between separately loaded programs of different languages (or
//! of different PL/I modules).
//!
//! Arguments are passed by reference: the caller copies each argument's
//! storage into an [`XArg`], the callee's run time places the bytes in
//! its own storage for the call, and the caller copies them back
//! afterwards. Data keeps its z/OS representation on both sides.

/// An argument of a cross-module call.
#[derive(Debug, Clone, Default)]
pub struct XArg {
    pub data: Vec<u8>,
    /// OMITTED (a null address).
    pub omitted: bool,
    /// PL/I descriptor: bounds of each dimension (empty: none known).
    pub dims: Vec<(i64, i64)>,
    /// PL/I descriptor: string length.
    pub len: Option<u32>,
}

impl XArg {
    pub fn bytes(data: Vec<u8>) -> XArg {
        XArg {
            data,
            ..XArg::default()
        }
    }

    pub fn omitted() -> XArg {
        XArg {
            omitted: true,
            ..XArg::default()
        }
    }
}

/// How the callee ended.
#[derive(Debug, Clone, Default)]
pub struct XEnd {
    /// Return code (COBOL RETURN-CODE, PL/I PLIRETV).
    pub rc: i32,
    /// The value of a function (in the callee's RETURNS representation).
    pub ret: Option<Vec<u8>>,
    /// The callee ended the run unit (STOP RUN, STOP, EXIT).
    pub stop: bool,
}

/// Error text of a callee that ended abnormally: this prefix and the
/// abend code (e.g. `U4038`).
pub const ABEND: &str = "\u{0}abend ";

/// Finds and runs programs that are not in the calling run time.
pub trait Linker {
    /// Calls `name`; None when no such program is known.
    fn call(&self, name: &str, args: &mut [XArg]) -> Option<Result<XEnd, String>>;
}

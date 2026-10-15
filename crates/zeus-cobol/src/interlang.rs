//! Calls between programs of different languages (and between PL/I
//! modules) in one run unit.
//!
//! The run unit has one storage: that of its COBOL machine
//! ([`crate::vm::Machine`]), which also holds the COBOL programs, their
//! files and the EXEC interface. A PL/I module keeps its static storage,
//! heap and stack in it, and runs with the storage lent to it. Arguments
//! are passed by reference: their addresses in this storage.

use crate::vm::Machine;

/// An argument passed by reference.
#[derive(Debug, Clone, Default)]
pub struct XRef {
    /// Its address (None: OMITTED).
    pub addr: Option<usize>,
    /// Length of its storage.
    pub len: usize,
    /// PL/I descriptor: bounds of each dimension (empty: none known).
    pub dims: Vec<(i64, i64)>,
    /// PL/I descriptor: string length.
    pub strlen: Option<u32>,
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

/// Finds and runs the programs that are not COBOL programs of the run
/// unit (PL/I modules).
pub trait Linker {
    /// Number of raw address arguments for a fixed OS parameter list, when
    /// the loaded entry has a known signature.
    fn parameter_count(&self, _name: &str) -> Option<usize> {
        None
    }
    /// Calls `name` with the run unit's storage `sys` lent to it; gives
    /// the storage back, and None as the result when no such program is
    /// known.
    fn call(
        &self,
        name: &str,
        sys: Machine,
        args: &[XRef],
    ) -> (Machine, Option<Result<XEnd, String>>);

    /// Loads program `name` (FETCH); false when no such program is known.
    fn load(&self, _name: &str) -> bool {
        false
    }

    /// Releases program `name` (RELEASE): the next FETCH loads it afresh.
    fn release(&self, _name: &str) {}
}

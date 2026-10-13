//! Dependency analysis of COBOL, PL/I and JCL sources (`zdeps`).
//!
//! Files are nodes and references (CALL, COPY, EXEC PGM=, ...) are edges,
//! with the places they are read from. The analysis follows references
//! recursively from the root files and reports, as CSV files, the nodes,
//! the edges, their attributes, the locations of the edges and the
//! missing assets, with the masters that define the node types, edge
//! types and attributes (see `master`). See docs/proposal/0011.

pub mod graph;
pub mod master;
pub mod report;
pub mod scan;

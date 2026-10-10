# Zeus

A z/OS-compatible application runtime written in Rust: a COBOL compiler for
the Enterprise COBOL 4.2 dialect (z/OS 1.12 era), together with JCL, data sets
/ VSAM, CICS and Db2 support, aimed at running CardDemo, GenApp and Bank of Z
from source.

Plans and design decisions are in [docs/proposal](docs/proposal):

- [0002](docs/proposal/0002-zos-application-runtime-plan.md) - overall plan
- [0003](docs/proposal/0003-cobol-compiler.md) - the COBOL compiler; first
  milestone: passing the NIST CCVS85 COBOL-85 suite

## Status

Phase 0: the CCVS85 harness is in place. The compiler driver `zcobc` exists
only as a stub, so every program currently fails to compile; GnuCOBOL results
are shown alongside as the comparison baseline.

## Layout

| Path | Content |
|---|---|
| `crates/zeus-cobol` | `zcobc`, the COBOL compiler driver (stub) |
| `crates/zeus-ccvs` | CCVS85 extractor, runner and reporter |
| `ccvs85/` | Harness data (X-card values, reference manifest) |
| `tools/` | Download scripts for test suites |
| `docs/oracle/` | Exclusions and known oracle differences |

See [CONTRIBUTING.md](CONTRIBUTING.md) for the clean-room rules and how to
run the suite.

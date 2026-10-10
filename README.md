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

Phase 1 (COBOL compiler): `zcobc` compiles COBOL-85 to ZIR and runs it in a
Rust interpreter, keeping data in EBCDIC with z/OS storage layouts. It passes
all eight required CCVS85 modules (NC, SM, IC, SQ, RL, IX, ST, IF), with the
same test counts as the GnuCOBOL baseline (and more programs in IF, where
GnuCOBOL cannot compile some). The optional modules SG (segmentation), OB
(obsolete elements), DB (debugging) and RW (Report Writer) pass as well; the
only module out of scope is CM (communication).

Phase 1 continues with Enterprise COBOL extensions for the applications.
The AWS CardDemo batch process runs end to end: `tools/carddemo.sh all`
catalogs the sample data, compiles the batch programs and submits the
initialization, daily batch and report jobs through `zjob`, all ending with
RC 0.

## Layout

| Path | Content |
|---|---|
| `crates/zeus-cobol` | `zcobc`: COBOL front end, semantics, ZIR code generator and interpreter |
| `crates/zeus-ds` | Data set layer: DD allocation, record formats, sequential, relative and keyed (KSDS + AIX style) data sets, catalog with GDGs |
| `crates/zeus-jcl` | `zjob`: JCL parsing and PROC expansion, job execution, IEFBR14 / IEBGENER / IDCAMS / SORT |
| `crates/zeus-decimal` | Packed / zoned / binary decimal arithmetic (checked against Hercules) |
| `crates/zeus-ccvs` | CCVS85 extractor, runner and reporter |
| `crates/zeus-hercdiff` | Records Hercules results for packed-decimal instructions (oracle for the decimal arithmetic) |
| `ccvs85/` | Harness data (X-card values, Zeus overlay, reference manifest) |
| `tools/` | Download scripts for test suites and CardDemo, `carddemo.sh` |
| `docs/oracle/` | Exclusions and known oracle differences |

See [CONTRIBUTING.md](CONTRIBUTING.md) for the clean-room rules and how to
run the suite.

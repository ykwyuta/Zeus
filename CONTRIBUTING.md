# Contributing to Zeus

## Clean-room rules

Zeus is licensed under GPL-3.0-or-later. To keep it free of code we may not
use:

- **Do not read, copy or port Hercules source code.** Hercules is under the
  Q Public License, which is incompatible with the GPL. Hercules may be run as
  an external program and used as a black-box comparison oracle only.
- **Do not copy IBM code or IBM-supplied material** (macros, copybooks,
  BMS/CICS definitions, sample listings). Implement from the published
  specifications (Principles of Operation, Enterprise COBOL Language
  Reference / Programming Guide, CICS and Db2 references).
- **GnuCOBOL is a comparison oracle, never a dependency.** Zeus must not link
  to, embed or invoke GnuCOBOL at build or run time. It is used only by the
  test tooling (`zeus-ccvs --toolchain gnucobol`) to produce reference results.
- Test suites and OS images (CCVS85, MVS 3.8j, ...) are downloaded by scripts
  under `tools/` and are never committed.

- Code page tables come from ICU (Unicode License V3): `tools/icu-codepages.sh`
  downloads its mapping files at a pinned commit and regenerates
  `crates/zeus-cobol/codepages`, which carries ICU's license. Zeus converts
  with its own code; it does not use iconv or ICU at run time.

When an oracle and the IBM specification disagree, the specification wins;
record the case under `docs/oracle/`.

## Development

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all
```

All three must pass before a change is merged (CI runs them).

## CCVS85 conformance runs

The first milestone is passing the required modules of the NIST CCVS85 suite
(see `docs/proposal/0003-cobol-compiler.md` section 5.2 and `ccvs85/README.md`).

```sh
sudo apt-get install gnucobol3        # comparison oracle
tools/fetch-ccvs85.sh                 # downloads newcob.val into .cache/ccvs85
cargo build --release
target/release/zeus-ccvs extract --newcob .cache/ccvs85/newcob.val \
    --out work/suite --verify ccvs85/extract-manifest.txt
target/release/zeus-ccvs run --suite work/suite --toolchain gnucobol --out work/gnucobol.json
target/release/zeus-ccvs extract --newcob .cache/ccvs85/newcob.val \
    --overlay ccvs85/exec85-zeus.conf --out work/suite-zeus
target/release/zeus-ccvs run --suite work/suite-zeus --toolchain zeus --out work/zeus.json
target/release/zeus-ccvs report --results work/zeus.json --baseline work/gnucobol.json
```

## CardDemo batch

`tools/carddemo.sh all` downloads AWS CardDemo at a pinned commit into
`.cache/carddemo`, catalogs its sample data in `work/carddemo` (override with
`ZEUS_HOME`), compiles the batch programs into a load library and submits the
initialization, daily batch and report jobs with `zjob`. It fails unless every
job ends with RC 0; job output is in `work/carddemo/spool/<JOB>/`. Single jobs
can be resubmitted with `tools/carddemo.sh run POSTTRAN ...`.

## CardDemo online

After `tools/carddemo.sh all`, `tools/carddemo-online.sh test` compiles the
online programs, writes the mapsets as JSON and runs the terminal scripts in
`tools/carddemo-online/` with `zcics` (they expect freshly set up data). A
single script runs with `tools/carddemo-online.sh run SCRIPT`; `show` in a
script prints the screen. `tools/carddemo-online.sh serve` starts the TN3270
server on port 3270 for a 3270 emulator; `tools/carddemo-online.sh tn3270`
signs on through `s3270` (`apt-get install s3270`) as CI does.

## Application inventory

`tools/apps-inventory.sh` fetches CardDemo, GenApp and Bank of Z, generates
their BMS symbolic maps, compiles every COBOL program and rewrites
`docs/inventory/apps.md` (compilation results and the statements, functions,
data clauses and EXEC commands the programs use). Commit the regenerated
file when a change alters it; CI checks that it is up to date.

`tools/corpora-inventory.sh` does the same for the additional corpora of
`docs/proposal/0022-additional-cobol-corpora.md` (fetched by
`tools/fetch-corpora.sh` into `.cache/corpora`) and rewrites
`docs/inventory/corpora.md`, listing the programs left out for reasons
outside Zeus. `tools/corpora-testgen.sh` generates C1 test cases for
them with `ztestgen`, replays them with `ztest` and prints the coverage of
each program (seeds in `tools/corpora-seeds`).

## Hercules decimal-instruction oracle

`zeus-hercdiff` generates packed-decimal instruction cases (AP, SP, ZAP, CP,
MP, DP, SRP, MVO, PACK, UNPK), runs them on Hercules in ESA/390 mode and
records operands, condition codes and program interruption codes. These are
the expected results for the decimal arithmetic of the compiler runtime.

```sh
sudo apt-get install hercules
cargo run --release -p zeus-hercdiff -- run --count 300 --seed 1 --out work/decimal-oracle.json
```

## Db2

Db2 is reached through DRDA using the standalone
[`rust-db2-driver`](https://github.com/ykwyuta/rust-db2-driver) crate, pinned
to a Git revision in `crates/zeus-db2/Cargo.toml`. The driver was extracted from
Zeus's DRDA / DDM implementation; it requires no ODBC or IBM client library.
`zeus-db2` provides the EXEC SQL, host-variable, SQLCA, and dialect compatibility
layer above it. The server is Db2 Community Edition in a container
(Docker), which users install themselves; it is not redistributed:

```sh
export ZEUS_DB2=$(tools/db2ce.sh start)   # db2://db2inst1:zeuspass@localhost:50000/ZEUSDB
cargo test -p zeus-db2                   # the Db2 tests are skipped without ZEUS_DB2
tools/carddemo.sh setup
tools/carddemo-db2.sh test                # CardDemo's Db2 module, batch and online
```

`ZEUS_DB2_SCHEMA` sets the default schema; `ZEUS_DRDA_TRACE=1` (or the driver's
`DB2_DRDA_TRACE=1`) prints DRDA flows after authentication. The trace can contain
SQL text and row data. DDM control strings use CCSID 37 independently of
`ZEUS_CCSID`; the Zeus compatibility layer converts application host variables
using the application's code page.

## 3270 terminals

`tools/carddemo-online.sh tn3270`, `tools/genapp.sh tn3270` and
`tools/tn3270-test.sh` drive the TN3270 server with the x3270 family's
scripted display emulator `s3270` and printer emulator `pr3287`
(`apt-get install s3270 pr3287`). `ZEUS_TN3270_TRACE=1` prints the records
the server sends and receives.

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
script prints the screen.

## Application inventory

`tools/apps-inventory.sh` fetches CardDemo, GenApp and Bank of Z, generates
their BMS symbolic maps, compiles every COBOL program and rewrites
`docs/inventory/apps.md` (compilation results and the statements, functions,
data clauses and EXEC commands the programs use). Commit the regenerated
file when a change alters it; CI checks that it is up to date.

## Hercules decimal-instruction oracle

`zeus-hercdiff` generates packed-decimal instruction cases (AP, SP, ZAP, CP,
MP, DP, SRP, MVO, PACK, UNPK), runs them on Hercules in ESA/390 mode and
records operands, condition codes and program interruption codes. These are
the expected results for the decimal arithmetic of the compiler runtime.

```sh
sudo apt-get install hercules
cargo run --release -p zeus-hercdiff -- run --count 300 --seed 1 --out work/decimal-oracle.json
```

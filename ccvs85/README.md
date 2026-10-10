# CCVS85 harness data

The NIST COBOL-85 test suite (CCVS85, `newcob.val`) is downloaded by
`tools/fetch-ccvs85.sh`; it is not stored in the repository. This directory
holds the data the harness (`crates/zeus-ccvs`) needs:

| File | Purpose |
|---|---|
| `exec85.conf` | EXEC85 control cards: the implementor values substituted for the `XXXXXnnn` markers (X-cards) in the test programs. The values (file names, mnemonic names) are those used by the GnuCOBOL driver, except for X-82/X-83 (computer name `ZEUS`). |
| `exec85-zeus.conf` | Overlay applied on top of `exec85.conf` when extracting the suite run with zcobc (`zeus-ccvs extract --overlay`). It changes only implementor-defined values that depend on the character set: X-63/X-64, the native collating sequence used by the SORT/MERGE tests (ST137A, ST144A, ST147A), which is EBCDIC for Zeus. |
| `extract-manifest.txt` | SHA-256 of every member as extracted by the **original EXEC85 program** (compiled with GnuCOBOL) using `exec85.conf`. `zeus-ccvs extract --verify` checks that the Rust extractor produces byte-identical programs. |

## How the reference manifest was produced

EXEC85 is the first member of `newcob.val`. It was compiled with GnuCOBOL
3.1.2 (file names patched into its SELECT clauses), run once per module with
`*SELECT-MODULE <MOD>` followed by `exec85.conf`, and the resulting
`newcob.tmp` files were turned into the manifest with:

```sh
zeus-ccvs manifest --out ccvs85/extract-manifest.txt <MOD>/newcob.tmp...
```

Regenerate it only when `exec85.conf` changes.

The overlay changes only the members that use X-63/X-64, so `--verify` is
run on the plain extraction; CI extracts both (`work/suite` for GnuCOBOL,
`work/suite-zeus` for zcobc).

## Layout of an extracted suite

```text
copy/<member>          COPY library        copyalt/ALTLB   alternate library (X-48)
<MOD>/<prog>.CBL       main program        <MOD>/<prog>.SUB follow-on program
<MOD>/lib/<prog>.CBL   called subprogram   <MOD>/<prog>.DAT standard input
```

Data cards filled up to column 80 continue on the next card, so a `.DAT`
line can be longer than 80 characters.

## Running

`zeus-ccvs run` compiles and runs every program of the selected modules in
`work/run/<toolchain>/<MOD>` and writes a JSON result file. The report file of
each program is assigned through the `DD_REPORT` environment variable.
Programs needing special treatment (compile-only tests, programs without a
report, inspection of output) are listed in `crates/zeus-ccvs/src/special.rs`.

The pass criteria are defined in `docs/proposal/0003-cobol-compiler.md`
section 5.2; intentional exclusions go to `docs/oracle/ccvs85-exclusions.md`.

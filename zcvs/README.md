# ZCVS — z/OS Enterprise COBOL Validation Suite

ZCVS is a clean-room validation suite (`GPL-3.0-or-later`) targeting IBM Enterprise COBOL for z/OS V4.2 / V6.4 language semantics, compiler options, EBCDIC/DBCS/National encodings, pointers and subprogram linkage, QSAM/VSAM/AWS-tape file I/O, system interfaces, and negative compile-time diagnostics. Implemented and missing scenarios are distinguished in the audit below.

Runtime programs use the CCVS85-compatible self-checking report format (`PASS`, `FAIL*`, summary counts, `END OF TEST-2`) via shared copybooks in `zcvs/copy/`. Negative programs expect compilation errors; paired positive controls expect successful compilation. The JCL exporter prepares z/OS members, but local results do not certify execution on IBM z/OS.

See [SCENARIOS.md](SCENARIOS.md) for the requirement-by-requirement audit, missing scenarios, specification constraints, and acceptance rules. Passing all implemented checks does not mean proposal 0019 is fully covered. [coverage.json](coverage.json) is the reviewed inventory of executable checks, not a claim of complete language coverage.

## Modules

| Module | Category | Programs |
|--------|----------|----------|
| `ZD` | Packed/zoned/binary storage, layout, ODO, edited PICTURE, figurative constants, collation | `ZD101A` .. `ZD115A` (15) |
| `ZA` | TRUNC, arithmetic, NUMPROC/NUMCLS sign matrix, HFP, intrinsic functions, ARITH(EXTEND) | `ZA101A` .. `ZA128A` (28) |
| `ZN` | National, DBCS, NSYMBOL, CCSID conversion, UTF-16 boundaries | `ZN101A` .. `ZN115A`, including `ZN102B` and `ZN106M` (15) |
| `ZP` | Subprograms, ENTRY, recursion, storage lifetime, pointers, visibility, CALL arguments, debugging declaratives | `ZP101A` .. `ZP117A` + 19 `lib/` subprograms (17 mains) |
| `ZF` | QSAM F/V/S/U, CODE-SET, KSDS/RRDS status, alternate indexes, shared areas, AWS tape | `ZF101A` .. `ZF114A`, `ZF201M`, `ZF202M` (16) |
| `ZS` | Special registers, date/window functions, LE services, PARM, Gregorian boundaries, REPLACE scope, batch compilation | `ZS101A` .. `ZS109A`, including `ZS103M` (9) |
| `ZX` | Optional control flow, SET FALSE/INITIALIZE, COPY/REPLACE, directives, TRIM/TEST-NUMVAL | `ZX101A` .. `ZX110A` (10) |
| `ZE` | Negative compile diagnostics and positive controls | `ZE101N` .. `ZE147N`, `ZE126P` .. `ZE147P` (69) |

The suite contains 170 main programs: 103 runtime programs with 1,553 assertions,
46 negative compile checks, and 21 successful-compilation controls. Cargo tests
run both Zeus backends and require the exact program set, assertion IDs in
report order, counts, and zero deleted/manual checks. They rebuild `zcobc` and
`zeus-rt` through Cargo to avoid testing stale binaries:

```bash
cargo test -p zeus-ccvs --test zcvs
```

New scenario groups correspond to audit IDs D04 (`ZD108A`), A05/A06
(`ZA110A`/`ZA109A`), N07 (`ZN107A`), P05 (`ZP107A`), F01 (`ZF106A`),
S05 (`ZS104A`), X01 (`ZX105A`), and E01/E02/E07/E08 (`ZE126N/P` ..
`ZE133N/P`). The audit records the remaining partitions within those groups.

A further 14 programs add 335 assertions for native/custom EBCDIC collation,
NUMPROC/NUMCLS sign validation and normalization, pointer rebinding and ENTRY
aliases, KSDS alternate-index creation/update/deletion/recovery and duplicate
status `02`, tape HDR2 mismatches, REPLACE scope, INITIALIZE precedence, and
TRIM/TEST-NUMVAL/ABS. See audit IDs D02, A07, P06, F05–F07, S01, X03–X05.

The latest expansion adds 53 mains and 7 helpers: 29 runtime programs with
766 assertions, 12 negative diagnostics and 12 matching positive controls.
It covers packed/zoned byte matrices, ODO moving tails, nested SYNC strides,
TRUNC aliases and width limits, independent arithmetic receivers, external
floats, national categories and four Japanese CCSIDs, storage lifetime,
recursive PERFORM frames, ENTRY arities, RETURNING groups/77 items, sparse
RRDS, partial START, multiple alternate indexes, QSAM sequencing/EXTEND,
four tape volumes, and national/decimal-comma intrinsic functions.

The 2026-11-12 expansion adds 4 mains and 1 helper with 34 assertions chosen
from the audit's remaining partitions (A06, A07, N03, P05, P07, S04, S07,
X01–X03): ROUNDED carry into SIZE ERROR, separate and embedded signs, PERFORM
VARYING AFTER carry order, SET TO FALSE on ranges and national condition
names, argument aliasing and BY CONTENT, GLOBAL shadowing, RETURN-CODE from
a separately compiled callee, century windows computed from the current year,
and INITIALIZE skipping REDEFINES. `ZS107A` also uses reference modification
of a function result (`FUNCTION CURRENT-DATE(1:4)`), which no earlier program
did.

The Rust harness additionally inspects all four `ZF202M` AWS images without
the production decoder: label lengths/order, HDR2 format, EOV versus EOF,
tape marks, raw payloads, physical block counts and previous-length chains.
These physical checks are separate from the 1,465 COBOL assertions.

`ZA111A`–`ZA114A` use Zeus's `CBL NUMCLS` selection to test both NUMCLS
installation settings. On IBM z/OS, NUMCLS is an installation setting rather
than a source compiler option: align that setting and remove this Zeus
selection before compiling these four sources. `ZA115A` explicitly targets
the legacy COBOL 4.2 `NUMPROC(MIG)` option, which is unavailable in COBOL 6.

## Usage

Run ZCVS with the Zeus interpreter (`zeus`):

```bash
cargo run -p zeus-ccvs -- run --suite zcvs --toolchain zeus --out work/zcvs-zeus.json
cargo run -p zeus-ccvs -- report --results work/zcvs-zeus.json --programs
```

Run ZCVS with the Zeus native backend (`zeus-native`):

```bash
cargo run -p zeus-ccvs -- run --suite zcvs --toolchain zeus-native --out work/zcvs-native.json
cargo run -p zeus-ccvs -- report --results work/zcvs-native.json --programs
```

Export 80-column z/OS PDS datasets and `IGYWCLG` batch JCL jobs:

```bash
cargo run -p zeus-ccvs -- export-jcl --suite zcvs --out work/zcvs-zos --hlq IBMUSER.ZCVS
```

`ZS106A` preserves the production-source idioms from proposal 0022: literal
COPY REPLACING, tag-word REPLACE, OPEN/CLOSE statement boundaries, INSPECT
on a function result, and per-program compiler options in batch compilation.

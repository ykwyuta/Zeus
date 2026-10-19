# ZCVS — z/OS Enterprise COBOL Validation Suite

ZCVS is a clean-room validation suite (`GPL-3.0-or-later`) targeting IBM Enterprise COBOL for z/OS V4.2 / V6.4 language semantics, compiler options, EBCDIC/DBCS/National encodings, pointers and subprogram linkage, QSAM/VSAM/AWS-tape file I/O, system interfaces, and negative compile-time diagnostics. Implemented and missing scenarios are distinguished in the audit below.

Runtime programs use the CCVS85-compatible self-checking report format (`PASS`, `FAIL*`, summary counts, `END OF TEST-2`) via shared copybooks in `zcvs/copy/`. Negative programs expect compilation errors; paired positive controls expect successful compilation. The JCL exporter prepares z/OS members, but local results do not certify execution on IBM z/OS.

See [SCENARIOS.md](SCENARIOS.md) for the requirement-by-requirement audit, missing scenarios, specification constraints, and acceptance rules. Passing all implemented checks does not mean proposal 0019 is fully covered. [coverage.json](coverage.json) is the reviewed inventory of executable checks, not a claim of complete language coverage.

## Modules

| Module | Category | Programs |
|--------|----------|----------|
| `ZD` | Packed/zoned/binary storage, layout, ODO, edited PICTURE, figurative constants, collation | `ZD101A` .. `ZD110A` (10) |
| `ZA` | TRUNC, arithmetic, NUMPROC/NUMCLS sign matrix, HFP, intrinsic functions, ARITH(EXTEND) | `ZA101A` .. `ZA115A` (15) |
| `ZN` | National, DBCS, NSYMBOL, CCSID conversion, UTF-16 boundaries | `ZN101A` .. `ZN107A`, including `ZN102B` and `ZN106M` (8) |
| `ZP` | Subprograms, ENTRY, recursion, storage lifetime, pointers, visibility, CALL arguments | `ZP101A` .. `ZP108A` + 9 `lib/` subprograms (8 mains) |
| `ZF` | QSAM F/V/S/U, CODE-SET, KSDS/RRDS status, alternate indexes, shared areas, AWS tape | `ZF101A` .. `ZF109A`, `ZF201M` (10) |
| `ZS` | Special registers, date/window functions, LE services, PARM, Gregorian boundaries, REPLACE scope | `ZS101A` .. `ZS105A`, including `ZS103M` (5) |
| `ZX` | Optional control flow, SET FALSE/INITIALIZE, COPY/REPLACE, directives, TRIM/TEST-NUMVAL | `ZX101A` .. `ZX107A` (7) |
| `ZE` | Negative compile diagnostics and positive controls | `ZE101N` .. `ZE133N`, `ZE126P` .. `ZE133P` (41) |

The suite contains 104 main programs: 63 runtime programs with 660 assertions,
33 negative compile checks, and 8 successful-compilation controls. Cargo tests
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

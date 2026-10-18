# ZCVS — z/OS Enterprise COBOL Validation Suite

ZCVS is a clean-room validation suite (`GPL-3.0-or-later`) targeting IBM Enterprise COBOL for z/OS V4.2 / V6.4 language semantics, compiler options (`CBL`/`PROCESS`), EBCDIC/DBCS/National encodings, pointer & subprogram linkage (`NODYNAM`/`DYNAM`), QSAM/VSAM/AWS-tape file I/O, system interfaces (`PARM`, `RETURN-CODE`, `CEE3ABD`), XML (`XML PARSE`, `XML GENERATE`), and negative compile-time diagnostics.

All programs use the CCVS85-compatible self-checking report format (`PASS`, `FAIL*`, `SKIP`, `END OF TEST-SUMMARY`) via shared copybooks in `zcvs/copy/` so they can be executed both locally with `zeus-ccvs` and on an IBM z/OS system via `IGYWCLG`.

## Modules

| Module | Category | Programs |
|--------|----------|----------|
| `ZD` | Data Representation & Binary/Packed/Float/Pointer | `ZD101A` .. `ZD107A` (7) |
| `ZA` | Compiler Options & Arithmetic (`TRUNC`, `NUMPROC`, `ARITH`, `ZWB`) | `ZA101A` .. `ZA108A` (8) |
| `ZN` | National (`UTF-16BE`), DBCS (`CCSID 930/939`), Intrinsic Functions | `ZN101A` .. `ZN106M` (7) |
| `ZP` | Subprograms, `ENTRY`, `RECURSIVE`, `LOCAL-STORAGE`, `CANCEL`, `GOBACK` | `ZP101A` .. `ZP106A` + 7 `lib/` subprograms (6) |
| `ZF` | QSAM (`RECORDING MODE F/V`), AWS Tape, VSAM KSDS/ESDS/RRDS, `SORT` | `ZF101A` .. `ZF201M` (6) |
| `ZS` | Source Manipulation (`COPY REPLACING`, `LIB`, `EJECT`/`SKIP`) & System (`PARM`, `RETURN-CODE`, `CEE3ABD`) | `ZS101A` .. `ZS103M` (3) |
| `ZX` | Optional XML Processing (`XML PARSE`, `XML GENERATE`) | `ZX101A` .. `ZX104A` (4) |
| `ZE` | Negative Compile-Error Diagnostics | `ZE101N` .. `ZE125N` (25) |

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

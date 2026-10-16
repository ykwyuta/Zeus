# GnuCOBOL as an oracle: known differences

GnuCOBOL is used only as a comparison oracle (docs/proposal/0003 section 5.1).
This file records where its results cannot be taken as the expected
behaviour for Zeus, either because GnuCOBOL fails a test or because it
differs from Enterprise COBOL.

Baseline: GnuCOBOL 3.1.2 (Ubuntu 24.04 package `gnucobol3`), options
`-std=cobol85 --debug`, harness `zeus-ccvs`, no source updates applied.

## CCVS85 results of the baseline

| Module | Programs OK | Note |
|---|---|---|
| NC SM IC SQ RL IX ST | all | Usable as oracle |
| IF | 33/45 | 12 programs do not compile: subscripts `ALL` in function arguments, e.g. `FUNCTION MAX(IND(ALL))` (IF119A-IF123A, IF128A, IF129A, IF132A, IF137A, IF138A, IF141A, IF402M). The GnuCOBOL project patches these programs in its own driver; we do not, so these programs have no GnuCOBOL baseline (zcobc passes all 45). |
| SG OB DB RW | all | Usable as oracle |
| CM | 1/9 | Communication module, out of scope |

Run-time differences between GnuCOBOL and Enterprise COBOL (ASCII vs EBCDIC
collating sequence, binary truncation, sign handling, ...) will be added here
as the differential tests (0003 section 5.1) find them.

| Area | GnuCOBOL behaviour | Enterprise COBOL behaviour | Found by |
|---|---|---|---|
| Native collating sequence (X-63/X-64 cards of ST137A, ST144A, ST147A) | ASCII | EBCDIC; Zeus extracts these programs with `ccvs85/exec85-zeus.conf` | CCVS85 ST |

## ZCVS results & IBM Enterprise COBOL vs GnuCOBOL `-std=ibm` differences

When running ZCVS (`zcvs/`) against GnuCOBOL (`-std=ibm`), the following architectural and semantic differences apply where Zeus follows IBM Enterprise COBOL for z/OS V4.2 / V6.4 rather than GnuCOBOL:

| Area | GnuCOBOL (`-std=ibm`) behaviour | IBM Enterprise COBOL / Zeus behaviour | ZCVS Module / Programs |
|---|---|---|---|
| `CBL` / `PROCESS` card options (`TRUNC`, `NUMPROC`, `ZWB`, `NSYMBOL`, `SSRANGE`, `DYNAM`, `LIB`) | Ignores or rejects z/OS-specific `CBL`/`PROCESS` options unless mapped manually to CLI flags | Parsed directly from source deck header cards before `IDENTIFICATION DIVISION` | `ZA101A`–`ZA108A`, `ZN101A`–`ZN102B`, `ZP102A`, `ZS101A`, `ZE125N` |
| Native character encoding & hex literals | ASCII internal storage (`'A' = X'41'`, `'0' = X'30'`), ASCII collating order (`'0' < 'A' < 'a'`) | EBCDIC internal storage (`'A' = X'C1'`, `'0' = X'F0'`), EBCDIC collating order (`'a' < 'A' < '0'`) | `ZD101A`, `ZA105A`, `ZN102A`, `ZF103A`, `ZF105A` |
| Zoned decimal sign nibbles (`NUMPROC(NOPFD)` / `NUMPROC(PFD)` / `NUMPROC(MIG)`) | ASCII sign byte encoding (`0x30`..`0x39` / `0x70`..`0x79`) | EBCDIC zone nibble (`X'F0'` unsigned, `X'C0'` preferred `+`, `X'D0'` preferred `-`) and `NUMPROC` canonicalization rules | `ZD101A`, `ZA103A`, `ZA104A` |
| Hexadecimal floating-point (`COMP-1` / `COMP-2`) | Host IEEE 754 binary floating-point | IBM S/390 Hexadecimal Floating Point (HFP, base-16 characteristic + fraction; `1.0 = X'41100000'`) | `ZD104A` |
| `PIC N` (`USAGE NATIONAL`) & DBCS (`PIC G`, `CCSID 930/939`, SO/SI `X'0E'`/`X'0F'`) | Platform-dependent national encoding; no EBCDIC SO/SI shift-code framing or CCSID 930/939 DBCS transcoding | UTF-16BE (`PIC N`) with `NATIONAL-OF`/`DISPLAY-OF` conversion to/from EBCDIC SBCS/DBCS (`CCSID 930`/`939`/`1047`) with `X'0E'`/`X'0F'` framing | `ZN101A`–`ZN106M` |
| QSAM `RECORDING MODE V`, `BLOCK CONTAINS`, AWS HET/OMA tape (`UNIT=3480`), VSAM `RECORDING MODE` rules | Host OS flat/indexed file formats; no z/OS RDW/BDW or AWS virtual tape block headers | z/OS QSAM `RECORDING MODE F/V/U/S` rules, AWS virtual tape 6-byte headers, VSAM KSDS/ESDS/RRDS semantics & negative `BLOCK CONTAINS` checks | `ZF101A`–`ZF201M`, `ZE117N`–`ZE123N` |
| Language Environment (`CEE3ABD`, JCL `PARM=` `HALFWORD + TEXT` linkage, `RETURN-CODE`) | POSIX `argc`/`argv` command-line convention; no LE callable services | OS linkage `PARM` (`PIC S9(4) COMP` length + data), `RETURN-CODE` (`PIC S9(4) COMP`), and `CEE3ABD` abend trapping | `ZS102A`, `ZS103M` |
| IBM XML special registers (`XML-CODE`, `XML-EVENT`, `XML-TEXT`, `XML-NTEXT`) | Depends on optional libxml2 configuration and GnuCOBOL version | Built-in `XML PARSE` event state machine and `XML GENERATE` with `COUNT IN` | `ZX101A`–`ZX104A` |


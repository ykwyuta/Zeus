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

Online programs: `zcobc` compiles EXEC CICS, EXEC SQL and EXEC DLI (run
through a host interface that the CICS, Db2 and IMS runtimes provide)
and `zbms` generates BMS symbolic maps; 111 of the 115 COBOL programs of
CardDemo, GenApp and Bank of Z compile. The syntax usage inventory of the
three applications is in [docs/inventory/apps.md](docs/inventory/apps.md).

CICS: `zcics` runs transactions against a 3270 screen model, driven by a
script or by 3270 emulators through its TN3270 server
(`tools/carddemo-online.sh serve`, then connect an emulator such as x3270 to
port 3270 and enter `CC00`). The CardDemo online transactions
(sign-on, menus, account view and update, card list, view and update,
transaction list, view and add, bill payment, report submission through the
internal reader, user administration) work on the data the batch jobs set
up.

Db2: `zeus-drda` is a DRDA client (written from the Open Group DRDA
specification) and `zeus-db2` runs the EXEC SQL of COBOL programs on Db2
Community Edition through it (host variables, indicator variables, cursors,
SQLCA), adapting Db2 for z/OS DDL that Db2 for Linux lacks. `zjob` runs
IKJEFT01 `DSN RUN PROGRAM` (with DSNTIAD / DSNTEP2 / DSNTEP4 as SQL script
runners) and `zcics` gives transactions a Db2 connection committed at
SYNCPOINT and task end, together with the recoverable files (VSAM
LOG(UNDO), FILE RECOVERY), whose changes ROLLBACK and abends back out.
CardDemo's optional Db2 module (transaction types) works: table creation, the MNTTRDB2 batch maintenance job and the list,
update, delete and add screens (`tools/carddemo-db2.sh test`, with
`ZEUS_DB2` naming the server started by `tools/db2ce.sh start`).

GenApp: `tools/genapp.sh setup` applies the installation's customization to
its jobs and runs them (VSAM files, Db2 database and sample rows), compiles
the programs and maps and takes the resource definitions from its CSD job.
Its conversational menu transactions (customer inquiry, add and update;
motor, endowment and house policy inquiry, add, update and delete;
commercial policy inquiry, add and delete) work on Db2 and VSAM, with customer numbers from
named counters (`tools/genapp.sh test`).

## Layout

| Path | Content |
|---|---|
| `crates/zeus-cobol` | `zcobc`: COBOL front end, semantics, ZIR code generator and interpreter |
| `crates/zeus-ds` | Data set layer: DD allocation, record formats, sequential, relative and keyed (KSDS + AIX style) data sets, catalog with GDGs |
| `crates/zeus-jcl` | `zjob`: JCL parsing and PROC expansion, job execution, IEFBR14 / IEBGENER / IDCAMS / SORT |
| `crates/zeus-cics` | `zcics`: CICS region, tasks, EXEC CICS commands, BMS on a 3270 screen model |
| `crates/zeus-drda` | DRDA client for Db2 (connection, dynamic SQL, queries) |
| `crates/zeus-db2` | EXEC SQL runtime on Db2: host variables, cursors, SQLCA, z/OS SQL dialect |
| `crates/zeus-bms` | `zbms`: BMS mapsets, COBOL symbolic maps |
| `crates/zeus-decimal` | Packed / zoned / binary decimal arithmetic (checked against Hercules) |
| `crates/zeus-ccvs` | CCVS85 extractor, runner and reporter |
| `crates/zeus-hercdiff` | Records Hercules results for packed-decimal instructions (oracle for the decimal arithmetic) |
| `ccvs85/` | Harness data (X-card values, Zeus overlay, reference manifest) |
| `tools/` | Download scripts for test suites and the applications, `carddemo.sh`, `carddemo-online.sh`, `carddemo-db2.sh`, `genapp.sh`, `db2ce.sh`, `apps-inventory.sh` |
| `docs/oracle/` | Exclusions and known oracle differences |

See [CONTRIBUTING.md](CONTRIBUTING.md) for the clean-room rules and how to
run the suite.

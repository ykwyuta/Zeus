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

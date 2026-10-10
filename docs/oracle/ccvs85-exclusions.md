# CCVS85 exclusions

Tests deleted (DELETED) or programs excluded on purpose. Per
docs/proposal/0003 section 5.2, an exclusion is allowed only for a feature
that Enterprise COBOL does not support either. One entry per test or program.

| Module | Program | Test / paragraph | Reason | Enterprise COBOL reference |
|---|---|---|---|---|
| CM | (all) | - | Communication module; not supported by Enterprise COBOL | Out of scope (0003 section 5.2) |
| RW | (all) | - | Report Writer; not supported by Enterprise COBOL | Out of scope (0003 section 5.2) |

No test of a required module is excluded yet.

# CCVS85 exclusions

Tests deleted (DELETED) or programs excluded on purpose. Per
docs/proposal/0003 section 5.2, an exclusion is allowed only for a feature
that Enterprise COBOL does not support either. One entry per test or program.

| Module | Program | Test / paragraph | Reason | Enterprise COBOL reference |
|---|---|---|---|---|
| CM | (all) | - | Communication module; not supported by Enterprise COBOL | Out of scope (0003 section 5.2) |

CM is the only module excluded; every other module passes. Tests that the
suite itself deletes unconditionally (for example after "DELETE THE NEXT
LINE TO DELETE THE TEST") are reported as DELETED by every implementation.
The
communication description (CD) and the communication statements are parsed
so that DB205A and DB304M (debugging on a cd-name) compile; executing a
communication statement is a run-time error.

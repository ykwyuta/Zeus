# Application HLASM and shared storage

`zas` compiles the executable assembler sources present in the pinned test
corpus into Zeus `.zo` modules. CardDemo contains COBDATFT (date formatting)
and MVSWAIT (STIMER wait), with COCDATFT COPY and ASMWAIT macro members.
GenApp's assembler-style BMS sources use `zbms`; Bank of Z's IMS DBD/PSB
definitions use `zims`. Those resource definitions are not executable HLASM.
The downloaded application sources remain outside the repository.

HLASM executes through the shared-storage instruction engine. Generating
host-CPU native code from HLASM is explicitly out of scope, including future
work, and is not a missing feature or a requirement for HLASM compatibility.
Further work targets language and instruction semantics, services, and
interoperation with COBOL, PL/I and JCL using this execution model.

## Compile and call

```sh
zas compile -I app/maclib -o loadlib/COBDATFT.zo app/asm/COBDATFT.asm
zas compile -I app/maclib -o loadlib/MVSWAIT.zo app/asm/MVSWAIT.asm
zas check -I app/maclib app/asm/COBDATFT.asm
zjob --loadlib loadlib submit job.jcl
```

COBOL `CALL 'NAME' USING ...` and PL/I `DCL NAME ENTRY OPTIONS(ASM RETCODE);`
with `CALL NAME(...)` resolve assembled modules in their load libraries.
An assembled module takes precedence over a registered Rust replacement.
The CardDemo setup script now builds both assembler programs from source.
Native COBOL executables can bundle HLASM subprogram modules in their image;
the COBOL caller runs as native code and the assembler subprogram uses the
same shared-storage instruction engine.

In JCL, `EXEC PGM=ASMA90` or `ASMAHL` reads SYSIN and searches SYSLIB for
COPY and macro members. It writes SYSLIN for the existing IEWL binder to
copy into a SYSLMOD member. Subsequent `EXEC PGM=NAME` resolves the module
through STEPLIB/JOBLIB or the command's load libraries. `PARM` supplies a
big-endian halfword length followed by EBCDIC bytes.

## Shared storage and lifetime

All languages in a run unit use the same `Machine` storage. Addresses are
31-bit offsets into this storage, rather than host pointers. Assembler R1
points to a list of big-endian fullword addresses, with the high bit set on
the last entry. R13 points to a 72-byte caller save area, R14 supplies the
return target and R15 supplies the entry address and return code.

BY REFERENCE arguments use the caller's original address. Stores through
those addresses immediately change COBOL or PL/I data; aliases refer to the
same bytes. COBOL BY CONTENT retains its existing temporary-copy behavior.
Callers must supply compatible field sizes, EBCDIC text and big-endian
binary layouts. DSECT describes offsets and allocates no storage.

Each loaded assembler module has its own static storage in that run unit,
retained across calls. CANCEL restores the initial bytes. A call's argument
list and caller save area are truncated on return, including execution
errors. Module storage and the whole run unit are dropped at JCL step end;
separate steps do not share these addresses. Registering two modules does
not implicitly alias their static data by matching symbol names.

## Supported scope

The complete current inventory is embedded in `zas --help`. Support extends
beyond the corpus to these language and instruction families:

| Area | Implemented |
| --- | --- |
| Source | Fixed-format column-72 continuation, continuation operands from column 16, COPY, comments |
| Expressions | Unary signs, arithmetic precedence, parentheses, same-section relocation cancellation, forward EQU chains, explicit EQU length/type, ordinary L'/T' attributes, binary/hex/character self-defining terms |
| Storage | C/X/B/H/F/FD/P/Z and integer A, explicit lengths, duplication and constant lists; A(symbol) and =A(symbol) relocation at load origin |
| Sections | Multiple CSECT/RSECT sections, reopening and LOCTR, DSECT, CNOP, START 0, ENTRY/END, named/dependent/multiple-register USING and ranges, DROP, PUSH/POP, ORG/LTORG, RSECT protection and RMODE 24 |
| Linking | EXTRN/WXTRN, A/V external address constants, transitive library inclusion, combined Zeus load modules |
| Macros | Positional and keyword/default arguments, label parameters and nested sublists, scalar/array SET symbols, created SET names, extended SET assignments, SETC substring/concatenation/duplication, K'/N' attributes, system symbols and built-ins below; AIF/AGO/ACTR/ANOP/MEXIT |
| Macro expansion/diagnostics | OPSYN snapshots and deletion, operation overrides, shielded nested definitions, source/COPY lookahead, MNOTE severities and structured source diagnostics |
| Execute and string/decimal | EX/EXRL templates, MVCL/CLCL, MVCLE/CLCLE, MVST/CLST/SRST, TRT/TRTR, SRP, ED/EDMK |
| Dynamic storage | GETMAIN/FREEMAIN R/RU/RC and STORAGE OBTAIN/RELEASE, subpool ownership, persistence across calls, explicit free, reuse/coalescing and run-unit/step lifetime |
| Integer | Fullword/halfword arithmetic and compare, logical arithmetic and carry/borrow CC, multiply/divide pairs, sign operations, byte insert/store, logical/arithmetic shifts |
| 64-bit integer | LG/STG and sign/zero-extending loads, register/memory arithmetic, logical carry/borrow, comparison, sign operations, immediate loads/add/compare and multiply, LMG/STMG; signed 20-bit memory displacements |
| Relative control flow | BRC/BRCL, BRAS/BRASL, BRCT/BRCTG/BRCTH, LARL; J/JG condition aliases (32 spellings) |
| Count branches | BCTR/BCTGR/BCTG, with 32/64-bit counters, target-register-zero suppression and original-target capture |
| Masked bytes | ICM/ICMY/ICMH, STCM/STCMY/STCMH, CLM/CLMY/CLMH; all four-bit masks, low/high register words, contiguous memory bytes and condition codes |
| Carry/borrow | ALC/ALCR/ALCG/ALCGR, SLB/SLBR/SLBG/SLBGR; 32/64-bit register and long-displacement memory operands |
| Partial-register immediates | IIHH/IIHL/IILH/IILL/IIHF/IILF, LLIHH/LLIHL/LLILH/LLILL/LLIHF/LLILF, NIHH/NIHL/NILH/NILL/NIHF/NILF, OIHH/OIHL/OILH/OILL/OIHF/OILF, XIHF/XILF, TMHH/TMHL/TMLH/TMLL |
| Long-displacement encodings | LY/STY/LHY/STHY/ICY/STCY; AY/SY/CY/CLY/ALY/SLY/NY/OY/XY, AHY/SHY/CHY; LMY/STMY; CLIY/MVIY/NIY/OIY/XIY/TMY; CVBY/CVDY |
| Byte order and signed bytes | LRVH/LRV/LRVG, LRVR/LRVGR, STRVH/STRV/STRVG; LB/LGB/LBR/LGBR |
| Shift/rotate and pair arithmetic | SLLG/SRLG/SLAG/SRAG/RLLG/RLL, SLDL/SRDL/SLDA/SRDA; ML/MLR/MLG/MLGR, DL/DLR/DLG/DLGR, DSG/DSGR/DSGF/DSGFR |
| Character/logical | MVC/CLC, MVN/MVZ, TR, XC/NC/OC, MVI/CLI, NI/OI/XI/TM and register/memory logical operations |
| Decimal | ZAP/AP/SP/CP/MP/DP, PACK/UNPK, CVB/CVD; lengths, decimal signs/digits, overflow and divide checks |
| Branch | Conditional aliases, BC/BCR/BR, BCT, BAL/BALR/BAS/BASR, NOP/NOPR |
| Service | STIMER WAIT,BINTVL=address (hundredths of a second) |

Registers retain 64 bits. Existing 32-bit instructions preserve their upper
halves, while the added wide operations use all bits. This does not change
the AMODE 31 shared-storage ABI or enable AMODE 64 linkage. FD constants
support signed decimal integer/fraction values (including i64 boundaries), explicit
byte lengths, lists and duplication. F/H/FD accept a signed integer S modifier
in -187..346 after the optional L modifier. Exact decimal digit arithmetic
multiplies the nominal by 2**S and rounds its magnitude to nearest, ties away
from zero. Negative scaling uses exact decimal division by powers of two,
and intermediate digits need not fit a host integer. The converted result
must fit i64; implicit H/F also require their signed 16/32-bit range.
The signed decimal E modifier follows optional L/S modifiers and applies to
every nominal in a list. A nominal E exponent affects only its own value.
Both the E modifier and the sum of modifier/nominal exponents must be in
-85..75. Decimal exponent conversion shifts exact decimal digits before binary
scaling and rounding, so there is no intermediate host floating-point rounding.
Exponents do not affect S'/I'. Tests cover cancellation, i64 boundaries,
small values, list duplication, literals and macro attributes. The range and
combination rules follow the published [exponent modifier specification](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=modifier-exponent).
F/H/FD L/S/E modifiers accept parenthesized absolute expressions, using the
shared assembler expression evaluator. This includes nested arithmetic,
self-defining terms, attribute references and resolved EQU constants. A sign
may precede an S/E expression. The ordinary layout pass can resolve forward
absolute EQU definitions; macro attributes and machine literals require
definitions available at their reference. Literal numeric metadata is captured
when its bytes are created and survives LTORG without re-evaluation. The shared
literal scanner now skips balanced modifier expressions and distinguishes
nominal quotes from self-defining/attribute quotes inside them. Unavailable,
relocatable and layout-dependent values are diagnosed. Expression-valued byte
lengths, duplication factors and bit fields have since been added for the
supported types described in the P2 completion section. Floating constants
and scalar floating-point execution are being added in the P3 work below.
The embedded `zas --help` lists each implemented mnemonic.

Relative instructions use checked signed 16/32-bit halfword displacements
from their own instruction address, without USING. Targets must be relocatable
expressions in the assembled image, including other CSECTs, or declared
external symbols combined with local labels and constant addends. Composite
expressions such as `TABLE+LIMIT-START` retain signed coefficients until
linking; repeated terms, cancellation and parentheses are supported. Their
net relocation coefficient must be one. Absolute numeric targets and multiplication
or division of relocatable terms remain unsupported. Relative references
retain their relative byte distances when modules are linked and loaded at a
new origin. BRAS/BRASL preserve the high register word and include the AMODE
31 bit in saved link information. Counter instructions wrap at the selected
32/64-bit width without changing CC. LARL computes a 31-bit address without
accessing target storage and preserves the upper register word.
Tests cover every condition mask, J/JG aliases, forward/backward calls, short
and long limits, counter wraparound, register aliasing, linked images and
multiple load origins. Native and interpreted COBOL call a routine with a
relative subroutine loop and data more than 64 KiB from the instructions.

Masked-byte operations select bytes in the low or high register word but
read/write a contiguous memory field. Unselected register bytes are retained.
ICM sets CC from only inserted bits and the first inserted bit's sign; CLM
uses unsigned byte comparison; STCM preserves CC. Access bounds cover the
selected byte count, with a one-byte check for mask zero. STCM's zero-mask
access check is an explicitly permitted model choice. CLM checks all selected
storage bytes, including bytes after a first mismatch (also permitted).
Tests cover every mask, both register halves, zero masks, negative/long
displacements, aliased base registers and access exactly at the final byte
of a call frame. Native and interpreted COBOL both pass shared strings
through masked byte insertion, storage and comparison.

Carry/borrow operations consume the high bit of the incoming condition
code, including both zero and nonzero prior results, and produce the next
carry/no-borrow bit. The 32-bit forms preserve the upper word. Tests cover
all incoming condition codes, carry/borrow at zero and maximum operands,
aliases and 128-bit arithmetic chains.
Partial-register immediate instructions use absolute 16-bit unsigned values
or 32-bit patterns, including negative signed fullword values. Insert preserves other fields; logical load zeros all other
bits. Logical immediate operations set CC from only the field modified.
Register mask tests distinguish mixed results by the leftmost selected bit,
without modifying the register. Native and interpreter COBOL tests call an
assembled routine that combines carry and partial-register instructions on
an eight-byte shared binary argument.

The long-displacement encodings retain their six-byte instruction sizes
and signed 20-bit displacements while sharing execution semantics with the
original 32-bit operations. Legacy RX instructions still require unsigned
12-bit displacements. RS/SI forms reject index registers. Reversed loads
and stores convert 2/4/8 bytes without relying on host byte order. LRVH
updates only the low 16 bits, LRV/LRVR only the low 32 bits; all preserve
condition codes. Signed byte loads sign extend to 32 or 64 bits, retaining
the upper word for LB/LBR. Native COBOL tests exercise reversed bytes and
long-displacement data references through shared arguments in both native
and interpreter execution.

Pair operations require an even first register. Unsigned 64-bit multiply
returns a 128-bit product; unsigned division accepts a 128-bit dividend.
The 32-bit variants use the low words of two registers and preserve both
upper words. Signed single division uses only the odd register as dividend,
returns remainder/quotient in the even/odd registers, and truncates toward
zero. Zero divisors and quotient overflow produce execution errors.
Shift counts come from the low six bits of the base-plus-displacement sum,
without accessing storage. Arithmetic shifts set condition codes; left
arithmetic shifts preserve the sign bit and detect overflow. Other new
shifts, rotates, multiply and divide operations preserve condition codes.
USING selection prefers the smallest nonnegative displacement before using
a negative displacement, then the smallest negative magnitude; tied base
addresses prefer the higher-numbered register.

Multiple control sections in one source are aligned within a common load
image. Internal address constants are fixed once at load time, retained
across calls and rebuilt on CANCEL. ENTRY aliases share the same module
storage; after the containing module is loaded, its aliases are available
to subsequent calls. An alias used as the first call still needs a load-library
member under that alias.

## External linkage and layout

`zas link [-L LIB] -o linked.zo main.zo sub.zo` combines assembler modules.
The first input supplies the main entry (including its END operand). CSECT
and ENTRY symbols are exported independently of that main entry. Internal
and external address relocations are preserved through serialization and
resolved against the actual image load origin.

Relative instruction references to EXTRN/WXTRN symbols are retained with
their addends in the object. The binder resolves them after final member
placement and validates halfword alignment and signed 16/32-bit displacement
ranges. EQU aliases, including complex definitions and forward alias chains,
and same-section differences in addends are supported. Complex EQU values
retain normalized coefficients, rather than being mistaken for absolute
constants. Intermediate values may have net coefficient zero, two or another
value; the final relative operand must have coefficient one. Subtraction can
reduce aliases to absolute or simply relocatable values for ordinary operands
and address constants. Inherited length attributes remain available via `L'`.
The location counter in EQU is captured at the definition, including when a
forward dependency postpones evaluation. Composite relative operands retain one coefficient per external
symbol and an image-origin coefficient for local terms. Linking rebases the
local part, evaluates each resolved term and retains unresolved weak terms
for later binding. Strong terms request library inclusion even when relocation
metadata is the only remaining reference.

`EQU VALUE[,LENGTH[,TYPE[,PROGRAM[,ASSEMBLER]]]]` supports explicit attributes,
including omitted fields and expressions. Lengths are absolute in 0..65535;
types are byte values in 0..255. The value can have forward dependencies,
but explicit attributes must be evaluatable at the definition. An independent
source-order symbol view prevents forward constants precomputed for layout
from leaking into attribute evaluation. Earlier pending EQU values become
available to attributes only after their dependencies have been defined.
Attributes captured at the definition are reapplied when forward values resolve.
Ordinary arithmetic expressions accept `L'SYMBOL` and `T'SYMBOL`; the latter
returns the numeric EBCDIC type byte. Default EQU types are U, even when the
first term has an explicit type. Default lengths follow the first term.
Instruction labels now carry their physical instruction length and type I;
DC/DS labels carry the first storage item's length and type. Tests cover
attribute boundaries, independent overrides, forward values, source-order
errors, aliases, implicit MVC lengths and native/interpreted COBOL/JCL calls.
The optional program attribute is an absolute 32-bit value evaluated at the
definition. Signed values and unsigned bit patterns are accepted; omitted
and explicitly zero attributes remain distinct. The optional assembler type
accepts AR, CR, CR32, CR64, FPR, GR, GR32, GR64 and VR (case insensitive).
Program and assembler types are retained with symbol values during assembly,
including after forward values resolve, and reset to null when omitted on
another EQU alias. Conflicting redefinitions are diagnosed. Typed register
equates work in instruction operands, including native/interpreted COBOL and
JCL calls. These annotations do not activate register type checking or add
execution support for unsupported instruction families. Register type-checking
controls remain unsupported.

Macro expressions support `SYSATTRP('symbol')` and `SYSATTRA('symbol')` after
parameter/SET substitution. They work in SETC, AIF and nested character/
arithmetic functions, including `C2A(SYSATTRP('symbol'))`. Program types return
all four bytes decoded from EBCDIC; assembler types return their unpadded
names. Undefined symbols, omitted attributes and null arguments return an
empty string. An explicitly zero program type remains four zero bytes.
A registry follows actually generated source order across COPY and nested
macro expansions; skipped statements and later definitions are not exposed.
EQU attributes are recorded independently of a forward value. Known absolute
EQU values can supply later attribute expressions, and pending constant
chains resolve as their definitions appear. Quoted query text stays literal,
and program values containing apostrophes or ampersands remain character data.
Supported DC/DS declarations report their implicit assembler types, including
FD extensions. Location-dependent attribute expressions cannot yet be evaluated
during expansion: their queries return a source diagnostic rather than assuming
a zero location. DC/DS P() program annotations and implicit assembler types are
implemented as described in the P2 progress section.
Attribute lookahead now scans subsequent open code and COPY text. Tests cover macro parameters, SET arrays,
COPY, nesting, null/zero, source order, byte values and native/COBOL/JCL calls.

Conditional expressions now accept ordinary `L'`, `T'` and `D'` attributes.
They are expanded only in SETA/SETB/SETC, AIF, computed AGO and ACTR, preserving
ordinary assembly expression behavior. L' is numeric, T' is character-valued
and D' is numeric 0/1. Generated DC/DS, instruction and EQU definitions supply
attributes; EXTRN/WXTRN declarations also mark symbols defined. Macro arguments,
SYSLIST entries and SETC array elements can name ordinary symbols, and L'/T'
of a parameter sublist use the first entry. SETA/SETB symbols have type N and
cannot be used for D'. The registry follows generation across COPY/macros
and conditional branches, enabling D' guards against duplicate definitions.
Machine labels retain physical instruction sizes. Explicit fixed-point
lengths use type G, including in ordinary numeric T' expressions.
Unavailable lengths and types M/N/O/T/U/$ are diagnosed. Lookahead skips macro
definitions/generation, performs no model-field substitution, ignores AIF/AGO,
and leaves D' definition status unchanged. Unresolved inherited EQU lengths
may remain unavailable. Supported system-variable T'/K'/N' values reflect
their numeric, character, scalar or sublist semantics.
Tests include quoted attribute text, arbitrary type-byte escaping, generated
source order, ordinary numeric T' behavior and shared-storage COBOL/JCL calls.

Literal L'/T'/D' attributes now work in conditional expressions, including
macro parameters, SYSLIST and SETC elements. Ordinary assembly expressions
also accept literal L' and numeric T'. A shared scanner handles quoted nominal
values, doubled apostrophes, duplication and address-constant parentheses.
L'/T' define metadata when their statement executes; D' merely observes it.
Machine literal operands and ordinary L'/T' references update the same
generation-order registry. Attribute-only references emit no pool storage
and do not resolve A/V nominal values or pull external libraries. Constant
syntax, supported types and declaration-size limits are still checked.
The literal specification supplies identity, so byte-equivalent but different
specifications are distinct. Location-counter references have statement-local
identity; character stars and arithmetic multiplication do not. The shared
location detector also fixes unnecessary duplicate physical pool entries for
character literals containing '*'. Tests cover source order, COPY, LTORG,
escaped text, macro substitution, ordinary numeric attributes, physical pool
relocation and the absence of storage/library side effects from attributes.
Numeric S'/I' attributes are available for ordinary symbols and literals in
assembly and conditional expressions, including macro parameters, SYSLIST and
SETC arrays. F/H/FD use the S modifier (default zero) and integer capacity
8*L'-S'-1. P/Z constants
accept a decimal point, which contributes no stored byte; their scale is the
number of fractional digits. Integer capacity is 2*L'-S'-1 for P and L'-S'
for Z. Lists use their first nominal value's length and numeric attributes;
duplication repeats the entire list. EQU resets both numeric attributes to
zero, independently of inherited length/type. Other types diagnose unavailable
numeric attributes. Attribute-only literal references still allocate no pool
storage. Other constant types' expression-valued lengths, floating constants and configurable
literal-type compatibility modes remain unsupported. Tests cover signed
decimal bytes, explicit lengths, lists, source order and shared-storage calls.
The same library inclusion path handles data relocations and instruction
relocations, including transitive calls and native embedded modules.
Callers using BRAS/BRASL must set R15 with LARL when the callee requires it;
relative call instructions only populate the specified link register.
Unresolved strong relative references reject linking/loading/execution.
Weak relative references never pull a library member and resolve if a
member is explicitly included. A single absent weak target is absolute zero,
including any addend, matching Zeus's weak address-constant convention.
Within a composite expression an absent weak symbol contributes zero while
the other terms and addend remain. The image-origin coefficient is retained
so mixed resolved/weak expressions evaluate correctly after serialization,
loading at another origin and subsequent links that supply a definition.
Short weak encodings are checked at the actual load origin as well.
Relinking a combined image rebases resolved references with their containing
image, while unresolved weak references remain available for later binding.
Single-symbol instruction relocation metadata uses Zeus module format 2;
composite instruction metadata uses format 3. Composite address relocations
use format 4. Readers retain support for formats 1 through 3. Images without
composite address relocations or pending instruction fixups use format 1.
Objects carrying relocation metadata
introduced after their declared format are rejected to avoid silently losing
a required fixup in older readers. Tests exercise positive and negative
external coefficients, local origin coefficients, library discovery, repeated
linking, weak definitions, malformed metadata and invalid target ranges.
Native/interpreted COBOL and JCL binder tests load complex EQU address
constants to set R15 before relative calls.

Fullword A-type constants now accept complex linear expressions, including
external differences, multiple positive/negative external coefficients, local
terms and complex EQU aliases. Unlike relative instruction targets, these
expressions need not have net relocation coefficient one. Each relocation
retains its addend, image-origin coefficient and remaining external terms.
After binding, the origin coefficient remains so serialization, rebinding
at another image offset, loading and CANCEL all recompute the correct value.
The stored result is the low 32 bits, including negative differences in
two's complement. Signed absolute fullword A constants are also supported.
Arithmetic overflow of the internal checked i64 representation is an error.
An absent weak symbol contributes zero within a composite expression while
the other terms and addend remain. Single weak-symbol constants retain their
zero-target value, retaining any constant addend. DSECT terms and relocatable
multiplication/division remain unsupported. Explicit A lengths, AD, VD and Y
forms now retain their widths through module format 7.
Tests cover duplicated constants and multiple nominal values, where `*`
advances per field, and address literals, where it denotes the referring
instruction. Strong data terms pull libraries even when the EXTRN metadata
has been removed. Invalid format downgrades and conflicting relocations
are rejected.

EXTRN and V-type constants create strong external references. Named members
are pulled transitively from load libraries as needed. WXTRN references are
resolved when a definition is explicitly included or brought in for another
strong reference; otherwise their address is zero. Weak references never
trigger automatic library inclusion. A-type external expressions support
addends, including negative ones. Missing strong definitions and duplicate
exports fail linking before output is written.

The runtime loader uses the same linker for COBOL/PL/I calls, `zas run` and
JCL EXEC, using their load libraries. The JCL IEWL path combines concatenated
HLASM SYSLIN modules and searches binary SYSLIB libraries. Its COBOL/PL/I
handling remains separate: this does not link IBM object decks or combine
COBOL/PL/I objects with assembler objects. Cross-language calls retain their
existing shared-storage runtime linkage.
Aliases share reference-counted module metadata, rather than duplicating the
module's initial storage and instruction list for each exported symbol.
Loading another linked image does not replace an existing independently
loaded instance of one of its members.

LTORG now emits a doubleword-aligned pool at the directive. Literals are
deduplicated within a pool and stably grouped by lengths divisible by 16,
8, 4, 2, then odd lengths. Later pools have independent references. A(*)
literals use the location of the referring instruction, including duplicated
address literals. Unemitted literals are placed at the end of the first
control section's most recently used LOCTR. Use explicit LTORG in another
section when that section's base registers must address the literals.
Pools in DSECT and configurable section alignment remain unsupported.

ORG supports relocatable expressions in the current section, optional
power-of-two boundary and absolute offset, and an omitted expression to
restore the highest allocated location. Data can overlay existing data;
DS can describe an overlay without erasing existing bytes. Overlapping
instructions and DC overwriting instructions fail explicitly. Execution
falling into a literal pool, data or an alignment gap also fails explicitly.

Execution uses a checked instruction engine, with storage bounds checks
and a limit of 1,000,000 instructions per invocation. The output is a Zeus
module, not an IBM object deck or native z/Architecture machine code.
Macro expansion is limited to 100,000 steps and nesting depth 32.
Unknown operations fail compilation with source diagnostics.

## Macro expressions and scopes

Local and global SETA/SETB/SETC symbols support arrays, arithmetic subscripts,
implicit local declaration on assignment, and extended assignment lists.
An omitted list item leaves that array element unchanged. Assignment can
grow the extent beyond the declaration. Global arrays are accessed directly
from shared expansion state; they are not copied for each generated statement.
Local arrays reset for each invocation. Storage uses sparse elements with
the declared type's default value for unassigned slots.

Created names such as `&(&KEY)` and `&(&KEY.S)(2)` provide indirect SET access.
Created SETs remain distinct from a same-named formal parameter. Parameters
and system symbols are read-only; inconsistent types, duplicate keywords,
undefined variables, invalid indices and out-of-range SETA values fail with
source diagnostics. Operand boundaries and keyword classification are kept
from the original invocation before substitution. A comma or equals sign in
a SETC value therefore does not create another macro argument.

SETC implements quoted strings with escaped apostrophes and ampersands,
period concatenation, duplication factors and one-based substring notation,
including `*` for the remaining characters. Substring selection occurs before
duplication. Numeric SET substitutions in quoted character contexts use
unsigned character representations. SETA supports signed 32-bit values and
bitwise AND/OR/XOR/NOT plus SLL/SRL/SLA/SRA. SETB/AIF compare numeric values
or EBCDIC character values, including substring and function expressions.

K' and N' attributes apply to variables and sublists. `&SYSLIST` supports
nested positional indices, omitted operands and index zero for the invocation
label, excluding keyword operands. `&SYSNEST` reflects actual macro nesting
rather than COPY depth. `&SYSMAC` identifies the current and enclosing macros,
ending with OPEN CODE; `&SYSNDX` distinguishes invocations, and `&SYSSTMT`
reports the next statement to be generated. Other system variables are not
implemented.

Implemented built-ins in function-call syntax:

- Arithmetic results: B2A/C2A/D2A/X2A; INDEX/FIND; ISBIN/ISDEC/ISHEX/ISSYM.
- Character results: A2B/A2C/A2D/A2X; BYTE/SIGNED; UPPER/LOWER/DOUBLE.

Computed AGO selects from a target list; extended AIF tests clauses in order.
ACTR limits taken conditional-assembly branches within a scope. END terminates
expansion even when reached inside a macro. Safety limits are 1,000,000 for
array indices, 4064 characters per SETC value, nesting 64 for macro expressions
and variables, and 1 MiB per expanded operand, in addition to the expansion
step/depth limits above. These are implementation limits, not a claim that
every HLASM limit, warning or listing detail has been reproduced.

## Remaining work for full HLASM compatibility

Full language support has **not** been achieved. In particular, the following
require additional implementation and conformance testing:

- AMODE 64 linkage, remaining scalar and 64-bit instructions, floating-point formats and
  instructions, vector instructions, privileged instructions and exceptions.
- IBM object-deck and cross-language object linking; pools in DSECT,
  private/COMMON/GOFF section facilities and configurable SECTALGN.
- External SETAF/SETCF functions, IBM maintenance/PTF identities, additional
  assembler option profiles, AREAD/AINSERT and configurable source formats.
- Additional constant types, floating/bit-length modifiers, remaining expression forms and address
  lengths; complete listing/control directives, diagnostics and ADATA.
- IBM macro libraries, Language Environment linkage variants and general
  z/OS services. No IBM-supplied macro source is embedded.

These gaps are explicit rather than accepted as no-op instructions. The
compiler does not certify compatibility merely because a particular program
assembles.

`zdeps` recognizes HLASM programs and macro/COPY dependencies. Its existing
compile diagnostics and unsupported syntax CSV files include assembler
results. As for other languages, compilation can stop at the first fatal
error; reports are not an exhaustive inventory of every unsupported use.
External-symbol edges follow EXTRN/WXTRN and V constants, resolving CSECT
and ENTRY aliases. Unresolved weak references use OPTIONAL status and are
excluded from required missing-asset counts. A strong reference to the same
symbol upgrades its status to MISSING.

## Implementation priorities

This is an engineering priority order for application HLASM used with Zeus
COBOL, PL/I and JCL. It is not a measured frequency ranking: the current
corpus has not been audited against every language feature and operand form.
Host-CPU native code generation for HLASM remains explicitly outside scope,
including future work; it is not a backlog item at any priority.

Priorities favor observable compilation/execution blockers, correct shared
storage and cross-language behavior, and facilities that unlock several other
features. Difficulty determines sequencing within a feature, not whether an
important feature is silently omitted. A source-confirmed blocker can be
promoted; promotion should record its file, line, expected behavior and test.

| Priority | Work | Reason and sequencing |
| --- | --- | --- |
| P0 | Diagnostic baseline and compatibility inventory | Distinguish fatal errors, warnings and informational messages; implement MNOTE severity behavior. Record unsupported operations and operand forms with source locations and distinguish missing macro members. Recover where safe to report independent errors; mark reports incomplete after unrecoverable errors. Keep existing shared-storage, relocation and call-frame regressions passing. |
| P1 | Macro compatibility | Extend system symbols used by source, O' and other remaining attributes, conversion/string built-ins and their expression forms; add OPSYN and macro-definition facilities. These can block expansion before implemented instructions are reached. Supply real context values rather than placeholders. |
| P1 | Layout and base-register facilities | CNOP, section reopening, LOCTR, named/dependent USING, PUSH/POP state and section attributes. Establish layout invariants before extending relocations and address constants that depend on them. |
| P1 | Application string and decimal instructions | EX, MVCL/CLCL and related string operations, TRT, SRP, ED/EDMK; fill scalar instruction gaps encountered in source. EX requires a deliberate instruction-template representation and decoding/execution design; it must not silently ignore modified instruction bytes. Implement individual families with CC, aliasing, bounds and exception tests. |
| P1 | Dynamic shared storage | GETMAIN/FREEMAIN and STORAGE-compatible allocation paths. Define ownership, lifetime and reuse in the run unit before adding callers. Persistent allocations must survive subroutine-frame cleanup and be reclaimed correctly on explicit free or step teardown. |
| P2 | Module services and reverse language calls | LOAD/DELETE, LINK/XCTL and HLASM-to-COBOL/PL/I dispatch. Build on allocator/lifetime rules and a Machine-level call interface; validate shared addresses, R15, save areas, error unwinding and module lifetime. |
| P2 | Remaining constants and expressions | Bit lengths, expression duplication, remaining length expressions, CA/CE/CU/G and Y/S/AD/VD/J/Q/R families, with relocation/section prerequisites first. Complete DC/DS attributes and annotations. Promote specific declaration blockers found in application source. |
| P2 | Source encodings and controls | EBCDIC/DBCS input, ICTL/ISEQ and source-format settings. Reuse the project's code-page handling and keep byte/column semantics explicit. Promote if actual input cannot otherwise be read correctly. |
| P2 | Remaining scalar facilities and basic services | Conditional/bit operations, compare-and-swap families and application service subsets such as WTO and WAIT/POST. Define a real concurrency model before claiming multithreaded atomic or synchronization compatibility. |
| P2 | Data-management macro/service integration | OPEN/CLOSE and record I/O paths through existing Zeus DD/data-set facilities, followed by source-required VSAM interfaces. Do not claim standard macro-library compatibility from a few recognized service names. |
| P3 | Language Environment and additional linkage | LE linkage variants, save-area conventions, exception integration and AMODE 64 with wide argument/address handling. These require an ABI design, not merely enabling 64-bit arithmetic instructions. Promote when a concrete calling convention requires them. |
| P3 | Floating-point formats and instructions | HFP/BFP/DFP constants, registers, conversion, arithmetic, rounding and exception semantics. Plan coherent format/instruction subsets; avoid claiming compatibility from host floating-point substitution. |
| P3 | IBM object interoperability | Object decks/GOFF, instruction decoding, relocation records, binder controls and IBM-produced cross-language objects. This is separate from host-native compilation and remains in scope, but is a substantial project beyond source-level Zeus modules. |
| P3 | Detailed assembler outputs and options | Listings, cross-references, ADATA, listing directives, ACONTROL/*PROCESS and remaining diagnostic/options compatibility beyond P0's useful diagnostics. |
| P4 | Vector, cryptographic and specialized facilities | Implement according to an explicit architecture level and observed demand, after application scalar semantics and ABI foundations. |
| P4 | Privileged/system architecture and broad OS emulation | PSW/interrupt/control-register behavior, address spaces, protection keys and other operating-system facilities. These need an explicit emulated machine/OS model and have the lowest default priority for application workloads. They are not implied by parsing the mnemonic. |

The initial execution sequence is: **MNOTE and diagnostic inventory**, then
**source-driven macro gaps**, **CNOP/layout**, **string/decimal instruction
families**, and **shared dynamic storage**. Layout and instruction families
should be split into reviewable increments; this order is not a requirement
to finish an entire category before addressing a demonstrated blocker.

For each increment, acceptance requires compilation and semantic execution
tests, serialization/linking tests where relevant, and a COBOL/PL/I/JCL
integration test when the shared ABI or services change. A successful compile
alone does not establish compatibility. Implement from published specifications
and independently written tests, following the clean-room contribution rules.
Do not add IBM-supplied macro source to achieve coverage.

## P0/P1 implementation status

The implementation of the P0/P1 increments listed above is delivered and
regression-tested. These are source-level application
facilities for the Zeus 31-bit run unit, not a claim of complete HLASM,
IBM object-code or z/OS compatibility.

- Structured `compile_report` diagnostics preserve source file/line, operation,
  category and severity. Unknown operations/missing macro members can be
  collected independently; unsafe recovery is marked incomplete and cannot
  emit a module. Fatal layout/operand errors still stop further assembly.
- MNOTE information/warnings no longer reject an otherwise valid module.
  The highest severity becomes the CLI/JCL return code; severities 8..255
  prevent module output. SYSM_HSEV and SYSM_SEV expose assembly and last
  direct macro-call severity. CLI `--diagnostics` writes a JSON report.
  Dependency analysis records each collected diagnostic plus report
  completeness and return code in node attributes.
- OPSYN supports operation/macro snapshots and deletion. Conditional O'
  queries reflect known operations, aliases, deletion and library discovery.
  Nested definitions shield inner variables; macros can override operations
  while saved aliases retain the original built-in behavior. Aliased MACRO
  and MEND preserve sequence-symbol scope.
- The complete A/B/C/D/X representation-conversion matrix, DCVAL/DEQUOTE/DCLEN,
  traditional logical-expression character/search forms, Boolean XOR and
  arithmetic truth values are supported. Character comparisons use length
  before EBCDIC collation. Attribute lookahead handles source/COPY declarations
  without expanding macros or changing D' status.
- AssemblyOptions/compile_report_with_options supply actual product/version,
  time, job/step, input/output and selected library/member context. Macro-local
  section/location/type, clock and sequence fields are frozen at entry.
  CLI --sysparm and JCL SYSPARM(...) preserve the supplied character value.
- CNOP emits executable no-ops, with module format 5 preserving alignment
  through linking and loading. Section reopening and LOCTR retain independent
  counters; LOCTR segments have no implicit alignment, CSECT resumes the last
  counter, and a named LOCTR can resume its owning section. Alignment phases
  are resolved iteratively without changing source order or USING domains.
- Labeled/dependent USING, explicit ranges, DROP and independent PUSH/POP
  USING/PRINT/ACONTROL stacks are supported. Additional assembler option tables
  and listing production remain separate work; ACONTROL OPTABLE(ZEUS) names
  the instruction table actually supplied by this implementation.
- RSECT stores raise protection exceptions during HLASM execution; installed
  ranges are shared across assembler calls. External writes are detected on
  re-entry, rather than relying on host page protection. Format 6 preserves
  protection and RMODE 24 loading limits through serialization and linking.
  AMODE 31/ANY/ANY31 declarations use the existing 31-bit calling convention.
- TRT/TRTR implement forward/reverse table tests and implied register/CC
  effects. SRP implements packed shift/rounding and truncated overflow results.
- EX/EXRL execute compiled instruction templates, overlaying their current
  second byte without changing the template. Relative branches use the target
  origin and link operations use the execute continuation. Known RRE opcode
  changes are decoded within their primary opcode; unsupported changes and
  non-template data are diagnosed. General IBM machine-code decoding is P3.
- MVCL/CLCL, MVCLE/CLCLE, MVST/CLST/SRST and ED/EDMK include condition codes,
  implied registers, padding/termination, partial resumption and bounds checks.
- GETMAIN/FREEMAIN R/RU/RC and STORAGE OBTAIN/RELEASE use run-unit ownership.
  Explicit frees split/coalesce/reuse storage and cannot free static data or
  live call frames. Allocations survive returns and errors, and disappear at
  explicit free or run-unit/step teardown. A retained-growth budget prevents
  page alignment from bypassing the active-allocation limit. Native COBOL,
  interpreter and PL/I/JCL tests exercise pointers after the allocating
  assembler call returns and release them in a later call.

The remaining full-compatibility items in the earlier section retain their
P2-P4 scope. In particular these services do not emulate OS authorization,
keys, address spaces, arbitrary IBM macro parameter-list formats or every
GETMAIN variant. IBM-specific maintenance identities are not fabricated.
Operand diagnostics explicitly mark unfinished recovery as incomplete.

## Validation and reference material

### P2 completion (application-source scope)

- Input controls: source-format auto preserves extended host remarks; fixed
  honors the physical continuation column, including records longer than 80
  bytes. Explicit ICTL uses strict columns in either mode. ICTL selects
  begin/end/continuation columns, with standard
  columns retained independently for COPY/library input. ISEQ checks physical
  records, including comments and continuations, and emits severity-4 warnings
  without preventing module output. UTF-8/BOM and fixed-record EBCDIC input
  support the five existing Zeus CCSIDs. Mixed DBCS input preserves byte columns
  and diagnoses malformed shifts; source-encoding selection also covers libraries.
- Interlocked instruction execution: CS/CSY/CSG, CDS/CDSY/CDSG, CSST, TS and the
  LAA/LAAG/LAAL/LAALG/LAN/LANG/LAO/LAOG/LAX/LAXG families. The engine exclusively
  owns a run unit's storage while executing an instruction; fetch/compare/store
  completes before any other program runs. LPD/LPDG fetch both naturally
  aligned operands before updating their register pair, with CC=0 in this
  serialized model. This is a serialized run-unit model,
  without independently running CPUs or concurrent external writers. CSST covers
  all three comparison widths and five additional-store widths, parameter/destination
  aliasing and ordered overlapping stores. Protection errors suppress both stores.
- LINK/XCTL dispatch through the owning Machine to HLASM, COBOL, registered
  host programs and a configured PL/I linker. Addresses and allocation metadata
  are lent together; failures restore ownership and release invocation frames.
  EP/EPLOC and PARAM/VL standard-source forms are implemented. Unchanged incoming
  R1 lists retain their known count and original terminator convention; known
  COBOL/PL/I signatures supply fixed counts. Arbitrary lists with no known count
  require a high-bit terminator.
- LOAD/DELETE manage assembler image responsibility counts across entry aliases.
  Executing images are retained until return; unused images and protection
  metadata are released, and reload restores their initial storage. Loaded
  entries support register branches with the caller's actual low/high registers,
  CC and return address. Implicit COBOL/PL/I CALL instances retain run-unit
  lifetime; LINK responsibility ends on return. COBOL/PL/I/host LOAD entries
  have virtual dispatch slots; existing language instances retain run-unit
  lifetime. Known COBOL/PL/I signatures allow fixed address lists.
- Constants: expression duplication/byte lengths, CA (CCSID 819), CE, CU
  (UTF-16BE), G with mixed ZEUS_CCSID, nominal lists, bit-field packing and
  Y/AD/VD/explicit address lengths. Module format 7 records relocation widths;
  linking/loading preserve them and older formats default to fullwords.
  Mixed declarations align each operand using its actual location.
- WTO writes to the run-unit output sink and retains routing/descriptor metadata
  for the most recent 1024 operator messages. POST updates ECBs and WAIT handles
  completion counts and high-bit-terminated ECB lists. Host producers can queue
  completions for cooperative WAIT dispatch. A wait without enough completed or
  queued events reports a deadlock and cleans up wait registrations; it never
  invents a successful completion. This does not supply independent host CPUs.
- Conditional LOC/LOCG/LOCFH, register forms, signed halfword immediate forms,
  STOC/STOCG/STOCFH and condition-suffix aliases preserve CC and unselected
  register halves. This engine validates storage access even when unselected.
- DCB and QSAM OPEN/CLOSE/GET/PUT/FREEPOOL integrate DD fixed/RDW/text datasets.
  Move/locate modes, EODAD/SYNAD, variable-record RDWs, copied DCBs, open-file
  pins, close/teardown flushes and image-lifetime cleanup are implemented.
  Module format 8 rebases and validates DCB metadata and compiled exits.
  Native COBOL and repeated JCL steps exercise this interface.
- S/SY constants use active USING at the declaration/pool, preserve referring
  instruction * in literals and advance * per duplicated DC nominal. Q/QD/QY,
  J/JD, DXD/CXD and XATTR PSECT with R/RD use module-format-9 section quantities.
  External dummy extents merge by maximum length/alignment. QY and three-byte
  R/RD validate their ranges instead of truncating them. Quantity references
  participate in library inclusion. CATTR classes separate protected code from
  writable PSECT data; NOLOAD PARTs merge by name and priority. Q/QY class-label
  references retain their element positions through linking and serialization.
  R resolves the start of the class/section containing its XATTR label, including
  external ENTRY labels. Mapped class spans retain inter-element Zeus image gaps;
  J measures the actual span and Q reaches the actual mapped field. NOLOAD
  classes/parts retain their separate logical allocation layout. Tests compare
  R+Q to exported field addresses and execute reads through that address in Machine.
- DC/DS P() annotations expose program and assembler types via SYSATTRP/SYSATTRA.
  Address/graphic type attributes include implicit J=A, R=V, G=@ and explicit
  address lengths=R. Existing stores now check protection before writing even
  when the new bytes would equal the old ones.
- RISBG/RISBGN/RISBHG/RISBLG and RNSBG/ROSBG/RXSBG support zero/test aliases,
  wrapping ranges and aliased inputs. FLOGR, POPCNT, three-register logical
  operations (including complemented forms), and SELR/SELGR/SELFHR families
  have CC, register-half, overlap and operand-validation regressions.
- Corpus audit: CardDemo contains two executable assembler sources, COBDATFT
  and MVSWAIT, with no VSAM macro use. The downloaded Bank of Z .asm files are
  under IMS PSB/DBD definition directories. No additional executable assembler
  sources were found in the fetched GenApp tree. Source-required VSAM work has
  no currently identified assembler caller; general VSAM service compatibility
  is not claimed.
- All five P2 rows above have completed implementation and regression checks
  within their stated application-source scope. Initialized PART binding,
  complete GOFF attributes and IBM object interchange are not provided by the
  new class metadata; those remain P3 object/binder work. The corpus-dependent
  VSAM condition has no current executable assembler caller, as recorded above;
  this does not establish general VSAM compatibility.

The completion checks map directly to the original priority rows:

| P2 row | Acceptance evidence |
| --- | --- |
| Module services and reverse language calls | `hlasm_services`, `hlasm_storage_services`, JCL `hlasm` and native executable regressions cover shared addresses, fixed/terminated parameter lists, R15, loaded register branches, repeated calls, unwinding and module cleanup across COBOL/PL/I/HLASM. |
| Remaining constants and expressions | `hlasm_p2_constants`, `hlasm_relative_link`, `hlasm_language` and `hlasm_literals` cover bit packing, expression lengths/duplication, character/address/section quantities, program attributes, relocation and actual R+Q field addresses after linking and loading. |
| Source encodings and controls | `hlasm_source` covers UTF-8 and EBCDIC/DBCS byte columns, ICTL/ISEQ, libraries and auto/fixed formats. Explicitly enabled CardDemo tests assemble and execute both executable assembler sources. |
| Remaining scalar facilities and basic services | `hlasm_bits`, `hlasm_conditional`, `hlasm_interlocked` and `hlasm_events` cover aliases, CC, register halves, alignments, overlapping operands, suppressed stores, queued completions and deadlock cleanup under the serialized run-unit model. |
| Data-management integration | `hlasm_io`, `hlasm_services`, JCL `hlasm` and native tests cover DD-backed fixed/variable/text records, move/locate I/O, EODAD/SYNAD, copied DCBs, close/error cleanup and repeated steps. Corpus inspection found no executable HLASM VSAM caller requiring the conditional follow-up. |

Final validation on Windows:

- `cargo test -p zeus-cobol -p zeus-pli -p zeus-jcl -p zeus-deps -p zeus-native`:
  448 passed, no failures; one corpus test ignored by default.
- `cargo test -p zeus-cobol --test hlasm -- --include-ignored`:
  all four passed, including the separately enabled real-source corpus test.
- Workspace Clippy with `--all-targets -- -D warnings`, all command binaries,
  formatting and diff checks passed. The embedded-help checker passed 60
  standalone invocations across 14 commands.
- The broader workspace test run encountered five IMS TM queue tests failing
  with Windows OS error 5 (access denied): `tm_preserves_ebcdic_dbcs_and_binary_bytes`,
  `tm_queue_commit_rollback_and_persistence`,
  `tm_queue_concurrent_producers_do_not_lose_messages`,
  `tm_reclaims_input_after_process_crash` and `tm_rejects_corrupt_queue_snapshot`.
  No IMS implementation was changed in this work; the full workspace test suite
  is therefore not recorded as passing.

### P3 progress (in progress; not complete)

The original four P3 rows remain the acceptance scope. No row is marked
complete merely because some of its source forms now compile.

| P3 row | Current state and remaining work |
| --- | --- |
| Language Environment and additional linkage | In progress: AMODE 24/31/64 source execution, 64-bit effective addresses and arguments, F4SA-sized save areas, BASSM/BSM/SAM mode switching, mixed-mode loaded entries and LINKX/XCTLX parameter-width controls. The existing seven LE date/time/abend services and CEENCOD condition-token construction support HLASM LINK and loaded-entry dispatch with checked AMODE 31 parameter lists. Synchronous OS AMODE 24/31 CEEHDLR/CEEHDLU/CEESGL/CEEGQDT and CEEDCOD now support dynamic handler lifetimes across HLASM, COBOL, PL/I and host callbacks. LE entry/exit variants, XPLINK, resume cursors, T_I_U and full HLL/hardware exception integration remain pending. |
| Floating-point formats and instructions | In progress: exact integer-based HFP/BFP/DFP constants; architectural FPR/FPC state; HFP and BFP scalar arithmetic, BFP fused arithmetic/square root, HFP/BFP integer conversions, and DFP arithmetic/comparison and short/long/extended format conversion, signed/unsigned 32/64-bit integer conversion, floating-point integer rounding, quantum/significance extraction/insertion, QUANTIZE and REROUND; SPM/IPM and HFP underflow/significance masks. HFP halve and square root are implemented. HFP lengthening, integer rounding and fused/unnormalized families, BFP explicit rounding forms, register and memory packed/zoned DFP conversions, and basic HFP/BFP radix conversions are implemented. BFP/DFP classification, DFP groups, biased-exponent comparison and significand shifts are implemented. All 54 PFPO cross-radix functions, BFP conversion/ignored-scale warnings and DIEBR/DIDBR integer quotient/remainder are implemented. Specialized instructions, detailed exception edges and further diagnostic compatibility require additional implementation and validation. |
| IBM object interoperability | In progress: basic OBJ/GOFF loading, architectural byte decoding on fetch, signed/compressed address and element-length relocation, single-external relative-immediate fixups, source/object linking and initial conventional OBJ/GOFF output with checked architectural source encodings, external RI emission and GOFF element placement/protection preservation. Architectural OBJ/GOFF bytes run through the shared interpreter from COBOL, PL/I and JCL. Full decoding/encoding, remaining record/relocation forms and GOFF attributes, binder controls, initialized PART binding and IBM-produced cross-language objects remain pending. Zeus source templates remain distinct from IBM bytes. |
| Detailed assembler outputs and options | In progress: expanded source listings, initialized data bytes, final offsets/lengths, symbol definitions/references, macro-call provenance, resolved USING snapshots, text/JSON CLI output, PRINT state and stack, TITLE/SPACE/EJECT/CEJECT and UHEAD. Empty macro calls and macro-definition file/line provenance are included. Original physical records retain comments, blank/continuation/sequence records and full read macro definitions; literal-pool data and references, AFPR/NOAFPR, ACONTROL stack restoration and invocation PCONTROL locks are included. ADATA syntax, physical-record transport/RDW/continuations and a CLI analysis stream with identification, normalized SD, expanded source, instructions, diagnostics, symbols/xrefs, libraries and user information are available. Open-code model statements retain original text and per-iteration expansions, with NOGEN first-data handling. Initial *PROCESS PCONTROL/OPTABLE and source/invocation precedence are implemented. Complete ADATA record families/attributes and automatic UT epoch conversion, remaining conditional-assembly listing details and remaining ACONTROL/*PROCESS/options compatibility remain pending. |

The floating-point increment introduces `num-bigint` and `num-traits` for exact
integer arithmetic. Host `f32`/`f64` are used only as an independent finite-BFP
test oracle, never to compute assembler constants or instruction results.
Tests cover 32/64/128-bit binary and decimal encodings, short/long/extended
HFP, all 1000 canonical DPD declets and all 1024 decoding patterns, signed zeros,
rounding ties, extreme exponents, aliases and register halves, FPC flags/traps,
scaled trap results, preferred quantum, loaded-entry FPR/FPC transfer and COBOL
COMP-2 shared arguments. `hlasm_float` currently has 73 integration tests;
additional internal tests exercise DPD and interrupting-result details.
`hlasm_abi` covers 64-bit parameter/save layouts, EXEC PARM's four-byte list,
mode switching, class-entry inheritance, wide implicit string operands,
64-bit MVCLE partial lengths, supervisor pointer restrictions and mixed-mode
LINKX calls. LINK and loaded-entry calls share FPRs, FPC and program masks.
The mask/ABI/DFP-width checkpoint passed 50 library tests and 76 integration
tests (35 floating-point, 16 ABI, 14 strings, 11 services), and workspace
Clippy with warnings denied. The subsequent class-entry test is additional;
these checkpoint counts are not a claim that all P3 acceptance tests pass.

Source constants follow the published [HLASM Language Reference](https://publibfp.boulder.ibm.com/epubs/pdf/asmr1022.pdf),
and floating-point instruction formats and arithmetic follow the
[z/Architecture Principles of Operation](https://publibfp.boulder.ibm.com/epubs/pdf/a227832d.pdf).
The implementation remains a source-decoded engine; host-CPU native generation
for HLASM remains outside scope.

Input controls follow the published [ICTL](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-ictl-instruction)
and [ISEQ](https://www.ibm.com/docs/en/hla-and-tf/1.6?topic=statements-iseq-instruction)
definitions; interlocked instructions follow the Principles of Operation,
and module calls follow the published
[LINK description](https://www.ibm.com/docs/en/zos/3.2.0?topic=module-link-linkx-description).

Integration tests assemble original CardDemo COBDATFT/MVSWAIT and execute
date conversions in both directions and a zero-interval wait. Synthetic
tests cover aliased shared arguments, repeated calls without frame growth,
storage bounds failures, fallback precedence, and JCL assembly/binding and
COBOL/PL/I calls. JCL main-program tests check PARM and R15 return status.
Native executable tests also check an embedded assembler subprogram from
both native COBOL and the interpreter.
The corpus test is ignored unless explicitly enabled after fetching CardDemo:

```sh
cargo test -p zeus-cobol --test hlasm -- --include-ignored
cargo test -p zeus-jcl --test hlasm
cargo test -p zeus-cobol --test hlasm_language
cargo test -p zeus-cobol --test hlasm_link
cargo test -p zeus-cobol --test hlasm_relative_link
cargo test -p zeus-cobol --test hlasm_relative
cargo test -p zeus-cobol --test hlasm_masked
cargo test -p zeus-cobol --test hlasm_bits
cargo test -p zeus-cobol --test hlasm_memory
cargo test -p zeus-cobol --test hlasm_integer
cargo test -p zeus-cobol --test hlasm_wide
cargo test -p zeus-cobol --test hlasm_macros
```

Public language references used for linkage and storage conventions:
[IBM COBOL data passing](https://www.ibm.com/docs/en/cobol-zos/6.5.0?topic=data-passing),
[HLASM DSECT](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-dsect-instruction),
[HLASM USING](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=instruction-how-use-using).
For expression relocation and macro semantics, see the published
[HLASM language reference](https://www.ibm.com/docs/en/SSLTBW_2.2.0/pdf/asmr1022.pdf)
and [conditional assembly summary](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=reference-macro-conditional-assembly-language-summary).
External and layout behavior follows the published references for
[symbolic linkage](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=linkages-establishing-symbolic-linkage),
[WXTRN](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-wxtrn-instruction),
[literal pools](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=instruction-literal-pool)
and [ORG](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-org-instruction).
Macro character and system semantics also refer to the public descriptions of
[substring notation](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=expressions-substring-notation),
[created SET symbols](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=symbols-created-set),
[SETA](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=symbols-seta-instruction)
and [system symbols](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=information-system-variable-symbols).

64-bit register behavior and instruction formats follow the published
[IBM z/Architecture Reference Summary](https://www.ibm.com/support/pages/sites/default/files/2021-05/SA22-7871-10.pdf)
and [64-bit binary operations](https://www.ibm.com/docs/en/zos/3.1.0?topic=space-64-bit-binary-operations).
FD layout follows [fixed-point constants](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=value-fixed-point-constantsf-h).

Pair arithmetic, shifts and rotate semantics also follow the public
[IBM z/Architecture Principles of Operation](https://www.ibm.com/docs/en/module_1678991624569/pdf/SA22-7832-14.pdf?cp=HW11W).
Long-displacement USING selection follows the language-reference clarification
in [IBM APAR PQ74561](https://www.ibm.com/support/pages/apar/PQ74561).

Signed fullword immediate representations also follow the assembler's
[instruction operand conventions](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=instructions-about-supported-table).

External relative fixups follow the published [HLASM RI-format operand rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=instructions-ri-format)
and [relative-immediate relocation dictionary entries](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=listing-relocation-dictionary-rld).
Complex EQU substitution follows the published [EQU instruction rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-equ-instruction).
Attribute query results follow the published [HLASM language reference](https://www.ibm.com/docs/en/SSLTBW_2.2.0/pdf/asmr1022.pdf).
Conditional attributes follow the published [defined attribute](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=attributes-defined-attribute-d),
[length attribute](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=attributes-length-attribute-l)
and [type attribute](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=attributes-type-attribute-t) rules.
Literal attribute definition behavior follows the clarification in
[IBM APAR PH43846](https://www.ibm.com/support/pages/apar/PH43846).
Composite A constants follow the published [A and Y address-constant rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=constants-address-constantsa-y).

The P0/P1 increments also follow the published
[MNOTE rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-mnote-instruction),
[LOCTR rules](https://www.ibm.com/docs/en/SSLTBW_2.2.0/pdf/asmr1022.pdf),
[lookahead rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=instructions-lookahead),
[dependent USING](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=ui-dependent-using-instruction),
[STORAGE RELEASE](https://www.ibm.com/docs/en/zos/2.5.0?topic=storage-release-option)
and [z/Architecture Principles of Operation](https://www.ibm.com/docs/en/module_1678991624569/pdf/SA22-7832-14.pdf).


P2 section quantities and annotations follow the published
[DC/DS annotation rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=option-extensions-dc-ds-equ-assembler-instructions),
[Q constants](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=constants-offset-constantq),
[J constants](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=constants-length-constantj),
[R constants](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=constants-psect-referencer),
[DXD](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-dxd-instruction),
[CATTR](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-cattr-instruction-zos-cms)
and [XATTR](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=statements-xattr-instruction-zos-cms) descriptions.


P3 scalar/output checkpoint (still in progress):

- HFP halve/square root, DFP format/integer conversion, integer rounding,
  QUANTIZE/REROUND and exponent/significance manipulation are covered by
  `hlasm_float` (75 tests), with internal interrupt-result and DPD tests.
- `hlasm_le` has four integration tests for LINK, LOAD/fixed arguments,
  AMODE 64-to-31 parameter width, malformed areas and original ABEND codes.
  The seven AWIs retain their documented AMODE 31 interface; see
  [LE callable services](https://www.ibm.com/docs/en/zos/3.2.0?topic=guide-language-environment-callable-services).
- `hlasm_listing` has eight tests for source/member locations, unchanged module
  output, type/quoted-text filtering, CLI files, PRINT state/stack, macro
  provenance, pages and resolved USING headers. The listing uses Zeus formatting;
  its instruction slots are not IBM instruction bytes. JSON is not ADATA.
- The later related-package regression checkpoint passed 537 tests with no
  failures (one ignored corpus test), including LE/listing integration and
  page controls. Subsequent scalar extensions passed 54 library tests,
  73 floating-point tests and seven EXECUTE-template tests. These are
  incremental checkpoints, not final P3 acceptance.

GETMAIN's AMODE 64 successful pointer result and supervisor parameter bounds
follow the public [assembler services reference](https://publibfp.boulder.ibm.com/epubs/pdf/iea2a2a0.pdf)
and [addressing-mode restrictions](https://www.ibm.com/docs/en/zos/2.5.0?topic=us-addressing-mode-amode).


The scalar conversion checkpoint additionally covers signed/unsigned register
packed decimal, packed/zoned memory, sign-ignore and force-plus-zero controls,
ASCII/EBCDIC zones, malformed digits, completed decimal-overflow stores,
BFP explicit rounding/suppression, HFP/BFP conversion CC and unchanged IEEE
flags, and maximum-finite overflow under preparation rounding. Source nominal
exponents use unbounded integers before modifier cancellation and safe range
reduction; coefficients and actual encoded precision remain exact.


Classification/quantum-operation checkpoint:

- TEST DATA CLASS covers every sign/class bit for all BFP/DFP widths. The
  decimal normal/subnormal mask ordering follows its distinct architectural
  bit assignments. Classification and group tests leave IEEE state unchanged.
- DFP TEST DATA GROUP chooses the permitted safe-nonzero grouping for
  subnormal values with a nonextreme exponent. Programs must not depend on
  which of the two architecturally permitted groups a machine chooses.
- Biased-exponent comparisons include infinities and signaling/quiet NaNs.
  Digit shifts preserve quantum and sign, discard shifted-out digits and
  clear reserved special-value fields. Wide mask/count values are reduced
  before converting them to run-unit offsets, so ignored high bits do not
  become spurious memory errors.
- All 14 commands built, and the standalone embedded-help checker passed
  60 invocations. The subsequent classification changes require a fresh
  static/regression checkpoint. The later scalar regression passed 563 tests
  with no failures (one ignored), before the PFPO and conversion-warning
  increments below.


PFPO/conversion-diagnostic checkpoint (still in progress):

- PFPO implements all 54 cross-radix functions using exact ratios, with
  implicit FPR4/FPR6 sources, FPR0/FPR2 targets, rounding selectors, validity
  tests, sign/payload conversion, preferred quantum, and GR1 return codes.
- Range traps deliver target-precision scaled values and signed exponents;
  alternate action completes with CC 2. Inexact suppression does not remove
  range DXC details or independent DFP quantum recognition. Invalid traps
  suppress destination/GR1; inexact and quantum traps complete with the
  documented exception priority. BFP tininess uses the precise input even
  when rounding delivers the smallest normal value.
- Nine internal PFPO tests cover function/control combinations, exception
  state and independent exact/inexact scaling oracles. Integration tests
  exercise all 54 functions with positive/negative exact values and signed
  zeros through EX and serialized modules, plus explicit/current rounding.
- BFP constants report severity-4 conversion warnings per nominal, including
  inexact/range cases. Scale modifiers are ignored with a warning. Layout
  retries and duplication factors do not multiply diagnostics; original
  literal/member locations are retained. Uninitialized and zero-duplication
  fields do not emit numerical rounding warnings. Warning modules remain
  usable and diagnostics propagate to stderr, JSON and text listings.
- PFPO follows the public Principles of Operation chapter 9, pages 9-35--9-47;
  BFP conversion and scale warnings follow the Language Reference chapter 5,
  pages 155--157. These are incremental validations, not P3 completion.


Initial OBJ interoperability checkpoint (still in progress):

- Module format 11 records imported architectural text ranges. The interpreter
  fetches real instruction bytes at executed addresses, retaining interleaved
  literals/data and self-modified instructions. Zeus source instructions can
  share a linked image with imported text; EX decodes the temporary byte overlay.
- Fixed 80-byte OBJ card images support ESD section/private/label/external/weak
  definitions, TXT placement and overwrites, signed/compressed A/V RLD fields,
  numeric/symbolic END entries, deferred lengths and multiple object modules.
  Nonzero translator origins are removed before placement and relocation.
- Synthetic tests cover OBJ serialization, internal/external/weak relocation,
  source/object calls in both directions, self-modification, EX, invalid records
  and shared COBOL arguments. A JCL integration test runs raw OBJ programs and
  invokes them from COBOL and PL/I over multiple steps. Libraries accept .obj
  in addition to Zeus .zo and extensionless members.
- The decoder currently handles a basic RR/RX/RS/SI/SS/RI/RIL subset. It rejects
  unsupported opcodes when fetched; accepting an OBJ container does not imply
  all instructions or language-produced IBM objects can execute.
- GOFF, XSD, common/pseudo-register allocation, Q/CXD and further relocation
  forms, complete opcode coverage, object output and binder controls remain
  pending. Format fields follow the public Program Management Advanced
  Facilities Appendix A record diagrams, independently of IBM implementation.
- The previous PFPO/diagnostic checkpoint passed 580 related-package tests,
  zero failures and one ignored corpus test, plus workspace Clippy. The newer
  object work requires a new broad regression/static checkpoint.


Initial GOFF interoperability checkpoint (still in progress):

- Fixed 80-byte and RDW-framed GOFF input reconstructs logical continuation
  records without including padding. HDR/ESD/TXT/RLD/LEN/END validate sequence,
  version, IDs/parents, counts, declared lengths and relocation bounds.
- Loadable byte-stream elements retain class/alignment/fill/protection data;
  TXT supports the documented repeat encoding, deferred lengths, nonzero
  assembler element origins, and NOLOAD structured/unstructured metadata.
- Signed and compressed address RLD items support internal elements/labels,
  external and weak references, plus fetch/store semantics. Unsupported
  mode-sensitive, offset/length/relative/long-displacement, conditional and
  bit-field relocations are explicit errors, not silently ignored.
- Nine GOFF integration tests cover fixed/RDW input, continuations, repeated
  text, origins, entries, compressed relocation, source linking, malformed
  input and standalone CLI loading/linking. A second JCL test runs raw GOFF
  from COBOL, PL/I and direct EXEC programs across multiple steps, using .goff
  library lookup. Fixtures are independently constructed from record fields.
- The machine decoder additionally handles RXY long-displacement memory and
  RRE/RRF/RRD register forms, including BFP/DFP and their distinct masks and
  register positions. Eight machine tests cover 64-bit execution, both float
  layouts, self-modification, EX and mixed source/object calls.
- The earlier OBJ checkpoint passed 595 related-package tests, zero failures,
  one ignored corpus test, and workspace Clippy. New GOFF changes require
  the next broad/static checkpoint. This does not complete P3.

GOFF field definitions additionally follow the published
[Program Management Advanced Facilities V1R13](https://publib.boulder.ibm.com/epubs/pdf/iea2b2b1.pdf),
GOFF record and relocation tables. No IBM implementation or macro source is used.


GOFF addressing-mode/class checkpoint (still in progress):

- Mode-sensitive V relocations retain explicit target-mode metadata. Linking
  resolves external target modes, and loading applies the AMODE 24/31/64 tag
  using the actual origin. Weak unresolved targets remain zero. Metadata is
  validated for version, mode, bounds and overlap, including after serialization.
- Tests execute BASSM from imported GOFF into source entries in all three
  addressing modes and pull libraries from relocation expressions without
  redundant external-name metadata.
- Class contributions are grouped by class name, independently of the owning
  SD name. Class names normalize to upper case; private and long section names
  retain their distinct semantics. Byte-aligned text ranges can contain properly
  aligned executed instructions. Empty elements remain linkable.
- The prior GOFF regression passed 605 tests with no failures and one ignored
  corpus test, plus workspace Clippy. Subsequent mode/class tests require the
  next complete checkpoint. Offset/length/relative/bit-field/conditional
  relocation and other original P3 work remain pending.


Macro-listing provenance checkpoint (still in progress):

- Listing JSON retains a complete invocation sequence with insertion positions,
  depth, generated-statement counts, caller PRINT snapshots, and definition
  prototype file/line. Empty, nested, suppressed and trailing calls are retained
  independently of whether any expanded statement was generated.
- Text walks calls and expanded rows in linear order. Call visibility uses the
  caller's PRINT state, even when the first macro-body statement changes PRINT.
  Empty calls do not claim the following unrelated data row's bytes.
- Expanded rows retain original macro-body definition file/line alongside the
  caller location. This metadata does not change macro context variables or the
  module's executable representation. It is definition provenance, rather than
  a complete original-source/comment/macro-definition listing.
- Twelve listing tests cover these cases and JSON round trips; context and
  conditional-attribute regressions pass. The prior full related-package
  checkpoint (through GOFF mode/class changes) passed 612 tests, zero failures,
  and one ignored corpus test. P3 remains incomplete.


Original-source/literal/output checkpoint (still in progress):

- `--source-listing FILE` exports the physical decoded input independently of
  PRINT suppression in the expanded listing. JSON retains encoding, format,
  member role, physical record lengths, and DBCS Unicode-to-byte columns.
  Records are captured on the actual read, including comments, blank lines,
  continuations, sequence fields, macro definitions and records after END.
  Previously decoded input survives subsequent parse/expansion failures.
- Listing literal pools include final bytes and offsets after relocation and
  placement, pool statement and all expanded references. Reused literals share
  an entry within a pool; later pools and location-dependent literals remain
  distinct. Pool PRINT/DATA controls apply to text, while JSON/xref remain full.
- Seventeen listing tests cover raw/expanded separation, member snapshots,
  EBCDIC DBCS positions, failure/CLI/help behavior, literal reuse, implicit pools,
  address literals and PRINT suppression. Listing collection leaves modules
  unchanged. The full related-package regression and workspace Clippy passed
  after these output additions; subsequent changes need a new checkpoint.

Additional floating/machine/options checkpoint (still in progress):

- BFP LDEB/LXDB/LXEB and MXDB/MXDBR now cover memory lengthening and exact
  long-to-extended multiplication, with pair diagnostics and preserved special
  values. Tests exercise a product containing a bit at 2^-104 and conversion
  payloads which require precision beyond host double/extended arithmetic.
- Imported RXE/RXF/RSL-b covers supported BFP/HFP memory arithmetic,
  lengthening, fused/unnormalized operations, BFP/DFP classification, DFP shifts
  and packed/zoned memory conversion with their distinct field layouts.
- Imported RSY/SIY covers save/restore, shifts/rotates, byte immediates,
  masked/conditional memory and atomic operations using signed 20-bit offsets.
  Thirteen machine tests execute independently constructed byte sequences.
- ACONTROL AFPR/NOAFPR is applied per expanded statement, with nested stack
  restoration across COPY/macro expansion. FPR restrictions do not restrict
  GPR or mask operands. Seventeen operand-position cases verify the distinction.
  Defaults match the public Language Reference's AFPR-enabled initial state.
- These are incremental P3 changes. LE entry/exit/XPLINK/condition integration,
  remaining object/binder/relocation/decoder coverage, IBM ADATA and remaining
  option compatibility are still required by the original acceptance scope.


ADATA user-information checkpoint (still in progress):

- ADATA source syntax validates four optional signed decimal fullwords, the
  quoted string/escape syntax, sequence labels, representability and the
  255-encoded-byte limit. COPY/macro expansion is reflected in listing JSON.
- `UserData::record(CCSID)` encodes an individual X'0070' record with the
  architecture-level-3, edition-1 common header and documented binary fields.
  It does not produce the complete assembly ADATA stream; identification,
  compilation-unit framing and the other original P3 ADATA record types remain
  pending. No Zeus instruction-template bytes are presented as IBM bytes.
- Five tests cover source validation with listing disabled/enabled, macro/COPY
  provenance, independent binary field reads, signed limits, fixed-column
  continuations, EBCDIC DBCS input and encoded lengths/unmappable characters.
- Definitions follow the normative record/syntax tables in IBM's
  [HLASM V1R6 Programmer's Guide](https://publibfp.boulder.ibm.com/epubs/pdf/asmp1022.pdf)
  (common header and User-supplied Information X'0070') and
  [ADATA instruction](https://www.ibm.com/docs/SSENW6_1.6.0/asma400/adata.html).
  No IBM macro or implementation source is used.
- The preceding full output/float/AFPR checkpoint passed 630 related-package
  tests, zero failures and one ignored corpus test; workspace Clippy, all
  command builds and 60 standalone help invocations across 14 commands passed.
  Subsequent ADATA changes require another full checkpoint. P3 is incomplete.


GOFF element-length checkpoint (still in progress):

- R-length element/label relocations accept fetch/store, add/subtract,
  compressed fields and ordinary widths 1..8. Deferred LEN values are used
  after definition resolution, and translator/load origins are not added to
  lengths. Label length is the architectural zero. Class/PART lengths remain
  explicit unsupported cases, along with other pending relocation families.
- Store semantics reset both prior expressions and mode-sensitive attributes.
  Nineteen GOFF tests include length arithmetic, compression, rebasing,
  serialization, deferred lengths and invalid referent/mode combinations.
- The preceding ADATA checkpoint passed 635 related-package tests, zero
  failures, one ignored corpus test, workspace Clippy, all command builds and
  60 standalone help invocations. These new length changes require the next
  broad checkpoint. The original P3 scope remains incomplete.


RI immediate interoperability checkpoint (still in progress):

- LHI/MHI/CHI are available in source and architectural imported text. Signed
  halfword operands preserve GPR high halves; load/multiply preserve CC,
  comparison sets signed ordering, and multiply does not signal fixed overflow.
- Source halfword immediate expressions require absolute signed 16-bit values;
  this also fixes previously accepted relocatable AHI/LGHI/AGHI/CGHI/MGHI
  operands. Same-section absolute differences remain valid.
- Sixteen machine integration tests now include EX register overlays, high
  halves, multiplication wrapping under an enabled overflow mask, condition
  codes, and signed/relocatable boundaries across all eight RI forms.
- The preceding GOFF length checkpoint passed 638 related-package tests, zero
  failures, one ignored corpus test, workspace Clippy, all command builds and
  60 standalone help invocations. The new immediate changes need the next
  broad checkpoint. No original P3 row is complete yet.


RIL interoperability checkpoint (still in progress):

- Architectural AGFI/CGFI, BRCTH and EXRL dispatch to the existing shared
  integer/relative execution paths. An independent byte fixture combines signed
  fullword immediates, a high-half counter loop with preserved low half, and
  PC-relative EX with a temporary MVC length overlay.
- Source LGFI/AGFI/CGFI operands require absolute signed fullwords, rejecting
  silently truncated section references while accepting absolute differences.
- The preceding RI checkpoint passed 641 related-package tests, zero failures,
  one ignored corpus test, workspace Clippy, all command builds and 60 standalone
  help invocations. Eighteen machine tests now cover the additional RIL behavior
  and fullword source limits; this increment requires the next broad checkpoint.
- Original P3 rows remain in progress. Remaining work includes LE entry/exit,
  XPLINK and conditions; specialized floating/exception diagnostics; full
  object/binder/relocation and machine coverage; complete ADATA and remaining
  assembler option profiles. Host-native HLASM generation remains excluded.


Latest verification checkpoint (P3 remains in progress):

- After RIL and signed-immediate validation, the related-package suite passed
  643 tests, zero failures, one ignored corpus test (68 test suites).
- Workspace all-target Clippy with warnings denied, all workspace command
  builds, formatting checks and 60 standalone help invocations across 14
  commands passed. This is not a full-workspace test-success claim.
- PROCESS requirements identified at that checkpoint (implemented below):
  primary-input initial records (except ICTL), at most ten process statements,
  no physical continuation, and scanning ends at unquoted whitespace. SYSPARM,
  ADATA, LIST, GOFF and the other prohibited invocation-only options must not
  be implemented as process overrides. PCONTROL locks applicable PRINT options;
  it is more than an initial PRINT state. The normative option tables were
  consulted; these behaviors are not implemented yet.


Invocation PCONTROL checkpoint (still in progress):

- `AssemblyOptions.pcontrol` and `zas --pcontrol OPTIONS` lock named PRINT
  options throughout expansion. PUSH/POP PRINT and macro/COPY PRINT cannot
  replace those options. Unspecified options continue to follow source state.
- PCONTROL(OFF) suppresses the entire expanded-text section, including control
  lines, while structured rows/calls, xref and original-source archives remain.
  GEN/NOGEN, DATA/NODATA, MCALL, MSOURCE and UHEAD retain their independent
  interactions. Initial UHEAD now matches the documented standard PRINT state.
- Four tests cover all six locked settings, nested calls, stacks, complete
  suppression, MSOURCE vs object-data display, JSON round trips, unchanged
  executable modules, invalid profiles/API keys and CLI help-looking values.
- The new CLI accepts canonical named options and rejects contradictory pairs;
  At that checkpoint, *PROCESS scanning, source/invocation precedence and
  duplicate-option warnings remained pending; the later PROCESS checkpoint below
  implements those paths.
- The preceding RIL checkpoint passed 643 related-package tests, zero failures,
  one ignored corpus test, workspace Clippy, all command builds and standalone
  help checks. This PCONTROL increment requires the next broad checkpoint.


NOGEN first-object checkpoint (still in progress):

- The first generated object statement is indexed independently of control,
  zero-length and non-object directive prefixes. Macro NOGEN data display uses
  the first DC's eight bytes, and stops at a first executable instruction even
  though Zeus source templates have no architectural bytes to show. It never
  borrows the bytes of a later unrelated DC or an empty macro's following row.
- Selection is linear in expanded rows plus calls. Scope bounds preserve nested
  and empty call behavior, and old listing JSON keeps its previous fallback.
- Five PCONTROL and seventeen listing tests pass after this fix. The preceding
  broad PCONTROL checkpoint passed 647 tests, zero failures, one ignored corpus
  test; workspace Clippy, command builds and standalone help passed. The next
  broad check will include the additional first-object test. P3 is not complete.


Latest completed check (original P3 work remains pending): 648 related-package
tests passed, zero failed, one ignored corpus test, across 69 suites. Workspace
all-target Clippy with warnings denied, all command builds, formatting and 60
standalone help invocations across 14 commands passed after the NOGEN fix.


Initial PROCESS and instruction-table selection:

- Primary input recognizes up to ten initial `*PROCESS` records after optional
  ICTL, without expanding them into macro statement numbers. Library records
  remain comments. Continuation and forbidden options receive diagnostics.
- PCONTROL/NOPCONTROL and OPTABLE/OP support independent precedence:
  OVERRIDE, invocation, ordinary PROCESS, defaults. Conflicting settings at one
  level use the last value with severity-4 diagnostics; ignored invocation/source
  conflicts are reported. Invalid invocation settings cannot be hidden by source.
- `--optable UNI|ZEUS[,NOLIST]` and ACONTROL select the implemented union;
  initial/current option macro symbols and ACONTROL stack restoration reflect
  the selection. Older architecture-specific availability and table LIST output
  still require implementation. This is not a claim of complete ISA coverage.
- Listing JSON retains PROCESS locations/options and effective initial options.
  These additions leave the original four P3 acceptance rows in progress.


Additional P3 execution and output work:

- BFP DIEBR/DIDBR use integer-ratio quotient/remainder computation, independent
  final-quotient/current-FPC rounding, partial results, scaled quotient overflow,
  NaN precedence and signed zero. Invalid traps suppress both destinations;
  underflow/inexact traps complete them. Final tiny remainders and iterative
  partial remainders have distinct underflow behavior. RRF-b architectural fetch
  and EX opcode overlays preserve register and modifier fields. Implemented from
  Principles of Operation, sections 19-28 through 19-31.
- Open-code model statements retain original variable-bearing text separately
  from every expansion, including repeated conditional-assembly loops. NOGEN
  prints the model and the first eight generated DC bytes; ordinary escaped
  ampersands do not mark a statement as a model. A literal pool can also be the
  first object generated by a macro, without claiming a later DC's bytes.
- Caller return mode and parameter-list mode are validated before acquiring
  invocation storage. Repeated failures leave memory and allocation counts intact.
- GOFF reference type 6 now supports one additive external label per signed
  halfword/fullword field. The normalized expression retains byte displacement;
  load initialization checks even alignment and signed range before encoding
  halfword units. Fetch addends are signed; store clears earlier value and mode
  attributes. Module format 12 prevents older readers from treating this as an
  address constant. Forward/backward targets, serialization, repeated linking,
  load origins and library discovery have synthetic execution tests.
  Internal/multiple/subtractive referents remain diagnosed. This initial RI-con
  implementation is based on the published
  [RLD field definitions](https://www.ibm.com/docs/en/zos/2.5.0?topic=record-relocation-directory-data-item)
  and [RI operand rules](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=instructions-ri-format),
  without an IBM-produced object oracle. Broader binder/ABI interoperability
  remains an original P3 acceptance requirement.
- No original P3 row is complete yet: LE entry/exit/XPLINK/condition integration,
  remaining floating-point semantics and families, object output and additional
  binder relocations/attributes, full ADATA production and remaining assembler
  options/diagnostic compatibility still require work.


ACONTROL macro-operand compatibility:

- COMPAT(MACROCASE/NOMACROCASE), with CPAT(MC/NOMC) abbreviations,
  controls conversion of unquoted macro operand values before expansion.
  Fully quoted operands retain case; unquoted character terms are converted
  in full. Positional/keyword parameter values and SYSLIST agree. Defaults in
  macro prototypes and original invocation text remain unchanged.
- The initial behavior is NOMACROCASE. Independent PUSH/POP ACONTROL restores
  the mode together with AFPR and OPTABLE, without changing PRINT state.
  Other COMPAT suboptions are still explicitly diagnosed.
- Behavior follows the public [COMPAT option description](https://www.ibm.com/docs/en/hla-and-tf/1.6.0?topic=options-compat)
  and Programmer's Guide section COMPAT. Focused operations/macro tests pass.


BFP modifier validation:

- Executed RRF-e BFP rounding/conversion forms now reject reserved M3/M4
  values before changing GPRs, FPRs, FPC or CC. Source parsing already rejected
  these values; imported machine text and decoded module instructions now use
  the same restriction. M4=4 remains valid inexact suppression. Synthetic raw
  instruction bytes and direct state-preservation tests cover invalid modifiers.


COMPAT invocation and PROCESS integration:

- `--compat MACROCASE|NOMACROCASE` (MC/NOMC) initializes macro operand
  compatibility. `*PROCESS COMPAT(...)` and CPAT(...) support the same modes,
  with independent invocation/source/OVERRIDE precedence and conflict warnings.
  ACONTROL remains able to change this initial option after assembly starts.
- Listing JSON records the effective initial `macrocase` option. CLI argument
  validation treats help-looking values as values; other commands reject this
  compile/check-only option. Unsupported compatibility suboptions remain errors.


Latest verification checkpoint (2026-10-15; original P3 rows still in progress):

- Related packages zeus-cobol, zeus-pli, zeus-jcl, zeus-deps and zeus-native:
  681 tests passed, zero failed, one corpus test ignored, across 70 suites.
- Workspace all-target Clippy with warnings denied, workspace binary build,
  formatting check and standalone embedded-help checks all pass. Standalone
  help exercised 60 invocations across 14 commands.
- A workspace all-target test run before the last COMPAT/BFP increments reached
  816 passing tests, six ignored and five failing IMS TM tests. The failures
  reproduce the existing Windows access-denied error while persisting the TM
  queue; no IMS source was changed. This is not a claim that all workspace
  tests or the original four P3 acceptance rows pass.


LE condition-token construction:

- CEENCOD is available from COBOL CALL and HLASM LINK/LOAD dispatch, using
  shared argument addresses and the existing checked four-byte LE ABI.
  Conditions preserve the two condition-ID halfwords and instance-information
  fullword, with case/severity/control and EBCDIC facility fields encoded in
  the 12-byte token. Case 1 and case 2 are supported.
- Published feedback conditions 401/402/403/404/452 distinguish invalid fields
  and the non-alphanumeric-facility warning. Severe errors preserve the token;
  warning 404 constructs it. Feedback severity is represented correctly rather
  than forcing every result to severity 3. Without feedback, an unhandled
  severe constructor error follows the existing U4038 default action.
- Common LE preflight validates all input/output ranges and output protection
  before changes. Inputs are read before aliased output is written. Input
  fields may reside in RSECT. Missing or invalid addresses do not partially
  change outputs. Native and interpreted COBOL execute the same service.
- The implementation follows the public
  [CEENCOD interface and condition table](https://www.ibm.com/docs/en/zos/2.5.0?topic=services-ceencodconstruct-condition-token),
  using original fixtures rather than IBM macros/copybooks/samples. Condition
  handler registration, signaling/resumption, LE prolog/epilog and XPLINK
  remain pending; constructing a token does not complete condition management.


Latest CEENCOD verification checkpoint (2026-10-15):

- Related five-package regression: 690 tests passed, zero failed and one
  corpus test ignored, across 70 suites. Native and interpreted executable
  coverage exercises the same constructed token bytes.
- Workspace all-target Clippy with warnings denied, all workspace binaries,
  formatting and 60 standalone help invocations across 14 commands pass.
- Full workspace all-target tests: 830 passed, 5 failed and 6 ignored.
  The only failing target remains zeus-ims/tests/tm.rs, reproducing the five
  existing Windows access-denied failures during queue persistence. No IMS
  implementation was changed. Original P3 acceptance remains incomplete.


Conventional OBJ output (initial supported subset):

- The public `object_bytes(Module)` API and `zas compile/link --output-format
  obj` emit actual ESD/TXT/RLD/END card images. The default remains Zeus JSON;
  compile defaults to `.obj` only when the output format is explicitly selected.
  Complete encoding precedes file writes, preserving an existing file when
  instruction/attribute validation fails. Embedded help documents this choice.
- The writer uses one doubleword-aligned control section, LD exports, ER/WX
  externals (including unused declarations), 56-byte text chunks and grouped
  full RLD items. Signed local/external terms and 1..8-byte address fields
  retain their behavior at different load origins and after external binding.
  EBCDIC-blank ESD header ambiguity is avoided even in large dictionaries.
- Source RR/RX integer and HFP, RS shifts/multiple-register transfers, SI byte
  immediates, SS character/packed decimal, RI/RIL immediates/local relative
  branches, common RRE 64-bit registers and RXY memory forms have independent
  IBM architectural encodings. Zeus template bytes are never substituted for
  missing encodings. Imported architectural text is retained, including bytes
  that are data rather than executed instructions.
- Uniform AMODE 24/31/64, RMODE 24/31 and fully read-only images are retained.
  Unsupported instructions include a source location in the error. Long names,
  mixed modes, partial RSECT protection, alignment above eight, extended
  binding/DCB metadata and mode-sensitive/relative-immediate object fixups
  require further output work and are rejected. The source runtime's extra
  writable scratch tail is retained, so ordinary source RSECT modules are
  partially protected and currently rejected by this single-section writer.
- Tests cover exact independent record/opcode bytes, backward branches,
  integer/string/packed/HFP/64-bit execution, negative RXY displacements,
  internal and signed/weak external fixups, all eight field widths, nonzero
  entry points, multiple text/RLD records, determinism, a 16,500-name external
  dictionary, imported-text re-emission and failure preservation. Generated
  objects are also tested through a ten-step JCL job with repeated direct,
  COBOL and PL/I calls in the original shared storage.
- The record layout follows Appendix A, figures 6 through 10 of the public
  [Program Management Advanced Facilities reference](https://publib.boulder.ibm.com/epubs/pdf/iea2b220.pdf).
  Fixtures and the implementation are original, without IBM macros or sample
  listings. IBM-produced binder round trips have not been validated.
- Full scalar encoding, multiple-control-section/GOFF output, remaining
  binder/record attributes and the other original P3 rows remain unfinished.
  This emits IBM architectural object bytes, not host-CPU native code.


OBJ output verification checkpoint (2026-10-15):

- Related five-package regression: 702 passed, zero failed, one corpus test
  ignored across 71 suites. The eleven output tests cover the additional ESD
  boundary and default-entry rejection cases; generated-object JCL integration
  runs alongside the independent raw OBJ and GOFF fixtures.
- Workspace all-target tests: 842 passed, 5 failed, 6 ignored across 101 suites.
  The only failing target remains `zeus-ims --test tm`, with the same five
  Windows queue-persistence access-denied failures. The final ESD boundary and
  default-entry checks were additionally verified in the focused output suite.
- Workspace all-target Clippy with warnings denied, all workspace binaries,
  formatting and 60 standalone help invocations across 14 commands pass.
  No IMS implementation or host-native HLASM generator was added or modified.
  The original four P3 rows remain incomplete.


GOFF output and case-sensitive class identity (initial supported subset):

- `goff_bytes(Module)` and `zas compile/link --output-format goff` emit GOFF
  architecture-level-1 HDR/ESD/TXT/RLD/END records in fixed 80-byte physical
  records. The default output remains Zeus JSON. Explicit GOFF compile output
  defaults to `.goff`; complete encoding precedes writes to preserve existing
  files on encoding failure. Embedded help describes invocation and limits.
- The encoder shares the independent architectural instruction encoder with
  conventional OBJ. Long representable CCSID-37 names (up to 32767 bytes),
  sequential ESDIDs, SD/ED/LD/ER relationships, continuation flags, reserved
  zeros and logical END record counts are emitted. TXT and RLD data span
  logical records and physical continuations without interpreting padding.
  SD identifies the section and the corresponding LD carries its callable
  addressing mode, following the published OBJ-to-GOFF mapping.
- The current writer uses one complete loadable concatenating byte-stream
  element, normally B_TEXT. A complete imported class retains its name;
  contiguous contributions to the same class may be combined. Uniform
  AMODE 24/31/64, additional exported label modes, RMODE 24/31, alignment
  8/16/4096, fully read-only images and non-executable data are retained.
  Source scratch storage is retained and may make source RSECT partial.
- Ordinary signed local/external expressions retain 1..8-byte address fields
  and weak-reference semantics. Mode-sensitive 4/8-byte pointers use LD/ER
  referents and preserve their tags after linking and load-origin changes.
  Known modes need an exported label of that mode. Tagged initializers that
  wrap a field shorter than eight bytes are rejected rather than emitting a
  value the current normalized loader would misinterpret.
- Supported source relative instructions may refer to an external symbol
  before linking. Single-additive-external RI fixups preserve signed 16/32-bit
  halfword fields, positive/negative addends, forward/backward binding and
  unresolved weak references. Already resolved PC-relative displacements are
  materialized as invariant architectural bytes. Unrepresentable expressions,
  odd addends and out-of-range initializer fields produce errors.
- The GOFF reader now preserves case-sensitive class names, as required by
  the public format. Differently cased names remain separate classes; this
  corrects the earlier reader's unconditional uppercasing. Mixed-case class
  identity also survives output/reloading.
- Original fixtures verify independent record/opcode bytes; 170/240/32767-byte
  names; large TXT and signed RLD dictionaries; all adcon widths; known and
  external AMODE tags; weak/negative RI values; re-emission; non-executable
  data and class case. A generated GOFF ten-step JCL test runs repeated direct,
  COBOL and PL/I calls in shared run-unit storage alongside raw input fixtures.
- Normative field and mapping references: Appendix C, tables 48-55, 63-67,
  71 and 73 of the public
  [Program Management Advanced Facilities reference](https://publib.boulder.ibm.com/epubs/pdf/iea2b2b1.pdf).
  No IBM macros, source, copybooks or sample listings were used.
- Partial protection/executability, separate classes or image gaps needing
  multiple elements, unresolved section quantities/PSECTs/DCBs, XPLINK,
  complete scalar encodings, structured/NOLOAD and deferred-length output,
  remaining binder attributes and IBM-produced binder round trips remain
  pending. Main/section entries currently require the default AMODE. The
  original four P3 rows remain incomplete. Host-CPU native HLASM generation
  remains out of scope.


GOFF output verification checkpoint (2026-10-15):

- Related five-package regression: 716 passed, zero failed, one corpus test
  ignored across 72 suites. This includes the generated GOFF ten-step job
  with direct, COBOL and PL/I calls and the independent raw input jobs.
- Full workspace all-target tests: 856 passed, 5 failed, 6 ignored across
  102 suites. Only `zeus-ims --test tm` failed, reproducing the same five
  Windows queue-persistence access-denied errors. No IMS source was changed.
- Workspace all-target Clippy with warnings denied, all workspace binaries,
  formatting and 60 standalone help invocations across 14 commands pass.
  The final class-metadata rejection checks were also verified in the
  focused GOFF/OBJ input/output suites after the broader regression.
- The original four P3 rows remain in progress; this checkpoint does not
  claim complete GOFF/binder interoperability or complete LE/FP/ADATA support.


GOFF multi-element output (in progress):

- GOFF output now plans elements before assigning ESDIDs and writes each LD,
  TXT, RLD position pointer and END relative to its owning element. Normalized
  image-relative expressions retain their original meaning at different load
  origins. Later-element RI fixups account for both the instruction position
  and the element's position; mode-sensitive pointers use the same mapping.
- Source/OBJ images without named element metadata split at read-only and
  executable boundaries. This allows source RSECT code/protected constants,
  separate writable CSECT data and the writable scratch tail to be emitted.
  Non-executable bytes retain their contents and offsets. Adjacent fragments
  with the same protection/executability are combined; byte-aligned later
  elements avoid introducing padding that would change existing addresses.
- Generated 16-character Z_ translator class names include a stable module-name
  fingerprint and fragment ordinal, avoiding identical fragment class names
  for differently named sections in the covered fixtures. Exported symbol
  names remain unchanged; IBM binder validation remains pending.
- Existing named concatenating classes and individual element boundaries may
  be re-emitted with their names, protection, executability and alignment.
  Case remains significant. Implicit alignment gaps are retained only when
  their exact position follows from the next element's alignment and their
  bytes are zero, writable and non-executable. Class order that would move
  image bytes is rejected. Synthetic multi-element output can itself be
  reloaded and re-emitted without losing its layout or protection.
- Tests exercise writable data reached through a protected address constant,
  every protected byte, runtime rejection of stores into RSECT data, later
  element END and external RI fixups, AMODE 24/31/64 pointer tags, nonzero
  non-executable holes, mixed-case classes with implicit padding, re-emission,
  generated class-name separation, matching ASCII/EBCDIC ordinal collation
  and boundary-crossing error paths. The
  generated GOFF ten-step JCL job now uses real source RSECT modules for
  repeated direct, COBOL and PL/I calls.
- Instructions and fixup fields that cross elements, mixed attributes within
  a pre-existing named element needing a further split, zero-length elements,
  unowned interior/nonzero/protected alignment gaps and unmapped source/class
  spans requiring reordering remain diagnosed. PSECT/quantity/DCB, structured/NOLOAD/deferred-LEN output,
  complete binder/record attributes and IBM-produced binder round trips remain
  pending, along with the other original P3 rows. No host-native HLASM
  generation is introduced.


- Mixed GOFF/Zeus-source links can now emit trailing source/data contributions
  as new elements after existing named elements, when class collation preserves
  their physical order. This includes trailing RSECT code, protected address
  constants, writable data and alignment bytes. Re-emission preserves the same
  ranges. CLI coverage links `caller.goff` with `helper.zo` into runnable GOFF;
  a separate fixture follows a protected pointer to later writable data at
  multiple load origins. Interior unowned spans and class reorderings remain
  unsupported. Generated fragment ordinals use letters so both ASCII and
  EBCDIC comparisons preserve element order across the 10th/26th fragments.


GOFF multi-element verification checkpoint:

- Related five-package regression: 725 passed, zero failed, one corpus test
  ignored across 72 suites. GOFF output has 21 tests, including the mixed
  source/RSECT link; the generated GOFF JCL job uses protected RSECT code
  and writable scratch areas through repeated direct, COBOL and PL/I calls.
- Full workspace all-target tests: 865 passed, 5 failed, 6 ignored across
  102 suites. The only failing target remains `zeus-ims --test tm`, with the
  same five Windows queue-persistence access-denied failures. No IMS source
  was changed. This is not a claim that the full workspace passes.
- Workspace all-target Clippy with warnings denied, all workspace binaries,
  formatting and 60 standalone help invocations across 14 commands pass.
- The original four P3 rows remain incomplete: remaining LE/condition/XPLINK,
  floating-point conformance edges, binder/record metadata and complete ADATA
  and option compatibility remain part of the acceptance scope.


P3 combined implementation checkpoint (original acceptance scope retained):

- LE: CEEHDLR/CEEHDLU own registrations in dynamic language frames. Newest
  handlers run first, token values are copied, duplicate registrations refresh
  their LIFO position with warning 256, missing removals return warning 252 and
  invalid entries return condition 257. COBOL CALL/RETURN, HLASM LINK/loaded
  entries, external runtime lends, host callbacks and PL/I activations track
  lifetimes; normal return, error unwind and STOP RUN discard registrations.
- CEESGL traverses live handlers and avoids reinvoking an active registration
  for nested signals. OS 24/31 handlers receive four shared-storage arguments.
  Actions 10,20,21,30,31,60 are implemented for explicit signalling with the
  documented restrictions in `zas --help`. CEEGQDT retrieves per-condition
  qualifying data; instance state is released at the outermost language return.
  CEEDCOD decomposes both token cases and zero success tokens after complete
  parameter/protection preflight. Invalid flags report 102. Inputs are copied
  before outputs so aliasing does not corrupt unread fields.
- Independent original tests cover HLASM LIFO/explicit removal, saved token
  values, nested signals and q_data, local duplicate/removal diagnostics,
  malformed arguments, HLASM child-frame cleanup, direct COBOL handlers via
  LOAD dispatch and COBOL subprogram cleanup, and direct PL/I LE calls with
  procedure-return cleanup. These are synchronous service subsets, not complete
  language exception integration.
- OBJ/GOFF output now independently encodes implemented floating RR/RRE/RRF/
  RRD/RXE/RXF/RSL-b families: HFP/BFP/DFP arithmetic, fused/unnormalized forms,
  conversions/rounding, classification, packed/zoned and digit operations,
  DIEBR/DIDBR and FPC controls. RSY/SIY output covers long-displacement
  multiple-register, shifts, masked/conditional/atomic and byte-immediate forms.
  Successful scalar arithmetic fixtures now also traverse OBJ/GOFF output,
  loader and architectural decoding and compare complete output data against
  source execution. Independent opcode/layout expectations cover 26 forms.
- ADATA physical-record transport preserves every record type (including future
  opaque types), editions, continuations and common-header length byte order.
  Optional RDWs and lengths/reserved flags/truncation/continuation chains are
  checked. Output is bounded to 64 MiB. `zas compile/check --adata FILE
  --adata-ut UINT64 [--adata-ccsid UINT16] [--adata-rdw]` produces one framed
  analysis unit. UT is an explicit raw caller value, avoiding an unverified
  conversion of its epoch. Character fields are encoded in the declared CCSID;
  unrepresentable text fails before the destination is touched.
- The generated subset includes 0002/0001/0000/000A/0020/0030/0032/0036/0042/
  0044/0060/0070. End counts include both unit delimiters; source token spans
  use encoded byte offsets; instructions use the independent object encoder;
  user information retains source order; long xref sets use physical
  continuation records. The producer identifies itself as ZEUS. ESD metadata
  describes the normalized Zeus image. Library IDs for COPY/macro provenance
  are separate from primary-input IDs. Complete encoding precedes file writes.
- ADATA tests use independent readers of headers, offsets, counts, machine
  bytes, signed user fields and 9001-reference continuations, plus all record
  families in raw transport, both length byte orders, RDWs, malformed/truncated
  input, CLI invocation restrictions and preservation on encoding failure.

Remaining original P3 acceptance work is still substantive:

| Area | Remaining acceptance work |
| --- | --- |
| LE/linkage | LE prolog/epilog/entry variants and control-block layouts; XPLINK 32/64 and environment descriptors; resume-cursor movement beyond the original signal frame; T_I_U, runtime options, complete HLL/hardware exception translation and remaining condition services. Explicit CEESGL without feedback cannot yet request nonlocal resumption. |
| Floating point | Inventory every specialized scalar form against the applicable architecture, add missing semantics, validate detailed exceptions/diagnostics independently. The new output coverage is not an architectural completeness proof. |
| Objects/binder | Complete scalar decoding/encoding; XSD/common/PR/PART/PSECT/quantity/DCB binding and output; remaining class/offset/long-displacement/bitfield/conditional RLD; binder attributes/controls; IBM-produced cross-language ABI and external binder validation. |
| Outputs/options | Full semantic completeness of generated ADATA records (including option fields, COPY/member-reference roles, source sections, specialized RLD and statistics); complete symbol/source/ESD attributes, conditional/macro-definition record fidelity, automatic UT conversion and full option/ACONTROL/COMPAT/architecture-profile diagnostics. Raw transport support for a record is distinct from generation of its semantic contents. |

No original P3 row is marked complete. No host-CPU native HLASM generator was
added; the architectural object bytes execute through the shared interpreter.


Combined checkpoint verification:

- Full workspace/all-target regression: 877 passed, 5 failed, 6 ignored across
  103 suites. The only failing target is still `zeus-ims --test tm`; all five
  failures reproduce Windows access denied (OS error 5) in queue persistence.
  IMS source has not been modified. This does not claim a clean workspace run.
- Workspace/all-target Clippy with warnings denied, all workspace binaries,
  formatting and `git diff --check` pass. All 60 standalone help invocations
  across 14 commands pass, including rebuilt embedded `zas` documentation.
- Focused LE tests: 14 integration cases plus 8 LE unit tests. The new PL/I
  lifetime integration test passes. All 82 floating-point tests pass with the
  additional architectural object paths for successful arithmetic fixtures;
  12 conventional OBJ output and 21 GOFF output cases pass. ADATA has 8 cases.
- Original P3 rows remain incomplete as enumerated above; transport support,
  recognized names and internal round trips are not external conformance proof.


P3 remaining-work implementation checkpoint (2026-10-16):

- LE handler promotion actions 32/33 restart from the first handler of the
  handle-cursor/original resume-cursor frame respectively. Returning an equivalent
  condition percolates; promotion cycles have a bounded diagnostic. Original
  fixtures distinguish the two actions across separate live language frames.
  This does not implement nonlocal resume-cursor movement or XPLINK descriptors.
- IBM architectural scalar output/input now additionally covers 30 RI/RIL partial
  immediate operations, seven RIE-f rotate/select operations, three RIE-g
  conditional immediate loads, RRF logical/select/conditional register forms,
  POPCNT controls, carry/borrow/reverse/byte register operations, and additional
  RXY arithmetic/logical/load/store/reverse/CVBY/CVDY forms. POPCNT's mask is
  preserved by binary decoding. LLC/LLH now execute on source and binary paths,
  preserving high register bits and condition code.
- Listing data retains every DC/DS operand's origin, bit position, duplication,
  nominal lengths/bytes, program/assembler type and numeric attributes. Implicit
  character/hex/packed/zoned lengths and ordinary dummy-section constants are
  handled; repeated location-dependent constants use the final assembled image.
- ADATA generation now includes records 000B, 0010, 0034, 0035, 0040, 0045,
  0062, 0080 and 0090. Signed ordinary relocation widths 1..8 and A/V type,
  general-register references (including USING/DROP and implicit pairs), macro
  and COPY references (with nested caller identities and assembly-wide member
  totals), USING maps and retained statistics are reported. Continuations
  clear previously emitted fixed fields and retain per-record counts/offsets.
- These are semantic subsets, not full IBM ADATA compatibility. Original section
  ESD identities, all option fields/metrics, every register/member role, COPY and
  unexpanded macro/conditional fidelity, dummy-section address relocation layout,
  all binder attributes/RLD families and automatic UT conversion remain open.
  The original four P3 acceptance rows therefore remain incomplete. HLASM
  host-CPU native code generation remains excluded.


Verification of the 2026-10-16 remaining-work checkpoint:

- Full workspace/all-target regression: 890 passed, 5 failed, 6 ignored across
  103 suites. Only `zeus-ims --test tm` failed; the same five queue-persistence
  tests report Windows access denied (OS error 5). IMS code was not changed.
- Workspace/all-target Clippy with warnings denied, all workspace binary builds,
  formatting and diff whitespace checks pass. 98 standalone --help/-h invocations
  across 14 commands pass, including documented subcommands and the rebuilt
  ADATA, condition-handler and scalar-object explanations in zas.
- Focused coverage now includes 17 conventional OBJ output cases, 21 GOFF
  output cases, 13 ADATA cases, 18 listing cases and 15 LE integration cases.
  Separate LE unit fixtures verify frame 32/33 distinction, equivalent-condition
  percolation, bounded cyclic promotion and dispatch-storage cleanup. All 82
  floating-point integration cases pass in the full regression.
- No external IBM binder/XPLINK interoperability or architectural completeness
  is inferred from these checks; the remaining acceptance table is authoritative.

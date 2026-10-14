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
relocatable and layout-dependent values are diagnosed. Other constant types'
expression-valued lengths, bit lengths, expression duplication factors and
floating constants remain unsupported.
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
a zero location. Extended DC/DS program annotations remain unsupported.
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
existing zero-target convention. DSECT terms, relocatable multiplication/
division and explicit A lengths/AD/Y forms remain unsupported.
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

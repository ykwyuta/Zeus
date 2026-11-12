#!/usr/bin/env python3
"""Build the coverage inventory of the validation suites (docs/inventory/*-elements.md).

The element lists follow the chapter structure of the IBM manuals:
Enterprise COBOL for z/OS Language Reference, Enterprise PL/I for z/OS Language
Reference, HLASM Language Reference and z/Architecture Principles of Operation.
For each element the script lists the suite programs whose source uses it.
"Used" is a mechanical finding: whether the use reaches boundaries and internal
representation is a reviewed judgment recorded in the `reviewed` column of the
generated table (kept from the previous file).

usage: tools/suite-inventory.py [--ccvs85 path/to/newcob.val]
The CCVS85 source is not bundled (it is fetched by URL and hash); when given,
the COBOL table also counts the CCVS85 programs that use each element.
"""
import argparse
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# --------------------------------------------------------------------------- COBOL
COBOL = {
    'Statements': [
        'ACCEPT', 'ADD', 'ALLOCATE', 'ALTER', 'CALL', 'CANCEL', 'CLOSE', 'COMPUTE', 'CONTINUE',
        'DELETE', 'DISPLAY', 'DIVIDE', 'ENTRY', 'EVALUATE', 'EXIT PROGRAM', 'EXIT PERFORM',
        'EXIT PARAGRAPH', 'EXIT SECTION', 'FREE', 'GOBACK', 'GO TO', 'GO TO DEPENDING', 'IF',
        'INITIALIZE', 'INSPECT TALLYING', 'INSPECT REPLACING', 'INSPECT CONVERTING', 'INVOKE',
        'JSON GENERATE', 'JSON PARSE', 'MERGE', 'MOVE', 'MOVE CORRESPONDING', 'MULTIPLY', 'OPEN',
        'PERFORM', 'PERFORM TIMES', 'PERFORM UNTIL', 'PERFORM VARYING', 'PERFORM AFTER',
        'WITH TEST AFTER', 'READ', 'READ NEXT', 'READ PREVIOUS', 'RELEASE', 'RETURN', 'REWRITE',
        'SEARCH', 'SEARCH ALL', 'SET TO TRUE', 'SET TO FALSE', 'SET UP BY', 'SET DOWN BY',
        'SET ADDRESS OF', 'SORT', 'START', 'STOP RUN', 'STRING', 'SUBTRACT', 'UNSTRING', 'USE',
        'WRITE', 'XML GENERATE', 'XML PARSE'],
    'Phrases': [
        'ON SIZE ERROR', 'NOT ON SIZE ERROR', 'ROUNDED', 'ROUNDED MODE', 'ON OVERFLOW',
        'ON EXCEPTION', 'NOT ON EXCEPTION', 'INVALID KEY', 'NOT INVALID KEY', 'AT END',
        'NOT AT END', 'AT END-OF-PAGE', 'ADVANCING', 'BY REFERENCE', 'BY CONTENT', 'BY VALUE',
        'RETURNING', 'OMITTED', 'CORRESPONDING', 'GIVING', 'REMAINDER', 'DELIMITED BY',
        'WITH POINTER', 'TALLYING IN', 'COUNT IN', 'ALSO', 'THRU', 'WHEN OTHER',
        'NEXT SENTENCE', 'WITH NO ADVANCING', 'UPON'],
    'Data description': [
        'BLANK WHEN ZERO', 'EXTERNAL', 'GLOBAL', 'GROUP-USAGE', 'JUSTIFIED', 'OCCURS',
        'OCCURS DEPENDING', 'ASCENDING KEY', 'DESCENDING KEY', 'INDEXED BY', 'REDEFINES',
        'RENAMES', 'SIGN LEADING', 'SIGN TRAILING', 'SEPARATE CHARACTER', 'SYNCHRONIZED',
        'VALUE', 'VALUE THRU', 'WHEN SET TO FALSE', 'VOLATILE', 'DYNAMIC LENGTH',
        'LOCAL-STORAGE', 'LINKAGE SECTION', 'FILLER', 'LEVEL 66', 'LEVEL 77', 'LEVEL 88'],
    'USAGE': [
        'DISPLAY', 'DISPLAY-1', 'NATIONAL', 'UTF-8', 'BINARY', 'COMP', 'COMP-1', 'COMP-2',
        'COMP-3', 'COMP-4', 'COMP-5', 'PACKED-DECIMAL', 'INDEX', 'POINTER',
        'PROCEDURE-POINTER', 'FUNCTION-POINTER', 'OBJECT REFERENCE'],
    'PICTURE symbols': [
        'PIC A', 'PIC X', 'PIC 9', 'PIC S', 'PIC V', 'PIC P', 'PIC Z', 'PIC *', 'PIC B',
        'PIC 0', 'PIC /', 'PIC ,', 'PIC .', 'PIC +', 'PIC -', 'PIC CR', 'PIC DB', 'PIC $',
        'PIC E', 'PIC G', 'PIC N', 'PIC U'],
    'File description and control': [
        'ORGANIZATION SEQUENTIAL', 'ORGANIZATION INDEXED', 'ORGANIZATION RELATIVE',
        'ORGANIZATION LINE SEQUENTIAL', 'ACCESS SEQUENTIAL', 'ACCESS RANDOM', 'ACCESS DYNAMIC',
        'RECORD KEY', 'ALTERNATE RECORD KEY', 'WITH DUPLICATES', 'RELATIVE KEY', 'FILE STATUS',
        'RESERVE', 'PADDING CHARACTER', 'RECORD DELIMITER', 'BLOCK CONTAINS', 'RECORD CONTAINS',
        'RECORD VARYING', 'RECORDING MODE', 'LABEL RECORDS', 'CODE-SET', 'LINAGE', 'SD',
        'SAME AREA', 'SAME RECORD AREA', 'SAME SORT AREA', 'RERUN', 'APPLY WRITE-ONLY',
        'MULTIPLE FILE TAPE', 'OPEN EXTEND', 'OPEN I-O', 'CLOSE REEL', 'CLOSE UNIT', 'WITH LOCK',
        'WITH NO REWIND', 'REVERSED'],
    'Environment and structure': [
        'DECIMAL-POINT IS COMMA', 'CURRENCY SIGN', 'ALPHABET', 'CLASS', 'SYMBOLIC CHARACTERS',
        'UPSI', 'C01', 'PROGRAM COLLATING SEQUENCE', 'XML-SCHEMA', 'NESTED PROGRAM', 'COMMON',
        'INITIAL', 'RECURSIVE', 'COPY', 'COPY REPLACING', 'REPLACE', 'DECLARATIVES', 'CBL',
        'PROCESS', '>>CALLINTERFACE', '>>DEFINE', '>>EVALUATE', '>>IF'],
    'Intrinsic functions': [
        'ABS', 'ACOS', 'ANNUITY', 'ASIN', 'ATAN', 'BIT-OF', 'BIT-TO-CHAR', 'BYTE-LENGTH', 'CHAR',
        'COMBINED-DATETIME', 'COS', 'CURRENT-DATE', 'DATE-OF-INTEGER', 'DATE-TO-YYYYMMDD',
        'DAY-OF-INTEGER', 'DAY-TO-YYYYDDD', 'DISPLAY-OF', 'E', 'EXP', 'EXP10', 'FACTORIAL',
        'FORMATTED-CURRENT-DATE', 'FORMATTED-DATE', 'FORMATTED-DATETIME', 'FORMATTED-TIME',
        'HEX-OF', 'HEX-TO-CHAR', 'INTEGER', 'INTEGER-OF-DATE', 'INTEGER-OF-DAY',
        'INTEGER-OF-FORMATTED-DATE', 'INTEGER-PART', 'LENGTH', 'LOG', 'LOG10', 'LOWER-CASE',
        'MAX', 'MEAN', 'MEDIAN', 'MIDRANGE', 'MIN', 'MOD', 'NATIONAL-OF', 'NUMVAL', 'NUMVAL-C',
        'NUMVAL-F', 'ORD', 'ORD-MAX', 'ORD-MIN', 'PI', 'PRESENT-VALUE', 'RANDOM', 'RANGE', 'REM',
        'REVERSE', 'SECONDS-FROM-FORMATTED-TIME', 'SECONDS-PAST-MIDNIGHT', 'SIGN', 'SIN', 'SQRT',
        'STANDARD-DEVIATION', 'SUM', 'TAN', 'TEST-DATE-YYYYMMDD', 'TEST-DAY-YYYYDDD',
        'TEST-FORMATTED-DATETIME', 'TEST-NUMVAL', 'TEST-NUMVAL-C', 'TEST-NUMVAL-F', 'TRIM',
        'TRIML', 'TRIMR', 'ULENGTH', 'UPOS', 'UPPER-CASE', 'USUBSTR', 'USUPPLEMENTARY', 'UVALID',
        'UWIDTH', 'VARIANCE', 'WHEN-COMPILED', 'YEAR-TO-YYYY'],
    'Special registers': [
        'ADDRESS OF', 'LENGTH OF', 'LINAGE-COUNTER', 'RETURN-CODE', 'SHIFT-OUT', 'SHIFT-IN',
        'SORT-CONTROL', 'SORT-CORE-SIZE', 'SORT-FILE-SIZE', 'SORT-MESSAGE', 'SORT-MODE-SIZE',
        'SORT-RETURN', 'TALLY', 'WHEN-COMPILED', 'XML-CODE', 'XML-EVENT', 'XML-TEXT',
        'XML-NTEXT', 'JSON-CODE', 'JSON-STATUS', 'DEBUG-ITEM'],
    'Compiler options (semantic)': [
        'ARITH', 'CODEPAGE', 'DBCS', 'INTDATE', 'NSYMBOL', 'NUMCLS', 'NUMPROC', 'NUMCHECK',
        'TRUNC', 'ZWB', 'DYNAM', 'QUALIFY', 'INITCHECK', 'SQLCCSID'],
}

COBOL_PATTERNS = {
    'EXIT PROGRAM': r'\bEXIT\s+PROGRAM\b', 'EXIT PERFORM': r'\bEXIT\s+PERFORM\b',
    'EXIT PARAGRAPH': r'\bEXIT\s+PARAGRAPH\b', 'EXIT SECTION': r'\bEXIT\s+SECTION\b',
    'GO TO': r'\bGO\s+(TO\s+)?[A-Z0-9-]+\s*\.', 'GO TO DEPENDING': r'\bDEPENDING\s+ON\b(?![^.]*OCCURS)',
    'INSPECT TALLYING': r'\bINSPECT\b[^.]*\bTALLYING\b', 'INSPECT REPLACING': r'\bINSPECT\b[^.]*\bREPLACING\b',
    'INSPECT CONVERTING': r'\bINSPECT\b[^.]*\bCONVERTING\b',
    'MOVE CORRESPONDING': r'\bMOVE\s+(CORR|CORRESPONDING)\b',
    'PERFORM TIMES': r'\bPERFORM\b[^.]*\bTIMES\b', 'PERFORM UNTIL': r'\bPERFORM\b[^.]*\bUNTIL\b',
    'PERFORM VARYING': r'\bPERFORM\b[^.]*\bVARYING\b', 'PERFORM AFTER': r'\bPERFORM\b[^.]*\bVARYING\b[^.]*\bAFTER\b',
    'WITH TEST AFTER': r'\bTEST\s+AFTER\b', 'READ NEXT': r'\bREAD\s+[A-Z0-9-]+\s+NEXT\b',
    'READ PREVIOUS': r'\bREAD\s+[A-Z0-9-]+\s+PREVIOUS\b', 'SET TO TRUE': r'\bSET\b[^.]*\bTO\s+TRUE\b',
    'SET TO FALSE': r'\bSET\b[^.]*\bTO\s+FALSE\b', 'SET UP BY': r'\bUP\s+BY\b', 'SET DOWN BY': r'\bDOWN\s+BY\b',
    'SET ADDRESS OF': r'\bSET\s+ADDRESS\s+OF\b', 'SEARCH ALL': r'\bSEARCH\s+ALL\b', 'SEARCH': r'\bSEARCH\b(?!\s+ALL)',
    'STOP RUN': r'\bSTOP\s+RUN\b', 'USE': r'\bUSE\s+(AFTER|FOR|BEFORE)\b',
    'ROUNDED': r'\bROUNDED\b', 'ROUNDED MODE': r'\bROUNDED\s+MODE\b', 'ALSO': r'\bALSO\b',
    'VALUE THRU': r'\bVALUES?\b[^.]*\b(THRU|THROUGH)\b', 'WHEN SET TO FALSE': r'\bFALSE\b(?![^.]*\bSET\b)',
    'LEVEL 66': r'^\s*66\s', 'LEVEL 77': r'^\s*77\s', 'LEVEL 88': r'^\s*88\s',
    'OCCURS DEPENDING': r'\bOCCURS\b[^.]*\bDEPENDING\b', 'SIGN LEADING': r'\bSIGN\s+(IS\s+)?LEADING\b',
    'SIGN TRAILING': r'\bSIGN\s+(IS\s+)?TRAILING\b', 'SEPARATE CHARACTER': r'\bSEPARATE\b',
    'SYNCHRONIZED': r'\bSYNC(HRONIZED)?\b', 'DISPLAY': r'\bUSAGE\s+(IS\s+)?DISPLAY\b(?!-)',
    'BINARY': r'\bBINARY\b', 'COMP': r'\bCOMP(UTATIONAL)?\b(?!-)', 'INDEX': r'\bUSAGE\s+(IS\s+)?INDEX\b|\bINDEX\s*\.',
    'POINTER': r'\bUSAGE\s+(IS\s+)?POINTER\b|\bPOINTER\s*\.', 'OBJECT REFERENCE': r'\bOBJECT\s+REFERENCE\b',
    'NESTED PROGRAM': r'\bEND\s+PROGRAM\b[\s\S]*\bPROGRAM-ID\b', 'COMMON': r'\bIS\s+COMMON\b|\bCOMMON\s+PROGRAM\b',
    'INITIAL': r'\bPROGRAM-ID\.\s*[A-Z0-9-]+\s+(IS\s+)?([A-Z ]+\s)?INITIAL\b', 'RECURSIVE': r'\bRECURSIVE\b', 'COPY': r'\bCOPY\b',
    'COPY REPLACING': r'\bCOPY\b[^.]*\bREPLACING\b', 'REPLACE': r'^\s*REPLACE\b',
    'CBL': r'^\s*CBL\b', 'PROCESS': r'^\s*PROCESS\b', 'SD': r'^\s*SD\s', 'C01': r'\bC01\b',
    'CLASS': r'\bCLASS\s+[A-Z0-9-]+\s+(IS\s+)?[\'"A-Z]', 'ALPHABET': r'\bALPHABET\s+[A-Z0-9-]+',
    'SAME AREA': r'\bSAME\s+AREA\b', 'SAME RECORD AREA': r'\bSAME\s+RECORD\b', 'SAME SORT AREA': r'\bSAME\s+SORT\b',
    'OPEN EXTEND': r'\bOPEN\b[^.]*\bEXTEND\b', 'OPEN I-O': r'\bOPEN\b[^.]*\bI-O\b',
    'CLOSE REEL': r'\bCLOSE\b[^.]*\bREEL\b', 'CLOSE UNIT': r'\bCLOSE\b[^.]*\bUNIT\b',
    'WITH LOCK': r'\bWITH\s+LOCK\b', 'WITH NO REWIND': r'\bNO\s+REWIND\b', 'REVERSED': r'\bREVERSED\b',
    'LINKAGE SECTION': r'\bLINKAGE\s+SECTION\b', 'FILLER': r'\bFILLER\b',
    'ARITH': r'\bARITH\s*\(', 'CODEPAGE': r'\bCODEPAGE\s*\(|\bCP\s*\(', 'DBCS': r'^\s*(CBL|PROCESS)\b.*\bDBCS\b',
    'INTDATE': r'\bINTDATE\b', 'NSYMBOL': r'\bNSYMBOL\b', 'NUMCLS': r'\bNUMCLS\b', 'NUMPROC': r'\bNUMPROC\b',
    'NUMCHECK': r'\bNUMCHECK\b', 'TRUNC': r'\bTRUNC\s*\(', 'ZWB': r'\bN?OZWB\b|\bZWB\b', 'DYNAM': r'\bDYNAM\b',
    'QUALIFY': r'\bQUALIFY\b', 'INITCHECK': r'\bINITCHECK\b', 'SQLCCSID': r'\bSQLCCSID\b',
    'XML-SCHEMA': r'\bXML-SCHEMA\b', 'UPSI': r'\bUPSI\b', 'BY VALUE': r'\bBY\s+VALUE\b',
    'BY REFERENCE': r'\bBY\s+REFERENCE\b', 'BY CONTENT': r'\bBY\s+CONTENT\b', 'THRU': r'\bTHRU|THROUGH\b',
    'WHEN OTHER': r'\bWHEN\s+OTHER\b', 'UPON': r'\bUPON\b', 'RETURNING': r'\bRETURNING\b',
}


COBOL_CATEGORY_PATTERNS = {
    ('Statements', 'DISPLAY'): r'\bDISPLAY\s+(?!-)[\'"A-Z0-9(]',
    ('USAGE', 'DISPLAY'): r'\bUSAGE\s+(IS\s+)?DISPLAY\b(?!-)',
    ('Statements', 'ENTRY'): r'^\s*ENTRY\s+[\'"]',
    ('Statements', 'RETURN'): r'^\s*RETURN\s+[A-Z]',
    ('Statements', 'START'): r'^\s*START\s+[A-Z]',
    ('Statements', 'DELETE'): r'^\s*DELETE\s+[A-Z]',
    ('Statements', 'CALL'): r'\bCALL\s+[\'"A-Z]',
    ('Statements', 'COPY'): r'\bCOPY\b',
}


def cobol_pattern(category, name):
    if (category, name) in COBOL_CATEGORY_PATTERNS:
        return COBOL_CATEGORY_PATTERNS[(category, name)]
    if name in COBOL_PATTERNS:
        return COBOL_PATTERNS[name]
    if category == 'Intrinsic functions':
        return r'\bFUNCTION\s+' + re.escape(name) + r'\b'
    if category == 'PICTURE symbols':
        symbol = name.split(' ', 1)[1]
        if symbol in ('CR', 'DB'):
            return r'\bPIC(TURE)?\s+(IS\s+)?\S*' + symbol
        if symbol == 'P':
            return r'\bPIC(TURE)?\s+(IS\s+)?S?[9V]*P'
        return r'\bPIC(TURE)?\s+(IS\s+)?\S*' + re.escape(symbol)
    # a word boundary only before a word character (>>DEFINE starts with '>')
    lead = r'\b' if re.match(r'\w', name) else ''
    return lead + r'\s+'.join(re.escape(w) for w in name.split()) + r'\b'


def cobol_text(path):
    lines = []
    with open(path, encoding='latin-1') as f:
        for line in f:
            line = line.rstrip('\n')
            if len(line) > 6 and line[6] in '*/':
                continue
            lines.append(line[7:72] if len(line) > 7 else '')
    return '\n'.join(lines).upper()


# --------------------------------------------------------------------------- PL/I
PLI = {
    'Statements': [
        'ALLOCATE', 'ASSERT', 'ATTACH', 'BEGIN', 'CALL', 'CLOSE', 'DECLARE', 'DEFAULT', 'DEFINE ALIAS',
        'DEFINE ORDINAL', 'DEFINE STRUCTURE', 'DELAY', 'DELETE', 'DETACH', 'DISPLAY', 'DO', 'DO WHILE',
        'DO UNTIL', 'DO LOOP', 'DO REPEAT', 'END', 'ENTRY', 'EXIT', 'FETCH', 'FLUSH', 'FORMAT', 'FREE',
        'GET', 'GO TO', 'IF', 'ITERATE', 'LEAVE', 'LOCATE', 'ON', 'OPEN', 'PACKAGE', 'PROCEDURE', 'PUT',
        'READ', 'RELEASE', 'RESIGNAL', 'RETURN', 'REVERT', 'REWRITE', 'SELECT', 'SIGNAL', 'STOP',
        'WAIT', 'WRITE'],
    'Data attributes': [
        'ALIGNED', 'AREA', 'AUTOMATIC', 'BASED', 'BIGENDIAN', 'BINARY', 'BIT', 'BUILTIN', 'BYADDR',
        'BYVALUE', 'CHARACTER', 'COMPLEX', 'CONDITION', 'CONNECTED', 'CONTROLLED', 'DATE', 'DECIMAL',
        'DEFINED', 'DIMENSION', 'ENTRY', 'EXTERNAL', 'FILE', 'FIXED', 'FLOAT', 'FORMAT', 'GENERIC',
        'GRAPHIC', 'HANDLE', 'HEXADEC', 'IEEE', 'INITIAL', 'INTERNAL', 'LABEL', 'LIKE', 'LIMITED',
        'LITTLEENDIAN', 'NONASSIGNABLE', 'NONVARYING', 'OFFSET', 'OPTIONAL', 'OPTIONS', 'ORDINAL',
        'PARAMETER', 'PICTURE', 'POINTER', 'POSITION', 'PRECISION', 'REAL', 'REFER', 'RESERVED',
        'RETURNS', 'SIGNED', 'STATIC', 'TYPE', 'UNALIGNED', 'UNION', 'UNSIGNED', 'VALUE', 'VARIABLE',
        'VARYING', 'VARYINGZ', 'WIDECHAR'],
    'File attributes and options': [
        'BUFFERED', 'DIRECT', 'ENVIRONMENT', 'INPUT', 'KEYED', 'OUTPUT', 'PRINT', 'RECORD',
        'SEQUENTIAL', 'STREAM', 'UPDATE', 'TITLE', 'LINESIZE', 'PAGESIZE', 'KEY', 'KEYFROM', 'KEYTO',
        'SET', 'INTO', 'FROM', 'PAGE', 'SKIP', 'LINE', 'LIST', 'EDIT', 'DATA', 'STRING', 'COPY'],
    'Conditions': [
        'AREA', 'ATTENTION', 'CONDITION', 'CONVERSION', 'ENDFILE', 'ENDPAGE', 'ERROR', 'FINISH',
        'FIXEDOVERFLOW', 'INVALIDOP', 'KEY', 'NAME', 'OVERFLOW', 'RECORD', 'SIZE', 'STORAGE',
        'STRINGRANGE', 'STRINGSIZE', 'SUBSCRIPTRANGE', 'TRANSMIT', 'UNDEFINEDFILE', 'UNDERFLOW',
        'ZERODIVIDE'],
    'Built-in functions': [
        'ABS', 'ACOS', 'ADD', 'ADDR', 'ADDRDATA', 'ALL', 'ALLOCATION', 'ALLOCSIZE', 'ANY', 'ASIN',
        'ATAN', 'ATAND', 'ATANH', 'BINARY', 'BINARYVALUE', 'BINVALUE', 'BIT', 'BITLOCATION', 'BOOL',
        'BYTE', 'CDS', 'CEIL', 'CENTER', 'CENTRE', 'CHARACTER', 'CHARGRAPHIC', 'CHARVAL', 'COLLATE',
        'COMPARE', 'COMPLEX', 'CONJG', 'COPY', 'COS', 'COSD', 'COSH', 'COUNT', 'CS', 'CURRENTSIZE',
        'CURRENTSTORAGE', 'DATAFIELD', 'DATE', 'DATETIME', 'DAYS', 'DAYSTODATE', 'DAYSTOSECS',
        'DECIMAL', 'DIMENSION', 'DIVIDE', 'EDIT', 'EMPTY', 'ENDFILE', 'ENTRYADDR', 'EPSILON', 'ERF',
        'ERFC', 'EXP', 'EXPONENT', 'FIXED', 'FLOAT', 'FLOOR', 'GAMMA', 'GETENV', 'GRAPHIC', 'HANDLE',
        'HBOUND', 'HEX', 'HEXIMAGE', 'HIGH', 'HUGE', 'IAND', 'IEOR', 'IMAG', 'INDEX', 'INOT', 'IOR',
        'ISIGNED', 'ISLL', 'ISRL', 'IUNSIGNED', 'JULIANTOSECS', 'LBOUND', 'LEFT', 'LENGTH', 'LINENO',
        'LOCATION', 'LOG', 'LOG10', 'LOG2', 'LOGGAMMA', 'LOW', 'LOWERCASE', 'LOWER2', 'MAX',
        'MAXEXP', 'MAXLENGTH', 'MIN', 'MINEXP', 'MOD', 'MULTIPLY', 'NEW', 'NULL', 'OFFSET',
        'OFFSETADD', 'OFFSETDIFF', 'OFFSETSUBTRACT', 'OFFSETVALUE', 'OMITTED', 'ONCHAR', 'ONCODE',
        'ONCONDCOND', 'ONCONDID', 'ONCOUNT', 'ONFILE', 'ONKEY', 'ONLOC', 'ONSOURCE', 'ONWCHAR',
        'ORDINALNAME', 'ORDINALPRED', 'ORDINALSUCC', 'PAGENO', 'PLIRETV', 'POINTER', 'POINTERADD',
        'POINTERDIFF', 'POINTERSUBTRACT', 'POINTERVALUE', 'POPCNT', 'PRECISION', 'PRESENT', 'PROD',
        'RADIX', 'RAISE2', 'RANDOM', 'RANK', 'REAL', 'REM', 'REPEAT', 'REPLACEBY2', 'REVERSE',
        'RIGHT', 'ROUND', 'ROUNDDEC', 'SAMEKEY', 'SCALE', 'SEARCH', 'SEARCHR', 'SECS', 'SECSTODATE',
        'SECSTODAYS', 'SIGN', 'SIN', 'SIND', 'SINH', 'SIZE', 'SQRT', 'STG', 'STORAGE', 'STRING',
        'SUBSTR', 'SUBTRACT', 'SUCC', 'SUM', 'SYSNULL', 'TALLY', 'TAN', 'TAND', 'TANH', 'TIME',
        'TINY', 'TRANSLATE', 'TRIM', 'TRUNC', 'TYPE', 'UNSPEC', 'UPPERCASE', 'UTCDATETIME',
        'VALID', 'VALIDDATE', 'VARGLIST', 'VERIFY', 'VERIFYR', 'WEEKDAY', 'WHITESPACECOLLAPSE',
        'XMLCHAR', 'Y4DATE', 'Y4JULIAN', 'Y4YEAR'],
    'Pseudovariables': ['ONCHAR', 'ONSOURCE', 'REAL', 'IMAG', 'STRING', 'SUBSTR', 'UNSPEC', 'ENTRYADDR'],
    'Built-in subroutines': ['PLIRETC', 'PLIFREE', 'PLISRTA', 'PLISRTB', 'PLISRTC', 'PLISRTD',
                             'PLIDUMP', 'PLITEST', 'PLICANC', 'PLICKPT', 'PLIREST'],
    'Preprocessor': [
        '%ACTIVATE', '%DEACTIVATE', '%DECLARE', '%DO', '%END', '%GO TO', '%IF', '%INCLUDE',
        '%XINCLUDE', '%INSCAN', '%ITERATE', '%LEAVE', '%NOTE', '%PAGE', '%POP', '%PRINT',
        '%PROCEDURE', '%PUSH', '%REPLACE', '%SELECT', '%SKIP', 'STATEMENT', 'COMPILEDATE',
        'COMPILETIME', 'COUNTER', 'PARMSET', 'QUOTE', 'SYSPARM', 'SYSTEM', 'SYSVERSION', 'MACNAME'],
    'Procedure options': ['OPTIONS(MAIN)', 'NOEXECOPS', 'FETCHABLE', 'RECURSIVE', 'REORDER',
                          'BYVALUE', 'LINKAGE', 'ASSEMBLER', 'COBOL', 'INTER', 'RETCODE'],
}

PLI_PATTERNS = {
    'DEFINE ALIAS': r'\bDEFINE\s+ALIAS\b', 'DEFINE ORDINAL': r'\bDEFINE\s+ORDINAL\b',
    'DEFINE STRUCTURE': r'\bDEFINE\s+STRUCTURE\b', 'DO WHILE': r'\bDO\s+WHILE\b',
    'DO UNTIL': r'\bDO\s+UNTIL\b', 'DO LOOP': r'\bDO\s+(LOOP|FOREVER)\b', 'DO REPEAT': r'\bREPEAT\b',
    'GO TO': r'\bGO\s*TO\b', 'CHARACTER': r'\bCHAR(ACTER)?\b', 'BINARY': r'\bBIN(ARY)?\b',
    'DECIMAL': r'\bDEC(IMAL)?\b', 'DIMENSION': r'\bDIM(ENSION)?\b', 'CONTROLLED': r'\bCONTROLLED\b|\bCTL\b',
    'AUTOMATIC': r'\bAUTO(MATIC)?\b', 'INITIAL': r'\bINIT(IAL)?\b', 'PICTURE': r'\bPIC(TURE)?\b',
    'POINTER': r'\bPOINTER\b|\bPTR\b', 'PRECISION': r'\bPREC(ISION)?\b', 'UNALIGNED': r'\bUNAL(IGNED)?\b',
    'VARYING': r'\bVAR(YING)?\b', 'ENVIRONMENT': r'\bENV(IRONMENT)?\b', 'CONVERSION': r'\bCONV(ERSION)?\b',
    'FIXEDOVERFLOW': r'\bFIXEDOVERFLOW\b|\bFOFL\b', 'OVERFLOW': r'\bOVERFLOW\b|\bOFL\b',
    'ZERODIVIDE': r'\bZERODIVIDE\b|\bZDIV\b', 'UNDERFLOW': r'\bUNDERFLOW\b|\bUFL\b',
    'STRINGRANGE': r'\bSTRINGRANGE\b|\bSTRG\b', 'STRINGSIZE': r'\bSTRINGSIZE\b|\bSTRZ\b',
    'SUBSCRIPTRANGE': r'\bSUBSCRIPTRANGE\b|\bSUBRG\b', 'UNDEFINEDFILE': r'\bUNDEFINEDFILE\b|\bUNDF\b',
    'CONDITION': r'\bCOND(ITION)?\b', 'OPTIONS(MAIN)': r'\bOPTIONS\s*\([^)]*\bMAIN\b',
    'STATEMENT': r'%\s*[A-Z0-9_]+\s*:\s*PROC[^;]*\bSTATEMENT\b',
    'CENTRE': r'\bCENTRE\s*\(', 'CENTER': r'\bCENTER\s*\(',
    # abbreviations the Language Reference allows
    'DECLARE': r'\bDECLARE\b|\bDCL\b', 'PROCEDURE': r'\bPROC(EDURE)?\b',
    'STORAGE': r'\b(STORAGE|STG)\s*\(', 'CURRENTSTORAGE': r'\b(CURRENTSTORAGE|CSTG)\s*\(',
}
for p in ['%ACTIVATE', '%DEACTIVATE', '%DECLARE', '%DO', '%END', '%IF', '%INCLUDE', '%XINCLUDE',
          '%INSCAN', '%ITERATE', '%LEAVE', '%NOTE', '%PAGE', '%POP', '%PRINT', '%PROCEDURE', '%PUSH',
          '%REPLACE', '%SELECT', '%SKIP']:
    word = p[1:]
    alt = {'DECLARE': 'DECLARE|DCL', 'PROCEDURE': 'PROCEDURE|PROC', 'ACTIVATE': 'ACTIVATE|ACT',
           'DEACTIVATE': 'DEACTIVATE|DEACT'}.get(word, word)
    PLI_PATTERNS[p] = r'%\s*([A-Z0-9_]+\s*:\s*)?(' + alt + r')\b'
PLI_PATTERNS['%GO TO'] = r'%\s*GO\s*TO\b'


def pli_pattern(category, name):
    if name in PLI_PATTERNS:
        return PLI_PATTERNS[name]
    if category in ('Built-in functions', 'Pseudovariables', 'Built-in subroutines'):
        return r'\b' + re.escape(name) + r'\s*\('
    return r'\b' + r'\s+'.join(re.escape(w) for w in name.split()) + r'\b'


def pli_text(path):
    with open(path, encoding='latin-1') as f:
        text = f.read()
    return re.sub(r'/\*.*?\*/', ' ', text, flags=re.S).upper()


# --------------------------------------------------------------------------- HLASM
HLASM_ASM = [
    'ACONTROL', 'ADATA', 'AINSERT', 'ALIAS', 'AMODE', 'CATTR', 'CCW', 'CCW0', 'CCW1', 'CEJECT',
    'CNOP', 'COM', 'COPY', 'CSECT', 'CXD', 'DC', 'DROP', 'DS', 'DSECT', 'DXD', 'EJECT', 'END',
    'ENTRY', 'EQU', 'EXITCTL', 'EXTRN', 'ICTL', 'ISEQ', 'LOCTR', 'LTORG', 'MNOTE', 'OPSYN', 'ORG',
    'POP', 'PRINT', 'PUNCH', 'PUSH', 'REPRO', 'RMODE', 'RSECT', 'SPACE', 'START', 'TITLE', 'USING',
    'WXTRN', 'XATTR']
HLASM_DC = ['A', 'AD', 'B', 'C', 'CA', 'CE', 'CU', 'D', 'DB', 'DD', 'DH', 'E', 'EB', 'ED', 'EH',
            'F', 'FD', 'G', 'H', 'HD', 'J', 'L', 'LB', 'LD', 'LH', 'LQ', 'P', 'Q', 'QD', 'QY', 'R',
            'RD', 'S', 'SY', 'V', 'VD', 'X', 'Y', 'Z']
HLASM_MACRO = [
    'ACTR', 'AEJECT', 'AGO', 'AIF', 'ANOP', 'AREAD', 'ASPACE', 'GBLA', 'GBLB', 'GBLC', 'LCLA',
    'LCLB', 'LCLC', 'MACRO', 'MEND', 'MEXIT', 'MHELP', 'SETA', 'SETB', 'SETC', 'SETAF', 'SETCF',
    '&SYSNDX', '&SYSECT', '&SYSLIST', '&SYSDATE', '&SYSDATC', '&SYSTIME', '&SYSPARM', '&SYSLOC',
    '&SYSNEST', '&SYSMAC', '&SYSSTYP', '&SYSOPT_RENT', "T'", "L'", "S'", "I'", "K'", "N'", "D'",
    "O'", 'A2B', 'A2C', 'A2D', 'A2X', 'B2A', 'B2C', 'B2D', 'B2X', 'BYTE', 'C2A', 'C2B', 'C2D',
    'C2X', 'D2A', 'D2B', 'D2C', 'D2X', 'DCLEN', 'DCVAL', 'DEQUOTE', 'DOUBLE', 'FIND', 'INDEX',
    'ISBIN', 'ISDEC', 'ISHEX', 'ISSYM', 'LOWER', 'SIGNED', 'SLA', 'SLL', 'SRA', 'SRL', 'UPPER',
    'X2A', 'X2B', 'X2C', 'X2D', 'AND', 'OR', 'XOR', 'NOT']


def hlasm_instructions():
    """Every machine mnemonic the Zeus decoder knows (machine.rs, machine/*.rs)."""
    names = set()
    base = os.path.join(ROOT, 'crates/zeus-cobol/src/hlasm')
    for rel in ['machine.rs', 'machine/registers.rs', 'machine/long_storage.rs',
                'machine/floating.rs', 'template.rs']:
        path = os.path.join(base, rel)
        if os.path.exists(path):
            names |= set(re.findall(r'"([A-Z][A-Z0-9]{0,7})"', open(path).read()))
    # names in the decoder files that are system macros, not instructions
    macros = {'STORAGE', 'STIMER', 'XCTL'}
    return sorted(n for n in names if n not in HLASM_ASM and n not in macros)


def hlasm_text(path):
    lines = []
    with open(path, encoding='latin-1') as f:
        for line in f:
            if line.startswith('*') or line.startswith('.*'):
                continue
            lines.append(line[:71].upper())
    return '\n'.join(lines)


def hlasm_pattern(category, name):
    if category == 'Machine instructions' or category == 'Assembler instructions':
        return r'^\S*\s+' + re.escape(name) + r'(\s|$)'
    if category == 'DC types':
        # nominal values are quoted; address constants (A, V, S, Y, Q, R, J ...) are
        # parenthesized
        return r'^\S*\s+D[CS]\s+(\S*,)?[0-9()]*' + re.escape(name) + r"(L[0-9.]+)?['(]"
    if name.startswith('&') or name.endswith("'"):
        return re.escape(name)
    if name in ('AND', 'OR', 'XOR', 'NOT'):
        return r'\(\s*[^)]*\s' + name + r'\s'
    if name in ('INDEX', 'FIND', 'BYTE', 'UPPER', 'LOWER', 'DOUBLE', 'SIGNED') or name[1:2] == '2' or \
            name.startswith('IS') or name in ('DCLEN', 'DCVAL', 'DEQUOTE', 'SLA', 'SLL', 'SRA', 'SRL'):
        return r'\b' + name + r"\s*\(|\(\s*" + name + r'\s'
    return r'^\S*\s+' + re.escape(name) + r'(\s|$)'


# --------------------------------------------------------------------------- common
# Report infrastructure: the self-checking report copybooks, macros and writers.
# What they use is not a check of the element.
INFRASTRUCTURE = {'ZCVSRPT', 'ZCVSPROC', 'ZCVSHEX', 'ZHVSCHK', 'ZHVSHDR', 'ZHVSPUT', 'ZHVSSUM',
                  'ZHVRPT', 'ZPVSRPT'}


def sources(suite, exts):
    out = {}
    base = os.path.join(ROOT, suite)
    for dirpath, _, files in os.walk(base):
        for f in sorted(files):
            if os.path.splitext(f)[1].upper() in exts and \
                    os.path.splitext(f)[0].upper() not in INFRASTRUCTURE:
                out[os.path.relpath(os.path.join(dirpath, f), base)] = os.path.join(dirpath, f)
    return out


def ccvs85_programs(path):
    """CCVS85 members as (name, text) from newcob.val (*HEADER marks each member)."""
    members, name, lines = [], None, []
    with open(path, encoding='latin-1') as f:
        for line in f:
            if line.startswith('*HEADER'):
                if name:
                    members.append((name, '\n'.join(lines).upper()))
                parts = line.split(',')
                name, lines = (parts[2].strip() if len(parts) > 2 else line[8:].strip()), []
                continue
            line = line.rstrip('\n')
            if len(line) > 6 and line[6] in '*/':
                continue
            lines.append(line[7:72] if len(line) > 7 else '')
    if name:
        members.append((name, '\n'.join(lines).upper()))
    return members


def reviewed_marks(path):
    marks = {}
    if os.path.exists(path):
        for line in open(path, encoding='utf-8'):
            cells = [c.strip() for c in line.strip().strip('|').split('|')]
            if len(cells) >= 6 and cells[0] not in ('分類', '---'):
                marks[(cells[0], cells[1])] = cells[-1]
    return marks


def table(title, groups, texts, pattern_of, out_path, ccvs=None, note=''):
    marks = reviewed_marks(out_path)
    lines = [f'# {title}', '', note, '',
             '状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う',
             'ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に',
             '「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。', '']
    header = '| 分類 | 要素 | 状態 | スイートでの使用 | CCVS85 | 確認 |' if ccvs is not None else \
        '| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |'
    lines += [header, '| --- | --- | --- | --- | --- | --- |']
    totals = {'used': 0, 'ccvs85': 0, 'missing': 0}
    for category, names in groups.items():
        for name in names:
            rx = re.compile(pattern_of(category, name), re.M)
            used = sorted({os.path.splitext(os.path.basename(p))[0] for p, t in texts.items() if rx.search(t)})
            shown = ', '.join(used[:6]) + (f' ほか {len(used) - 6}' if len(used) > 6 else '')
            c = '-'
            hits = []
            if ccvs is not None:
                hits = [n for n, t in ccvs if rx.search(t)]
                c = str(len(hits)) if hits else '0'
            # an element only NIST CCVS85 uses is measured there (CCVS85 also runs on z/OS)
            state = '使用あり' if used else ('CCVS85' if hits else '未網羅')
            totals[{'使用あり': 'used', 'CCVS85': 'ccvs85', '未網羅': 'missing'}[state]] += 1
            mark = marks.get((category, name.replace('|', '\\|')), '')
            lines.append(f'| {category} | {name.replace("|", chr(92) + "|")} | {state} | {shown} | {c} | {mark} |')
    lines[2] = note + (f' 要素 {sum(totals.values())}、使用あり {totals["used"]}、'
                       + (f'CCVS85 だけが測る {totals["ccvs85"]}、' if ccvs is not None else '')
                       + f'未網羅 {totals["missing"]}。')
    with open(out_path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines) + '\n')
    return totals


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ccvs85')
    a = ap.parse_args()
    inv = os.path.join(ROOT, 'docs/inventory')
    os.makedirs(inv, exist_ok=True)
    ccvs = ccvs85_programs(a.ccvs85) if a.ccvs85 else None
    cob = {p: cobol_text(f) for p, f in sources('zcvs', {'.CBL', '.CPY'}).items()}
    t1 = None
    if ccvs is None:
        # without the CCVS85 corpus the COBOL table would lose its CCVS85 column
        # and classification; leave the committed table as it is
        print('zcvs: not regenerated (pass --ccvs85 newcob.val)')
    else:
        t1 = table('ZCVS 網羅台帳 (Enterprise COBOL for z/OS)', COBOL, cob, cobol_pattern,
                   os.path.join(inv, 'zcvs-elements.md'), ccvs,
                   'Enterprise COBOL for z/OS Language Reference の章立てから列挙した。CCVS85 の列は、'
                   'その要素を使う CCVS85 のプログラム数。ZCVS が使わず CCVS85 だけが使う要素は'
                   '状態を「CCVS85」とする (CCVS85 も z/OS で流す)。')
    pli = {p: pli_text(f) for p, f in sources('zpvs', {'.PLI', '.INC'}).items()}
    t2 = table('ZPVS 網羅台帳 (Enterprise PL/I for z/OS)', PLI, pli, pli_pattern,
               os.path.join(inv, 'zpvs-elements.md'), None,
               'Enterprise PL/I for z/OS Language Reference の章立てから列挙した。')
    asm = {p: hlasm_text(f) for p, f in sources('zhvs', {'.ASM', '.MAC'}).items()}
    groups = {'Assembler instructions': HLASM_ASM, 'DC types': HLASM_DC,
              'Conditional assembly': HLASM_MACRO, 'Machine instructions': hlasm_instructions()}
    t3 = table('ZHVS 網羅台帳 (HLASM と z/Architecture)', groups, asm, hlasm_pattern,
               os.path.join(inv, 'zhvs-elements.md'), None,
               'HLASM Language Reference の章立てと、Zeus の命令の解読表 (machine.rs ほか) が知る命令を'
               '列挙した。命令の一覧は Principles of Operation の全命令ではなく、Zeus が知る命令である。')
    for name, t in [('zcvs', t1), ('zpvs', t2), ('zhvs', t3)]:
        if t is not None:
            extra = f', ccvs85 {t["ccvs85"]}' if t["ccvs85"] else ''
            print(f'{name}: used {t["used"]}{extra}, missing {t["missing"]}')


if __name__ == '__main__':
    sys.exit(main())

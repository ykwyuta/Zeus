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
        'WRITE', 'XML GENERATE', 'XML PARSE', 'ENTER'],
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
        'ULENGTH', 'UPOS', 'UPPER-CASE', 'CONTENT-OF', 'UUID4', 'USUBSTR', 'USUPPLEMENTARY', 'UVALID',
        'UWIDTH', 'VARIANCE', 'WHEN-COMPILED', 'YEAR-TO-YYYY'],
    'Special registers': [
        'ADDRESS OF', 'LENGTH OF', 'LINAGE-COUNTER', 'RETURN-CODE', 'SHIFT-OUT', 'SHIFT-IN',
        'SORT-CONTROL', 'SORT-CORE-SIZE', 'SORT-FILE-SIZE', 'SORT-MESSAGE', 'SORT-MODE-SIZE',
        'SORT-RETURN', 'TALLY', 'WHEN-COMPILED', 'XML-CODE', 'XML-EVENT', 'XML-TEXT',
        'XML-NTEXT', 'JSON-CODE', 'JSON-STATUS', 'DEBUG-ITEM', 'JNIENVPTR', 'IGY-JAVAIOP-CALL-EXCEPTION', 'XML-INFORMATION', 'XML-NAMESPACE', 'XML-NAMESPACE-PREFIX', 'XML-NNAMESPACE', 'XML-NNAMESPACE-PREFIX'],
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
        'WAIT', 'WRITE', 'CANCEL THREAD', 'OTHERWISE', 'QUALIFY', 'REINIT', 'WHEN', 'XDECLARE', 'XDEFINE ALIAS', 'XDEFINE ORDINAL', 'XDEFINE STRUCTURE', 'XPROCEDURE'],
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
        'ZERODIVIDE', 'ANYCONDITION', 'ASSERTION', 'CONFORMANCE'],
    # The index of built-in functions, subroutines and pseudovariables of the
    # Enterprise PL/I for z/OS 6.1 Language Reference (SC31-5716-00), read 2026-11-12.
    'Built-in functions': [
        'ABS', 'ACOS', 'ADD', 'ADDR', 'ADDRDATA', 'ALL', 'ALLCOMPARE', 'ALLOC31', 'ALLOCATE',
        'ALLOCATION', 'ALLOCNEXT', 'ALLOCSIZE', 'ANY', 'ASIN', 'ATAN', 'ATAND', 'ATANH',
        'AUTOMATIC', 'AVAILABLEAREA', 'BASE64DECODE', 'BASE64DECODE8', 'BASE64DECODE16',
        'BASE64ENCODE', 'BASE64ENCODE8', 'BASE64ENCODE16', 'BETWEEN', 'BETWEENEXCLUSIVE',
        'BETWEENLEFTEXCLUSIVE', 'BETWEENRIGHTEXCLUSIVE', 'BINARY', 'BINARYVALUE', 'BINSEARCH',
        'BINSEARCHX', 'BIT', 'BITLOCATION', 'BOOL', 'BYTE', 'BYTELENGTH', 'CDS', 'CEIL',
        'CENTERLEFT', 'CENTERRIGHT', 'CENTRELEFT', 'CENTRERIGHT', 'CHARACTER', 'CHARGRAPHIC',
        'CHARVAL', 'CHECKSTG', 'CHECKSUM', 'CODEPAGE', 'COLLATE', 'COLLAPSE', 'COMPARE', 'COMPLEX',
        'CONJG', 'COPY', 'COS', 'COSD', 'COSH', 'COUNT', 'CS', 'CURRENTSIZE', 'CURRENTSTORAGE',
        'DATAFIELD', 'DATE', 'DATETIME', 'DAYS', 'DAYSTODATE', 'DAYSTOMICROSECS', 'DAYSTOSECS',
        'DECIMAL', 'DIMENSION', 'DIVIDE', 'EDIT', 'EMPTY', 'ENDFILE', 'ENTRYADDR', 'EPSILON', 'ERF',
        'ERFC', 'EXP', 'EXPONENT', 'FILEDDINT', 'FILEDDTEST', 'FILEDDWORD', 'FILEID', 'FILENEW',
        'FILEOPEN', 'FILEREAD', 'FILESEEK', 'FILETELL', 'FILEWRITE', 'FIXED', 'FIXEDBIN',
        'FIXEDDEC', 'FLOAT', 'FLOATBIN', 'FLOATDEC', 'FLOOR', 'FOLDEDFULLMATCH',
        'FOLDEDSIMPLEMATCH', 'GAMMA', 'GETENV', 'GETJCLSYMBOL', 'GETSYSINT', 'GETSYSWORD',
        'GRAPHIC', 'GTCA', 'HANDLE', 'HBOUND', 'HBOUNDACROSS', 'HEX', 'HEX8', 'HEXDECODE',
        'HEXDECODE8', 'HEXENCODE', 'HEXENCODE8', 'HEXIMAGE', 'HEXIMAGE8', 'HIGH', 'HUGE', 'IAND',
        'ICLZ', 'IEOR', 'IFTHENELSE', 'IMAG', 'INARRAY', 'INDEX', 'INDEXR', 'INDICATORS', 'INLIST',
        'INOT', 'IOR', 'IRLL', 'IRRL', 'ISFINITE', 'ISIGNED', 'ISINF', 'ISJCLSYMBOL', 'ISLL',
        'ISMAIN', 'ISNAN', 'ISNORMAL', 'ISRL', 'ISZERO', 'IUNSIGNED', 'JSONGETARRAYEND',
        'JSONGETARRAYSTART', 'JSONGETCOLON', 'JSONGETCOMMA', 'JSONGETMEMBER', 'JSONGETOBJECTEND',
        'JSONGETOBJECTSTART', 'JSONGETVALUE', 'JSONPUTARRAYEND', 'JSONPUTARRAYSTART',
        'JSONPUTCOLON', 'JSONPUTCOMMA', 'JSONPUTMEMBER', 'JSONPUTOBJECTEND', 'JSONPUTOBJECTSTART',
        'JSONPUTVALUE', 'JSONVALID', 'JULIANTOSMF', 'LBOUND', 'LBOUNDACROSS', 'LEFT', 'LENGTH',
        'LINENO', 'LOCATION', 'LOCNEWSPACE', 'LOCNEWVALUE', 'LOCSTG', 'LOCVAL', 'LOG', 'LOGGAMMA',
        'LOG2', 'LOG10', 'LOW', 'LOWERASCII', 'LOWERCASE', 'LOWERLATIN1', 'LOWER2', 'MAINNAME',
        'MAX', 'MAXDATE', 'MAXEXP', 'MAXLENGTH', 'MAXVAL', 'MEMCOLLAPSE', 'MEMCONVERT', 'MEMCU12',
        'MEMCU14', 'MEMCU21', 'MEMCU24', 'MEMCU41', 'MEMCU42', 'MEMINDEX', 'MEMREPLACE',
        'MEMSEARCH', 'MEMSEARCHR', 'MEMSQUEEZE', 'MEMUVALID8', 'MEMUVALID16', 'MEMVERIFY',
        'MEMVERIFYR', 'MICROSECS', 'MICROSECSTODATE', 'MICROSECSTODAYS', 'MIN', 'MINDATE', 'MINEXP',
        'MINVAL', 'MOD', 'MPSTR', 'MULTIPLY', 'NULL', 'NULLENTRY', 'OFFSET', 'OFFSETADD',
        'OFFSETDIFF', 'OFFSETSUBTRACT', 'OFFSETVALUE', 'OMITTED', 'ONACTUAL', 'ONAREA', 'ONCHAR',
        'ONCODE', 'ONCONDCOND', 'ONCONDID', 'ONCOUNT', 'ONEXPECTED', 'ONFILE', 'ONGSOURCE',
        'ONHBOUND', 'ONJSONNAME', 'ONKEY', 'ONLBOUND', 'ONLINE', 'ONLOC', 'ONOFFSET', 'ONOPERATOR',
        'ONPACKAGE', 'ONPROCEDURE', 'ONSOURCE', 'ONSUBCODE', 'ONSUBCODE2', 'ONSUBSCRIPT', 'ONTEXT',
        'ONUCHAR', 'ONUSOURCE', 'ONWCHAR', 'ONWSOURCE', 'ORDINALNAME', 'ORDINALPRED', 'ORDINALSUCC',
        'PACKAGENAME', 'PAGENO', 'PICSPEC', 'PLACES', 'PLIASCII', 'PLIATTN', 'PLICANC', 'PLICKPT',
        'PLIDELETE', 'PLIDUMP', 'PLIEBCDIC', 'PLIFILL', 'PLIFREE', 'PLIMOVE', 'PLIOVER', 'PLIPARSE',
        'PLIREST', 'PLIRETC', 'PLIRETV', 'PLISAXA', 'PLISAXB', 'PLISAXC', 'PLISAXD', 'PLISRTA',
        'PLISRTB', 'PLISRTC', 'PLISRTD', 'PLISTCK', 'PLISTCKE', 'PLISTCKELOCAL', 'PLISTCKEUTC',
        'PLISTCKF', 'PLISTCKLOCAL', 'PLISTCKP', 'PLISTCKPLOCAL', 'PLISTCKPUTC', 'PLISTCKUTC',
        'PLITRAN11', 'PLITRAN12', 'PLITRAN21', 'PLITRAN22', 'POINTER', 'POINTERADD', 'POINTERDIFF',
        'POINTERSUBTRACT', 'POINTERVALUE', 'POLY', 'POPCNT', 'PRECISION', 'PRECVAL', 'PRED',
        'PRESENT', 'PROCEDURENAME', 'PROD', 'PUTENV', 'QUICKSORT', 'QUICKSORTX', 'RADIX', 'RAISE2',
        'RANDOM', 'RANK', 'REAL', 'REGEX', 'REM', 'REPATTERN', 'REPEAT', 'REPLACE', 'REPLACEBY2',
        'REVERSE', 'RIGHT', 'ROUND', 'ROUNDAWAYFROMZERO', 'ROUNDTOEVEN', 'SAMEKEY', 'SCALE',
        'SCALEVAL', 'SCRUBOUT', 'SEARCH', 'SEARCHR', 'SECS', 'SECSTODATE', 'SECSTODAYS',
        'SHA1DIGEST', 'SHA1FINAL', 'SHA1INIT', 'SHA1UPDATE', 'SHA2DIGEST224', 'SHA2DIGEST256',
        'SHA2DIGEST384', 'SHA2DIGEST512', 'SHA2FINAL224', 'SHA2FINAL256', 'SHA2FINAL384',
        'SHA2FINAL512', 'SHA2INIT224', 'SHA2INIT256', 'SHA2INIT384', 'SHA2INIT512', 'SHA2UPDATE224',
        'SHA2UPDATE256', 'SHA2UPDATE384', 'SHA2UPDATE512', 'SHA3DIGEST224', 'SHA3DIGEST256',
        'SHA3DIGEST384', 'SHA3DIGEST512', 'SHA3FINAL224', 'SHA3FINAL256', 'SHA3FINAL384',
        'SHA3FINAL512', 'SHA3INIT224', 'SHA3INIT256', 'SHA3INIT384', 'SHA3INIT512', 'SHA3UPDATE224',
        'SHA3UPDATE256', 'SHA3UPDATE384', 'SHA3UPDATE512', 'SIGN', 'SIGNED', 'SIN', 'SIND', 'SINH',
        'SIZE', 'SMFTOJULIAN', 'SOURCEFILE', 'SOURCELINE', 'SQRT', 'SQRTF', 'SQUEEZE', 'STACKADDR',
        'STCKETODATE', 'STCKTODATE', 'STORAGE', 'STRING', 'SUBSTR', 'SUBTO', 'SUBTRACT', 'SUCC',
        'SUM', 'SYSNULL', 'SYSTEM', 'TALLY', 'TAN', 'TAND', 'TANH', 'THREADID', 'TIME', 'TIMESTAMP',
        'TINY', 'TRANSLATE', 'TRIM', 'TRUNC', 'TYPE', 'UHIGH', 'ULENGTH', 'ULENGTH8', 'ULENGTH16',
        'ULOW', 'UNALLOCATED', 'UNHEX', 'UNSIGNED', 'UNSPEC', 'UPOS', 'UPPERASCII', 'UPPERCASE',
        'UPPERLATIN1', 'USUBSTR', 'USUPPLEMENTARY', 'UTCDATETIME', 'UTCMICROSECS', 'UTCSECS',
        'UTF8', 'UTF8STG', 'UTF8TOCHAR', 'UTF8TOWCHAR', 'UUID', 'UUID4', 'UVALID', 'UWIDTH',
        'VALID', 'VALIDDATE', 'VALIDVALUE', 'VARGLIST', 'VARGSIZE', 'VERIFY', 'VERIFYR', 'WCHARVAL',
        'WEEKDAY', 'WHIGH', 'WHEREDIFF', 'WIDECHAR', 'WLOW', 'WSCOLLAPSE', 'WSCOLLAPSE16',
        'WSREPLACE', 'WSREPLACE16', 'XMLCHAR', 'XMLSCRUB', 'XMLSCRUB16', 'XMLUCHAR', 'Y4DATE',
        'Y4JULIAN', 'Y4YEAR',],
    # preprocessor statements, directives and built-in functions of the Enterprise PL/I
    # 6.1 Language Reference ("Preprocessor facilities"), checked 2026-11-12
    'Preprocessor': ['%ACTIVATE', '%DEACTIVATE', '%DECLARE', '%DO', '%END', '%GO TO', '%IF', '%INCLUDE', '%INSCAN', '%ITERATE', '%LEAVE', '%NOTE', '%REPLACE', '%SELECT', '%XINCLUDE', '%XINSCAN', '%PROCEDURE', 'RETURN (preprocessor)', 'ANSWER (preprocessor)', 'CALL (preprocessor)', '%LINE', '%NOPRINT', '%PAGE', '%POP', '%PRINT', '%PROCESS', '%PROCINC', '%PUSH', '%SKIP', 'STATEMENT'],
    'Preprocessor built-in functions': ['COLLATE', 'COMMENT', 'COMPILEDATE', 'COMPILETIME', 'COPY', 'COUNTER', 'DIMENSION', 'HBOUND', 'INDEX', 'LBOUND', 'LENGTH', 'LOWERCASE', 'MACCOL', 'MACLMAR', 'MACNAME', 'MACRMAR', 'MAX', 'MIN', 'PARMSET', 'QUOTE', 'REPEAT', 'SUBSTR', 'SYSDIMSIZE', 'SYSOFFSETSIZE', 'SYSPARM', 'SYSPOINTERSIZE', 'SYSTEM', 'SYSVERSION', 'TRANSLATE', 'TRIM', 'UPPERCASE', 'VERIFY'],

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
    # CENTER/CENTRE are the short forms of CENTERLEFT/CENTRELEFT
    'CENTERLEFT': r'\b(CENTERLEFT|CENTRELEFT|CENTER|CENTRE)\s*\(',
    'CENTRELEFT': r'\b(CENTERLEFT|CENTRELEFT|CENTER|CENTRE)\s*\(',
    # abbreviations the Language Reference allows
    'DECLARE': r'\bDECLARE\b|\bDCL\b', 'PROCEDURE': r'\bPROC(EDURE)?\b',
    'STORAGE': r'\b(STORAGE|STG)\s*\(', 'CURRENTSTORAGE': r'\b(CURRENTSTORAGE|CSTG)\s*\(',
}
for p in ['%ACTIVATE', '%DEACTIVATE', '%DECLARE', '%DO', '%END', '%IF', '%INCLUDE', '%XINCLUDE',
          '%INSCAN', '%XINSCAN', '%ITERATE', '%LEAVE', '%NOTE', '%PAGE', '%POP', '%PRINT', '%PROCEDURE',
          '%PUSH', '%REPLACE', '%SELECT', '%SKIP', '%LINE', '%NOPRINT',
          '%PROCESS', '%PROCINC']:
    word = p[1:]
    alt = {'DECLARE': 'DECLARE|DCL', 'PROCEDURE': 'PROCEDURE|PROC', 'ACTIVATE': 'ACTIVATE|ACT',
           'DEACTIVATE': 'DEACTIVATE|DEACT'}.get(word, word)
    PLI_PATTERNS[p] = r'%\s*([A-Z0-9_]+\s*:\s*)?(' + alt + r')\b'
PLI_PATTERNS['%GO TO'] = r'%\s*GO\s*TO\b'
# the RETURN, ANSWER and CALL statements of a preprocessor procedure have no
# leading % (Language Reference, "Preprocessor procedures")
for w, alt in [('RETURN', 'RETURN'), ('ANSWER', 'ANSWER|ANS'), ('CALL', 'CALL')]:
    PLI_PATTERNS[w + ' (preprocessor)'] = r'%\s*[A-Z0-9_]+\s*:\s*PROC[^%]*\b(' + alt + r')\b'


PP_NOARGS = {'COLLATE', 'COMPILEDATE', 'COMPILETIME', 'COUNTER', 'MACCOL', 'MACLMAR', 'MACNAME',
             'MACRMAR', 'SYSDIMSIZE', 'SYSOFFSETSIZE', 'SYSPARM', 'SYSPOINTERSIZE', 'SYSTEM',
             'SYSVERSION'}


def pli_pattern(category, name):
    if category == 'Preprocessor built-in functions':
        # in a %statement, or in the body of a preprocessor procedure; the
        # functions without arguments are written without parentheses
        word = r'\b' + re.escape(name) + (r'\b' if name in PP_NOARGS else r'\s*\(')
        return r'(^[ \t]*%[^\n]*|%\s*[A-Z0-9_]+\s*:\s*PROC[^%]*)' + word
    if name in PLI_PATTERNS:
        return PLI_PATTERNS[name]
    if category == 'Built-in functions':
        return r'\b' + re.escape(name) + r'\s*\('
    return r'\b' + r'\s+'.join(re.escape(w) for w in name.split()) + r'\b'


def pli_text(path):
    with open(path, encoding='latin-1') as f:
        text = f.read()
    return re.sub(r'/\*.*?\*/', ' ', text, flags=re.S).upper()


# --------------------------------------------------------------------------- HLASM
HLASM_ASM = [
    '*PROCESS', 'ACONTROL', 'ADATA', 'AINSERT', 'ALIAS', 'AMODE', 'CATTR', 'CCW', 'CCW0', 'CCW1', 'CEJECT',
    'CNOP', 'COM', 'COPY', 'CSECT', 'CXD', 'DC', 'DROP', 'DS', 'DSECT', 'DXD', 'EJECT', 'END',
    'ENTRY', 'EQU', 'EXITCTL', 'EXTRN', 'ICTL', 'ISEQ', 'LOCTR', 'LTORG', 'MNOTE', 'OPSYN', 'ORG',
    'POP', 'PRINT', 'PUNCH', 'PUSH', 'REPRO', 'RMODE', 'RSECT', 'SPACE', 'START', 'TITLE', 'USING',
    'WXTRN', 'XATTR']
HLASM_DC = ['A', 'AD', 'B', 'C', 'CA', 'CE', 'CU', 'D', 'DB', 'DD', 'DH', 'E', 'EB', 'ED', 'EH',
            'F', 'FD', 'G', 'H', 'J', 'JD', 'L', 'LB', 'LD', 'LH', 'LQ', 'P', 'Q', 'QD', 'QY', 'R',
            'RD', 'S', 'SY', 'V', 'VD', 'X', 'Y', 'Z']
HLASM_MACRO = [
    # instructions, attributes, functions (Table 56) and system variable symbols of the
    # HLASM V1R6 Language Reference (SC26-4940-09), checked 2026-11-12
    'ACTR', 'AEJECT', 'AGO', 'AIF', 'ANOP', 'AREAD', 'ASPACE', 'GBLA', 'GBLB', 'GBLC', 'LCLA',
    'LCLB', 'LCLC', 'MACRO', 'MEND', 'MEXIT', 'MHELP', 'SETA', 'SETB', 'SETC', 'SETAF', 'SETCF',
    '"T\'"', '"L\'"', '"S\'"', '"I\'"', '"K\'"', '"N\'"', '"D\'"', '"O\'"', 'A2B', 'A2C', 'A2D',
    'A2X', 'B2A', 'B2C', 'B2D', 'B2X', 'BYTE', 'C2A', 'C2B', 'C2D', 'C2X', 'D2A', 'D2B', 'D2C',
    'D2X', 'DCLEN', 'DCVAL', 'DEQUOTE', 'DOUBLE', 'FIND', 'INDEX', 'ISBIN', 'ISDEC', 'ISHEX',
    'ISSYM', 'LOWER', 'SIGNED', 'SLA', 'SLL', 'SRA', 'SRL', 'UPPER', 'X2A', 'X2B', 'X2C', 'X2D',
    'AND', 'OR', 'XOR', 'NOT', '&SYSADATA_DSN', '&SYSADATA_MEMBER', '&SYSADATA_VOLUME', '&SYSASM',
    '&SYSCLOCK', '&SYSDATC', '&SYSDATE', '&SYSECT', '&SYSIN_DSN', '&SYSIN_MEMBER', '&SYSIN_VOLUME',
    '&SYSJOB', '&SYSLIB_DSN', '&SYSLIB_MEMBER', '&SYSLIB_VOLUME', '&SYSLIN_DSN', '&SYSLIN_MEMBER',
    '&SYSLIN_VOLUME', '&SYSLIST', '&SYSLOC', '&SYSMAC', '&SYSM_HSEV', '&SYSM_SEV', '&SYSNDX',
    '&SYSNEST', '&SYSOPT_DBCS', '&SYSOPT_OPTABLE', '&SYSOPT_RENT', '&SYSOPT_XOBJECT', '&SYSPARM',
    '&SYSPRINT_DSN', '&SYSPRINT_MEMBER', '&SYSPRINT_VOLUME', '&SYSPUNCH_DSN', '&SYSPUNCH_MEMBER',
    '&SYSPUNCH_VOLUME', '&SYSSEQF', '&SYSSTEP', '&SYSSTMT', '&SYSSTYP', '&SYSTEM_ID',
    '&SYSTERM_DSN', '&SYSTERM_MEMBER', '&SYSTERM_VOLUME', '&SYSTIME', '&SYSVER', 'ESYM', 'SYSATTRA',
    'SYSATTRP',]


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
            if (line.startswith('*') and not line.upper().startswith('*PROCESS')) \
                    or line.startswith('.*'):
                continue
            lines.append(line[:71].upper())
    return '\n'.join(lines)


def hlasm_pattern(category, name):
    if name == '*PROCESS':
        return r'^\*PROCESS\s'
    if category == 'Machine instructions' or category == 'Assembler instructions':
        return r'^\S*\s+' + re.escape(name) + r'(\s|$)'
    if category == 'DC types':
        # nominal values are quoted; address constants (A, V, S, Y, Q, R, J ...) are
        # parenthesized
        return r'^\S*\s+D[CS]\s+(\S*,)?[0-9()]*' + re.escape(name) + r"(L[0-9.]+)?['(]"
    if name.startswith('&') or name.endswith("'"):
        return re.escape(name)
    if name in ('AND', 'OR', 'XOR', 'SLA', 'SLL', 'SRA', 'SRL'):
        # a binary operator of a conditional-assembly expression, (1 SLL 4),
        # on a SETx or AIF statement (SRA alone is also a machine instruction)
        return r'^\S*\s+(SET[ABC]|AIF)\s.*[\w)]\s+' + name + r'\s'
    if name == 'NOT':
        return r'\(\s*NOT\s|\(\s*[^)]*\sNOT\s'
    if name in ('INDEX', 'FIND', 'BYTE', 'UPPER', 'LOWER', 'DOUBLE', 'SIGNED') or name[1:2] == '2' or \
            name.startswith('IS') or name in ('DCLEN', 'DCVAL', 'DEQUOTE'):
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
    totals = {'used': 0, 'ccvs85': 0, 'excluded': 0, 'missing': 0}
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
            mark = marks.get((category, name.replace('|', '\\|')), '')
            if state == '未網羅' and mark.startswith('対象外'):
                # a reviewed exclusion (the reason is in the review column)
                state = '対象外'
            totals[{'使用あり': 'used', 'CCVS85': 'ccvs85', '未網羅': 'missing',
                    '対象外': 'excluded'}[state]] += 1
            lines.append(f'| {category} | {name.replace("|", chr(92) + "|")} | {state} | {shown} | {c} | {mark} |')
    lines[2] = note + (f' 要素 {sum(totals.values())}、使用あり {totals["used"]}、'
                       + (f'CCVS85 だけが測る {totals["ccvs85"]}、' if ccvs is not None else '')
                       + (f'対象外 {totals["excluded"]}、' if totals['excluded'] else '')
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
            extra += f', excluded {t["excluded"]}' if t["excluded"] else ''
            print(f'{name}: used {t["used"]}{extra}, missing {t["missing"]}')


if __name__ == '__main__':
    sys.exit(main())

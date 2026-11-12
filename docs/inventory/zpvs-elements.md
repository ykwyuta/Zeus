# ZPVS 網羅台帳 (Enterprise PL/I for z/OS)

Enterprise PL/I for z/OS Language Reference の章立てから列挙した。 要素 411、使用あり 212、未網羅 199。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |
| --- | --- | --- | --- | --- | --- |
| Statements | ALLOCATE | 使用あり | PE113N, PE113P, PE114N, PE114P, PS101A, PS102A ほか 2 | - |  |
| Statements | ASSERT | 未網羅 |  | - |  |
| Statements | ATTACH | 未網羅 |  | - |  |
| Statements | BEGIN | 使用あり | PC104A, PC106A, PC107A, PC108A, PF104M | - |  |
| Statements | CALL | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 38 | - |  |
| Statements | CLOSE | 使用あり | PF102A, PF103A, PF104M, PF105A, PF106A | - |  |
| Statements | DECLARE | 未網羅 |  | - |  |
| Statements | DEFAULT | 使用あり | PD105A, PD108A, PF106A | - |  |
| Statements | DEFINE ALIAS | 使用あり | PD104A | - |  |
| Statements | DEFINE ORDINAL | 使用あり | PD105A | - |  |
| Statements | DEFINE STRUCTURE | 使用あり | PD104A, PS106A | - |  |
| Statements | DELAY | 未網羅 |  | - |  |
| Statements | DELETE | 使用あり | PF104M | - |  |
| Statements | DETACH | 未網羅 |  | - |  |
| Statements | DISPLAY | 未網羅 |  | - |  |
| Statements | DO | 使用あり | PC101A, PE108P, PE109N, PE109P, PM101A, PS102A ほか 1 | - |  |
| Statements | DO WHILE | 使用あり | PC101A | - |  |
| Statements | DO UNTIL | 使用あり | PC101A | - |  |
| Statements | DO LOOP | 未網羅 |  | - |  |
| Statements | DO REPEAT | 使用あり | PC108A, PM104M | - |  |
| Statements | END | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 67 | - |  |
| Statements | ENTRY | 使用あり | PC103A, PC104A, PM102A, PM104M, PM106A | - |  |
| Statements | EXIT | 未網羅 |  | - |  |
| Statements | FETCH | 使用あり | PC104A | - |  |
| Statements | FLUSH | 未網羅 |  | - |  |
| Statements | FORMAT | 未網羅 |  | - |  |
| Statements | FREE | 使用あり | PS101A, PS102A, PS106A | - |  |
| Statements | GET | 使用あり | PF101A | - |  |
| Statements | GO TO | 使用あり | PC104A, PC106A | - |  |
| Statements | IF | 使用あり | PC101A, PC102A, PM101A | - |  |
| Statements | ITERATE | 使用あり | PC101A, PE109N, PE109P | - |  |
| Statements | LEAVE | 使用あり | PC101A, PE108N, PE108P | - |  |
| Statements | LOCATE | 使用あり | PE111N, PE111P | - |  |
| Statements | ON | 使用あり | PC104A, PC105A, PC106A, PC107A, PC108A, PF102A ほか 4 | - |  |
| Statements | OPEN | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 36 | - |  |
| Statements | PACKAGE | 未網羅 |  | - |  |
| Statements | PROCEDURE | 未網羅 |  | - |  |
| Statements | PUT | 使用あり | PF101A, PF102A, PF106A | - |  |
| Statements | READ | 使用あり | PF103A, PF104M, PF105A, PF106A | - |  |
| Statements | RELEASE | 使用あり | PC104A | - |  |
| Statements | RESIGNAL | 未網羅 |  | - |  |
| Statements | RETURN | 使用あり | PC102A, PC103A, PC109A, PE110N, PE110P, PM102A ほか 1 | - |  |
| Statements | REVERT | 使用あり | PC104A, PC105A | - |  |
| Statements | REWRITE | 使用あり | PF104M | - |  |
| Statements | SELECT | 使用あり | PC101A | - |  |
| Statements | SIGNAL | 使用あり | PC105A | - |  |
| Statements | STOP | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 36 | - |  |
| Statements | WAIT | 未網羅 |  | - |  |
| Statements | WRITE | 使用あり | PF103A, PF104M, PF105A, PF106A, PS104A, PS105A | - |  |
| Data attributes | ALIGNED | 使用あり | PD103A | - |  |
| Data attributes | AREA | 使用あり | PE114P, PE115P, PS103A | - |  |
| Data attributes | AUTOMATIC | 使用あり | PC109A | - |  |
| Data attributes | BASED | 使用あり | PE106P, PE107N, PE107P, PE111P, PE114N, PE114P ほか 3 | - |  |
| Data attributes | BIGENDIAN | 未網羅 |  | - |  |
| Data attributes | BINARY | 使用あり | PA101A, PA102A, PA104A, PC101A, PC102A, PC103A ほか 39 | - |  |
| Data attributes | BIT | 使用あり | PA102A, PD102A, PF103A, PF104M, PF105A | - |  |
| Data attributes | BUILTIN | 使用あり | PD108A | - |  |
| Data attributes | BYADDR | 未網羅 |  | - |  |
| Data attributes | BYVALUE | 未網羅 |  | - |  |
| Data attributes | CHARACTER | 使用あり | PA102A, PA104A, PC103A, PC107A, PC108A, PD102A ほか 26 | - |  |
| Data attributes | COMPLEX | 使用あり | PD106A, PE103N, PE103P | - |  |
| Data attributes | CONDITION | 使用あり | PC105A | - |  |
| Data attributes | CONNECTED | 未網羅 |  | - |  |
| Data attributes | CONTROLLED | 使用あり | PE113P, PS102A | - |  |
| Data attributes | DATE | 使用あり | PM105A | - |  |
| Data attributes | DECIMAL | 使用あり | PA101A, PA102A, PA105A, PC102A, PC106A, PC107A ほか 10 | - |  |
| Data attributes | DEFINED | 使用あり | PS105A | - |  |
| Data attributes | DIMENSION | 使用あり | PA104A, PS102A, PS104A | - |  |
| Data attributes | ENTRY | 使用あり | PC103A, PC104A, PM102A, PM104M, PM106A | - |  |
| Data attributes | EXTERNAL | 使用あり | PC104A | - |  |
| Data attributes | FILE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 38 | - |  |
| Data attributes | FIXED | 使用あり | PA101A, PA102A, PA104A, PA105A, PC101A, PC102A ほか 45 | - |  |
| Data attributes | FLOAT | 使用あり | PA103A, PD106A | - |  |
| Data attributes | FORMAT | 未網羅 |  | - |  |
| Data attributes | GENERIC | 使用あり | PC103A, PE112N, PE112P | - |  |
| Data attributes | GRAPHIC | 使用あり | PD106A | - |  |
| Data attributes | HANDLE | 使用あり | PS106A | - |  |
| Data attributes | HEXADEC | 未網羅 |  | - |  |
| Data attributes | IEEE | 未網羅 |  | - |  |
| Data attributes | INITIAL | 使用あり | PA104A, PC102A, PC105A, PC106A, PC107A, PC108A ほか 13 | - |  |
| Data attributes | INTERNAL | 未網羅 |  | - |  |
| Data attributes | LABEL | 使用あり | PC101A | - |  |
| Data attributes | LIKE | 使用あり | PD104A, PE104N, PE104P, PE105N, PE105P | - |  |
| Data attributes | LIMITED | 未網羅 |  | - |  |
| Data attributes | LITTLEENDIAN | 未網羅 |  | - |  |
| Data attributes | NONASSIGNABLE | 未網羅 |  | - |  |
| Data attributes | NONVARYING | 未網羅 |  | - |  |
| Data attributes | OFFSET | 使用あり | PE115N, PE115P, PS103A | - |  |
| Data attributes | OPTIONAL | 未網羅 |  | - |  |
| Data attributes | OPTIONS | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 67 | - |  |
| Data attributes | ORDINAL | 使用あり | PD105A | - |  |
| Data attributes | PARAMETER | 未網羅 |  | - |  |
| Data attributes | PICTURE | 使用あり | PA103A, PA105A | - |  |
| Data attributes | POINTER | 使用あり | PS101A, PS103A, PS104A | - |  |
| Data attributes | POSITION | 使用あり | PS105A | - |  |
| Data attributes | PRECISION | 未網羅 |  | - |  |
| Data attributes | REAL | 使用あり | PD106A | - |  |
| Data attributes | REFER | 使用あり | PE106N, PE106P, PE107N, PE107P, PS104A | - |  |
| Data attributes | RESERVED | 未網羅 |  | - |  |
| Data attributes | RETURNS | 使用あり | PC102A, PC103A, PC109A, PM102A, PM106A | - |  |
| Data attributes | SIGNED | 未網羅 |  | - |  |
| Data attributes | STATIC | 使用あり | PC104A, PC109A, PCS01A | - |  |
| Data attributes | TYPE | 使用あり | PD104A, PD105A, PS106A | - |  |
| Data attributes | UNALIGNED | 使用あり | PD102A, PD103A | - |  |
| Data attributes | UNION | 未網羅 |  | - |  |
| Data attributes | UNSIGNED | 使用あり | PD107A | - |  |
| Data attributes | VALUE | 使用あり | PA105A, PD105A, PD108A, PM106A, PM107A, PM108A ほか 1 | - |  |
| Data attributes | VARIABLE | 使用あり | PC103A, PD108A | - |  |
| Data attributes | VARYING | 使用あり | PD102A, PM107A, PM108A | - |  |
| Data attributes | VARYINGZ | 未網羅 |  | - |  |
| Data attributes | WIDECHAR | 使用あり | PD106A | - |  |
| File attributes and options | BUFFERED | 未網羅 |  | - |  |
| File attributes and options | DIRECT | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | ENVIRONMENT | 使用あり | PF102A, PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | INPUT | 使用あり | PC107A, PC108A, PF103A, PF105A | - |  |
| File attributes and options | KEYED | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | OUTPUT | 使用あり | PE111N, PE111P, PF102A, PF103A, PF104M, PF105A ほか 1 | - |  |
| File attributes and options | PRINT | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | RECORD | 使用あり | PE111N, PE111P, PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | SEQUENTIAL | 未網羅 |  | - |  |
| File attributes and options | STREAM | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | UPDATE | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | TITLE | 使用あり | PC104A, PF102A, PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | LINESIZE | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | PAGESIZE | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | KEY | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | KEYFROM | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | KEYTO | 未網羅 |  | - |  |
| File attributes and options | SET | 使用あり | PS103A | - |  |
| File attributes and options | INTO | 使用あり | PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | FROM | 使用あり | PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | PAGE | 使用あり | PF102A | - |  |
| File attributes and options | SKIP | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | LINE | 使用あり | PF102A | - |  |
| File attributes and options | LIST | 使用あり | PF101A | - |  |
| File attributes and options | EDIT | 使用あり | PF101A, PF102A, PF106A, PS104A | - |  |
| File attributes and options | DATA | 使用あり | PF101A, PS101A, PS103A | - |  |
| File attributes and options | STRING | 使用あり | PF101A | - |  |
| File attributes and options | COPY | 使用あり | PC103A, PD104A, PM103A, PS103A | - |  |
| Conditions | AREA | 使用あり | PE114P, PE115P, PS103A | - |  |
| Conditions | ATTENTION | 未網羅 |  | - |  |
| Conditions | CONDITION | 使用あり | PC105A | - |  |
| Conditions | CONVERSION | 使用あり | PC107A, PC108A | - |  |
| Conditions | ENDFILE | 使用あり | PF103A, PF105A | - |  |
| Conditions | ENDPAGE | 使用あり | PF102A, PF106A | - |  |
| Conditions | ERROR | 使用あり | PC104A | - |  |
| Conditions | FINISH | 未網羅 |  | - |  |
| Conditions | FIXEDOVERFLOW | 未網羅 |  | - |  |
| Conditions | INVALIDOP | 未網羅 |  | - |  |
| Conditions | KEY | 使用あり | PF104M, PF106A | - |  |
| Conditions | NAME | 使用あり | PA104A, PF101A, PM106A | - |  |
| Conditions | OVERFLOW | 未網羅 |  | - |  |
| Conditions | RECORD | 使用あり | PE111N, PE111P, PF103A, PF104M, PF105A, PF106A | - |  |
| Conditions | SIZE | 使用あり | PC106A, PD103A | - |  |
| Conditions | STORAGE | 使用あり | PS101A | - |  |
| Conditions | STRINGRANGE | 未網羅 |  | - |  |
| Conditions | STRINGSIZE | 未網羅 |  | - |  |
| Conditions | SUBSCRIPTRANGE | 未網羅 |  | - |  |
| Conditions | TRANSMIT | 未網羅 |  | - |  |
| Conditions | UNDEFINEDFILE | 未網羅 |  | - |  |
| Conditions | UNDERFLOW | 未網羅 |  | - |  |
| Conditions | ZERODIVIDE | 使用あり | PC106A | - |  |
| Built-in functions | ABS | 使用あり | PD106A, PM105A | - |  |
| Built-in functions | ACOS | 未網羅 |  | - |  |
| Built-in functions | ADD | 使用あり | PA101A | - |  |
| Built-in functions | ADDR | 使用あり | PS101A | - |  |
| Built-in functions | ADDRDATA | 未網羅 |  | - |  |
| Built-in functions | ALL | 未網羅 |  | - |  |
| Built-in functions | ALLOCATION | 使用あり | PS102A | - |  |
| Built-in functions | ALLOCSIZE | 未網羅 |  | - |  |
| Built-in functions | ANY | 未網羅 |  | - |  |
| Built-in functions | ASIN | 未網羅 |  | - |  |
| Built-in functions | ATAN | 未網羅 |  | - |  |
| Built-in functions | ATAND | 未網羅 |  | - |  |
| Built-in functions | ATANH | 未網羅 |  | - |  |
| Built-in functions | BINARY | 使用あり | PA101A, PA102A, PA104A, PC101A, PC102A, PC103A ほか 39 | - |  |
| Built-in functions | BINARYVALUE | 未網羅 |  | - |  |
| Built-in functions | BINVALUE | 使用あり | PD105A | - |  |
| Built-in functions | BIT | 使用あり | PA102A, PD102A, PF103A, PF104M, PF105A | - |  |
| Built-in functions | BITLOCATION | 未網羅 |  | - |  |
| Built-in functions | BOOL | 使用あり | PA102A | - |  |
| Built-in functions | BYTE | 未網羅 |  | - |  |
| Built-in functions | CDS | 未網羅 |  | - |  |
| Built-in functions | CEIL | 使用あり | PM103A | - |  |
| Built-in functions | CENTER | 使用あり | PM103A | - |  |
| Built-in functions | CENTRE | 未網羅 |  | - |  |
| Built-in functions | CHARACTER | 使用あり | PA102A, PA104A, PC103A, PC107A, PC108A, PD102A ほか 26 | - |  |
| Built-in functions | CHARGRAPHIC | 未網羅 |  | - |  |
| Built-in functions | CHARVAL | 未網羅 |  | - |  |
| Built-in functions | COLLATE | 未網羅 |  | - |  |
| Built-in functions | COMPARE | 未網羅 |  | - |  |
| Built-in functions | COMPLEX | 未網羅 |  | - |  |
| Built-in functions | CONJG | 使用あり | PD106A | - |  |
| Built-in functions | COPY | 使用あり | PM103A | - |  |
| Built-in functions | COS | 使用あり | PM105A | - |  |
| Built-in functions | COSD | 未網羅 |  | - |  |
| Built-in functions | COSH | 未網羅 |  | - |  |
| Built-in functions | COUNT | 未網羅 |  | - |  |
| Built-in functions | CS | 未網羅 |  | - |  |
| Built-in functions | CURRENTSIZE | 使用あり | PS104A | - |  |
| Built-in functions | CURRENTSTORAGE | 未網羅 |  | - |  |
| Built-in functions | DATAFIELD | 未網羅 |  | - |  |
| Built-in functions | DATE | 未網羅 |  | - |  |
| Built-in functions | DATETIME | 未網羅 |  | - |  |
| Built-in functions | DAYS | 使用あり | PM105A | - |  |
| Built-in functions | DAYSTODATE | 使用あり | PM105A | - |  |
| Built-in functions | DAYSTOSECS | 未網羅 |  | - |  |
| Built-in functions | DECIMAL | 使用あり | PA101A, PA102A, PA105A, PC102A, PC106A, PC107A ほか 10 | - |  |
| Built-in functions | DIMENSION | 使用あり | PA104A, PS102A, PS104A | - |  |
| Built-in functions | DIVIDE | 使用あり | PA101A | - |  |
| Built-in functions | EDIT | 使用あり | PF101A, PF102A, PF106A | - |  |
| Built-in functions | EMPTY | 使用あり | PS103A | - |  |
| Built-in functions | ENDFILE | 使用あり | PF103A, PF105A | - |  |
| Built-in functions | ENTRYADDR | 未網羅 |  | - |  |
| Built-in functions | EPSILON | 未網羅 |  | - |  |
| Built-in functions | ERF | 未網羅 |  | - |  |
| Built-in functions | ERFC | 未網羅 |  | - |  |
| Built-in functions | EXP | 未網羅 |  | - |  |
| Built-in functions | EXPONENT | 未網羅 |  | - |  |
| Built-in functions | FIXED | 使用あり | PD108A | - |  |
| Built-in functions | FLOAT | 未網羅 |  | - |  |
| Built-in functions | FLOOR | 使用あり | PM103A | - |  |
| Built-in functions | GAMMA | 未網羅 |  | - |  |
| Built-in functions | GETENV | 未網羅 |  | - |  |
| Built-in functions | GRAPHIC | 使用あり | PD106A | - |  |
| Built-in functions | HANDLE | 使用あり | PS106A | - |  |
| Built-in functions | HBOUND | 未網羅 |  | - |  |
| Built-in functions | HEX | 使用あり | PA103A, PA105A, PD101A, PD102A, PD103A, PD106A ほか 2 | - |  |
| Built-in functions | HEXIMAGE | 未網羅 |  | - |  |
| Built-in functions | HIGH | 未網羅 |  | - |  |
| Built-in functions | HUGE | 未網羅 |  | - |  |
| Built-in functions | IAND | 未網羅 |  | - |  |
| Built-in functions | IEOR | 未網羅 |  | - |  |
| Built-in functions | IMAG | 使用あり | PD106A | - |  |
| Built-in functions | INDEX | 使用あり | PF101A, PM103A | - |  |
| Built-in functions | INOT | 未網羅 |  | - |  |
| Built-in functions | IOR | 未網羅 |  | - |  |
| Built-in functions | ISIGNED | 未網羅 |  | - |  |
| Built-in functions | ISLL | 未網羅 |  | - |  |
| Built-in functions | ISRL | 未網羅 |  | - |  |
| Built-in functions | IUNSIGNED | 未網羅 |  | - |  |
| Built-in functions | JULIANTOSECS | 未網羅 |  | - |  |
| Built-in functions | LBOUND | 未網羅 |  | - |  |
| Built-in functions | LEFT | 使用あり | PM103A | - |  |
| Built-in functions | LENGTH | 使用あり | PC103A, PD102A, PM103A, PM107A, PM108A | - |  |
| Built-in functions | LINENO | 使用あり | PF102A, PF106A | - |  |
| Built-in functions | LOCATION | 未網羅 |  | - |  |
| Built-in functions | LOG | 使用あり | PM105A | - |  |
| Built-in functions | LOG10 | 未網羅 |  | - |  |
| Built-in functions | LOG2 | 未網羅 |  | - |  |
| Built-in functions | LOGGAMMA | 未網羅 |  | - |  |
| Built-in functions | LOW | 未網羅 |  | - |  |
| Built-in functions | LOWERCASE | 使用あり | PM103A | - |  |
| Built-in functions | LOWER2 | 未網羅 |  | - |  |
| Built-in functions | MAX | 使用あり | PM105A | - |  |
| Built-in functions | MAXEXP | 未網羅 |  | - |  |
| Built-in functions | MAXLENGTH | 使用あり | PD102A | - |  |
| Built-in functions | MIN | 使用あり | PM105A | - |  |
| Built-in functions | MINEXP | 未網羅 |  | - |  |
| Built-in functions | MOD | 使用あり | PC101A, PM103A | - |  |
| Built-in functions | MULTIPLY | 使用あり | PA101A | - |  |
| Built-in functions | NEW | 使用あり | PS106A | - |  |
| Built-in functions | NULL | 使用あり | PS101A | - |  |
| Built-in functions | OFFSET | 使用あり | PE115N, PE115P, PS103A | - |  |
| Built-in functions | OFFSETADD | 未網羅 |  | - |  |
| Built-in functions | OFFSETDIFF | 未網羅 |  | - |  |
| Built-in functions | OFFSETSUBTRACT | 未網羅 |  | - |  |
| Built-in functions | OFFSETVALUE | 未網羅 |  | - |  |
| Built-in functions | OMITTED | 未網羅 |  | - |  |
| Built-in functions | ONCHAR | 使用あり | PC107A | - |  |
| Built-in functions | ONCODE | 使用あり | PC104A, PC106A, PF104M, PF106A | - |  |
| Built-in functions | ONCONDCOND | 未網羅 |  | - |  |
| Built-in functions | ONCONDID | 未網羅 |  | - |  |
| Built-in functions | ONCOUNT | 未網羅 |  | - |  |
| Built-in functions | ONFILE | 未網羅 |  | - |  |
| Built-in functions | ONKEY | 未網羅 |  | - |  |
| Built-in functions | ONLOC | 未網羅 |  | - |  |
| Built-in functions | ONSOURCE | 使用あり | PC108A | - |  |
| Built-in functions | ONWCHAR | 未網羅 |  | - |  |
| Built-in functions | ORDINALNAME | 使用あり | PD105A | - |  |
| Built-in functions | ORDINALPRED | 使用あり | PD105A | - |  |
| Built-in functions | ORDINALSUCC | 使用あり | PD105A | - |  |
| Built-in functions | PAGENO | 使用あり | PF102A, PF106A | - |  |
| Built-in functions | PLIRETV | 使用あり | PM104M, PM107A, PM108A | - |  |
| Built-in functions | POINTER | 使用あり | PS101A, PS103A, PS104A | - |  |
| Built-in functions | POINTERADD | 使用あり | PS101A | - |  |
| Built-in functions | POINTERDIFF | 未網羅 |  | - |  |
| Built-in functions | POINTERSUBTRACT | 未網羅 |  | - |  |
| Built-in functions | POINTERVALUE | 未網羅 |  | - |  |
| Built-in functions | POPCNT | 未網羅 |  | - |  |
| Built-in functions | PRECISION | 未網羅 |  | - |  |
| Built-in functions | PRESENT | 未網羅 |  | - |  |
| Built-in functions | PROD | 未網羅 |  | - |  |
| Built-in functions | RADIX | 未網羅 |  | - |  |
| Built-in functions | RAISE2 | 未網羅 |  | - |  |
| Built-in functions | RANDOM | 未網羅 |  | - |  |
| Built-in functions | RANK | 未網羅 |  | - |  |
| Built-in functions | REAL | 使用あり | PD106A | - |  |
| Built-in functions | REM | 使用あり | PM103A | - |  |
| Built-in functions | REPEAT | 未網羅 |  | - |  |
| Built-in functions | REPLACEBY2 | 未網羅 |  | - |  |
| Built-in functions | REVERSE | 使用あり | PM103A | - |  |
| Built-in functions | RIGHT | 使用あり | PM103A | - |  |
| Built-in functions | ROUND | 使用あり | PM103A | - |  |
| Built-in functions | ROUNDDEC | 未網羅 |  | - |  |
| Built-in functions | SAMEKEY | 未網羅 |  | - |  |
| Built-in functions | SCALE | 未網羅 |  | - |  |
| Built-in functions | SEARCH | 使用あり | PM103A | - |  |
| Built-in functions | SEARCHR | 未網羅 |  | - |  |
| Built-in functions | SECS | 未網羅 |  | - |  |
| Built-in functions | SECSTODATE | 未網羅 |  | - |  |
| Built-in functions | SECSTODAYS | 未網羅 |  | - |  |
| Built-in functions | SIGN | 未網羅 |  | - |  |
| Built-in functions | SIN | 使用あり | PM105A | - |  |
| Built-in functions | SIND | 未網羅 |  | - |  |
| Built-in functions | SINH | 未網羅 |  | - |  |
| Built-in functions | SIZE | 未網羅 |  | - |  |
| Built-in functions | SQRT | 使用あり | PM105A | - |  |
| Built-in functions | STG | 使用あり | PD103A, PD104A, PD105A, PD106A, PD108A, PS104A | - |  |
| Built-in functions | STORAGE | 未網羅 |  | - |  |
| Built-in functions | STRING | 使用あり | PF101A | - |  |
| Built-in functions | SUBSTR | 使用あり | PA104A, PD102A, PD108A, PF101A | - |  |
| Built-in functions | SUBTRACT | 未網羅 |  | - |  |
| Built-in functions | SUCC | 未網羅 |  | - |  |
| Built-in functions | SUM | 使用あり | PA104A, PS102A, PS105A | - |  |
| Built-in functions | SYSNULL | 使用あり | PS101A | - |  |
| Built-in functions | TALLY | 使用あり | PM103A | - |  |
| Built-in functions | TAN | 未網羅 |  | - |  |
| Built-in functions | TAND | 未網羅 |  | - |  |
| Built-in functions | TANH | 未網羅 |  | - |  |
| Built-in functions | TIME | 未網羅 |  | - |  |
| Built-in functions | TINY | 未網羅 |  | - |  |
| Built-in functions | TRANSLATE | 使用あり | PM103A | - |  |
| Built-in functions | TRIM | 使用あり | PA102A, PM103A | - |  |
| Built-in functions | TRUNC | 未網羅 |  | - |  |
| Built-in functions | TYPE | 未網羅 |  | - |  |
| Built-in functions | UNSPEC | 使用あり | PA103A, PA104A, PA105A, PD101A, PD102A, PD103A ほか 3 | - |  |
| Built-in functions | UPPERCASE | 使用あり | PM103A | - |  |
| Built-in functions | UTCDATETIME | 未網羅 |  | - |  |
| Built-in functions | VALID | 未網羅 |  | - |  |
| Built-in functions | VALIDDATE | 未網羅 |  | - |  |
| Built-in functions | VARGLIST | 未網羅 |  | - |  |
| Built-in functions | VERIFY | 使用あり | PM103A | - |  |
| Built-in functions | VERIFYR | 未網羅 |  | - |  |
| Built-in functions | WEEKDAY | 未網羅 |  | - |  |
| Built-in functions | WHITESPACECOLLAPSE | 未網羅 |  | - |  |
| Built-in functions | XMLCHAR | 未網羅 |  | - |  |
| Built-in functions | Y4DATE | 未網羅 |  | - |  |
| Built-in functions | Y4JULIAN | 未網羅 |  | - |  |
| Built-in functions | Y4YEAR | 未網羅 |  | - |  |
| Pseudovariables | ONCHAR | 使用あり | PC107A | - |  |
| Pseudovariables | ONSOURCE | 使用あり | PC108A | - |  |
| Pseudovariables | REAL | 使用あり | PD106A | - |  |
| Pseudovariables | IMAG | 使用あり | PD106A | - |  |
| Pseudovariables | STRING | 使用あり | PF101A | - |  |
| Pseudovariables | SUBSTR | 使用あり | PA104A, PD102A, PD108A, PF101A | - |  |
| Pseudovariables | UNSPEC | 使用あり | PA103A, PA104A, PA105A, PD101A, PD102A, PD103A ほか 3 | - |  |
| Pseudovariables | ENTRYADDR | 未網羅 |  | - |  |
| Built-in subroutines | PLIRETC | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 36 | - |  |
| Built-in subroutines | PLIFREE | 使用あり | PS106A | - |  |
| Built-in subroutines | PLISRTA | 未網羅 |  | - |  |
| Built-in subroutines | PLISRTB | 未網羅 |  | - |  |
| Built-in subroutines | PLISRTC | 未網羅 |  | - |  |
| Built-in subroutines | PLISRTD | 未網羅 |  | - |  |
| Built-in subroutines | PLIDUMP | 未網羅 |  | - |  |
| Built-in subroutines | PLITEST | 未網羅 |  | - |  |
| Built-in subroutines | PLICANC | 未網羅 |  | - |  |
| Built-in subroutines | PLICKPT | 未網羅 |  | - |  |
| Built-in subroutines | PLIREST | 未網羅 |  | - |  |
| Preprocessor | %ACTIVATE | 使用あり | PM102A, PM106A | - |  |
| Preprocessor | %DEACTIVATE | 使用あり | PM101A | - |  |
| Preprocessor | %DECLARE | 使用あり | PM101A, PM102A, PM106A | - |  |
| Preprocessor | %DO | 使用あり | PM101A | - |  |
| Preprocessor | %END | 使用あり | PM101A, PM102A, PM106A | - |  |
| Preprocessor | %GO TO | 未網羅 |  | - |  |
| Preprocessor | %IF | 使用あり | PM101A | - |  |
| Preprocessor | %INCLUDE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 36 | - |  |
| Preprocessor | %XINCLUDE | 未網羅 |  | - |  |
| Preprocessor | %INSCAN | 未網羅 |  | - |  |
| Preprocessor | %ITERATE | 未網羅 |  | - |  |
| Preprocessor | %LEAVE | 未網羅 |  | - |  |
| Preprocessor | %NOTE | 未網羅 |  | - |  |
| Preprocessor | %PAGE | 未網羅 |  | - |  |
| Preprocessor | %POP | 未網羅 |  | - |  |
| Preprocessor | %PRINT | 未網羅 |  | - |  |
| Preprocessor | %PROCEDURE | 使用あり | PM102A, PM106A | - |  |
| Preprocessor | %PUSH | 未網羅 |  | - |  |
| Preprocessor | %REPLACE | 未網羅 |  | - |  |
| Preprocessor | %SELECT | 未網羅 |  | - |  |
| Preprocessor | %SKIP | 未網羅 |  | - |  |
| Preprocessor | STATEMENT | 使用あり | PM106A | - |  |
| Preprocessor | COMPILEDATE | 未網羅 |  | - |  |
| Preprocessor | COMPILETIME | 未網羅 |  | - |  |
| Preprocessor | COUNTER | 使用あり | PCS01A | - |  |
| Preprocessor | PARMSET | 未網羅 |  | - |  |
| Preprocessor | QUOTE | 未網羅 |  | - |  |
| Preprocessor | SYSPARM | 未網羅 |  | - |  |
| Preprocessor | SYSTEM | 未網羅 |  | - |  |
| Preprocessor | SYSVERSION | 未網羅 |  | - |  |
| Preprocessor | MACNAME | 未網羅 |  | - |  |
| Procedure options | OPTIONS(MAIN) | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 66 | - |  |
| Procedure options | NOEXECOPS | 使用あり | PM108A | - |  |
| Procedure options | FETCHABLE | 使用あり | PCS01A | - |  |
| Procedure options | RECURSIVE | 使用あり | PC102A | - |  |
| Procedure options | REORDER | 未網羅 |  | - |  |
| Procedure options | BYVALUE | 未網羅 |  | - |  |
| Procedure options | LINKAGE | 未網羅 |  | - |  |
| Procedure options | ASSEMBLER | 未網羅 |  | - |  |
| Procedure options | COBOL | 使用あり | PM104M | - |  |
| Procedure options | INTER | 未網羅 |  | - |  |
| Procedure options | RETCODE | 使用あり | PM104M | - |  |

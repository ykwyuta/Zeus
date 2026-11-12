# ZPVS 網羅台帳 (Enterprise PL/I for z/OS)

Enterprise PL/I for z/OS Language Reference の章立てから列挙した。 要素 708、使用あり 520、対象外 8、未網羅 180。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |
| --- | --- | --- | --- | --- | --- |
| Statements | ALLOCATE | 使用あり | PC112A, PE113N, PE113P, PE114N, PE114P, PS101A ほか 3 | - |  |
| Statements | ASSERT | 使用あり | PC112A | - |  |
| Statements | ATTACH | 未網羅 |  | - |  |
| Statements | BEGIN | 使用あり | PC104A, PC106A, PC107A, PC108A, PC110A, PC111A ほか 5 | - |  |
| Statements | CALL | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 52 | - |  |
| Statements | CLOSE | 使用あり | PF102A, PF103A, PF104M, PF105A, PF106A, PM109A ほか 1 | - |  |
| Statements | DECLARE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 73 | - |  |
| Statements | DEFAULT | 使用あり | PD105A, PD108A, PD110A, PF106A, PM114A | - |  |
| Statements | DEFINE ALIAS | 使用あり | PD104A | - |  |
| Statements | DEFINE ORDINAL | 使用あり | PD105A | - |  |
| Statements | DEFINE STRUCTURE | 使用あり | PD104A, PD109A, PS106A | - |  |
| Statements | DELAY | 未網羅 |  | - |  |
| Statements | DELETE | 使用あり | PF104M | - |  |
| Statements | DETACH | 未網羅 |  | - |  |
| Statements | DISPLAY | 未網羅 |  | - |  |
| Statements | DO | 使用あり | PC101A, PE108P, PE109N, PE109P, PM101A, PM110A ほか 2 | - |  |
| Statements | DO WHILE | 使用あり | PC101A | - |  |
| Statements | DO UNTIL | 使用あり | PC101A | - |  |
| Statements | DO LOOP | 使用あり | PM110A | - |  |
| Statements | DO REPEAT | 使用あり | PC108A, PD109A, PM104M, PM117A | - |  |
| Statements | END | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 81 | - |  |
| Statements | ENTRY | 使用あり | PC103A, PC104A, PD109A, PM102A, PM104M, PM106A | - |  |
| Statements | EXIT | 未網羅 |  | - |  |
| Statements | FETCH | 使用あり | PC104A | - |  |
| Statements | FLUSH | 未網羅 |  | - |  |
| Statements | FORMAT | 使用あり | PM110A | - |  |
| Statements | FREE | 使用あり | PS101A, PS102A, PS106A | - |  |
| Statements | GET | 使用あり | PF101A | - |  |
| Statements | GO TO | 使用あり | PC104A, PC106A, PC110A, PC111A, PC112A, PM110A ほか 2 | - |  |
| Statements | IF | 使用あり | PC101A, PC102A, PM101A, PM110A, PM117A | - |  |
| Statements | ITERATE | 使用あり | PC101A, PE109N, PE109P, PM110A | - |  |
| Statements | LEAVE | 使用あり | PC101A, PE108N, PE108P, PM110A | - |  |
| Statements | LOCATE | 使用あり | PE111N, PE111P | - |  |
| Statements | ON | 使用あり | PC104A, PC105A, PC106A, PC107A, PC108A, PC110A ほか 10 | - |  |
| Statements | OPEN | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 50 | - |  |
| Statements | PACKAGE | 未網羅 |  | - |  |
| Statements | PROCEDURE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 81 | - |  |
| Statements | PUT | 使用あり | PF101A, PF102A, PF106A, PM110A, PM115A | - |  |
| Statements | READ | 使用あり | PD110A, PF103A, PF104M, PF105A, PF106A | - |  |
| Statements | RELEASE | 使用あり | PC104A | - |  |
| Statements | RESIGNAL | 使用あり | PM110A | - |  |
| Statements | RETURN | 使用あり | PC102A, PC103A, PC109A, PE110N, PE110P, PM102A ほか 3 | - |  |
| Statements | REVERT | 使用あり | PC104A, PC105A, PC110A, PC111A, PC112A, PM110A ほか 1 | - |  |
| Statements | REWRITE | 使用あり | PF104M | - |  |
| Statements | SELECT | 使用あり | PC101A, PM117A | - |  |
| Statements | SIGNAL | 使用あり | PC105A, PC110A, PC112A, PM110A, PM112A | - |  |
| Statements | STOP | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 50 | - |  |
| Statements | WAIT | 未網羅 |  | - |  |
| Statements | WRITE | 使用あり | PF103A, PF104M, PF105A, PF106A, PS104A, PS105A | - |  |
| Statements | CANCEL THREAD | 未網羅 |  | - |  |
| Statements | OTHERWISE | 使用あり | PC101A, PM117A | - |  |
| Statements | QUALIFY | 未網羅 |  | - |  |
| Statements | REINIT | 使用あり | PC109A | - |  |
| Statements | WHEN | 使用あり | PC101A, PC103A, PE112N, PE112P, PM117A | - |  |
| Statements | XDECLARE | 未網羅 |  | - |  |
| Statements | XDEFINE ALIAS | 未網羅 |  | - |  |
| Statements | XDEFINE ORDINAL | 未網羅 |  | - |  |
| Statements | XDEFINE STRUCTURE | 未網羅 |  | - |  |
| Statements | XPROCEDURE | 未網羅 |  | - |  |
| Data attributes | ALIGNED | 使用あり | PD103A | - |  |
| Data attributes | AREA | 使用あり | PC112A, PD109A, PE114P, PE115P, PM111A, PS103A | - |  |
| Data attributes | AUTOMATIC | 使用あり | PC109A | - |  |
| Data attributes | BASED | 使用あり | PC112A, PD110A, PE106P, PE107N, PE107P, PE111P ほか 6 | - |  |
| Data attributes | BIGENDIAN | 使用あり | PD110A | - |  |
| Data attributes | BINARY | 使用あり | PA101A, PA102A, PA104A, PC101A, PC102A, PC103A ほか 52 | - |  |
| Data attributes | BIT | 使用あり | PA102A, PC110A, PC111A, PC112A, PD102A, PD110A ほか 7 | - |  |
| Data attributes | BUILTIN | 使用あり | PD108A, PM109A, PM110A, PM112A | - |  |
| Data attributes | BYADDR | 未網羅 |  | - |  |
| Data attributes | BYVALUE | 使用あり | PD110A | - |  |
| Data attributes | CHARACTER | 使用あり | PA102A, PA104A, PC103A, PC107A, PC108A, PC110A ほか 40 | - |  |
| Data attributes | COMPLEX | 使用あり | PD106A, PE103N, PE103P, PM110A | - |  |
| Data attributes | CONDITION | 使用あり | PC105A, PC110A, PC112A, PM112A | - |  |
| Data attributes | CONNECTED | 未網羅 |  | - |  |
| Data attributes | CONTROLLED | 使用あり | PE113P, PS102A | - |  |
| Data attributes | DATE | 使用あり | PM105A, PM110A, PM114A, PM117A | - |  |
| Data attributes | DECIMAL | 使用あり | PA101A, PA102A, PA105A, PC102A, PC106A, PC107A ほか 19 | - |  |
| Data attributes | DEFINED | 使用あり | PS105A | - |  |
| Data attributes | DIMENSION | 使用あり | PA104A, PS102A, PS104A | - |  |
| Data attributes | ENTRY | 使用あり | PC103A, PC104A, PD109A, PM102A, PM104M, PM106A | - |  |
| Data attributes | EXTERNAL | 使用あり | PC104A | - |  |
| Data attributes | FILE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 52 | - |  |
| Data attributes | FIXED | 使用あり | PA101A, PA102A, PA104A, PA105A, PC101A, PC102A ほか 58 | - |  |
| Data attributes | FLOAT | 使用あり | PA103A, PC110A, PD106A, PD109A, PD110A, PM109A ほか 5 | - |  |
| Data attributes | FORMAT | 使用あり | PM110A | - |  |
| Data attributes | GENERIC | 使用あり | PC103A, PE112N, PE112P | - |  |
| Data attributes | GRAPHIC | 使用あり | PD106A | - |  |
| Data attributes | HANDLE | 使用あり | PD109A, PS106A | - |  |
| Data attributes | HEXADEC | 使用あり | PD110A | - |  |
| Data attributes | IEEE | 未網羅 |  | - | 未網羅: Zeus は HFP と big-endian の 2 進しか格納しないので、2026-11-12 から黙って既定の表現にせず断る。表現を実装するまで検査は書けない |
| Data attributes | INITIAL | 使用あり | PA104A, PC102A, PC105A, PC106A, PC107A, PC108A ほか 26 | - |  |
| Data attributes | INTERNAL | 未網羅 |  | - |  |
| Data attributes | LABEL | 使用あり | PC101A, PD109A | - |  |
| Data attributes | LIKE | 使用あり | PD104A, PE104N, PE104P, PE105N, PE105P | - |  |
| Data attributes | LIMITED | 未網羅 |  | - |  |
| Data attributes | LITTLEENDIAN | 未網羅 |  | - | 未網羅: Zeus は HFP と big-endian の 2 進しか格納しないので、2026-11-12 から黙って既定の表現にせず断る。表現を実装するまで検査は書けない |
| Data attributes | NONASSIGNABLE | 使用あり | PD110A | - |  |
| Data attributes | NONVARYING | 使用あり | PD110A | - |  |
| Data attributes | OFFSET | 使用あり | PD109A, PE115N, PE115P, PM111A, PS103A | - |  |
| Data attributes | OPTIONAL | 使用あり | PD110A | - |  |
| Data attributes | OPTIONS | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 81 | - |  |
| Data attributes | ORDINAL | 使用あり | PD105A | - |  |
| Data attributes | PARAMETER | 未網羅 |  | - |  |
| Data attributes | PICTURE | 使用あり | PA103A, PA105A, PM115A | - |  |
| Data attributes | POINTER | 使用あり | PC112A, PM110A, PM111A, PM116A, PS101A, PS103A ほか 1 | - |  |
| Data attributes | POSITION | 使用あり | PS105A | - |  |
| Data attributes | PRECISION | 使用あり | PM110A, PM115A | - |  |
| Data attributes | REAL | 使用あり | PD106A, PM110A | - |  |
| Data attributes | REFER | 使用あり | PE106N, PE106P, PE107N, PE107P, PS104A | - |  |
| Data attributes | RESERVED | 未網羅 |  | - |  |
| Data attributes | RETURNS | 使用あり | PC102A, PC103A, PC109A, PM102A, PM106A, PM115A ほか 1 | - |  |
| Data attributes | SIGNED | 使用あり | PD110A, PM115A | - |  |
| Data attributes | STATIC | 使用あり | PC104A, PC109A, PCS01A, PD109A | - |  |
| Data attributes | TYPE | 使用あり | PD104A, PD105A, PS106A | - |  |
| Data attributes | UNALIGNED | 使用あり | PD102A, PD103A | - |  |
| Data attributes | UNION | 未網羅 |  | - |  |
| Data attributes | UNSIGNED | 使用あり | PD107A, PD110A, PM112A, PM114A, PM115A | - |  |
| Data attributes | VALUE | 使用あり | PA105A, PD105A, PD108A, PD109A, PM106A, PM107A ほか 2 | - |  |
| Data attributes | VARIABLE | 使用あり | PC103A, PD108A, PD109A | - |  |
| Data attributes | VARYING | 使用あり | PC111A, PC112A, PD102A, PD109A, PM107A, PM108A ほか 6 | - |  |
| Data attributes | VARYINGZ | 使用あり | PD110A | - |  |
| Data attributes | WIDECHAR | 使用あり | PD106A | - |  |
| File attributes and options | BUFFERED | 未網羅 |  | - |  |
| File attributes and options | DIRECT | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | ENVIRONMENT | 使用あり | PF102A, PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | INPUT | 使用あり | PC107A, PC108A, PC111A, PF103A, PF105A, PM115A | - |  |
| File attributes and options | KEYED | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | OUTPUT | 使用あり | PE111N, PE111P, PF102A, PF103A, PF104M, PF105A ほか 2 | - |  |
| File attributes and options | PRINT | 使用あり | PF102A, PF106A, PM117A | - |  |
| File attributes and options | RECORD | 使用あり | PC111A, PE111N, PE111P, PF103A, PF104M, PF105A ほか 2 | - |  |
| File attributes and options | SEQUENTIAL | 未網羅 |  | - |  |
| File attributes and options | STREAM | 使用あり | PF102A, PF106A, PM115A, PM116A | - |  |
| File attributes and options | UPDATE | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | TITLE | 使用あり | PC104A, PC111A, PF102A, PF103A, PF104M, PF105A ほか 2 | - |  |
| File attributes and options | LINESIZE | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | PAGESIZE | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | KEY | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | KEYFROM | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | KEYTO | 未網羅 |  | - |  |
| File attributes and options | SET | 使用あり | PC112A, PS103A | - |  |
| File attributes and options | INTO | 使用あり | PF103A, PF104M, PF105A, PF106A | - |  |
| File attributes and options | FROM | 使用あり | PF103A, PF104M, PF105A, PF106A, PM112A, PM117A | - |  |
| File attributes and options | PAGE | 使用あり | PF102A | - |  |
| File attributes and options | SKIP | 使用あり | PF102A, PF106A, PM117A | - |  |
| File attributes and options | LINE | 使用あり | PF102A | - |  |
| File attributes and options | LIST | 使用あり | PF101A, PM115A | - |  |
| File attributes and options | EDIT | 使用あり | PF101A, PF102A, PF106A, PM110A, PS104A | - |  |
| File attributes and options | DATA | 使用あり | PF101A, PS101A, PS103A | - |  |
| File attributes and options | STRING | 使用あり | PF101A, PM110A | - |  |
| File attributes and options | COPY | 使用あり | PC103A, PD104A, PD110A, PM103A, PM117A, PS103A | - |  |
| Conditions | AREA | 使用あり | PC112A, PD109A, PE114P, PE115P, PM111A, PS103A | - |  |
| Conditions | ATTENTION | 対象外 |  | - | 対象外: 端末の注意割込みで、バッチでは起こせない |
| Conditions | CONDITION | 使用あり | PC105A, PC110A, PC112A, PM112A | - |  |
| Conditions | CONVERSION | 使用あり | PC107A, PC108A, PC110A | - |  |
| Conditions | ENDFILE | 使用あり | PF103A, PF105A | - |  |
| Conditions | ENDPAGE | 使用あり | PF102A, PF106A | - |  |
| Conditions | ERROR | 使用あり | PC104A, PC110A, PM110A | - |  |
| Conditions | FINISH | 使用あり | PC112A | - |  |
| Conditions | FIXEDOVERFLOW | 使用あり | PC111A | - |  |
| Conditions | INVALIDOP | 使用あり | PC112A | - |  |
| Conditions | KEY | 使用あり | PF104M, PF106A | - |  |
| Conditions | NAME | 使用あり | PA104A, PF101A, PM106A, PM112A | - |  |
| Conditions | OVERFLOW | 使用あり | PC110A | - |  |
| Conditions | RECORD | 使用あり | PC111A, PE111N, PE111P, PF103A, PF104M, PF105A ほか 2 | - |  |
| Conditions | SIZE | 使用あり | PC106A, PC110A, PD103A, PD109A, PM109A | - |  |
| Conditions | STORAGE | 使用あり | PD103A, PD104A, PD105A, PD106A, PD108A, PD109A ほか 2 | - |  |
| Conditions | STRINGRANGE | 使用あり | PC110A | - |  |
| Conditions | STRINGSIZE | 使用あり | PC111A | - |  |
| Conditions | SUBSCRIPTRANGE | 使用あり | PC110A, PC112A | - |  |
| Conditions | TRANSMIT | 対象外 |  | - | 対象外: 入出力の伝送エラーで、決まった形で起こせない |
| Conditions | UNDEFINEDFILE | 使用あり | PC111A, PM115A | - |  |
| Conditions | UNDERFLOW | 使用あり | PC110A | - |  |
| Conditions | ZERODIVIDE | 使用あり | PC106A, PC110A, PC112A, PM112A, PM115A | - |  |
| Conditions | ANYCONDITION | 使用あり | PC112A | - |  |
| Conditions | ASSERTION | 使用あり | PC112A | - |  |
| Conditions | CONFORMANCE | 未網羅 |  | - |  |
| Built-in functions | ABS | 使用あり | PD106A, PD109A, PM105A, PM109A, PM110A, PM112A | - |  |
| Built-in functions | ACOS | 使用あり | PM110A | - |  |
| Built-in functions | ADD | 使用あり | PA101A | - |  |
| Built-in functions | ADDR | 使用あり | PD110A, PM109A, PM111A, PM112A, PM113A, PM115A ほか 2 | - |  |
| Built-in functions | ADDRDATA | 使用あり | PM111A | - |  |
| Built-in functions | ALL | 使用あり | PM109A | - |  |
| Built-in functions | ALLCOMPARE | 未網羅 |  | - |  |
| Built-in functions | ALLOC31 | 未網羅 |  | - |  |
| Built-in functions | ALLOCATE | 未網羅 |  | - |  |
| Built-in functions | ALLOCATION | 使用あり | PS102A | - |  |
| Built-in functions | ALLOCNEXT | 未網羅 |  | - |  |
| Built-in functions | ALLOCSIZE | 未網羅 |  | - |  |
| Built-in functions | ANY | 使用あり | PM109A | - |  |
| Built-in functions | ASIN | 使用あり | PM110A | - |  |
| Built-in functions | ATAN | 使用あり | PM109A | - |  |
| Built-in functions | ATAND | 使用あり | PM109A | - |  |
| Built-in functions | ATANH | 使用あり | PM109A | - |  |
| Built-in functions | AUTOMATIC | 使用あり | PC109A | - |  |
| Built-in functions | AVAILABLEAREA | 未網羅 |  | - |  |
| Built-in functions | BASE64DECODE | 使用あり | PM113A | - |  |
| Built-in functions | BASE64DECODE8 | 使用あり | PM115A | - |  |
| Built-in functions | BASE64DECODE16 | 使用あり | PM115A | - |  |
| Built-in functions | BASE64ENCODE | 使用あり | PM113A | - |  |
| Built-in functions | BASE64ENCODE8 | 使用あり | PM113A | - |  |
| Built-in functions | BASE64ENCODE16 | 使用あり | PM115A | - |  |
| Built-in functions | BETWEEN | 使用あり | PM112A | - |  |
| Built-in functions | BETWEENEXCLUSIVE | 使用あり | PM112A | - |  |
| Built-in functions | BETWEENLEFTEXCLUSIVE | 使用あり | PM112A | - |  |
| Built-in functions | BETWEENRIGHTEXCLUSIVE | 使用あり | PM112A | - |  |
| Built-in functions | BINARY | 使用あり | PA101A, PA102A, PA104A, PC101A, PC102A, PC103A ほか 52 | - |  |
| Built-in functions | BINARYVALUE | 使用あり | PM110A, PM111A | - |  |
| Built-in functions | BINSEARCH | 未網羅 |  | - |  |
| Built-in functions | BINSEARCHX | 未網羅 |  | - |  |
| Built-in functions | BIT | 使用あり | PA102A, PC110A, PC111A, PC112A, PD102A, PD110A ほか 7 | - |  |
| Built-in functions | BITLOCATION | 使用あり | PM111A | - |  |
| Built-in functions | BOOL | 使用あり | PA102A | - |  |
| Built-in functions | BYTE | 使用あり | PM109A | - |  |
| Built-in functions | BYTELENGTH | 未網羅 |  | - |  |
| Built-in functions | CDS | 使用あり | PM111A | - |  |
| Built-in functions | CEIL | 使用あり | PM103A | - |  |
| Built-in functions | CENTERLEFT | 使用あり | PM103A, PM110A | - |  |
| Built-in functions | CENTERRIGHT | 使用あり | PM115A | - |  |
| Built-in functions | CENTRELEFT | 使用あり | PM103A, PM110A | - |  |
| Built-in functions | CENTRERIGHT | 使用あり | PM115A | - |  |
| Built-in functions | CHARACTER | 使用あり | PA102A, PA104A, PC103A, PC107A, PC108A, PC110A ほか 40 | - |  |
| Built-in functions | CHARGRAPHIC | 未網羅 |  | - |  |
| Built-in functions | CHARVAL | 使用あり | PM109A | - |  |
| Built-in functions | CHECKSTG | 未網羅 |  | - |  |
| Built-in functions | CHECKSUM | 未網羅 |  | - |  |
| Built-in functions | CODEPAGE | 未網羅 |  | - |  |
| Built-in functions | COLLATE | 使用あり | PM109A | - |  |
| Built-in functions | COLLAPSE | 使用あり | PM113A | - |  |
| Built-in functions | COMPARE | 使用あり | PC112A, PM112A | - |  |
| Built-in functions | COMPLEX | 使用あり | PM110A | - |  |
| Built-in functions | CONJG | 使用あり | PD106A | - |  |
| Built-in functions | COPY | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | COS | 使用あり | PM105A | - |  |
| Built-in functions | COSD | 使用あり | PM109A | - |  |
| Built-in functions | COSH | 使用あり | PM109A | - |  |
| Built-in functions | COUNT | 使用あり | PM115A | - |  |
| Built-in functions | CS | 使用あり | PM111A | - |  |
| Built-in functions | CURRENTSIZE | 使用あり | PS104A | - |  |
| Built-in functions | CURRENTSTORAGE | 使用あり | PM109A | - |  |
| Built-in functions | DATAFIELD | 未網羅 |  | - |  |
| Built-in functions | DATE | 使用あり | PM110A | - |  |
| Built-in functions | DATETIME | 使用あり | PM110A | - |  |
| Built-in functions | DAYS | 使用あり | PM105A, PM114A | - |  |
| Built-in functions | DAYSTODATE | 使用あり | PM105A, PM114A | - |  |
| Built-in functions | DAYSTOMICROSECS | 使用あり | PM114A | - |  |
| Built-in functions | DAYSTOSECS | 使用あり | PM109A | - |  |
| Built-in functions | DECIMAL | 使用あり | PA101A, PA102A, PA105A, PC102A, PC106A, PC107A ほか 19 | - |  |
| Built-in functions | DIMENSION | 使用あり | PA104A, PS102A, PS104A | - |  |
| Built-in functions | DIVIDE | 使用あり | PA101A | - |  |
| Built-in functions | EDIT | 使用あり | PF101A, PF102A, PF106A, PM110A | - |  |
| Built-in functions | EMPTY | 使用あり | PS103A | - |  |
| Built-in functions | ENDFILE | 使用あり | PF103A, PF105A | - |  |
| Built-in functions | ENTRYADDR | 未網羅 |  | - |  |
| Built-in functions | EPSILON | 使用あり | PM109A | - |  |
| Built-in functions | ERF | 使用あり | PM112A | - |  |
| Built-in functions | ERFC | 使用あり | PM112A | - |  |
| Built-in functions | EXP | 使用あり | PM109A | - |  |
| Built-in functions | EXPONENT | 使用あり | PM111A | - |  |
| Built-in functions | FILEDDINT | 未網羅 |  | - |  |
| Built-in functions | FILEDDTEST | 未網羅 |  | - |  |
| Built-in functions | FILEDDWORD | 未網羅 |  | - |  |
| Built-in functions | FILEID | 未網羅 |  | - |  |
| Built-in functions | FILENEW | 未網羅 |  | - |  |
| Built-in functions | FILEOPEN | 未網羅 |  | - |  |
| Built-in functions | FILEREAD | 未網羅 |  | - |  |
| Built-in functions | FILESEEK | 未網羅 |  | - |  |
| Built-in functions | FILETELL | 未網羅 |  | - |  |
| Built-in functions | FILEWRITE | 未網羅 |  | - |  |
| Built-in functions | FIXED | 使用あり | PD108A | - |  |
| Built-in functions | FIXEDBIN | 使用あり | PM115A | - |  |
| Built-in functions | FIXEDDEC | 使用あり | PM115A, PM117A | - |  |
| Built-in functions | FLOAT | 使用あり | PM110A | - |  |
| Built-in functions | FLOATBIN | 使用あり | PM115A | - |  |
| Built-in functions | FLOATDEC | 使用あり | PM115A | - |  |
| Built-in functions | FLOOR | 使用あり | PM103A | - |  |
| Built-in functions | FOLDEDFULLMATCH | 未網羅 |  | - |  |
| Built-in functions | FOLDEDSIMPLEMATCH | 未網羅 |  | - |  |
| Built-in functions | GAMMA | 使用あり | PM112A | - |  |
| Built-in functions | GETENV | 使用あり | PM115A | - |  |
| Built-in functions | GETJCLSYMBOL | 未網羅 |  | - |  |
| Built-in functions | GETSYSINT | 未網羅 |  | - |  |
| Built-in functions | GETSYSWORD | 未網羅 |  | - |  |
| Built-in functions | GRAPHIC | 使用あり | PD106A | - |  |
| Built-in functions | GTCA | 未網羅 |  | - |  |
| Built-in functions | HANDLE | 使用あり | PS106A | - |  |
| Built-in functions | HBOUND | 使用あり | PM109A | - |  |
| Built-in functions | HBOUNDACROSS | 未網羅 |  | - |  |
| Built-in functions | HEX | 使用あり | PA103A, PA105A, PD101A, PD102A, PD103A, PD106A ほか 5 | - |  |
| Built-in functions | HEX8 | 使用あり | PM113A | - |  |
| Built-in functions | HEXDECODE | 使用あり | PM113A | - |  |
| Built-in functions | HEXDECODE8 | 使用あり | PM115A | - |  |
| Built-in functions | HEXENCODE | 使用あり | PM113A | - |  |
| Built-in functions | HEXENCODE8 | 使用あり | PM113A | - |  |
| Built-in functions | HEXIMAGE | 使用あり | PM112A | - |  |
| Built-in functions | HEXIMAGE8 | 使用あり | PM115A | - |  |
| Built-in functions | HIGH | 使用あり | PM109A | - |  |
| Built-in functions | HUGE | 使用あり | PM109A | - |  |
| Built-in functions | IAND | 使用あり | PM109A | - |  |
| Built-in functions | ICLZ | 使用あり | PM112A | - |  |
| Built-in functions | IEOR | 使用あり | PM109A | - |  |
| Built-in functions | IFTHENELSE | 使用あり | PM112A | - |  |
| Built-in functions | IMAG | 使用あり | PD106A, PM110A | - |  |
| Built-in functions | INARRAY | 未網羅 |  | - |  |
| Built-in functions | INDEX | 使用あり | PF101A, PM103A, PM115A, PM117A | - |  |
| Built-in functions | INDEXR | 使用あり | PM113A | - |  |
| Built-in functions | INDICATORS | 未網羅 |  | - |  |
| Built-in functions | INLIST | 使用あり | PM112A | - |  |
| Built-in functions | INOT | 使用あり | PM109A | - |  |
| Built-in functions | IOR | 使用あり | PM109A | - |  |
| Built-in functions | IRLL | 使用あり | PM112A | - |  |
| Built-in functions | IRRL | 使用あり | PM112A | - |  |
| Built-in functions | ISFINITE | 未網羅 |  | - |  |
| Built-in functions | ISIGNED | 使用あり | PM112A | - |  |
| Built-in functions | ISINF | 未網羅 |  | - |  |
| Built-in functions | ISJCLSYMBOL | 未網羅 |  | - |  |
| Built-in functions | ISLL | 使用あり | PM109A | - |  |
| Built-in functions | ISMAIN | 使用あり | PM115A | - |  |
| Built-in functions | ISNAN | 未網羅 |  | - |  |
| Built-in functions | ISNORMAL | 未網羅 |  | - |  |
| Built-in functions | ISRL | 使用あり | PM109A | - |  |
| Built-in functions | ISZERO | 未網羅 |  | - |  |
| Built-in functions | IUNSIGNED | 使用あり | PM112A | - |  |
| Built-in functions | JSONGETARRAYEND | 未網羅 |  | - |  |
| Built-in functions | JSONGETARRAYSTART | 未網羅 |  | - |  |
| Built-in functions | JSONGETCOLON | 未網羅 |  | - |  |
| Built-in functions | JSONGETCOMMA | 未網羅 |  | - |  |
| Built-in functions | JSONGETMEMBER | 未網羅 |  | - |  |
| Built-in functions | JSONGETOBJECTEND | 未網羅 |  | - |  |
| Built-in functions | JSONGETOBJECTSTART | 未網羅 |  | - |  |
| Built-in functions | JSONGETVALUE | 未網羅 |  | - |  |
| Built-in functions | JSONPUTARRAYEND | 未網羅 |  | - |  |
| Built-in functions | JSONPUTARRAYSTART | 未網羅 |  | - |  |
| Built-in functions | JSONPUTCOLON | 未網羅 |  | - |  |
| Built-in functions | JSONPUTCOMMA | 未網羅 |  | - |  |
| Built-in functions | JSONPUTMEMBER | 未網羅 |  | - |  |
| Built-in functions | JSONPUTOBJECTEND | 未網羅 |  | - |  |
| Built-in functions | JSONPUTOBJECTSTART | 未網羅 |  | - |  |
| Built-in functions | JSONPUTVALUE | 未網羅 |  | - |  |
| Built-in functions | JSONVALID | 未網羅 |  | - |  |
| Built-in functions | JULIANTOSMF | 使用あり | PM114A | - |  |
| Built-in functions | LBOUND | 使用あり | PM109A | - |  |
| Built-in functions | LBOUNDACROSS | 未網羅 |  | - |  |
| Built-in functions | LEFT | 使用あり | PM103A | - |  |
| Built-in functions | LENGTH | 使用あり | PC103A, PD102A, PD110A, PM103A, PM107A, PM108A ほか 6 | - |  |
| Built-in functions | LINENO | 使用あり | PF102A, PF106A | - |  |
| Built-in functions | LOCATION | 使用あり | PM112A | - |  |
| Built-in functions | LOCNEWSPACE | 未網羅 |  | - |  |
| Built-in functions | LOCNEWVALUE | 未網羅 |  | - |  |
| Built-in functions | LOCSTG | 未網羅 |  | - |  |
| Built-in functions | LOCVAL | 未網羅 |  | - |  |
| Built-in functions | LOG | 使用あり | PM105A | - |  |
| Built-in functions | LOGGAMMA | 使用あり | PM112A | - |  |
| Built-in functions | LOG2 | 使用あり | PM109A | - |  |
| Built-in functions | LOG10 | 使用あり | PM109A | - |  |
| Built-in functions | LOW | 使用あり | PD110A, PM109A | - |  |
| Built-in functions | LOWERASCII | 未網羅 |  | - |  |
| Built-in functions | LOWERCASE | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | LOWERLATIN1 | 未網羅 |  | - |  |
| Built-in functions | LOWER2 | 使用あり | PM112A | - |  |
| Built-in functions | MAINNAME | 使用あり | PM115A | - |  |
| Built-in functions | MAX | 使用あり | PM105A, PM117A | - |  |
| Built-in functions | MAXDATE | 使用あり | PM114A | - |  |
| Built-in functions | MAXEXP | 使用あり | PM109A | - |  |
| Built-in functions | MAXLENGTH | 使用あり | PD102A | - |  |
| Built-in functions | MAXVAL | 使用あり | PM112A | - |  |
| Built-in functions | MEMCOLLAPSE | 使用あり | PM113A | - |  |
| Built-in functions | MEMCONVERT | 未網羅 |  | - |  |
| Built-in functions | MEMCU12 | 未網羅 |  | - |  |
| Built-in functions | MEMCU14 | 未網羅 |  | - |  |
| Built-in functions | MEMCU21 | 未網羅 |  | - |  |
| Built-in functions | MEMCU24 | 未網羅 |  | - |  |
| Built-in functions | MEMCU41 | 未網羅 |  | - |  |
| Built-in functions | MEMCU42 | 未網羅 |  | - |  |
| Built-in functions | MEMINDEX | 使用あり | PM113A | - |  |
| Built-in functions | MEMREPLACE | 使用あり | PM113A | - |  |
| Built-in functions | MEMSEARCH | 使用あり | PM113A | - |  |
| Built-in functions | MEMSEARCHR | 使用あり | PM113A | - |  |
| Built-in functions | MEMSQUEEZE | 使用あり | PM113A | - |  |
| Built-in functions | MEMUVALID8 | 未網羅 |  | - |  |
| Built-in functions | MEMUVALID16 | 未網羅 |  | - |  |
| Built-in functions | MEMVERIFY | 使用あり | PM113A | - |  |
| Built-in functions | MEMVERIFYR | 使用あり | PM113A | - |  |
| Built-in functions | MICROSECS | 使用あり | PM114A | - |  |
| Built-in functions | MICROSECSTODATE | 使用あり | PM114A | - |  |
| Built-in functions | MICROSECSTODAYS | 使用あり | PM114A | - |  |
| Built-in functions | MIN | 使用あり | PM105A, PM117A | - |  |
| Built-in functions | MINDATE | 使用あり | PM114A | - |  |
| Built-in functions | MINEXP | 使用あり | PM109A | - |  |
| Built-in functions | MINVAL | 使用あり | PM112A | - |  |
| Built-in functions | MOD | 使用あり | PC101A, PM103A | - |  |
| Built-in functions | MPSTR | 未網羅 |  | - |  |
| Built-in functions | MULTIPLY | 使用あり | PA101A | - |  |
| Built-in functions | NULL | 使用あり | PD109A, PS101A | - |  |
| Built-in functions | NULLENTRY | 未網羅 |  | - |  |
| Built-in functions | OFFSET | 使用あり | PD109A, PE115N, PE115P, PM111A, PS103A | - |  |
| Built-in functions | OFFSETADD | 使用あり | PM111A | - |  |
| Built-in functions | OFFSETDIFF | 使用あり | PM111A | - |  |
| Built-in functions | OFFSETSUBTRACT | 使用あり | PM111A | - |  |
| Built-in functions | OFFSETVALUE | 使用あり | PM111A | - |  |
| Built-in functions | OMITTED | 使用あり | PD110A | - |  |
| Built-in functions | ONACTUAL | 使用あり | PC112A | - |  |
| Built-in functions | ONAREA | 使用あり | PC112A | - |  |
| Built-in functions | ONCHAR | 使用あり | PC107A | - |  |
| Built-in functions | ONCODE | 使用あり | PC104A, PC106A, PC110A, PC111A, PC112A, PF104M ほか 1 | - |  |
| Built-in functions | ONCONDCOND | 使用あり | PM112A | - |  |
| Built-in functions | ONCONDID | 使用あり | PC112A, PM112A | - |  |
| Built-in functions | ONCOUNT | 使用あり | PM110A | - |  |
| Built-in functions | ONEXPECTED | 使用あり | PC112A | - |  |
| Built-in functions | ONFILE | 使用あり | PM115A | - |  |
| Built-in functions | ONGSOURCE | 未網羅 |  | - |  |
| Built-in functions | ONHBOUND | 使用あり | PC112A | - |  |
| Built-in functions | ONJSONNAME | 未網羅 |  | - |  |
| Built-in functions | ONKEY | 未網羅 |  | - |  |
| Built-in functions | ONLBOUND | 使用あり | PC112A | - |  |
| Built-in functions | ONLINE | 使用あり | PC112A | - |  |
| Built-in functions | ONLOC | 使用あり | PM115A | - |  |
| Built-in functions | ONOFFSET | 未網羅 |  | - |  |
| Built-in functions | ONOPERATOR | 使用あり | PC112A | - |  |
| Built-in functions | ONPACKAGE | 使用あり | PC112A | - |  |
| Built-in functions | ONPROCEDURE | 使用あり | PM115A | - |  |
| Built-in functions | ONSOURCE | 使用あり | PC108A | - |  |
| Built-in functions | ONSUBCODE | 未網羅 |  | - |  |
| Built-in functions | ONSUBCODE2 | 未網羅 |  | - |  |
| Built-in functions | ONSUBSCRIPT | 使用あり | PC112A | - |  |
| Built-in functions | ONTEXT | 使用あり | PC112A | - |  |
| Built-in functions | ONUCHAR | 未網羅 |  | - |  |
| Built-in functions | ONUSOURCE | 未網羅 |  | - |  |
| Built-in functions | ONWCHAR | 未網羅 |  | - |  |
| Built-in functions | ONWSOURCE | 未網羅 |  | - |  |
| Built-in functions | ORDINALNAME | 使用あり | PD105A | - |  |
| Built-in functions | ORDINALPRED | 使用あり | PD105A | - |  |
| Built-in functions | ORDINALSUCC | 使用あり | PD105A | - |  |
| Built-in functions | PACKAGENAME | 使用あり | PM115A | - |  |
| Built-in functions | PAGENO | 使用あり | PF102A, PF106A | - |  |
| Built-in functions | PICSPEC | 未網羅 |  | - |  |
| Built-in functions | PLACES | 使用あり | PM112A | - |  |
| Built-in functions | PLIASCII | 未網羅 |  | - |  |
| Built-in functions | PLIATTN | 未網羅 |  | - |  |
| Built-in functions | PLICANC | 未網羅 |  | - |  |
| Built-in functions | PLICKPT | 未網羅 |  | - |  |
| Built-in functions | PLIDELETE | 未網羅 |  | - |  |
| Built-in functions | PLIDUMP | 未網羅 |  | - |  |
| Built-in functions | PLIEBCDIC | 未網羅 |  | - |  |
| Built-in functions | PLIFILL | 使用あり | PM115A | - |  |
| Built-in functions | PLIFREE | 使用あり | PS106A | - |  |
| Built-in functions | PLIMOVE | 使用あり | PM115A | - |  |
| Built-in functions | PLIOVER | 使用あり | PM115A | - |  |
| Built-in functions | PLIPARSE | 未網羅 |  | - |  |
| Built-in functions | PLIREST | 未網羅 |  | - |  |
| Built-in functions | PLIRETC | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 50 | - |  |
| Built-in functions | PLIRETV | 使用あり | PM104M, PM107A, PM108A | - |  |
| Built-in functions | PLISAXA | 未網羅 |  | - |  |
| Built-in functions | PLISAXB | 未網羅 |  | - |  |
| Built-in functions | PLISAXC | 未網羅 |  | - |  |
| Built-in functions | PLISAXD | 未網羅 |  | - |  |
| Built-in functions | PLISRTA | 未網羅 |  | - |  |
| Built-in functions | PLISRTB | 未網羅 |  | - |  |
| Built-in functions | PLISRTC | 未網羅 |  | - |  |
| Built-in functions | PLISRTD | 未網羅 |  | - |  |
| Built-in functions | PLISTCK | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKE | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKELOCAL | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKEUTC | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKF | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKLOCAL | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKP | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKPLOCAL | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKPUTC | 使用あり | PM114A | - |  |
| Built-in functions | PLISTCKUTC | 使用あり | PM114A | - |  |
| Built-in functions | PLITRAN11 | 未網羅 |  | - |  |
| Built-in functions | PLITRAN12 | 未網羅 |  | - |  |
| Built-in functions | PLITRAN21 | 未網羅 |  | - |  |
| Built-in functions | PLITRAN22 | 未網羅 |  | - |  |
| Built-in functions | POINTER | 使用あり | PC112A, PM110A, PM111A, PM116A, PS101A, PS103A ほか 1 | - |  |
| Built-in functions | POINTERADD | 使用あり | PM115A, PM116A, PS101A | - |  |
| Built-in functions | POINTERDIFF | 使用あり | PM109A | - |  |
| Built-in functions | POINTERSUBTRACT | 使用あり | PM111A | - |  |
| Built-in functions | POINTERVALUE | 使用あり | PM110A, PM111A | - |  |
| Built-in functions | POLY | 使用あり | PM115A | - |  |
| Built-in functions | POPCNT | 使用あり | PM112A | - |  |
| Built-in functions | PRECISION | 使用あり | PM110A, PM115A | - |  |
| Built-in functions | PRECVAL | 使用あり | PM112A | - |  |
| Built-in functions | PRED | 使用あり | PM112A | - |  |
| Built-in functions | PRESENT | 使用あり | PD110A | - |  |
| Built-in functions | PROCEDURENAME | 使用あり | PM115A | - |  |
| Built-in functions | PROD | 使用あり | PM109A | - |  |
| Built-in functions | PUTENV | 使用あり | PM115A | - |  |
| Built-in functions | QUICKSORT | 未網羅 |  | - |  |
| Built-in functions | QUICKSORTX | 未網羅 |  | - |  |
| Built-in functions | RADIX | 使用あり | PM109A | - |  |
| Built-in functions | RAISE2 | 使用あり | PM109A | - |  |
| Built-in functions | RANDOM | 使用あり | PD109A | - |  |
| Built-in functions | RANK | 使用あり | PM111A | - |  |
| Built-in functions | REAL | 使用あり | PD106A, PM110A | - |  |
| Built-in functions | REGEX | 未網羅 |  | - |  |
| Built-in functions | REM | 使用あり | PM103A | - |  |
| Built-in functions | REPATTERN | 未網羅 |  | - |  |
| Built-in functions | REPEAT | 使用あり | PD109A, PM117A | - |  |
| Built-in functions | REPLACE | 使用あり | PM113A | - |  |
| Built-in functions | REPLACEBY2 | 使用あり | PM112A | - |  |
| Built-in functions | REVERSE | 使用あり | PM103A | - |  |
| Built-in functions | RIGHT | 使用あり | PM103A | - |  |
| Built-in functions | ROUND | 使用あり | PM103A | - |  |
| Built-in functions | ROUNDAWAYFROMZERO | 使用あり | PM112A | - |  |
| Built-in functions | ROUNDTOEVEN | 使用あり | PM112A | - |  |
| Built-in functions | SAMEKEY | 未網羅 |  | - |  |
| Built-in functions | SCALE | 使用あり | PM111A | - |  |
| Built-in functions | SCALEVAL | 使用あり | PM112A | - |  |
| Built-in functions | SCRUBOUT | 使用あり | PM113A | - |  |
| Built-in functions | SEARCH | 使用あり | PM103A | - |  |
| Built-in functions | SEARCHR | 使用あり | PM109A | - |  |
| Built-in functions | SECS | 使用あり | PM114A | - |  |
| Built-in functions | SECSTODATE | 使用あり | PM111A | - |  |
| Built-in functions | SECSTODAYS | 使用あり | PM109A | - |  |
| Built-in functions | SHA1DIGEST | 使用あり | PM116A | - |  |
| Built-in functions | SHA1FINAL | 使用あり | PM116A | - |  |
| Built-in functions | SHA1INIT | 使用あり | PM116A | - |  |
| Built-in functions | SHA1UPDATE | 使用あり | PM116A | - |  |
| Built-in functions | SHA2DIGEST224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2DIGEST256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2DIGEST384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2DIGEST512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2FINAL224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2FINAL256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2FINAL384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2FINAL512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2INIT224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2INIT256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2INIT384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2INIT512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2UPDATE224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2UPDATE256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2UPDATE384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA2UPDATE512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3DIGEST224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3DIGEST256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3DIGEST384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3DIGEST512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3FINAL224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3FINAL256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3FINAL384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3FINAL512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3INIT224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3INIT256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3INIT384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3INIT512 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3UPDATE224 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3UPDATE256 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3UPDATE384 | 使用あり | PM116A | - |  |
| Built-in functions | SHA3UPDATE512 | 使用あり | PM116A | - |  |
| Built-in functions | SIGN | 使用あり | PM109A | - |  |
| Built-in functions | SIGNED | 使用あり | PM115A | - |  |
| Built-in functions | SIN | 使用あり | PM105A | - |  |
| Built-in functions | SIND | 使用あり | PM109A | - |  |
| Built-in functions | SINH | 使用あり | PM109A | - |  |
| Built-in functions | SIZE | 使用あり | PM109A | - |  |
| Built-in functions | SMFTOJULIAN | 使用あり | PM114A | - |  |
| Built-in functions | SOURCEFILE | 使用あり | PM115A | - |  |
| Built-in functions | SOURCELINE | 使用あり | PC112A, PM115A | - |  |
| Built-in functions | SQRT | 使用あり | PM105A | - |  |
| Built-in functions | SQRTF | 未網羅 |  | - |  |
| Built-in functions | SQUEEZE | 使用あり | PM113A | - |  |
| Built-in functions | STACKADDR | 未網羅 |  | - |  |
| Built-in functions | STCKETODATE | 使用あり | PM114A | - |  |
| Built-in functions | STCKTODATE | 使用あり | PM114A | - |  |
| Built-in functions | STORAGE | 使用あり | PD103A, PD104A, PD105A, PD106A, PD108A, PD109A ほか 2 | - |  |
| Built-in functions | STRING | 使用あり | PF101A, PM110A | - |  |
| Built-in functions | SUBSTR | 使用あり | PA104A, PC110A, PD102A, PD108A, PD109A, PD110A ほか 6 | - |  |
| Built-in functions | SUBTO | 使用あり | PM115A | - |  |
| Built-in functions | SUBTRACT | 使用あり | PM110A | - |  |
| Built-in functions | SUCC | 使用あり | PM112A | - |  |
| Built-in functions | SUM | 使用あり | PA104A, PS102A, PS105A | - |  |
| Built-in functions | SYSNULL | 使用あり | PD109A, PM113A, PM116A, PS101A | - |  |
| Built-in functions | SYSTEM | 未網羅 |  | - |  |
| Built-in functions | TALLY | 使用あり | PM103A | - |  |
| Built-in functions | TAN | 使用あり | PM110A | - |  |
| Built-in functions | TAND | 使用あり | PM109A | - |  |
| Built-in functions | TANH | 使用あり | PM109A | - |  |
| Built-in functions | THREADID | 未網羅 |  | - |  |
| Built-in functions | TIME | 使用あり | PM110A | - |  |
| Built-in functions | TIMESTAMP | 使用あり | PM114A | - |  |
| Built-in functions | TINY | 使用あり | PM109A | - |  |
| Built-in functions | TRANSLATE | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | TRIM | 使用あり | PA102A, PC112A, PM103A, PM117A | - |  |
| Built-in functions | TRUNC | 使用あり | PM109A | - |  |
| Built-in functions | TYPE | 未網羅 |  | - |  |
| Built-in functions | UHIGH | 未網羅 |  | - |  |
| Built-in functions | ULENGTH | 未網羅 |  | - |  |
| Built-in functions | ULENGTH8 | 未網羅 |  | - |  |
| Built-in functions | ULENGTH16 | 未網羅 |  | - |  |
| Built-in functions | ULOW | 未網羅 |  | - |  |
| Built-in functions | UNALLOCATED | 未網羅 |  | - |  |
| Built-in functions | UNHEX | 使用あり | PM113A | - |  |
| Built-in functions | UNSIGNED | 使用あり | PM115A | - |  |
| Built-in functions | UNSPEC | 使用あり | PA103A, PA104A, PA105A, PD101A, PD102A, PD103A ほか 9 | - |  |
| Built-in functions | UPOS | 未網羅 |  | - |  |
| Built-in functions | UPPERASCII | 未網羅 |  | - |  |
| Built-in functions | UPPERCASE | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | UPPERLATIN1 | 未網羅 |  | - |  |
| Built-in functions | USUBSTR | 未網羅 |  | - |  |
| Built-in functions | USUPPLEMENTARY | 未網羅 |  | - |  |
| Built-in functions | UTCDATETIME | 使用あり | PM114A | - |  |
| Built-in functions | UTCMICROSECS | 使用あり | PM114A | - |  |
| Built-in functions | UTCSECS | 使用あり | PM114A | - |  |
| Built-in functions | UTF8 | 未網羅 |  | - |  |
| Built-in functions | UTF8STG | 未網羅 |  | - |  |
| Built-in functions | UTF8TOCHAR | 未網羅 |  | - |  |
| Built-in functions | UTF8TOWCHAR | 未網羅 |  | - |  |
| Built-in functions | UUID | 未網羅 |  | - |  |
| Built-in functions | UUID4 | 使用あり | PM115A | - |  |
| Built-in functions | UVALID | 未網羅 |  | - |  |
| Built-in functions | UWIDTH | 未網羅 |  | - |  |
| Built-in functions | VALID | 使用あり | PM115A | - |  |
| Built-in functions | VALIDDATE | 使用あり | PM109A | - |  |
| Built-in functions | VALIDVALUE | 未網羅 |  | - |  |
| Built-in functions | VARGLIST | 未網羅 |  | - |  |
| Built-in functions | VARGSIZE | 未網羅 |  | - |  |
| Built-in functions | VERIFY | 使用あり | PM103A, PM110A, PM114A, PM117A | - |  |
| Built-in functions | VERIFYR | 使用あり | PM109A | - |  |
| Built-in functions | WCHARVAL | 未網羅 |  | - |  |
| Built-in functions | WEEKDAY | 使用あり | PM109A | - |  |
| Built-in functions | WHIGH | 使用あり | PM115A | - |  |
| Built-in functions | WHEREDIFF | 使用あり | PM113A | - |  |
| Built-in functions | WIDECHAR | 使用あり | PD106A | - |  |
| Built-in functions | WLOW | 使用あり | PM115A | - |  |
| Built-in functions | WSCOLLAPSE | 使用あり | PM113A | - |  |
| Built-in functions | WSCOLLAPSE16 | 使用あり | PM115A | - |  |
| Built-in functions | WSREPLACE | 使用あり | PM113A | - |  |
| Built-in functions | WSREPLACE16 | 使用あり | PM115A | - |  |
| Built-in functions | XMLCHAR | 未網羅 |  | - |  |
| Built-in functions | XMLSCRUB | 未網羅 |  | - |  |
| Built-in functions | XMLSCRUB16 | 未網羅 |  | - |  |
| Built-in functions | XMLUCHAR | 未網羅 |  | - |  |
| Built-in functions | Y4DATE | 使用あり | PM112A | - |  |
| Built-in functions | Y4JULIAN | 使用あり | PM112A | - |  |
| Built-in functions | Y4YEAR | 使用あり | PM112A | - |  |
| Preprocessor | %ACTIVATE | 使用あり | PM102A, PM106A, PM117A | - |  |
| Preprocessor | %DEACTIVATE | 使用あり | PM101A | - |  |
| Preprocessor | %DECLARE | 使用あり | PM101A, PM102A, PM106A, PM110A, PM117A | - |  |
| Preprocessor | %DO | 使用あり | PM101A, PM110A | - |  |
| Preprocessor | %END | 使用あり | PM101A, PM102A, PM106A, PM110A, PM117A | - |  |
| Preprocessor | %GO TO | 使用あり | PM110A | - |  |
| Preprocessor | %IF | 使用あり | PM101A, PM110A | - |  |
| Preprocessor | %INCLUDE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 50 | - |  |
| Preprocessor | %INSCAN | 使用あり | PM117A | - |  |
| Preprocessor | %ITERATE | 使用あり | PM110A | - |  |
| Preprocessor | %LEAVE | 使用あり | PM110A | - |  |
| Preprocessor | %NOTE | 対象外 |  | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | %REPLACE | 使用あり | PM110A | - |  |
| Preprocessor | %SELECT | 使用あり | PM117A | - |  |
| Preprocessor | %XINCLUDE | 使用あり | PM117A | - |  |
| Preprocessor | %XINSCAN | 使用あり | PM117A | - |  |
| Preprocessor | %PROCEDURE | 使用あり | PM102A, PM106A, PM117A | - |  |
| Preprocessor | RETURN (preprocessor) | 使用あり | PM102A, PM106A, PM117A | - |  |
| Preprocessor | ANSWER (preprocessor) | 使用あり | PM117A | - |  |
| Preprocessor | CALL (preprocessor) | 使用あり | PM117A | - |  |
| Preprocessor | %LINE | 未網羅 |  | - |  |
| Preprocessor | %NOPRINT | 使用あり | PM117A | - |  |
| Preprocessor | %PAGE | 対象外 |  | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | %POP | 対象外 |  | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | %PRINT | 使用あり | PM117A | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | %PROCESS | 使用あり | PM117A | - |  |
| Preprocessor | %PROCINC | 未網羅 |  | - |  |
| Preprocessor | %PUSH | 対象外 |  | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | %SKIP | 対象外 |  | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | STATEMENT | 使用あり | PM106A | - |  |
| Preprocessor built-in functions | COLLATE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | COMMENT | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | COMPILEDATE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | COMPILETIME | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | COPY | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | COUNTER | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | DIMENSION | 未網羅 |  | - |  |
| Preprocessor built-in functions | HBOUND | 未網羅 |  | - |  |
| Preprocessor built-in functions | INDEX | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | LBOUND | 未網羅 |  | - |  |
| Preprocessor built-in functions | LENGTH | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | LOWERCASE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MACCOL | 未網羅 |  | - |  |
| Preprocessor built-in functions | MACLMAR | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MACNAME | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MACRMAR | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MAX | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MIN | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | PARMSET | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | QUOTE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | REPEAT | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SUBSTR | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SYSDIMSIZE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SYSOFFSETSIZE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SYSPARM | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SYSPOINTERSIZE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SYSTEM | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | SYSVERSION | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | TRANSLATE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | TRIM | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | UPPERCASE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | VERIFY | 使用あり | PM117A | - |  |
| Procedure options | OPTIONS(MAIN) | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PC101A ほか 80 | - |  |
| Procedure options | NOEXECOPS | 使用あり | PM108A | - |  |
| Procedure options | FETCHABLE | 使用あり | PCS01A | - |  |
| Procedure options | RECURSIVE | 使用あり | PC102A | - |  |
| Procedure options | REORDER | 未網羅 |  | - |  |
| Procedure options | BYVALUE | 使用あり | PD110A | - |  |
| Procedure options | LINKAGE | 未網羅 |  | - |  |
| Procedure options | ASSEMBLER | 未網羅 |  | - |  |
| Procedure options | COBOL | 使用あり | PM104M | - |  |
| Procedure options | INTER | 対象外 |  | - | 対象外: 互換のために受け付けるだけで、Enterprise PL/I では効果が無い |
| Procedure options | RETCODE | 使用あり | PM104M | - |  |

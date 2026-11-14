# ZPVS 網羅台帳 (Enterprise PL/I for z/OS)

Enterprise PL/I for z/OS Language Reference の章立てから列挙した。 要素 708、使用あり 628、対象外 31、未網羅 49。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |
| --- | --- | --- | --- | --- | --- |
| Statements | ALLOCATE | 使用あり | PC112A, PD111A, PE113N, PE113P, PE114N, PE114P ほか 4 | - |  |
| Statements | ASSERT | 使用あり | PC112A | - |  |
| Statements | ATTACH | 使用あり | PS109A | - |  |
| Statements | BEGIN | 使用あり | PC104A, PC106A, PC107A, PC108A, PC110A, PC111A ほか 16 | - |  |
| Statements | CALL | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 83 | - |  |
| Statements | CLOSE | 使用あり | PF102A, PF103A, PF104M, PF105A, PF106A, PF107A ほか 4 | - |  |
| Statements | DECLARE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 104 | - |  |
| Statements | DEFAULT | 使用あり | PC119A, PD105A, PD108A, PD110A, PF106A, PM114A | - |  |
| Statements | DEFINE ALIAS | 使用あり | PD104A, PD116A | - |  |
| Statements | DEFINE ORDINAL | 使用あり | PC117A, PD105A, PD116A | - |  |
| Statements | DEFINE STRUCTURE | 使用あり | PD104A, PD109A, PS106A, PS111A | - |  |
| Statements | DELAY | 使用あり | PS107A | - |  |
| Statements | DELETE | 使用あり | PF104M | - |  |
| Statements | DETACH | 使用あり | PS109A | - |  |
| Statements | DISPLAY | 未網羅 |  | - |  |
| Statements | DO | 使用あり | PC101A, PC120A, PD115A, PD116A, PE108P, PE109N ほか 9 | - |  |
| Statements | DO WHILE | 使用あり | PC101A, PF107A, PS107A | - |  |
| Statements | DO UNTIL | 使用あり | PC101A | - |  |
| Statements | DO LOOP | 使用あり | PM110A, PM124A | - |  |
| Statements | DO REPEAT | 使用あり | PC108A, PD109A, PM104M, PM117A | - |  |
| Statements | END | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 112 | - |  |
| Statements | ENTRY | 使用あり | PC103A, PC104A, PC116A, PD109A, PD113A, PD114A ほか 4 | - |  |
| Statements | EXIT | 使用あり | PS109A | - |  |
| Statements | FETCH | 使用あり | PC104A | - |  |
| Statements | FLUSH | 使用あり | PS107A | - |  |
| Statements | FORMAT | 使用あり | PM110A | - |  |
| Statements | FREE | 使用あり | PD111A, PS101A, PS102A, PS106A | - |  |
| Statements | GET | 使用あり | PF101A | - |  |
| Statements | GO TO | 使用あり | PC104A, PC106A, PC110A, PC111A, PC112A, PC113A ほか 10 | - |  |
| Statements | IF | 使用あり | PA106A, PC101A, PC102A, PD116A, PM101A, PM110A ほか 2 | - |  |
| Statements | ITERATE | 使用あり | PC101A, PE109N, PE109P, PM110A | - |  |
| Statements | LEAVE | 使用あり | PC101A, PD116A, PE108N, PE108P, PM110A, PM124A | - |  |
| Statements | LOCATE | 使用あり | PE111N, PE111P | - |  |
| Statements | ON | 使用あり | PC104A, PC105A, PC106A, PC107A, PC108A, PC110A ほか 21 | - |  |
| Statements | OPEN | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 81 | - |  |
| Statements | PACKAGE | 使用あり | PD113A, PS108A, PS109A | - |  |
| Statements | PROCEDURE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 112 | - |  |
| Statements | PUT | 使用あり | PF101A, PF102A, PF106A, PM110A, PM115A | - |  |
| Statements | READ | 使用あり | PD110A, PF103A, PF104M, PF105A, PF106A, PF107A ほか 1 | - |  |
| Statements | RELEASE | 使用あり | PC104A | - |  |
| Statements | RESIGNAL | 使用あり | PM110A | - |  |
| Statements | RETURN | 使用あり | PA106A, PC102A, PC103A, PC109A, PC117A, PD113A ほか 10 | - |  |
| Statements | REVERT | 使用あり | PC104A, PC105A, PC110A, PC111A, PC112A, PC113A ほか 8 | - |  |
| Statements | REWRITE | 使用あり | PF104M | - |  |
| Statements | SELECT | 使用あり | PC101A, PM117A | - |  |
| Statements | SIGNAL | 使用あり | PC105A, PC110A, PC112A, PC117A, PM110A, PM112A ほか 1 | - |  |
| Statements | STOP | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 81 | - |  |
| Statements | WAIT | 使用あり | PS109A | - |  |
| Statements | WRITE | 使用あり | PF103A, PF104M, PF105A, PF106A, PF107A, PF108A ほか 3 | - |  |
| Statements | CANCEL THREAD | 使用あり | PS109A | - |  |
| Statements | OTHERWISE | 使用あり | PC101A, PM117A | - |  |
| Statements | QUALIFY | 未網羅 |  | - |  |
| Statements | REINIT | 使用あり | PC109A | - |  |
| Statements | WHEN | 使用あり | PC101A, PC103A, PE112N, PE112P, PM117A | - |  |
| Statements | XDECLARE | 使用あり | PD114A | - |  |
| Statements | XDEFINE ALIAS | 使用あり | PD114A | - |  |
| Statements | XDEFINE ORDINAL | 使用あり | PD114A | - |  |
| Statements | XDEFINE STRUCTURE | 使用あり | PD114A | - |  |
| Statements | XPROCEDURE | 使用あり | PD114A | - |  |
| Data attributes | ALIGNED | 使用あり | PC120A, PD103A | - |  |
| Data attributes | AREA | 使用あり | PC112A, PD109A, PD111A, PE114P, PE115P, PM111A ほか 2 | - |  |
| Data attributes | AUTOMATIC | 使用あり | PC109A | - |  |
| Data attributes | BASED | 使用あり | PA106A, PC112A, PC120A, PD110A, PD111A, PD115A ほか 12 | - |  |
| Data attributes | BIGENDIAN | 使用あり | PD110A | - |  |
| Data attributes | BINARY | 使用あり | PA101A, PA102A, PA104A, PA106A, PC101A, PC102A ほか 78 | - |  |
| Data attributes | BIT | 使用あり | PA102A, PC110A, PC111A, PC112A, PC113A, PC114A ほか 19 | - |  |
| Data attributes | BUILTIN | 使用あり | PD108A, PM109A, PM110A, PM112A | - |  |
| Data attributes | BYADDR | 使用あり | PD113A | - |  |
| Data attributes | BYVALUE | 使用あり | PA106A, PD110A, PS109A | - |  |
| Data attributes | CHARACTER | 使用あり | PA102A, PA104A, PA106A, PC103A, PC107A, PC108A ほか 64 | - |  |
| Data attributes | COMPLEX | 使用あり | PD106A, PE103N, PE103P, PM110A | - |  |
| Data attributes | CONDITION | 使用あり | PC105A, PC110A, PC112A, PD111A, PM112A | - |  |
| Data attributes | CONNECTED | 使用あり | PD113A | - |  |
| Data attributes | CONTROLLED | 使用あり | PE113P, PS102A | - |  |
| Data attributes | DATE | 使用あり | PM105A, PM110A, PM114A, PM117A | - |  |
| Data attributes | DECIMAL | 使用あり | PA101A, PA102A, PA105A, PA106A, PC102A, PC106A ほか 26 | - |  |
| Data attributes | DEFINED | 使用あり | PD113A, PS105A | - |  |
| Data attributes | DIMENSION | 使用あり | PA104A, PD115A, PM120A, PS102A, PS104A | - |  |
| Data attributes | ENTRY | 使用あり | PC103A, PC104A, PC116A, PD109A, PD113A, PD114A ほか 4 | - |  |
| Data attributes | EXTERNAL | 使用あり | PC104A | - |  |
| Data attributes | FILE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 83 | - |  |
| Data attributes | FIXED | 使用あり | PA101A, PA102A, PA104A, PA105A, PA106A, PC101A ほか 84 | - |  |
| Data attributes | FLOAT | 使用あり | PA103A, PC110A, PC120A, PD106A, PD109A, PD110A ほか 7 | - |  |
| Data attributes | FORMAT | 使用あり | PM110A | - |  |
| Data attributes | GENERIC | 使用あり | PC103A, PE112N, PE112P | - |  |
| Data attributes | GRAPHIC | 使用あり | PD106A, PM123A | - |  |
| Data attributes | HANDLE | 使用あり | PD109A, PS106A, PS111A | - |  |
| Data attributes | HEXADEC | 使用あり | PD110A | - |  |
| Data attributes | IEEE | 未網羅 |  | - | 未網羅: Zeus は HFP と big-endian の 2 進しか格納しないので、2026-11-12 から黙って既定の表現にせず断る。表現を実装するまで検査は書けない |
| Data attributes | INITIAL | 使用あり | PA104A, PA106A, PC102A, PC105A, PC106A, PC107A ほか 47 | - |  |
| Data attributes | INTERNAL | 使用あり | PD113A | - |  |
| Data attributes | LABEL | 使用あり | PC101A, PD109A | - |  |
| Data attributes | LIKE | 使用あり | PD104A, PD115A, PE104N, PE104P, PE105N, PE105P | - |  |
| Data attributes | LIMITED | 使用あり | PD113A | - |  |
| Data attributes | LITTLEENDIAN | 未網羅 |  | - | 未網羅: Zeus は HFP と big-endian の 2 進しか格納しないので、2026-11-12 から黙って既定の表現にせず断る。表現を実装するまで検査は書けない |
| Data attributes | NONASSIGNABLE | 使用あり | PD110A | - |  |
| Data attributes | NONVARYING | 使用あり | PD110A | - |  |
| Data attributes | OFFSET | 使用あり | PD109A, PE115N, PE115P, PM111A, PS103A, PS110A | - |  |
| Data attributes | OPTIONAL | 使用あり | PD110A | - |  |
| Data attributes | OPTIONS | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 112 | - |  |
| Data attributes | ORDINAL | 使用あり | PC117A, PD105A, PD114A, PD116A | - |  |
| Data attributes | PARAMETER | 使用あり | PD113A | - |  |
| Data attributes | PICTURE | 使用あり | PA103A, PA105A, PM115A | - |  |
| Data attributes | POINTER | 使用あり | PA106A, PC112A, PC116A, PD111A, PM110A, PM111A ほか 6 | - |  |
| Data attributes | POSITION | 使用あり | PS105A | - |  |
| Data attributes | PRECISION | 使用あり | PC120A, PM110A, PM115A | - |  |
| Data attributes | REAL | 使用あり | PD106A, PM110A | - |  |
| Data attributes | REFER | 使用あり | PE106N, PE106P, PE107N, PE107P, PS104A | - |  |
| Data attributes | RESERVED | 未網羅 |  | - |  |
| Data attributes | RETURNS | 使用あり | PA106A, PC102A, PC103A, PC109A, PD113A, PD114A ほか 7 | - |  |
| Data attributes | SIGNED | 使用あり | PD110A, PM115A | - |  |
| Data attributes | STATIC | 使用あり | PC104A, PC109A, PC116A, PC120A, PCS01A, PD109A ほか 4 | - |  |
| Data attributes | TYPE | 使用あり | PD104A, PD105A, PD114A, PD116A, PF108A, PS106A ほか 1 | - |  |
| Data attributes | UNALIGNED | 使用あり | PD102A, PD103A | - |  |
| Data attributes | UNION | 使用あり | PD112A | - |  |
| Data attributes | UNSIGNED | 使用あり | PC120A, PD107A, PD110A, PM112A, PM114A, PM115A | - |  |
| Data attributes | VALUE | 使用あり | PA105A, PD105A, PD108A, PD109A, PM106A, PM107A ほか 2 | - |  |
| Data attributes | VARIABLE | 使用あり | PC103A, PC116A, PD108A, PD109A, PF108A | - |  |
| Data attributes | VARYING | 使用あり | PC111A, PC112A, PC113A, PC114A, PC118A, PC121A ほか 19 | - |  |
| Data attributes | VARYINGZ | 使用あり | PD110A, PD111A | - |  |
| Data attributes | WIDECHAR | 使用あり | PC113A, PC114A, PD106A, PM118A, PM119A | - |  |
| File attributes and options | BUFFERED | 使用あり | PF107A | - |  |
| File attributes and options | DIRECT | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | ENVIRONMENT | 使用あり | PF102A, PF103A, PF104M, PF105A, PF106A, PF107A ほか 1 | - |  |
| File attributes and options | INPUT | 使用あり | PC107A, PC108A, PC111A, PF103A, PF105A, PF107A ほか 2 | - |  |
| File attributes and options | KEYED | 使用あり | PF104M, PF106A, PF107A | - |  |
| File attributes and options | OUTPUT | 使用あり | PE111N, PE111P, PF102A, PF103A, PF104M, PF105A ほか 5 | - |  |
| File attributes and options | PRINT | 使用あり | PF102A, PF106A, PF108A, PM117A | - |  |
| File attributes and options | RECORD | 使用あり | PC111A, PE111N, PE111P, PF103A, PF104M, PF105A ほか 5 | - |  |
| File attributes and options | SEQUENTIAL | 使用あり | PF107A, PF108A, PS107A | - |  |
| File attributes and options | STREAM | 使用あり | PF102A, PF106A, PF108A, PM115A, PM116A | - |  |
| File attributes and options | UPDATE | 使用あり | PF104M, PF106A | - |  |
| File attributes and options | TITLE | 使用あり | PC104A, PC111A, PF102A, PF103A, PF104M, PF105A ほか 5 | - |  |
| File attributes and options | LINESIZE | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | PAGESIZE | 使用あり | PF102A, PF106A | - |  |
| File attributes and options | KEY | 使用あり | PF104M, PF106A, PF107A | - |  |
| File attributes and options | KEYFROM | 使用あり | PF104M, PF106A, PF107A | - |  |
| File attributes and options | KEYTO | 使用あり | PF107A | - |  |
| File attributes and options | SET | 使用あり | PC112A, PS103A | - |  |
| File attributes and options | INTO | 使用あり | PF103A, PF104M, PF105A, PF106A, PF107A, PS107A | - |  |
| File attributes and options | FROM | 使用あり | PF103A, PF104M, PF105A, PF106A, PF107A, PF108A ほか 4 | - |  |
| File attributes and options | PAGE | 使用あり | PF102A | - |  |
| File attributes and options | SKIP | 使用あり | PF102A, PF106A, PM117A, PM119A | - |  |
| File attributes and options | LINE | 使用あり | PF102A | - |  |
| File attributes and options | LIST | 使用あり | PD116A, PF101A, PM115A | - |  |
| File attributes and options | EDIT | 使用あり | PF101A, PF102A, PF106A, PM110A, PS104A | - |  |
| File attributes and options | DATA | 使用あり | PF101A, PS101A, PS103A, PS110A | - |  |
| File attributes and options | STRING | 使用あり | PF101A, PM110A, PM124A | - |  |
| File attributes and options | COPY | 使用あり | PC103A, PD104A, PD110A, PM103A, PM117A, PS103A | - |  |
| Conditions | AREA | 使用あり | PC112A, PD109A, PD111A, PE114P, PE115P, PM111A ほか 2 | - |  |
| Conditions | ATTENTION | 対象外 |  | - | 対象外: 端末の注意割込みで、バッチでは起こせない |
| Conditions | CONDITION | 使用あり | PC105A, PC110A, PC112A, PD111A, PM112A | - |  |
| Conditions | CONVERSION | 使用あり | PC107A, PC108A, PC110A, PC122A, PM123A | - |  |
| Conditions | ENDFILE | 使用あり | PF103A, PF105A, PF107A, PS107A | - |  |
| Conditions | ENDPAGE | 使用あり | PF102A, PF106A | - |  |
| Conditions | ERROR | 使用あり | PC104A, PC110A, PC113A, PC114A, PC117A, PC118A ほか 3 | - |  |
| Conditions | FINISH | 使用あり | PC112A | - |  |
| Conditions | FIXEDOVERFLOW | 使用あり | PC111A | - |  |
| Conditions | INVALIDOP | 使用あり | PC112A | - |  |
| Conditions | KEY | 使用あり | PF104M, PF106A, PF107A | - |  |
| Conditions | NAME | 使用あり | PA104A, PF101A, PM106A, PM112A, PS110A | - |  |
| Conditions | OVERFLOW | 使用あり | PC110A | - |  |
| Conditions | RECORD | 使用あり | PC111A, PE111N, PE111P, PF103A, PF104M, PF105A ほか 5 | - |  |
| Conditions | SIZE | 使用あり | PC106A, PC110A, PD103A, PD109A, PD112A, PM109A ほか 1 | - |  |
| Conditions | STORAGE | 使用あり | PC120A, PC121A, PD103A, PD104A, PD105A, PD106A ほか 7 | - |  |
| Conditions | STRINGRANGE | 使用あり | PC110A | - |  |
| Conditions | STRINGSIZE | 使用あり | PC111A | - |  |
| Conditions | SUBSCRIPTRANGE | 使用あり | PC110A, PC112A | - |  |
| Conditions | TRANSMIT | 対象外 |  | - | 対象外: 入出力の伝送エラーで、決まった形で起こせない |
| Conditions | UNDEFINEDFILE | 使用あり | PC111A, PM115A | - |  |
| Conditions | UNDERFLOW | 使用あり | PC110A | - |  |
| Conditions | ZERODIVIDE | 使用あり | PC106A, PC110A, PC112A, PM112A, PM115A | - |  |
| Conditions | ANYCONDITION | 使用あり | PC112A | - |  |
| Conditions | ASSERTION | 使用あり | PC112A | - |  |
| Conditions | CONFORMANCE | 使用あり | PC117A | - |  |
| Built-in functions | ABS | 使用あり | PD106A, PD109A, PM105A, PM109A, PM110A, PM112A ほか 1 | - |  |
| Built-in functions | ACOS | 使用あり | PM110A | - |  |
| Built-in functions | ADD | 使用あり | PA101A | - |  |
| Built-in functions | ADDR | 使用あり | PA106A, PC114A, PC115A, PC120A, PD110A, PD111A ほか 10 | - |  |
| Built-in functions | ADDRDATA | 使用あり | PM111A | - |  |
| Built-in functions | ALL | 使用あり | PM109A, PM124A | - |  |
| Built-in functions | ALLCOMPARE | 使用あり | PC119A | - |  |
| Built-in functions | ALLOC31 | 使用あり | PD111A | - |  |
| Built-in functions | ALLOCATE | 使用あり | PD111A | - |  |
| Built-in functions | ALLOCATION | 使用あり | PS102A | - |  |
| Built-in functions | ALLOCNEXT | 使用あり | PD111A | - |  |
| Built-in functions | ALLOCSIZE | 使用あり | PD111A | - |  |
| Built-in functions | ANY | 使用あり | PM109A | - |  |
| Built-in functions | ASIN | 使用あり | PM110A | - |  |
| Built-in functions | ATAN | 使用あり | PM109A | - |  |
| Built-in functions | ATAND | 使用あり | PM109A | - |  |
| Built-in functions | ATANH | 使用あり | PM109A | - |  |
| Built-in functions | AUTOMATIC | 使用あり | PC109A | - |  |
| Built-in functions | AVAILABLEAREA | 使用あり | PD111A | - |  |
| Built-in functions | BASE64DECODE | 使用あり | PM113A | - |  |
| Built-in functions | BASE64DECODE8 | 使用あり | PM115A | - |  |
| Built-in functions | BASE64DECODE16 | 使用あり | PM115A | - |  |
| Built-in functions | BASE64ENCODE | 使用あり | PM113A | - |  |
| Built-in functions | BASE64ENCODE8 | 使用あり | PM113A | - |  |
| Built-in functions | BASE64ENCODE16 | 使用あり | PM115A | - |  |
| Built-in functions | BETWEEN | 使用あり | PD116A, PM112A | - |  |
| Built-in functions | BETWEENEXCLUSIVE | 使用あり | PM112A | - |  |
| Built-in functions | BETWEENLEFTEXCLUSIVE | 使用あり | PM112A | - |  |
| Built-in functions | BETWEENRIGHTEXCLUSIVE | 使用あり | PM112A | - |  |
| Built-in functions | BINARY | 使用あり | PA101A, PA102A, PA104A, PA106A, PC101A, PC102A ほか 78 | - |  |
| Built-in functions | BINARYVALUE | 使用あり | PD114A, PM110A, PM111A | - |  |
| Built-in functions | BINSEARCH | 使用あり | PA106A | - |  |
| Built-in functions | BINSEARCHX | 使用あり | PA106A | - |  |
| Built-in functions | BIT | 使用あり | PA102A, PC110A, PC111A, PC112A, PC113A, PC114A ほか 19 | - |  |
| Built-in functions | BITLOCATION | 使用あり | PM111A | - |  |
| Built-in functions | BOOL | 使用あり | PA102A | - |  |
| Built-in functions | BYTE | 使用あり | PM109A | - |  |
| Built-in functions | BYTELENGTH | 使用あり | PC121A | - |  |
| Built-in functions | CDS | 使用あり | PM111A | - |  |
| Built-in functions | CEIL | 使用あり | PM103A | - |  |
| Built-in functions | CENTERLEFT | 使用あり | PM103A, PM110A | - |  |
| Built-in functions | CENTERRIGHT | 使用あり | PM115A | - |  |
| Built-in functions | CENTRELEFT | 使用あり | PM103A, PM110A | - |  |
| Built-in functions | CENTRERIGHT | 使用あり | PM115A | - |  |
| Built-in functions | CHARACTER | 使用あり | PA102A, PA104A, PA106A, PC103A, PC107A, PC108A ほか 64 | - |  |
| Built-in functions | CHARGRAPHIC | 使用あり | PM123A | - |  |
| Built-in functions | CHARVAL | 使用あり | PM109A | - |  |
| Built-in functions | CHECKSTG | 使用あり | PD111A | - |  |
| Built-in functions | CHECKSUM | 未網羅 |  | - |  |
| Built-in functions | CODEPAGE | 使用あり | PM118A | - |  |
| Built-in functions | COLLATE | 使用あり | PC120A, PM109A | - |  |
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
| Built-in functions | DECIMAL | 使用あり | PA101A, PA102A, PA105A, PA106A, PC102A, PC106A ほか 26 | - |  |
| Built-in functions | DIMENSION | 使用あり | PA104A, PD115A, PM120A, PS102A, PS104A | - |  |
| Built-in functions | DIVIDE | 使用あり | PA101A | - |  |
| Built-in functions | EDIT | 使用あり | PF101A, PF102A, PF106A, PM110A | - |  |
| Built-in functions | EMPTY | 使用あり | PS103A | - |  |
| Built-in functions | ENDFILE | 使用あり | PF103A, PF105A, PF107A, PS107A | - |  |
| Built-in functions | ENTRYADDR | 使用あり | PC116A | - |  |
| Built-in functions | EPSILON | 使用あり | PM109A | - |  |
| Built-in functions | ERF | 使用あり | PM112A | - |  |
| Built-in functions | ERFC | 使用あり | PM112A | - |  |
| Built-in functions | EXP | 使用あり | PM109A | - |  |
| Built-in functions | EXPONENT | 使用あり | PM111A | - |  |
| Built-in functions | FILEDDINT | 使用あり | PF108A | - |  |
| Built-in functions | FILEDDTEST | 使用あり | PF108A | - |  |
| Built-in functions | FILEDDWORD | 使用あり | PF108A | - |  |
| Built-in functions | FILEID | 使用あり | PF108A | - |  |
| Built-in functions | FILENEW | 使用あり | PF108A | - |  |
| Built-in functions | FILEOPEN | 使用あり | PF108A | - |  |
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
| Built-in functions | FOLDEDFULLMATCH | 使用あり | PC121A | - |  |
| Built-in functions | FOLDEDSIMPLEMATCH | 使用あり | PC121A | - |  |
| Built-in functions | GAMMA | 使用あり | PM112A | - |  |
| Built-in functions | GETENV | 使用あり | PM115A | - |  |
| Built-in functions | GETJCLSYMBOL | 未網羅 |  | - |  |
| Built-in functions | GETSYSINT | 未網羅 |  | - |  |
| Built-in functions | GETSYSWORD | 未網羅 |  | - |  |
| Built-in functions | GRAPHIC | 使用あり | PD106A, PM123A | - |  |
| Built-in functions | GTCA | 未網羅 |  | - |  |
| Built-in functions | HANDLE | 使用あり | PS106A, PS111A | - |  |
| Built-in functions | HBOUND | 使用あり | PC118A, PD115A, PM109A, PM120A | - |  |
| Built-in functions | HBOUNDACROSS | 使用あり | PD115A | - |  |
| Built-in functions | HEX | 使用あり | PA103A, PA105A, PD101A, PD102A, PD103A, PD106A ほか 8 | - |  |
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
| Built-in functions | INARRAY | 使用あり | PM118A | - |  |
| Built-in functions | INDEX | 使用あり | PF101A, PM103A, PM115A, PM117A | - |  |
| Built-in functions | INDEXR | 使用あり | PM113A | - |  |
| Built-in functions | INDICATORS | 使用あり | PC118A | - |  |
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
| Built-in functions | JSONGETARRAYEND | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETARRAYSTART | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETCOLON | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETCOMMA | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETMEMBER | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETOBJECTEND | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETOBJECTSTART | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONGETVALUE | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTARRAYEND | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTARRAYSTART | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTCOLON | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTCOMMA | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTMEMBER | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTOBJECTEND | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTOBJECTSTART | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONPUTVALUE | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JSONVALID | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | JULIANTOSMF | 使用あり | PM114A | - |  |
| Built-in functions | LBOUND | 使用あり | PD115A, PM109A, PM120A | - |  |
| Built-in functions | LBOUNDACROSS | 使用あり | PD115A | - |  |
| Built-in functions | LEFT | 使用あり | PM103A | - |  |
| Built-in functions | LENGTH | 使用あり | PC103A, PC121A, PC122A, PD102A, PD110A, PM103A ほか 10 | - |  |
| Built-in functions | LINENO | 使用あり | PF102A, PF106A | - |  |
| Built-in functions | LOCATION | 使用あり | PM112A | - |  |
| Built-in functions | LOCNEWSPACE | 使用あり | PS110A | - |  |
| Built-in functions | LOCNEWVALUE | 使用あり | PS110A | - |  |
| Built-in functions | LOCSTG | 使用あり | PS110A | - |  |
| Built-in functions | LOCVAL | 使用あり | PS110A | - |  |
| Built-in functions | LOG | 使用あり | PM105A | - |  |
| Built-in functions | LOGGAMMA | 使用あり | PM112A | - |  |
| Built-in functions | LOG2 | 使用あり | PM109A | - |  |
| Built-in functions | LOG10 | 使用あり | PM109A | - |  |
| Built-in functions | LOW | 使用あり | PD110A, PM109A | - |  |
| Built-in functions | LOWERASCII | 使用あり | PC121A | - |  |
| Built-in functions | LOWERCASE | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | LOWERLATIN1 | 使用あり | PC121A | - |  |
| Built-in functions | LOWER2 | 使用あり | PM112A | - |  |
| Built-in functions | MAINNAME | 使用あり | PM115A | - |  |
| Built-in functions | MAX | 使用あり | PM105A, PM117A | - |  |
| Built-in functions | MAXDATE | 使用あり | PM114A | - |  |
| Built-in functions | MAXEXP | 使用あり | PM109A | - |  |
| Built-in functions | MAXLENGTH | 使用あり | PD102A | - |  |
| Built-in functions | MAXVAL | 使用あり | PM112A | - |  |
| Built-in functions | MEMCOLLAPSE | 使用あり | PM113A | - |  |
| Built-in functions | MEMCONVERT | 使用あり | PC115A | - |  |
| Built-in functions | MEMCU12 | 使用あり | PC115A | - |  |
| Built-in functions | MEMCU14 | 使用あり | PC115A | - |  |
| Built-in functions | MEMCU21 | 使用あり | PC115A | - |  |
| Built-in functions | MEMCU24 | 使用あり | PC115A | - |  |
| Built-in functions | MEMCU41 | 使用あり | PC115A | - |  |
| Built-in functions | MEMCU42 | 使用あり | PC115A | - |  |
| Built-in functions | MEMINDEX | 使用あり | PM113A | - |  |
| Built-in functions | MEMREPLACE | 使用あり | PM113A | - |  |
| Built-in functions | MEMSEARCH | 使用あり | PM113A | - |  |
| Built-in functions | MEMSEARCHR | 使用あり | PM113A | - |  |
| Built-in functions | MEMSQUEEZE | 使用あり | PM113A | - |  |
| Built-in functions | MEMUVALID8 | 使用あり | PC114A | - |  |
| Built-in functions | MEMUVALID16 | 使用あり | PC114A | - |  |
| Built-in functions | MEMVERIFY | 使用あり | PM113A | - |  |
| Built-in functions | MEMVERIFYR | 使用あり | PM113A | - |  |
| Built-in functions | MICROSECS | 使用あり | PM114A, PS107A | - |  |
| Built-in functions | MICROSECSTODATE | 使用あり | PM114A | - |  |
| Built-in functions | MICROSECSTODAYS | 使用あり | PM114A | - |  |
| Built-in functions | MIN | 使用あり | PM105A, PM117A | - |  |
| Built-in functions | MINDATE | 使用あり | PM114A | - |  |
| Built-in functions | MINEXP | 使用あり | PM109A | - |  |
| Built-in functions | MINVAL | 使用あり | PM112A | - |  |
| Built-in functions | MOD | 使用あり | PC101A, PC120A, PM103A | - |  |
| Built-in functions | MPSTR | 使用あり | PM123A | - |  |
| Built-in functions | MULTIPLY | 使用あり | PA101A | - |  |
| Built-in functions | NULL | 使用あり | PD109A, PS101A | - |  |
| Built-in functions | NULLENTRY | 使用あり | PC116A | - |  |
| Built-in functions | OFFSET | 使用あり | PD109A, PE115N, PE115P, PM111A, PS103A, PS110A | - |  |
| Built-in functions | OFFSETADD | 使用あり | PM111A | - |  |
| Built-in functions | OFFSETDIFF | 使用あり | PM111A | - |  |
| Built-in functions | OFFSETSUBTRACT | 使用あり | PM111A | - |  |
| Built-in functions | OFFSETVALUE | 使用あり | PM111A | - |  |
| Built-in functions | OMITTED | 使用あり | PD110A | - |  |
| Built-in functions | ONACTUAL | 使用あり | PC112A | - |  |
| Built-in functions | ONAREA | 使用あり | PC112A | - |  |
| Built-in functions | ONCHAR | 使用あり | PC107A | - |  |
| Built-in functions | ONCODE | 使用あり | PC104A, PC106A, PC110A, PC111A, PC112A, PC117A ほか 4 | - |  |
| Built-in functions | ONCONDCOND | 使用あり | PM112A | - |  |
| Built-in functions | ONCONDID | 使用あり | PC112A, PM112A | - |  |
| Built-in functions | ONCOUNT | 使用あり | PM110A | - |  |
| Built-in functions | ONEXPECTED | 使用あり | PC112A | - |  |
| Built-in functions | ONFILE | 使用あり | PM115A | - |  |
| Built-in functions | ONGSOURCE | 未網羅 |  | - |  |
| Built-in functions | ONHBOUND | 使用あり | PC112A | - |  |
| Built-in functions | ONJSONNAME | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | ONKEY | 使用あり | PF107A | - |  |
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
| Built-in functions | ONUCHAR | 使用あり | PC122A | - |  |
| Built-in functions | ONUSOURCE | 使用あり | PC122A | - |  |
| Built-in functions | ONWCHAR | 使用あり | PC122A | - |  |
| Built-in functions | ONWSOURCE | 使用あり | PC122A | - |  |
| Built-in functions | ORDINALNAME | 使用あり | PD105A | - |  |
| Built-in functions | ORDINALPRED | 使用あり | PC117A, PD105A | - |  |
| Built-in functions | ORDINALSUCC | 使用あり | PC117A, PD105A | - |  |
| Built-in functions | PACKAGENAME | 使用あり | PM115A, PS108A | - |  |
| Built-in functions | PAGENO | 使用あり | PF102A, PF106A | - |  |
| Built-in functions | PICSPEC | 使用あり | PM118A | - |  |
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
| Built-in functions | PLIRETC | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 81 | - |  |
| Built-in functions | PLIRETV | 使用あり | PM104M, PM107A, PM108A | - |  |
| Built-in functions | PLISAXA | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | PLISAXB | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | PLISAXC | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | PLISAXD | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
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
| Built-in functions | PLITRAN11 | 使用あり | PC120A | - |  |
| Built-in functions | PLITRAN12 | 使用あり | PC120A | - |  |
| Built-in functions | PLITRAN21 | 使用あり | PC120A | - |  |
| Built-in functions | PLITRAN22 | 使用あり | PC120A | - |  |
| Built-in functions | POINTER | 使用あり | PA106A, PC112A, PC116A, PD111A, PM110A, PM111A ほか 6 | - |  |
| Built-in functions | POINTERADD | 使用あり | PM115A, PM116A, PS101A | - |  |
| Built-in functions | POINTERDIFF | 使用あり | PM109A | - |  |
| Built-in functions | POINTERSUBTRACT | 使用あり | PM111A | - |  |
| Built-in functions | POINTERVALUE | 使用あり | PM110A, PM111A | - |  |
| Built-in functions | POLY | 使用あり | PM115A | - |  |
| Built-in functions | POPCNT | 使用あり | PM112A | - |  |
| Built-in functions | PRECISION | 使用あり | PC120A, PM110A, PM115A | - |  |
| Built-in functions | PRECVAL | 使用あり | PM112A | - |  |
| Built-in functions | PRED | 使用あり | PM112A | - |  |
| Built-in functions | PRESENT | 使用あり | PD110A | - |  |
| Built-in functions | PROCEDURENAME | 使用あり | PM115A | - |  |
| Built-in functions | PROD | 使用あり | PM109A | - |  |
| Built-in functions | PUTENV | 使用あり | PM115A | - |  |
| Built-in functions | QUICKSORT | 使用あり | PA106A | - |  |
| Built-in functions | QUICKSORTX | 使用あり | PA106A | - |  |
| Built-in functions | RADIX | 使用あり | PM109A | - |  |
| Built-in functions | RAISE2 | 使用あり | PM109A | - |  |
| Built-in functions | RANDOM | 使用あり | PD109A | - |  |
| Built-in functions | RANK | 使用あり | PM111A, PM118A | - |  |
| Built-in functions | REAL | 使用あり | PD106A, PM110A | - |  |
| Built-in functions | REGEX | 使用あり | PM124A | - |  |
| Built-in functions | REM | 使用あり | PM103A | - |  |
| Built-in functions | REPATTERN | 使用あり | PC118A | - |  |
| Built-in functions | REPEAT | 使用あり | PD109A, PM117A | - |  |
| Built-in functions | REPLACE | 使用あり | PM113A | - |  |
| Built-in functions | REPLACEBY2 | 使用あり | PM112A | - |  |
| Built-in functions | REVERSE | 使用あり | PM103A | - |  |
| Built-in functions | RIGHT | 使用あり | PM103A | - |  |
| Built-in functions | ROUND | 使用あり | PM103A | - |  |
| Built-in functions | ROUNDAWAYFROMZERO | 使用あり | PM112A | - |  |
| Built-in functions | ROUNDTOEVEN | 使用あり | PM112A | - |  |
| Built-in functions | SAMEKEY | 使用あり | PF107A | - |  |
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
| Built-in functions | SQRT | 使用あり | PM105A, PM122A | - |  |
| Built-in functions | SQRTF | 使用あり | PM122A | - |  |
| Built-in functions | SQUEEZE | 使用あり | PM113A | - |  |
| Built-in functions | STACKADDR | 未網羅 |  | - |  |
| Built-in functions | STCKETODATE | 使用あり | PM114A | - |  |
| Built-in functions | STCKTODATE | 使用あり | PM114A | - |  |
| Built-in functions | STORAGE | 使用あり | PC120A, PC121A, PD103A, PD104A, PD105A, PD106A ほか 7 | - |  |
| Built-in functions | STRING | 使用あり | PF101A, PM110A | - |  |
| Built-in functions | SUBSTR | 使用あり | PA104A, PC110A, PC115A, PC120A, PD102A, PD108A ほか 10 | - |  |
| Built-in functions | SUBTO | 使用あり | PM115A | - |  |
| Built-in functions | SUBTRACT | 使用あり | PM110A | - |  |
| Built-in functions | SUCC | 使用あり | PM112A | - |  |
| Built-in functions | SUM | 使用あり | PA104A, PS102A, PS105A | - |  |
| Built-in functions | SYSNULL | 使用あり | PC116A, PD109A, PD111A, PM113A, PM116A, PM119A ほか 2 | - |  |
| Built-in functions | SYSTEM | 未網羅 |  | - |  |
| Built-in functions | TALLY | 使用あり | PM103A | - |  |
| Built-in functions | TAN | 使用あり | PM110A | - |  |
| Built-in functions | TAND | 使用あり | PM109A | - |  |
| Built-in functions | TANH | 使用あり | PM109A | - |  |
| Built-in functions | THREADID | 使用あり | PS109A | - |  |
| Built-in functions | TIME | 使用あり | PM110A | - |  |
| Built-in functions | TIMESTAMP | 使用あり | PM114A | - |  |
| Built-in functions | TINY | 使用あり | PM109A | - |  |
| Built-in functions | TRANSLATE | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | TRIM | 使用あり | PA102A, PC112A, PD115A, PM103A, PM117A | - |  |
| Built-in functions | TRUNC | 使用あり | PM109A | - |  |
| Built-in functions | TYPE | 使用あり | PS111A | - |  |
| Built-in functions | UHIGH | 未網羅 |  | - |  |
| Built-in functions | ULENGTH | 使用あり | PC113A | - |  |
| Built-in functions | ULENGTH8 | 使用あり | PC113A | - |  |
| Built-in functions | ULENGTH16 | 使用あり | PC113A | - |  |
| Built-in functions | ULOW | 未網羅 |  | - |  |
| Built-in functions | UNALLOCATED | 使用あり | PD111A | - |  |
| Built-in functions | UNHEX | 使用あり | PM113A | - |  |
| Built-in functions | UNSIGNED | 使用あり | PM115A | - |  |
| Built-in functions | UNSPEC | 使用あり | PA103A, PA104A, PA105A, PC117A, PD101A, PD102A ほか 11 | - |  |
| Built-in functions | UPOS | 使用あり | PC113A | - |  |
| Built-in functions | UPPERASCII | 使用あり | PC121A | - |  |
| Built-in functions | UPPERCASE | 使用あり | PM103A, PM117A | - |  |
| Built-in functions | UPPERLATIN1 | 使用あり | PC121A | - |  |
| Built-in functions | USUBSTR | 使用あり | PC113A | - |  |
| Built-in functions | USUPPLEMENTARY | 使用あり | PC113A | - |  |
| Built-in functions | UTCDATETIME | 使用あり | PM114A | - |  |
| Built-in functions | UTCMICROSECS | 使用あり | PM114A | - |  |
| Built-in functions | UTCSECS | 使用あり | PM114A | - |  |
| Built-in functions | UTF8 | 使用あり | PC114A | - |  |
| Built-in functions | UTF8STG | 使用あり | PC120A | - |  |
| Built-in functions | UTF8TOCHAR | 使用あり | PC114A | - |  |
| Built-in functions | UTF8TOWCHAR | 使用あり | PC114A | - |  |
| Built-in functions | UUID | 未網羅 |  | - |  |
| Built-in functions | UUID4 | 使用あり | PM115A | - |  |
| Built-in functions | UVALID | 使用あり | PC113A | - |  |
| Built-in functions | UWIDTH | 使用あり | PC113A | - |  |
| Built-in functions | VALID | 使用あり | PM115A | - |  |
| Built-in functions | VALIDDATE | 使用あり | PM109A | - |  |
| Built-in functions | VALIDVALUE | 使用あり | PD116A | - |  |
| Built-in functions | VARGLIST | 未網羅 |  | - |  |
| Built-in functions | VARGSIZE | 未網羅 |  | - |  |
| Built-in functions | VERIFY | 使用あり | PM103A, PM110A, PM114A, PM117A | - |  |
| Built-in functions | VERIFYR | 使用あり | PM109A | - |  |
| Built-in functions | WCHARVAL | 使用あり | PM118A | - |  |
| Built-in functions | WEEKDAY | 使用あり | PM109A | - |  |
| Built-in functions | WHIGH | 使用あり | PM115A | - |  |
| Built-in functions | WHEREDIFF | 使用あり | PM113A | - |  |
| Built-in functions | WIDECHAR | 使用あり | PD106A, PM118A, PM119A | - |  |
| Built-in functions | WLOW | 使用あり | PM115A | - |  |
| Built-in functions | WSCOLLAPSE | 使用あり | PM113A | - |  |
| Built-in functions | WSCOLLAPSE16 | 使用あり | PM115A | - |  |
| Built-in functions | WSREPLACE | 使用あり | PM113A | - |  |
| Built-in functions | WSREPLACE16 | 使用あり | PM115A | - |  |
| Built-in functions | XMLCHAR | 使用あり | PM119A | - |  |
| Built-in functions | XMLSCRUB | 使用あり | PM119A | - |  |
| Built-in functions | XMLSCRUB16 | 使用あり | PM119A | - |  |
| Built-in functions | XMLUCHAR | 対象外 |  | - | 対象外: JSON/XML の対応は範囲の外とする (利用者の判断、2026-11-13) |
| Built-in functions | Y4DATE | 使用あり | PM112A | - |  |
| Built-in functions | Y4JULIAN | 使用あり | PM112A | - |  |
| Built-in functions | Y4YEAR | 使用あり | PM112A | - |  |
| Preprocessor | %ACTIVATE | 使用あり | PM102A, PM106A, PM117A | - |  |
| Preprocessor | %DEACTIVATE | 使用あり | PM101A | - |  |
| Preprocessor | %DECLARE | 使用あり | PD114A, PM101A, PM102A, PM106A, PM110A, PM117A ほか 2 | - |  |
| Preprocessor | %DO | 使用あり | PM101A, PM110A | - |  |
| Preprocessor | %END | 使用あり | PD114A, PM101A, PM102A, PM106A, PM110A, PM117A ほか 2 | - |  |
| Preprocessor | %GO TO | 使用あり | PM110A | - |  |
| Preprocessor | %IF | 使用あり | PM101A, PM110A | - |  |
| Preprocessor | %INCLUDE | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 81 | - |  |
| Preprocessor | %INSCAN | 使用あり | PM117A | - |  |
| Preprocessor | %ITERATE | 使用あり | PM110A | - |  |
| Preprocessor | %LEAVE | 使用あり | PM110A | - |  |
| Preprocessor | %NOTE | 対象外 |  | - | 対象外: リストと診断の制御で、実行時の値に出ない |
| Preprocessor | %REPLACE | 使用あり | PM110A | - |  |
| Preprocessor | %SELECT | 使用あり | PM117A | - |  |
| Preprocessor | %XINCLUDE | 使用あり | PM117A | - |  |
| Preprocessor | %XINSCAN | 使用あり | PM117A | - |  |
| Preprocessor | %PROCEDURE | 使用あり | PM102A, PM106A, PM117A, PM120A, PM121A | - |  |
| Preprocessor | RETURN (preprocessor) | 使用あり | PM102A, PM106A, PM117A, PM120A, PM121A | - |  |
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
| Preprocessor built-in functions | DIMENSION | 使用あり | PM120A | - |  |
| Preprocessor built-in functions | HBOUND | 使用あり | PM120A | - |  |
| Preprocessor built-in functions | INDEX | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | LBOUND | 使用あり | PM120A | - |  |
| Preprocessor built-in functions | LENGTH | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | LOWERCASE | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MACCOL | 使用あり | PM121A | - |  |
| Preprocessor built-in functions | MACLMAR | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MACNAME | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MACRMAR | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MAX | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | MIN | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | PARMSET | 使用あり | PM117A | - |  |
| Preprocessor built-in functions | QUOTE | 使用あり | PM117A, PM120A | - |  |
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
| Procedure options | OPTIONS(MAIN) | 使用あり | PA101A, PA102A, PA103A, PA104A, PA105A, PA106A ほか 111 | - |  |
| Procedure options | NOEXECOPS | 使用あり | PM108A | - |  |
| Procedure options | FETCHABLE | 使用あり | PCS01A | - |  |
| Procedure options | RECURSIVE | 使用あり | PC102A | - |  |
| Procedure options | REORDER | 未網羅 |  | - |  |
| Procedure options | BYVALUE | 使用あり | PA106A, PD110A, PS109A | - |  |
| Procedure options | LINKAGE | 使用あり | PA106A | - |  |
| Procedure options | ASSEMBLER | 未網羅 |  | - |  |
| Procedure options | COBOL | 使用あり | PM104M | - |  |
| Procedure options | INTER | 対象外 |  | - | 対象外: 互換のために受け付けるだけで、Enterprise PL/I では効果が無い |
| Procedure options | RETCODE | 使用あり | PM104M | - |  |

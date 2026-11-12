# ZCVS 網羅台帳 (Enterprise COBOL for z/OS)

Enterprise COBOL for z/OS Language Reference の章立てから列挙した。CCVS85 の列は、その要素を使う CCVS85 のプログラム数 (道具に newcob.val を渡したときだけ数える)。 要素 337、使用あり 237、未網羅 100。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | - | 確認 |
| --- | --- | --- | --- | --- | --- |
| Statements | ACCEPT | 未網羅 |  | - |  |
| Statements | ADD | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA109A ほか 29 | - |  |
| Statements | ALLOCATE | 未網羅 |  | - |  |
| Statements | ALTER | 使用あり | ZE110N | - |  |
| Statements | CALL | 使用あり | ZP101A, ZP102A, ZP103A, ZP104A, ZP105A, ZP106A ほか 13 | - |  |
| Statements | CANCEL | 使用あり | ZA110A, ZP101A, ZP109A, ZP112A | - |  |
| Statements | CLOSE | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 11 | - |  |
| Statements | COMPUTE | 使用あり | ZA102A, ZA104A, ZA105A, ZA107A, ZA108A, ZA109A ほか 23 | - |  |
| Statements | CONTINUE | 使用あり | ZE105N, ZE132N, ZE132P | - |  |
| Statements | DELETE | 使用あり | ZF107A, ZF108A, ZF110A, ZF112A | - |  |
| Statements | DISPLAY | 使用あり | ZA120A | - |  |
| Statements | DIVIDE | 使用あり | ZA102A, ZA104A, ZA105A, ZA109A, ZA119A, ZA123A | - |  |
| Statements | ENTRY | 使用あり | ZE113N, ZPS04B, ZPS11A | - |  |
| Statements | EVALUATE | 使用あり | ZS108A | - |  |
| Statements | EXIT PROGRAM | 未網羅 |  | - |  |
| Statements | EXIT PERFORM | 使用あり | ZX101A, ZX105A | - |  |
| Statements | EXIT PARAGRAPH | 使用あり | ZX101A | - |  |
| Statements | EXIT SECTION | 使用あり | ZX101A | - |  |
| Statements | FREE | 未網羅 |  | - |  |
| Statements | GOBACK | 使用あり | ZE102N, ZE110N, ZE111N, ZE112N, ZE113N, ZP106A ほか 21 | - |  |
| Statements | GO TO | 使用あり | ZE110N, ZE111N | - |  |
| Statements | GO TO DEPENDING | 使用あり | ZD105A, ZD113A, ZE144N, ZE144P, ZF101A, ZF102A ほか 2 | - |  |
| Statements | IF | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 100 | - |  |
| Statements | INITIALIZE | 使用あり | ZD107A, ZE106N, ZE107N, ZE108N, ZN102B, ZN103A ほか 7 | - |  |
| Statements | INSPECT TALLYING | 使用あり | ZS106A | - |  |
| Statements | INSPECT REPLACING | 未網羅 |  | - |  |
| Statements | INSPECT CONVERTING | 未網羅 |  | - |  |
| Statements | INVOKE | 未網羅 |  | - |  |
| Statements | JSON GENERATE | 未網羅 |  | - |  |
| Statements | JSON PARSE | 未網羅 |  | - |  |
| Statements | MERGE | 未網羅 |  | - |  |
| Statements | MOVE | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 113 | - |  |
| Statements | MOVE CORRESPONDING | 未網羅 |  | - |  |
| Statements | MULTIPLY | 使用あり | ZA102A, ZA109A, ZA119A | - |  |
| Statements | OPEN | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 11 | - |  |
| Statements | PERFORM | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 100 | - |  |
| Statements | PERFORM TIMES | 使用あり | ZX101A | - |  |
| Statements | PERFORM UNTIL | 使用あり | ZE105N, ZE132N, ZE132P, ZF114A, ZX101A, ZX105A ほか 1 | - |  |
| Statements | PERFORM VARYING | 使用あり | ZE132N, ZE132P, ZF114A, ZX101A, ZX105A, ZX110A | - |  |
| Statements | PERFORM AFTER | 使用あり | ZX110A | - |  |
| Statements | WITH TEST AFTER | 使用あり | ZE105N, ZX105A | - |  |
| Statements | READ | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 10 | - |  |
| Statements | READ NEXT | 使用あり | ZF107A, ZF108A, ZF110A, ZF111A, ZF112A | - |  |
| Statements | READ PREVIOUS | 未網羅 |  | - |  |
| Statements | RELEASE | 未網羅 |  | - |  |
| Statements | RETURN | 未網羅 |  | - |  |
| Statements | REWRITE | 使用あり | ZF107A, ZF108A, ZF110A, ZF112A, ZF113A | - |  |
| Statements | SEARCH | 使用あり | ZD105A | - |  |
| Statements | SEARCH ALL | 使用あり | ZD105A | - |  |
| Statements | SET TO TRUE | 使用あり | ZP113A, ZX102A, ZX110A | - |  |
| Statements | SET TO FALSE | 使用あり | ZE103N, ZE104N, ZE126N, ZE126P, ZS107A, ZX102A ほか 1 | - |  |
| Statements | SET UP BY | 使用あり | ZD105A | - |  |
| Statements | SET DOWN BY | 使用あり | ZD105A | - |  |
| Statements | SET ADDRESS OF | 使用あり | ZP105A, ZP108A | - |  |
| Statements | SORT | 未網羅 |  | - |  |
| Statements | START | 使用あり | ZF107A, ZF108A, ZF110A, ZF111A, ZF112A | - |  |
| Statements | STOP RUN | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 156 | - |  |
| Statements | STRING | 使用あり | ZPS04B, ZPS09A, ZPS09B, ZPS09I | - |  |
| Statements | SUBTRACT | 使用あり | ZA102A, ZA104A, ZA109A, ZA110A, ZA118A, ZA119A | - |  |
| Statements | UNSTRING | 未網羅 |  | - |  |
| Statements | USE | 未網羅 |  | - |  |
| Statements | WRITE | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 11 | - |  |
| Statements | XML GENERATE | 未網羅 |  | - |  |
| Statements | XML PARSE | 未網羅 |  | - |  |
| Phrases | ON SIZE ERROR | 使用あり | ZA101A, ZA105A, ZA109A, ZA110A, ZA118A, ZA119A ほか 1 | - |  |
| Phrases | NOT ON SIZE ERROR | 使用あり | ZA105A, ZA109A, ZA118A, ZA119A | - |  |
| Phrases | ROUNDED | 使用あり | ZA104A, ZA105A, ZA108A, ZA109A, ZA122A, ZX108A ほか 1 | - |  |
| Phrases | ROUNDED MODE | 未網羅 |  | - |  |
| Phrases | ON OVERFLOW | 未網羅 |  | - |  |
| Phrases | ON EXCEPTION | 使用あり | ZP103A, ZP110A | - |  |
| Phrases | NOT ON EXCEPTION | 未網羅 |  | - |  |
| Phrases | INVALID KEY | 使用あり | ZF107A, ZF108A, ZF111A | - |  |
| Phrases | NOT INVALID KEY | 使用あり | ZF107A, ZF108A | - |  |
| Phrases | AT END | 使用あり | ZD105A, ZF101A, ZF106A, ZF107A, ZF108A | - |  |
| Phrases | NOT AT END | 使用あり | ZF106A, ZF108A | - |  |
| Phrases | AT END-OF-PAGE | 未網羅 |  | - |  |
| Phrases | ADVANCING | 未網羅 |  | - |  |
| Phrases | BY REFERENCE | 使用あり | ZP104A, ZP107A, ZP111A, ZP113A, ZPS04A, ZPS11A | - |  |
| Phrases | BY CONTENT | 使用あり | ZP104A, ZP107A, ZP113A | - |  |
| Phrases | BY VALUE | 使用あり | ZP104A, ZP107A, ZP111A, ZPS04A, ZPS11A | - |  |
| Phrases | RETURNING | 使用あり | ZE112N, ZE113N, ZP103A, ZP105A, ZP108A, ZP110A ほか 5 | - |  |
| Phrases | OMITTED | 使用あり | ZP104A, ZP107A | - |  |
| Phrases | CORRESPONDING | 未網羅 |  | - |  |
| Phrases | GIVING | 使用あり | ZA102A, ZA104A, ZA105A, ZA109A, ZA111A, ZA112A ほか 3 | - |  |
| Phrases | REMAINDER | 使用あり | ZA102A, ZA104A, ZA109A, ZA119A | - |  |
| Phrases | DELIMITED BY | 使用あり | ZPS04B | - |  |
| Phrases | WITH POINTER | 未網羅 |  | - |  |
| Phrases | TALLYING IN | 未網羅 |  | - |  |
| Phrases | COUNT IN | 未網羅 |  | - |  |
| Phrases | ALSO | 未網羅 |  | - |  |
| Phrases | THRU | 使用あり | ZD104A, ZE107N, ZF105A, ZX102A, ZX110A | - |  |
| Phrases | WHEN OTHER | 使用あり | ZS108A | - |  |
| Phrases | NEXT SENTENCE | 使用あり | ZX104A | - |  |
| Phrases | WITH NO ADVANCING | 未網羅 |  | - |  |
| Phrases | UPON | 未網羅 |  | - |  |
| Data description | BLANK WHEN ZERO | 使用あり | ZD106A, ZE135N, ZE135P, ZE143N, ZE143P | - |  |
| Data description | EXTERNAL | 使用あり | ZE109N, ZE145N, ZP106A, ZPS06A | - |  |
| Data description | GLOBAL | 使用あり | ZP106A, ZP113A, ZS105A | - |  |
| Data description | GROUP-USAGE | 使用あり | ZE115N, ZN101A, ZN109A | - |  |
| Data description | JUSTIFIED | 使用あり | ZE134N, ZE134P, ZE141N, ZX106A | - |  |
| Data description | OCCURS | 使用あり | ZD105A, ZD113A, ZD114A, ZD115A, ZE144N, ZE144P ほか 3 | - |  |
| Data description | OCCURS DEPENDING | 使用あり | ZD105A, ZD113A, ZE144N, ZE144P | - |  |
| Data description | ASCENDING KEY | 使用あり | ZD105A | - |  |
| Data description | DESCENDING KEY | 使用あり | ZD105A | - |  |
| Data description | INDEXED BY | 使用あり | ZD105A | - |  |
| Data description | REDEFINES | 使用あり | ZA101A, ZA102A, ZA103A, ZA106A, ZA107A, ZA111A ほか 27 | - |  |
| Data description | RENAMES | 使用あり | ZD104A, ZE107N | - |  |
| Data description | SIGN LEADING | 使用あり | ZA122A, ZD102A, ZD112A, ZE136N, ZE136P, ZN102A ほか 1 | - |  |
| Data description | SIGN TRAILING | 使用あり | ZA106A, ZA122A, ZD102A, ZD112A, ZF103A, ZS107A | - |  |
| Data description | SEPARATE CHARACTER | 使用あり | ZA106A, ZA122A, ZD102A, ZD112A, ZE136N, ZE136P ほか 3 | - |  |
| Data description | SYNCHRONIZED | 使用あり | ZD104A, ZD114A, ZD115A | - |  |
| Data description | VALUE | 使用あり | ZA105A, ZA109A, ZA121A, ZA123A, ZCVSCOPY, ZCVSLIT ほか 56 | - |  |
| Data description | VALUE THRU | 使用あり | ZX102A, ZX110A | - |  |
| Data description | WHEN SET TO FALSE | 使用あり | ZE103N, ZE104N, ZE126N, ZE126P, ZS107A, ZX102A ほか 1 | - |  |
| Data description | VOLATILE | 使用あり | ZS108A | - |  |
| Data description | DYNAMIC LENGTH | 未網羅 |  | - |  |
| Data description | LOCAL-STORAGE | 使用あり | ZE109N, ZE145N, ZE145P, ZPS01A, ZPS02A, ZPS02B ほか 4 | - |  |
| Data description | LINKAGE SECTION | 使用あり | ZE113N, ZP105A, ZP108A, ZP113A, ZPS01A, ZPS02A ほか 16 | - |  |
| Data description | FILLER | 使用あり | ZD105A, ZD107A, ZX102A, ZX106A | - |  |
| Data description | LEVEL 66 | 使用あり | ZD104A, ZE107N | - |  |
| Data description | LEVEL 77 | 使用あり | ZPS10N | - |  |
| Data description | LEVEL 88 | 使用あり | ZE103N, ZE104N, ZE126N, ZE126P, ZP113A, ZS107A ほか 2 | - |  |
| USAGE | DISPLAY | 未網羅 |  | - |  |
| USAGE | DISPLAY-1 | 使用あり | ZN102B, ZN103A, ZN104A, ZN106M | - |  |
| USAGE | NATIONAL | 使用あり | ZA120A, ZE114N, ZE115N, ZE139N, ZE139P, ZN101A ほか 14 | - |  |
| USAGE | UTF-8 | 未網羅 |  | - |  |
| USAGE | BINARY | 使用あり | ZA116A, ZA117A, ZA118A, ZA123A, ZD103A, ZE115N | - |  |
| USAGE | COMP | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA108A, ZA116A ほか 47 | - |  |
| USAGE | COMP-1 | 使用あり | ZA107A, ZA123A, ZD114A | - |  |
| USAGE | COMP-2 | 使用あり | ZA107A, ZA108A, ZA121A, ZA123A, ZD114A, ZS102A | - |  |
| USAGE | COMP-3 | 使用あり | ZA104A, ZA105A, ZA106A, ZA107A, ZA108A, ZA109A ほか 23 | - |  |
| USAGE | COMP-4 | 使用あり | ZA116A, ZA117A, ZD103A | - |  |
| USAGE | COMP-5 | 使用あり | ZA101A, ZD103A, ZD108A, ZD115A, ZE120N, ZF104A ほか 10 | - |  |
| USAGE | PACKED-DECIMAL | 使用あり | ZS108A | - |  |
| USAGE | INDEX | 未網羅 |  | - |  |
| USAGE | POINTER | 使用あり | ZE108N, ZE137N, ZE137P, ZP105A, ZP108A, ZP111A ほか 1 | - |  |
| USAGE | PROCEDURE-POINTER | 使用あり | ZE116N, ZE137N, ZE137P, ZE138N, ZE138P, ZP105A ほか 2 | - |  |
| USAGE | FUNCTION-POINTER | 使用あり | ZP105A | - |  |
| USAGE | OBJECT REFERENCE | 未網羅 |  | - |  |
| PICTURE symbols | PIC A | 使用あり | ZD107A, ZS102A | - |  |
| PICTURE symbols | PIC X | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 124 | - |  |
| PICTURE symbols | PIC 9 | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 123 | - |  |
| PICTURE symbols | PIC S | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 60 | - |  |
| PICTURE symbols | PIC V | 使用あり | ZA104A, ZA105A, ZA107A, ZA108A, ZA109A, ZA120A ほか 10 | - |  |
| PICTURE symbols | PIC P | 使用あり | ZD106A | - |  |
| PICTURE symbols | PIC Z | 使用あり | ZD106A, ZE143P, ZN102A, ZN108A | - |  |
| PICTURE symbols | PIC * | 使用あり | ZD106A, ZE143N | - |  |
| PICTURE symbols | PIC B | 使用あり | ZD106A, ZN102A, ZN108A | - |  |
| PICTURE symbols | PIC 0 | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 102 | - |  |
| PICTURE symbols | PIC / | 使用あり | ZD106A, ZN102A | - |  |
| PICTURE symbols | PIC , | 使用あり | ZD106A | - |  |
| PICTURE symbols | PIC . | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 133 | - |  |
| PICTURE symbols | PIC + | 使用あり | ZA120A, ZD106A, ZE140N, ZE140P, ZE141N, ZE141P ほか 2 | - |  |
| PICTURE symbols | PIC - | 使用あり | ZA103A, ZA109A, ZA125A, ZD106A, ZS102A, ZX107A | - |  |
| PICTURE symbols | PIC CR | 使用あり | ZD106A | - |  |
| PICTURE symbols | PIC DB | 使用あり | ZD106A | - |  |
| PICTURE symbols | PIC $ | 使用あり | ZD106A | - |  |
| PICTURE symbols | PIC E | 使用あり | ZA120A, ZE140N, ZE140P, ZE141N, ZE141P, ZE142N ほか 2 | - |  |
| PICTURE symbols | PIC G | 使用あり | ZN102B, ZN103A, ZN104A, ZN106M | - |  |
| PICTURE symbols | PIC N | 使用あり | ZE139P, ZN101A, ZN102A, ZN102B, ZN103A, ZN104A ほか 12 | - |  |
| PICTURE symbols | PIC U | 使用あり | ZS102A | - |  |
| File description and control | ORGANIZATION SEQUENTIAL | 使用あり | ZF106A | - |  |
| File description and control | ORGANIZATION INDEXED | 使用あり | ZF107A, ZF108A, ZF111A, ZF112A | - |  |
| File description and control | ORGANIZATION RELATIVE | 使用あり | ZF110A | - |  |
| File description and control | ORGANIZATION LINE SEQUENTIAL | 未網羅 |  | - |  |
| File description and control | ACCESS SEQUENTIAL | 未網羅 |  | - |  |
| File description and control | ACCESS RANDOM | 未網羅 |  | - |  |
| File description and control | ACCESS DYNAMIC | 未網羅 |  | - |  |
| File description and control | RECORD KEY | 使用あり | ZF104A, ZF107A, ZF108A, ZF111A, ZF112A | - |  |
| File description and control | ALTERNATE RECORD KEY | 使用あり | ZF107A, ZF108A, ZF112A | - |  |
| File description and control | WITH DUPLICATES | 使用あり | ZF108A, ZF112A | - |  |
| File description and control | RELATIVE KEY | 使用あり | ZF104A, ZF110A | - |  |
| File description and control | FILE STATUS | 使用あり | ZE120N, ZF101A, ZF102A, ZF103A, ZF104A, ZF105A ほか 12 | - |  |
| File description and control | RESERVE | 未網羅 |  | - |  |
| File description and control | PADDING CHARACTER | 使用あり | ZE121N | - |  |
| File description and control | RECORD DELIMITER | 未網羅 |  | - |  |
| File description and control | BLOCK CONTAINS | 使用あり | ZE118N, ZF101A, ZF102A, ZF105A, ZF109A, ZF114A ほか 2 | - |  |
| File description and control | RECORD CONTAINS | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 96 | - |  |
| File description and control | RECORD VARYING | 使用あり | ZF106A, ZF109A | - |  |
| File description and control | RECORDING MODE | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 98 | - |  |
| File description and control | LABEL RECORDS | 使用あり | ZE123N, ZF109A, ZF201M, ZF202M | - |  |
| File description and control | CODE-SET | 使用あり | ZE119N, ZF103A | - |  |
| File description and control | LINAGE | 未網羅 |  | - |  |
| File description and control | SD | 未網羅 |  | - |  |
| File description and control | SAME AREA | 未網羅 |  | - |  |
| File description and control | SAME RECORD AREA | 使用あり | ZF105A | - |  |
| File description and control | SAME SORT AREA | 未網羅 |  | - |  |
| File description and control | RERUN | 使用あり | ZE122N | - |  |
| File description and control | APPLY WRITE-ONLY | 使用あり | ZF105A | - |  |
| File description and control | MULTIPLE FILE TAPE | 使用あり | ZF201M | - |  |
| File description and control | OPEN EXTEND | 使用あり | ZF114A | - |  |
| File description and control | OPEN I-O | 使用あり | ZF104A, ZF107A, ZF108A, ZF110A, ZF112A | - |  |
| File description and control | CLOSE REEL | 使用あり | ZF202M | - |  |
| File description and control | CLOSE UNIT | 使用あり | ZF202M | - |  |
| File description and control | WITH LOCK | 未網羅 |  | - |  |
| File description and control | WITH NO REWIND | 使用あり | ZF201M, ZF202M | - |  |
| File description and control | REVERSED | 使用あり | ZF201M | - |  |
| Environment and structure | DECIMAL-POINT IS COMMA | 使用あり | ZX109A | - |  |
| Environment and structure | CURRENCY SIGN | 未網羅 |  | - |  |
| Environment and structure | ALPHABET | 使用あり | ZD110A, ZE119N, ZF103A | - |  |
| Environment and structure | CLASS | 未網羅 |  | - |  |
| Environment and structure | SYMBOLIC CHARACTERS | 未網羅 |  | - |  |
| Environment and structure | UPSI | 使用あり | ZS108A | - |  |
| Environment and structure | C01 | 未網羅 |  | - |  |
| Environment and structure | PROGRAM COLLATING SEQUENCE | 使用あり | ZD110A | - |  |
| Environment and structure | XML-SCHEMA | 未網羅 |  | - |  |
| Environment and structure | NESTED PROGRAM | 使用あり | ZP106A, ZP113A, ZS105A, ZS106A | - |  |
| Environment and structure | COMMON | 使用あり | ZP106A | - |  |
| Environment and structure | INITIAL | 使用あり | ZPS09I | - |  |
| Environment and structure | RECURSIVE | 使用あり | ZE110N, ZE111N, ZE129N, ZE129P, ZP112A, ZPS02A ほか 2 | - |  |
| Environment and structure | COPY | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 97 | - |  |
| Environment and structure | COPY REPLACING | 使用あり | ZS106A, ZX103A | - |  |
| Environment and structure | REPLACE | 使用あり | ZE127N, ZE127P, ZE128N, ZE128P, ZE133N, ZE133P ほか 3 | - |  |
| Environment and structure | DECLARATIVES | 未網羅 |  | - |  |
| Environment and structure | CBL | 使用あり | ZA101A, ZA102A, ZA103A, ZA110A, ZA111A, ZA112A ほか 24 | - |  |
| Environment and structure | PROCESS | 未網羅 |  | - |  |
| Environment and structure | >>CALLINTERFACE | 未網羅 |  | - |  |
| Environment and structure | >>DEFINE | 未網羅 |  | - |  |
| Environment and structure | >>EVALUATE | 未網羅 |  | - |  |
| Environment and structure | >>IF | 未網羅 |  | - |  |
| Intrinsic functions | ABS | 使用あり | ZA121A, ZX107A | - |  |
| Intrinsic functions | ACOS | 使用あり | ZA121A | - |  |
| Intrinsic functions | ANNUITY | 使用あり | ZA121A | - |  |
| Intrinsic functions | ASIN | 使用あり | ZA121A | - |  |
| Intrinsic functions | ATAN | 使用あり | ZA121A | - |  |
| Intrinsic functions | BIT-OF | 使用あり | ZA125A | - |  |
| Intrinsic functions | BIT-TO-CHAR | 使用あり | ZA125A | - |  |
| Intrinsic functions | BYTE-LENGTH | 使用あり | ZN101A, ZN105A, ZN107A | - |  |
| Intrinsic functions | CHAR | 未網羅 |  | - |  |
| Intrinsic functions | COMBINED-DATETIME | 使用あり | ZA125A | - |  |
| Intrinsic functions | COS | 使用あり | ZA108A, ZA121A | - |  |
| Intrinsic functions | CURRENT-DATE | 使用あり | ZS101A, ZS107A | - |  |
| Intrinsic functions | DATE-OF-INTEGER | 使用あり | ZS101A, ZS104A | - |  |
| Intrinsic functions | DATE-TO-YYYYMMDD | 使用あり | ZS101A, ZS107A | - |  |
| Intrinsic functions | DAY-OF-INTEGER | 使用あり | ZS101A, ZS104A | - |  |
| Intrinsic functions | DAY-TO-YYYYDDD | 使用あり | ZS101A, ZS107A | - |  |
| Intrinsic functions | DISPLAY-OF | 使用あり | ZN104A, ZN110A, ZN111A, ZN112A, ZN113A | - |  |
| Intrinsic functions | E | 使用あり | ZA125A | - |  |
| Intrinsic functions | EXP | 使用あり | ZA125A | - |  |
| Intrinsic functions | EXP10 | 使用あり | ZA125A | - |  |
| Intrinsic functions | FACTORIAL | 使用あり | ZA108A | - |  |
| Intrinsic functions | FORMATTED-CURRENT-DATE | 未網羅 |  | - |  |
| Intrinsic functions | FORMATTED-DATE | 使用あり | ZA125A | - |  |
| Intrinsic functions | FORMATTED-DATETIME | 使用あり | ZA125A | - |  |
| Intrinsic functions | FORMATTED-TIME | 使用あり | ZA125A | - |  |
| Intrinsic functions | HEX-OF | 使用あり | ZA125A | - |  |
| Intrinsic functions | HEX-TO-CHAR | 使用あり | ZA125A | - |  |
| Intrinsic functions | INTEGER | 使用あり | ZA108A, ZA123A, ZA125A, ZS101A, ZS104A | - |  |
| Intrinsic functions | INTEGER-OF-DATE | 使用あり | ZA123A, ZS101A, ZS104A | - |  |
| Intrinsic functions | INTEGER-OF-DAY | 未網羅 |  | - |  |
| Intrinsic functions | INTEGER-OF-FORMATTED-DATE | 使用あり | ZA125A | - |  |
| Intrinsic functions | INTEGER-PART | 使用あり | ZA108A | - |  |
| Intrinsic functions | LENGTH | 使用あり | ZA125A, ZD115A, ZN101A, ZN105A, ZN107A, ZN109A ほか 2 | - |  |
| Intrinsic functions | LOG | 使用あり | ZA121A | - |  |
| Intrinsic functions | LOG10 | 使用あり | ZA108A | - |  |
| Intrinsic functions | LOWER-CASE | 未網羅 |  | - |  |
| Intrinsic functions | MAX | 使用あり | ZA108A | - |  |
| Intrinsic functions | MEAN | 使用あり | ZA108A | - |  |
| Intrinsic functions | MEDIAN | 使用あり | ZA108A | - |  |
| Intrinsic functions | MIDRANGE | 未網羅 |  | - |  |
| Intrinsic functions | MIN | 使用あり | ZA108A | - |  |
| Intrinsic functions | MOD | 使用あり | ZA108A, ZS107A | - |  |
| Intrinsic functions | NATIONAL-OF | 使用あり | ZN104A, ZN110A, ZN111A, ZN112A, ZN113A | - |  |
| Intrinsic functions | NUMVAL | 使用あり | ZA108A, ZA124A, ZA125A, ZX108A, ZX109A | - |  |
| Intrinsic functions | NUMVAL-C | 使用あり | ZA108A, ZX109A | - |  |
| Intrinsic functions | NUMVAL-F | 使用あり | ZA125A | - |  |
| Intrinsic functions | ORD | 未網羅 |  | - |  |
| Intrinsic functions | ORD-MAX | 未網羅 |  | - |  |
| Intrinsic functions | ORD-MIN | 未網羅 |  | - |  |
| Intrinsic functions | PI | 使用あり | ZA125A | - |  |
| Intrinsic functions | PRESENT-VALUE | 使用あり | ZA121A | - |  |
| Intrinsic functions | RANDOM | 使用あり | ZA123A | - |  |
| Intrinsic functions | RANGE | 未網羅 |  | - |  |
| Intrinsic functions | REM | 使用あり | ZA108A | - |  |
| Intrinsic functions | REVERSE | 使用あり | ZS106A | - |  |
| Intrinsic functions | SECONDS-FROM-FORMATTED-TIME | 使用あり | ZA125A | - |  |
| Intrinsic functions | SECONDS-PAST-MIDNIGHT | 使用あり | ZS108A | - |  |
| Intrinsic functions | SIGN | 使用あり | ZA125A | - |  |
| Intrinsic functions | SIN | 使用あり | ZA108A, ZA121A | - |  |
| Intrinsic functions | SQRT | 使用あり | ZA108A | - |  |
| Intrinsic functions | STANDARD-DEVIATION | 未網羅 |  | - |  |
| Intrinsic functions | SUM | 未網羅 |  | - |  |
| Intrinsic functions | TAN | 使用あり | ZA121A | - |  |
| Intrinsic functions | TEST-DATE-YYYYMMDD | 使用あり | ZA125A | - |  |
| Intrinsic functions | TEST-DAY-YYYYDDD | 使用あり | ZA125A | - |  |
| Intrinsic functions | TEST-FORMATTED-DATETIME | 使用あり | ZA125A | - |  |
| Intrinsic functions | TEST-NUMVAL | 使用あり | ZA124A, ZA125A, ZX107A, ZX108A, ZX109A | - |  |
| Intrinsic functions | TEST-NUMVAL-C | 使用あり | ZX107A, ZX109A | - |  |
| Intrinsic functions | TEST-NUMVAL-F | 使用あり | ZA125A | - |  |
| Intrinsic functions | TRIM | 使用あり | ZX107A, ZX108A | - |  |
| Intrinsic functions | TRIML | 使用あり | ZA125A | - |  |
| Intrinsic functions | TRIMR | 使用あり | ZA125A | - |  |
| Intrinsic functions | ULENGTH | 未網羅 |  | - |  |
| Intrinsic functions | UPOS | 未網羅 |  | - |  |
| Intrinsic functions | UPPER-CASE | 未網羅 |  | - |  |
| Intrinsic functions | USUBSTR | 未網羅 |  | - |  |
| Intrinsic functions | USUPPLEMENTARY | 未網羅 |  | - |  |
| Intrinsic functions | UVALID | 未網羅 |  | - |  |
| Intrinsic functions | UWIDTH | 未網羅 |  | - |  |
| Intrinsic functions | VARIANCE | 未網羅 |  | - |  |
| Intrinsic functions | WHEN-COMPILED | 使用あり | ZS101A | - |  |
| Intrinsic functions | YEAR-TO-YYYY | 使用あり | ZS101A, ZS107A | - |  |
| Special registers | ADDRESS OF | 使用あり | ZP105A, ZP108A, ZPS04A | - |  |
| Special registers | LENGTH OF | 使用あり | ZD104A, ZD105A, ZD113A, ZD114A, ZN102A, ZP105A | - |  |
| Special registers | LINAGE-COUNTER | 未網羅 |  | - |  |
| Special registers | RETURN-CODE | 使用あり | ZA109A, ZA110A, ZA111A, ZA112A, ZA113A, ZA114A ほか 62 | - |  |
| Special registers | SHIFT-OUT | 未網羅 |  | - |  |
| Special registers | SHIFT-IN | 未網羅 |  | - |  |
| Special registers | SORT-CONTROL | 未網羅 |  | - |  |
| Special registers | SORT-CORE-SIZE | 未網羅 |  | - |  |
| Special registers | SORT-FILE-SIZE | 未網羅 |  | - |  |
| Special registers | SORT-MESSAGE | 未網羅 |  | - |  |
| Special registers | SORT-MODE-SIZE | 未網羅 |  | - |  |
| Special registers | SORT-RETURN | 未網羅 |  | - |  |
| Special registers | TALLY | 使用あり | ZS101A | - |  |
| Special registers | WHEN-COMPILED | 使用あり | ZS101A | - |  |
| Special registers | XML-CODE | 未網羅 |  | - |  |
| Special registers | XML-EVENT | 未網羅 |  | - |  |
| Special registers | XML-TEXT | 未網羅 |  | - |  |
| Special registers | XML-NTEXT | 未網羅 |  | - |  |
| Special registers | JSON-CODE | 未網羅 |  | - |  |
| Special registers | JSON-STATUS | 未網羅 |  | - |  |
| Special registers | DEBUG-ITEM | 未網羅 |  | - |  |
| Compiler options (semantic) | ARITH | 使用あり | ZA110A, ZA116A, ZA117A, ZA118A, ZD108A, ZD111A ほか 3 | - |  |
| Compiler options (semantic) | CODEPAGE | 未網羅 |  | - |  |
| Compiler options (semantic) | DBCS | 使用あり | ZN102B, ZS106A | - |  |
| Compiler options (semantic) | INTDATE | 未網羅 |  | - |  |
| Compiler options (semantic) | NSYMBOL | 使用あり | ZN101A, ZN102A, ZN102B, ZN105A, ZN107A | - |  |
| Compiler options (semantic) | NUMCLS | 使用あり | ZA111A, ZA112A, ZA113A, ZA114A | - |  |
| Compiler options (semantic) | NUMPROC | 使用あり | ZA111A, ZA112A, ZA113A, ZA114A, ZA115A | - |  |
| Compiler options (semantic) | NUMCHECK | 未網羅 |  | - |  |
| Compiler options (semantic) | TRUNC | 使用あり | ZA101A, ZA102A, ZA103A, ZA116A, ZA117A, ZA118A ほか 2 | - |  |
| Compiler options (semantic) | ZWB | 未網羅 |  | - |  |
| Compiler options (semantic) | DYNAM | 使用あり | ZP101A, ZP109A, ZP110A, ZP111A, ZP112A | - |  |
| Compiler options (semantic) | QUALIFY | 未網羅 |  | - |  |
| Compiler options (semantic) | INITCHECK | 未網羅 |  | - |  |
| Compiler options (semantic) | SQLCCSID | 未網羅 |  | - |  |

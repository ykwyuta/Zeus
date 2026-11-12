# ZCVS 網羅台帳 (Enterprise COBOL for z/OS)

Enterprise COBOL for z/OS Language Reference の章立てから列挙した。CCVS85 の列は、その要素を使う CCVS85 のプログラム数 (道具に newcob.val を渡したときだけ数える)。 要素 337、使用あり 237、未網羅 100。

状態の「未網羅」と「使用あり」は道具が機械的に決める。「使用あり」は検査がその要素を使う
ことしか意味しない。境界・内部表現まで確かめたものは人が読んで最後の列 (確認) に
「網羅」と書く。対象外にするものは理由を書く。この列は再生成しても保たれる。

| 分類 | 要素 | 状態 | スイートでの使用 | CCVS85 | 確認 |
| --- | --- | --- | --- | --- | --- |
| Statements | ACCEPT | 未網羅 |  | 118 |  |
| Statements | ADD | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA109A ほか 29 | 414 |  |
| Statements | ALLOCATE | 未網羅 |  | 0 |  |
| Statements | ALTER | 使用あり | ZE110N | 28 |  |
| Statements | CALL | 使用あり | ZP101A, ZP102A, ZP103A, ZP104A, ZP105A, ZP106A ほか 13 | 34 |  |
| Statements | CANCEL | 使用あり | ZA110A, ZP101A, ZP109A, ZP112A | 7 |  |
| Statements | CLOSE | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 11 | 406 |  |
| Statements | COMPUTE | 使用あり | ZA102A, ZA104A, ZA105A, ZA107A, ZA108A, ZA109A ほか 23 | 46 |  |
| Statements | CONTINUE | 使用あり | ZE105N, ZE132N, ZE132P | 45 |  |
| Statements | DELETE | 使用あり | ZF107A, ZF108A, ZF110A, ZF112A | 16 |  |
| Statements | DISPLAY | 使用あり | ZA120A | 44 |  |
| Statements | DIVIDE | 使用あり | ZA102A, ZA104A, ZA105A, ZA109A, ZA119A, ZA123A | 18 |  |
| Statements | ENTRY | 使用あり | ZE113N, ZPS04B, ZPS11A | 0 |  |
| Statements | EVALUATE | 使用あり | ZS108A | 37 |  |
| Statements | EXIT PROGRAM | 未網羅 |  | 343 |  |
| Statements | EXIT PERFORM | 使用あり | ZX101A, ZX105A | 4 |  |
| Statements | EXIT PARAGRAPH | 使用あり | ZX101A | 0 |  |
| Statements | EXIT SECTION | 使用あり | ZX101A | 377 |  |
| Statements | FREE | 未網羅 |  | 0 |  |
| Statements | GOBACK | 使用あり | ZE102N, ZE110N, ZE111N, ZE112N, ZE113N, ZP106A ほか 21 | 0 |  |
| Statements | GO TO | 使用あり | ZE110N, ZE111N | 413 |  |
| Statements | GO TO DEPENDING | 使用あり | ZD105A, ZD113A, ZE144N, ZE144P, ZF101A, ZF102A ほか 2 | 33 |  |
| Statements | IF | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 100 | 413 |  |
| Statements | INITIALIZE | 使用あり | ZD107A, ZE106N, ZE107N, ZE108N, ZN102B, ZN103A ほか 7 | 10 |  |
| Statements | INSPECT TALLYING | 使用あり | ZS106A | 5 |  |
| Statements | INSPECT REPLACING | 未網羅 |  | 10 |  |
| Statements | INSPECT CONVERTING | 未網羅 |  | 2 |  |
| Statements | INVOKE | 未網羅 |  | 0 |  |
| Statements | JSON GENERATE | 未網羅 |  | 0 |  |
| Statements | JSON PARSE | 未網羅 |  | 0 |  |
| Statements | MERGE | 未網羅 |  | 7 |  |
| Statements | MOVE | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 113 | 441 |  |
| Statements | MOVE CORRESPONDING | 未網羅 |  | 10 |  |
| Statements | MULTIPLY | 使用あり | ZA102A, ZA109A, ZA119A | 22 |  |
| Statements | OPEN | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 11 | 406 |  |
| Statements | PERFORM | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 100 | 421 |  |
| Statements | PERFORM TIMES | 使用あり | ZX101A | 401 |  |
| Statements | PERFORM UNTIL | 使用あり | ZE105N, ZE132N, ZE132P, ZF114A, ZX101A, ZX105A ほか 1 | 65 |  |
| Statements | PERFORM VARYING | 使用あり | ZE132N, ZE132P, ZF114A, ZX101A, ZX105A, ZX110A | 27 |  |
| Statements | PERFORM AFTER | 使用あり | ZX110A | 12 |  |
| Statements | WITH TEST AFTER | 使用あり | ZE105N, ZX105A | 3 |  |
| Statements | READ | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 10 | 186 |  |
| Statements | READ NEXT | 使用あり | ZF107A, ZF108A, ZF110A, ZF111A, ZF112A | 21 |  |
| Statements | READ PREVIOUS | 未網羅 |  | 0 |  |
| Statements | RELEASE | 未網羅 |  | 23 |  |
| Statements | RETURN | 未網羅 |  | 24 |  |
| Statements | REWRITE | 使用あり | ZF107A, ZF108A, ZF110A, ZF112A, ZF113A | 117 |  |
| Statements | SEARCH | 使用あり | ZD105A | 12 |  |
| Statements | SEARCH ALL | 使用あり | ZD105A | 5 |  |
| Statements | SET TO TRUE | 使用あり | ZP113A, ZX102A, ZX110A | 2 |  |
| Statements | SET TO FALSE | 使用あり | ZE103N, ZE104N, ZE126N, ZE126P, ZS107A, ZX102A ほか 1 | 0 |  |
| Statements | SET UP BY | 使用あり | ZD105A | 9 |  |
| Statements | SET DOWN BY | 使用あり | ZD105A | 6 |  |
| Statements | SET ADDRESS OF | 使用あり | ZP105A, ZP108A | 0 |  |
| Statements | SORT | 未網羅 |  | 47 |  |
| Statements | START | 使用あり | ZF107A, ZF108A, ZF110A, ZF111A, ZF112A | 16 |  |
| Statements | STOP RUN | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 156 | 431 |  |
| Statements | STRING | 使用あり | ZPS04B, ZPS09A, ZPS09B, ZPS09I | 4 |  |
| Statements | SUBTRACT | 使用あり | ZA102A, ZA104A, ZA109A, ZA110A, ZA118A, ZA119A | 75 |  |
| Statements | UNSTRING | 未網羅 |  | 3 |  |
| Statements | USE | 未網羅 |  | 86 |  |
| Statements | WRITE | 使用あり | ZF101A, ZF102A, ZF103A, ZF104A, ZF105A, ZF106A ほか 11 | 405 |  |
| Statements | XML GENERATE | 未網羅 |  | 0 |  |
| Statements | XML PARSE | 未網羅 |  | 0 |  |
| Phrases | ON SIZE ERROR | 使用あり | ZA101A, ZA105A, ZA109A, ZA110A, ZA118A, ZA119A ほか 1 | 25 |  |
| Phrases | NOT ON SIZE ERROR | 使用あり | ZA105A, ZA109A, ZA118A, ZA119A | 13 |  |
| Phrases | ROUNDED | 使用あり | ZA104A, ZA105A, ZA108A, ZA109A, ZA122A, ZX108A ほか 1 | 23 |  |
| Phrases | ROUNDED MODE | 未網羅 |  | 0 |  |
| Phrases | ON OVERFLOW | 未網羅 |  | 8 |  |
| Phrases | ON EXCEPTION | 使用あり | ZP103A, ZP110A | 1 |  |
| Phrases | NOT ON EXCEPTION | 未網羅 |  | 1 |  |
| Phrases | INVALID KEY | 使用あり | ZF107A, ZF108A, ZF111A | 131 |  |
| Phrases | NOT INVALID KEY | 使用あり | ZF107A, ZF108A | 6 |  |
| Phrases | AT END | 使用あり | ZD105A, ZF101A, ZF106A, ZF107A, ZF108A | 160 |  |
| Phrases | NOT AT END | 使用あり | ZF106A, ZF108A | 9 |  |
| Phrases | AT END-OF-PAGE | 未網羅 |  | 3 |  |
| Phrases | ADVANCING | 未網羅 |  | 394 |  |
| Phrases | BY REFERENCE | 使用あり | ZP104A, ZP107A, ZP111A, ZP113A, ZPS04A, ZPS11A | 4 |  |
| Phrases | BY CONTENT | 使用あり | ZP104A, ZP107A, ZP113A | 4 |  |
| Phrases | BY VALUE | 使用あり | ZP104A, ZP107A, ZP111A, ZPS04A, ZPS11A | 0 |  |
| Phrases | RETURNING | 使用あり | ZE112N, ZE113N, ZP103A, ZP105A, ZP108A, ZP110A ほか 5 | 0 |  |
| Phrases | OMITTED | 使用あり | ZP104A, ZP107A | 2 |  |
| Phrases | CORRESPONDING | 未網羅 |  | 7 |  |
| Phrases | GIVING | 使用あり | ZA102A, ZA104A, ZA105A, ZA109A, ZA111A, ZA112A ほか 3 | 65 |  |
| Phrases | REMAINDER | 使用あり | ZA102A, ZA104A, ZA109A, ZA119A | 4 |  |
| Phrases | DELIMITED BY | 使用あり | ZPS04B | 4 |  |
| Phrases | WITH POINTER | 未網羅 |  | 2 |  |
| Phrases | TALLYING IN | 未網羅 |  | 1 |  |
| Phrases | COUNT IN | 未網羅 |  | 1 |  |
| Phrases | ALSO | 未網羅 |  | 8 |  |
| Phrases | THRU | 使用あり | ZD104A, ZE107N, ZF105A, ZX102A, ZX110A | 408 |  |
| Phrases | WHEN OTHER | 使用あり | ZS108A | 24 |  |
| Phrases | NEXT SENTENCE | 使用あり | ZX104A | 26 |  |
| Phrases | WITH NO ADVANCING | 未網羅 |  | 1 |  |
| Phrases | UPON | 未網羅 |  | 4 |  |
| Data description | BLANK WHEN ZERO | 使用あり | ZD106A, ZE135N, ZE135P, ZE143N, ZE143P | 3 |  |
| Data description | EXTERNAL | 使用あり | ZE109N, ZE145N, ZP106A, ZPS06A | 5 |  |
| Data description | GLOBAL | 使用あり | ZP106A, ZP113A, ZS105A | 4 |  |
| Data description | GROUP-USAGE | 使用あり | ZE115N, ZN101A, ZN109A | 0 |  |
| Data description | JUSTIFIED | 使用あり | ZE134N, ZE134P, ZE141N, ZX106A | 14 |  |
| Data description | OCCURS | 使用あり | ZD105A, ZD113A, ZD114A, ZD115A, ZE144N, ZE144P ほか 3 | 280 |  |
| Data description | OCCURS DEPENDING | 使用あり | ZD105A, ZD113A, ZE144N, ZE144P | 10 |  |
| Data description | ASCENDING KEY | 使用あり | ZD105A | 30 |  |
| Data description | DESCENDING KEY | 使用あり | ZD105A | 19 |  |
| Data description | INDEXED BY | 使用あり | ZD105A | 50 |  |
| Data description | REDEFINES | 使用あり | ZA101A, ZA102A, ZA103A, ZA106A, ZA107A, ZA111A ほか 27 | 398 |  |
| Data description | RENAMES | 使用あり | ZD104A, ZE107N | 4 |  |
| Data description | SIGN LEADING | 使用あり | ZA122A, ZD102A, ZD112A, ZE136N, ZE136P, ZN102A ほか 1 | 11 |  |
| Data description | SIGN TRAILING | 使用あり | ZA106A, ZA122A, ZD102A, ZD112A, ZF103A, ZS107A | 9 |  |
| Data description | SEPARATE CHARACTER | 使用あり | ZA106A, ZA122A, ZD102A, ZD112A, ZE136N, ZE136P ほか 3 | 14 |  |
| Data description | SYNCHRONIZED | 使用あり | ZD104A, ZD114A, ZD115A | 17 |  |
| Data description | VALUE | 使用あり | ZA105A, ZA109A, ZA121A, ZA123A, ZCVSCOPY, ZCVSLIT ほか 56 | 438 |  |
| Data description | VALUE THRU | 使用あり | ZX102A, ZX110A | 8 |  |
| Data description | WHEN SET TO FALSE | 使用あり | ZE103N, ZE104N, ZE126N, ZE126P, ZS107A, ZX102A ほか 1 | 11 |  |
| Data description | VOLATILE | 使用あり | ZS108A | 0 |  |
| Data description | DYNAMIC LENGTH | 未網羅 |  | 0 |  |
| Data description | LOCAL-STORAGE | 使用あり | ZE109N, ZE145N, ZE145P, ZPS01A, ZPS02A, ZPS02B ほか 4 | 0 |  |
| Data description | LINKAGE SECTION | 使用あり | ZE113N, ZP105A, ZP108A, ZP113A, ZPS01A, ZPS02A ほか 16 | 31 |  |
| Data description | FILLER | 使用あり | ZD105A, ZD107A, ZX102A, ZX106A | 412 |  |
| Data description | LEVEL 66 | 使用あり | ZD104A, ZE107N | 5 |  |
| Data description | LEVEL 77 | 使用あり | ZPS10N | 155 |  |
| Data description | LEVEL 88 | 使用あり | ZE103N, ZE104N, ZE126N, ZE126P, ZP113A, ZS107A ほか 2 | 21 |  |
| USAGE | DISPLAY | 未網羅 |  | 13 |  |
| USAGE | DISPLAY-1 | 使用あり | ZN102B, ZN103A, ZN104A, ZN106M | 0 |  |
| USAGE | NATIONAL | 使用あり | ZA120A, ZE114N, ZE115N, ZE139N, ZE139P, ZN101A ほか 14 | 395 |  |
| USAGE | UTF-8 | 未網羅 |  | 0 |  |
| USAGE | BINARY | 使用あり | ZA116A, ZA117A, ZA118A, ZA123A, ZD103A, ZE115N | 1 |  |
| USAGE | COMP | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA108A, ZA116A ほか 47 | 167 |  |
| USAGE | COMP-1 | 使用あり | ZA107A, ZA123A, ZD114A | 0 |  |
| USAGE | COMP-2 | 使用あり | ZA107A, ZA108A, ZA121A, ZA123A, ZD114A, ZS102A | 0 |  |
| USAGE | COMP-3 | 使用あり | ZA104A, ZA105A, ZA106A, ZA107A, ZA108A, ZA109A ほか 23 | 0 |  |
| USAGE | COMP-4 | 使用あり | ZA116A, ZA117A, ZD103A | 0 |  |
| USAGE | COMP-5 | 使用あり | ZA101A, ZD103A, ZD108A, ZD115A, ZE120N, ZF104A ほか 10 | 0 |  |
| USAGE | PACKED-DECIMAL | 使用あり | ZS108A | 0 |  |
| USAGE | INDEX | 未網羅 |  | 11 |  |
| USAGE | POINTER | 使用あり | ZE108N, ZE137N, ZE137P, ZP105A, ZP108A, ZP111A ほか 1 | 0 |  |
| USAGE | PROCEDURE-POINTER | 使用あり | ZE116N, ZE137N, ZE137P, ZE138N, ZE138P, ZP105A ほか 2 | 0 |  |
| USAGE | FUNCTION-POINTER | 使用あり | ZP105A | 0 |  |
| USAGE | OBJECT REFERENCE | 未網羅 |  | 0 |  |
| PICTURE symbols | PIC A | 使用あり | ZD107A, ZS102A | 51 |  |
| PICTURE symbols | PIC X | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 124 | 444 |  |
| PICTURE symbols | PIC 9 | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 123 | 447 |  |
| PICTURE symbols | PIC S | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 60 | 413 |  |
| PICTURE symbols | PIC V | 使用あり | ZA104A, ZA105A, ZA107A, ZA108A, ZA109A, ZA120A ほか 10 | 95 |  |
| PICTURE symbols | PIC P | 使用あり | ZD106A | 33 |  |
| PICTURE symbols | PIC Z | 使用あり | ZD106A, ZE143P, ZN102A, ZN108A | 23 |  |
| PICTURE symbols | PIC * | 使用あり | ZD106A, ZE143N | 15 |  |
| PICTURE symbols | PIC B | 使用あり | ZD106A, ZN102A, ZN108A | 21 |  |
| PICTURE symbols | PIC 0 | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 102 | 420 |  |
| PICTURE symbols | PIC / | 使用あり | ZD106A, ZN102A | 15 |  |
| PICTURE symbols | PIC , | 使用あり | ZD106A | 30 |  |
| PICTURE symbols | PIC . | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 133 | 454 |  |
| PICTURE symbols | PIC + | 使用あり | ZA120A, ZD106A, ZE140N, ZE140P, ZE141N, ZE141P ほか 2 | 12 |  |
| PICTURE symbols | PIC - | 使用あり | ZA103A, ZA109A, ZA125A, ZD106A, ZS102A, ZX107A | 392 |  |
| PICTURE symbols | PIC CR | 使用あり | ZD106A | 16 |  |
| PICTURE symbols | PIC DB | 使用あり | ZD106A | 3 |  |
| PICTURE symbols | PIC $ | 使用あり | ZD106A | 31 |  |
| PICTURE symbols | PIC E | 使用あり | ZA120A, ZE140N, ZE140P, ZE141N, ZE141P, ZE142N ほか 2 | 4 |  |
| PICTURE symbols | PIC G | 使用あり | ZN102B, ZN103A, ZN104A, ZN106M | 1 |  |
| PICTURE symbols | PIC N | 使用あり | ZE139P, ZN101A, ZN102A, ZN102B, ZN103A, ZN104A ほか 12 | 1 |  |
| PICTURE symbols | PIC U | 使用あり | ZS102A | 3 |  |
| File description and control | ORGANIZATION SEQUENTIAL | 使用あり | ZF106A | 17 |  |
| File description and control | ORGANIZATION INDEXED | 使用あり | ZF107A, ZF108A, ZF111A, ZF112A | 4 |  |
| File description and control | ORGANIZATION RELATIVE | 使用あり | ZF110A | 12 |  |
| File description and control | ORGANIZATION LINE SEQUENTIAL | 未網羅 |  | 0 |  |
| File description and control | ACCESS SEQUENTIAL | 未網羅 |  | 20 |  |
| File description and control | ACCESS RANDOM | 未網羅 |  | 8 |  |
| File description and control | ACCESS DYNAMIC | 未網羅 |  | 3 |  |
| File description and control | RECORD KEY | 使用あり | ZF104A, ZF107A, ZF108A, ZF111A, ZF112A | 84 |  |
| File description and control | ALTERNATE RECORD KEY | 使用あり | ZF107A, ZF108A, ZF112A | 12 |  |
| File description and control | WITH DUPLICATES | 使用あり | ZF108A, ZF112A | 8 |  |
| File description and control | RELATIVE KEY | 使用あり | ZF104A, ZF110A | 16 |  |
| File description and control | FILE STATUS | 使用あり | ZE120N, ZF101A, ZF102A, ZF103A, ZF104A, ZF105A ほか 12 | 77 |  |
| File description and control | RESERVE | 未網羅 |  | 13 |  |
| File description and control | PADDING CHARACTER | 使用あり | ZE121N | 3 |  |
| File description and control | RECORD DELIMITER | 未網羅 |  | 3 |  |
| File description and control | BLOCK CONTAINS | 使用あり | ZE118N, ZF101A, ZF102A, ZF105A, ZF109A, ZF114A ほか 2 | 99 |  |
| File description and control | RECORD CONTAINS | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 96 | 79 |  |
| File description and control | RECORD VARYING | 使用あり | ZF106A, ZF109A | 8 |  |
| File description and control | RECORDING MODE | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 98 | 0 |  |
| File description and control | LABEL RECORDS | 使用あり | ZE123N, ZF109A, ZF201M, ZF202M | 210 |  |
| File description and control | CODE-SET | 使用あり | ZE119N, ZF103A | 1 |  |
| File description and control | LINAGE | 未網羅 |  | 5 |  |
| File description and control | SD | 未網羅 |  | 36 |  |
| File description and control | SAME AREA | 未網羅 |  | 6 |  |
| File description and control | SAME RECORD AREA | 使用あり | ZF105A | 10 |  |
| File description and control | SAME SORT AREA | 未網羅 |  | 7 |  |
| File description and control | RERUN | 使用あり | ZE122N | 0 |  |
| File description and control | APPLY WRITE-ONLY | 使用あり | ZF105A | 0 |  |
| File description and control | MULTIPLE FILE TAPE | 使用あり | ZF201M | 8 |  |
| File description and control | OPEN EXTEND | 使用あり | ZF114A | 12 |  |
| File description and control | OPEN I-O | 使用あり | ZF104A, ZF107A, ZF108A, ZF110A, ZF112A | 128 |  |
| File description and control | CLOSE REEL | 使用あり | ZF202M | 6 |  |
| File description and control | CLOSE UNIT | 使用あり | ZF202M | 2 |  |
| File description and control | WITH LOCK | 未網羅 |  | 6 |  |
| File description and control | WITH NO REWIND | 使用あり | ZF201M, ZF202M | 5 |  |
| File description and control | REVERSED | 使用あり | ZF201M | 2 |  |
| Environment and structure | DECIMAL-POINT IS COMMA | 使用あり | ZX109A | 2 |  |
| Environment and structure | CURRENCY SIGN | 未網羅 |  | 1 |  |
| Environment and structure | ALPHABET | 使用あり | ZD110A, ZE119N, ZF103A | 19 |  |
| Environment and structure | CLASS | 未網羅 |  | 3 |  |
| Environment and structure | SYMBOLIC CHARACTERS | 未網羅 |  | 1 |  |
| Environment and structure | UPSI | 使用あり | ZS108A | 0 |  |
| Environment and structure | C01 | 未網羅 |  | 0 |  |
| Environment and structure | PROGRAM COLLATING SEQUENCE | 使用あり | ZD110A | 13 |  |
| Environment and structure | XML-SCHEMA | 未網羅 |  | 0 |  |
| Environment and structure | NESTED PROGRAM | 使用あり | ZP106A, ZP113A, ZS105A, ZS106A | 8 |  |
| Environment and structure | COMMON | 使用あり | ZP106A | 1 |  |
| Environment and structure | INITIAL | 使用あり | ZPS09I | 1 |  |
| Environment and structure | RECURSIVE | 使用あり | ZE110N, ZE111N, ZE129N, ZE129P, ZP112A, ZPS02A ほか 2 | 0 |  |
| Environment and structure | COPY | 使用あり | ZA101A, ZA102A, ZA103A, ZA104A, ZA105A, ZA106A ほか 97 | 402 |  |
| Environment and structure | COPY REPLACING | 使用あり | ZS106A, ZX103A | 6 |  |
| Environment and structure | REPLACE | 使用あり | ZE127N, ZE127P, ZE128N, ZE128P, ZE133N, ZE133P ほか 3 | 2 |  |
| Environment and structure | DECLARATIVES | 未網羅 |  | 88 |  |
| Environment and structure | CBL | 使用あり | ZA101A, ZA102A, ZA103A, ZA110A, ZA111A, ZA112A ほか 24 | 0 |  |
| Environment and structure | PROCESS | 未網羅 |  | 1 |  |
| Environment and structure | >>CALLINTERFACE | 未網羅 |  | 0 |  |
| Environment and structure | >>DEFINE | 未網羅 |  | 0 |  |
| Environment and structure | >>EVALUATE | 未網羅 |  | 0 |  |
| Environment and structure | >>IF | 未網羅 |  | 0 |  |
| Intrinsic functions | ABS | 使用あり | ZA121A, ZX107A | 0 |  |
| Intrinsic functions | ACOS | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | ANNUITY | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | ASIN | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | ATAN | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | BIT-OF | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | BIT-TO-CHAR | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | BYTE-LENGTH | 使用あり | ZN101A, ZN105A, ZN107A | 0 |  |
| Intrinsic functions | CHAR | 未網羅 |  | 2 |  |
| Intrinsic functions | COMBINED-DATETIME | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | COS | 使用あり | ZA108A, ZA121A | 2 |  |
| Intrinsic functions | CURRENT-DATE | 使用あり | ZS101A, ZS107A | 2 |  |
| Intrinsic functions | DATE-OF-INTEGER | 使用あり | ZS101A, ZS104A | 2 |  |
| Intrinsic functions | DATE-TO-YYYYMMDD | 使用あり | ZS101A, ZS107A | 0 |  |
| Intrinsic functions | DAY-OF-INTEGER | 使用あり | ZS101A, ZS104A | 2 |  |
| Intrinsic functions | DAY-TO-YYYYDDD | 使用あり | ZS101A, ZS107A | 0 |  |
| Intrinsic functions | DISPLAY-OF | 使用あり | ZN104A, ZN110A, ZN111A, ZN112A, ZN113A | 0 |  |
| Intrinsic functions | E | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | EXP | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | EXP10 | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | FACTORIAL | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | FORMATTED-CURRENT-DATE | 未網羅 |  | 0 |  |
| Intrinsic functions | FORMATTED-DATE | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | FORMATTED-DATETIME | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | FORMATTED-TIME | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | HEX-OF | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | HEX-TO-CHAR | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | INTEGER | 使用あり | ZA108A, ZA123A, ZA125A, ZS101A, ZS104A | 7 |  |
| Intrinsic functions | INTEGER-OF-DATE | 使用あり | ZA123A, ZS101A, ZS104A | 2 |  |
| Intrinsic functions | INTEGER-OF-DAY | 未網羅 |  | 2 |  |
| Intrinsic functions | INTEGER-OF-FORMATTED-DATE | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | INTEGER-PART | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | LENGTH | 使用あり | ZA125A, ZD115A, ZN101A, ZN105A, ZN107A, ZN109A ほか 2 | 5 |  |
| Intrinsic functions | LOG | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | LOG10 | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | LOWER-CASE | 未網羅 |  | 2 |  |
| Intrinsic functions | MAX | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | MEAN | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | MEDIAN | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | MIDRANGE | 未網羅 |  | 2 |  |
| Intrinsic functions | MIN | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | MOD | 使用あり | ZA108A, ZS107A | 2 |  |
| Intrinsic functions | NATIONAL-OF | 使用あり | ZN104A, ZN110A, ZN111A, ZN112A, ZN113A | 0 |  |
| Intrinsic functions | NUMVAL | 使用あり | ZA108A, ZA124A, ZA125A, ZX108A, ZX109A | 3 |  |
| Intrinsic functions | NUMVAL-C | 使用あり | ZA108A, ZX109A | 2 |  |
| Intrinsic functions | NUMVAL-F | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | ORD | 未網羅 |  | 5 |  |
| Intrinsic functions | ORD-MAX | 未網羅 |  | 2 |  |
| Intrinsic functions | ORD-MIN | 未網羅 |  | 2 |  |
| Intrinsic functions | PI | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | PRESENT-VALUE | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | RANDOM | 使用あり | ZA123A | 2 |  |
| Intrinsic functions | RANGE | 未網羅 |  | 2 |  |
| Intrinsic functions | REM | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | REVERSE | 使用あり | ZS106A | 2 |  |
| Intrinsic functions | SECONDS-FROM-FORMATTED-TIME | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | SECONDS-PAST-MIDNIGHT | 使用あり | ZS108A | 0 |  |
| Intrinsic functions | SIGN | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | SIN | 使用あり | ZA108A, ZA121A | 2 |  |
| Intrinsic functions | SQRT | 使用あり | ZA108A | 2 |  |
| Intrinsic functions | STANDARD-DEVIATION | 未網羅 |  | 2 |  |
| Intrinsic functions | SUM | 未網羅 |  | 2 |  |
| Intrinsic functions | TAN | 使用あり | ZA121A | 2 |  |
| Intrinsic functions | TEST-DATE-YYYYMMDD | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | TEST-DAY-YYYYDDD | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | TEST-FORMATTED-DATETIME | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | TEST-NUMVAL | 使用あり | ZA124A, ZA125A, ZX107A, ZX108A, ZX109A | 0 |  |
| Intrinsic functions | TEST-NUMVAL-C | 使用あり | ZX107A, ZX109A | 0 |  |
| Intrinsic functions | TEST-NUMVAL-F | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | TRIM | 使用あり | ZX107A, ZX108A | 0 |  |
| Intrinsic functions | TRIML | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | TRIMR | 使用あり | ZA125A | 0 |  |
| Intrinsic functions | ULENGTH | 未網羅 |  | 0 |  |
| Intrinsic functions | UPOS | 未網羅 |  | 0 |  |
| Intrinsic functions | UPPER-CASE | 未網羅 |  | 2 |  |
| Intrinsic functions | USUBSTR | 未網羅 |  | 0 |  |
| Intrinsic functions | USUPPLEMENTARY | 未網羅 |  | 0 |  |
| Intrinsic functions | UVALID | 未網羅 |  | 0 |  |
| Intrinsic functions | UWIDTH | 未網羅 |  | 0 |  |
| Intrinsic functions | VARIANCE | 未網羅 |  | 2 |  |
| Intrinsic functions | WHEN-COMPILED | 使用あり | ZS101A | 2 |  |
| Intrinsic functions | YEAR-TO-YYYY | 使用あり | ZS101A, ZS107A | 0 |  |
| Special registers | ADDRESS OF | 使用あり | ZP105A, ZP108A, ZPS04A | 0 |  |
| Special registers | LENGTH OF | 使用あり | ZD104A, ZD105A, ZD113A, ZD114A, ZN102A, ZP105A | 1 |  |
| Special registers | LINAGE-COUNTER | 未網羅 |  | 4 |  |
| Special registers | RETURN-CODE | 使用あり | ZA109A, ZA110A, ZA111A, ZA112A, ZA113A, ZA114A ほか 62 | 0 |  |
| Special registers | SHIFT-OUT | 未網羅 |  | 0 |  |
| Special registers | SHIFT-IN | 未網羅 |  | 0 |  |
| Special registers | SORT-CONTROL | 未網羅 |  | 0 |  |
| Special registers | SORT-CORE-SIZE | 未網羅 |  | 0 |  |
| Special registers | SORT-FILE-SIZE | 未網羅 |  | 0 |  |
| Special registers | SORT-MESSAGE | 未網羅 |  | 0 |  |
| Special registers | SORT-MODE-SIZE | 未網羅 |  | 0 |  |
| Special registers | SORT-RETURN | 未網羅 |  | 0 |  |
| Special registers | TALLY | 使用あり | ZS101A | 6 |  |
| Special registers | WHEN-COMPILED | 使用あり | ZS101A | 2 |  |
| Special registers | XML-CODE | 未網羅 |  | 0 |  |
| Special registers | XML-EVENT | 未網羅 |  | 0 |  |
| Special registers | XML-TEXT | 未網羅 |  | 0 |  |
| Special registers | XML-NTEXT | 未網羅 |  | 0 |  |
| Special registers | JSON-CODE | 未網羅 |  | 0 |  |
| Special registers | JSON-STATUS | 未網羅 |  | 0 |  |
| Special registers | DEBUG-ITEM | 未網羅 |  | 0 |  |
| Compiler options (semantic) | ARITH | 使用あり | ZA110A, ZA116A, ZA117A, ZA118A, ZD108A, ZD111A ほか 3 | 0 |  |
| Compiler options (semantic) | CODEPAGE | 未網羅 |  | 0 |  |
| Compiler options (semantic) | DBCS | 使用あり | ZN102B, ZS106A | 0 |  |
| Compiler options (semantic) | INTDATE | 未網羅 |  | 0 |  |
| Compiler options (semantic) | NSYMBOL | 使用あり | ZN101A, ZN102A, ZN102B, ZN105A, ZN107A | 0 |  |
| Compiler options (semantic) | NUMCLS | 使用あり | ZA111A, ZA112A, ZA113A, ZA114A | 0 |  |
| Compiler options (semantic) | NUMPROC | 使用あり | ZA111A, ZA112A, ZA113A, ZA114A, ZA115A | 0 |  |
| Compiler options (semantic) | NUMCHECK | 未網羅 |  | 0 |  |
| Compiler options (semantic) | TRUNC | 使用あり | ZA101A, ZA102A, ZA103A, ZA116A, ZA117A, ZA118A ほか 2 | 0 |  |
| Compiler options (semantic) | ZWB | 未網羅 |  | 0 |  |
| Compiler options (semantic) | DYNAM | 使用あり | ZP101A, ZP109A, ZP110A, ZP111A, ZP112A | 0 |  |
| Compiler options (semantic) | QUALIFY | 未網羅 |  | 0 |  |
| Compiler options (semantic) | INITCHECK | 未網羅 |  | 0 |  |
| Compiler options (semantic) | SQLCCSID | 未網羅 |  | 0 |  |

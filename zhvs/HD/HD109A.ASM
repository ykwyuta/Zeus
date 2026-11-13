*PROCESS DBCS
* HD109A: graphic (G-type) constants under the DBCS option. The
* assembled value is the double-byte data without the shift-out and
* shift-in characters, also when duplicated; a length modifier pads
* with double-byte spaces (X4040) or truncates on the right; the
* length attribute counts the double-byte data only and the type
* attribute is @ (Language Reference, graphic constant G). The DBCS
* code point of the kanji (X4562 in CCSID 939) is from the IBM
* conversion table.
HD109A CSECT
         ZHVSHDR
* G-NO-SO-SI
         MVC ACTUAL(2),G1
         ZHVSCHK G-NO-SO-SI,ACTUAL,ZE000,2
* G-DUPLICATION
         MVC ACTUAL(6),G3
         ZHVSCHK G-DUPLICATION,ACTUAL,ZE001,6
* G-LENGTH-PADDED
         MVC ACTUAL(6),GP
         ZHVSCHK G-LENGTH-PADDED,ACTUAL,ZE002,6
* G-LENGTH-TRUNCATED
         MVC ACTUAL(2),GT
         ZHVSCHK G-LENGTH-TRUNCATED,ACTUAL,ZE003,2
* G-LENGTH-ATTRIBUTE
         MVC ACTUAL(4),KLEN
         ZHVSCHK G-LENGTH-ATTRIBUTE,ACTUAL,ZE004,4
* G-TYPE-ATTRIBUTE
         MVC ACTUAL(1),KTYPE
         ZHVSCHK G-TYPE-ATTRIBUTE,ACTUAL,ZE005,1
* G-SYSOPT-DBCS
         MVC ACTUAL(1),KOPT
         ZHVSCHK G-SYSOPT-DBCS,ACTUAL,ZE006,1
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC X'4562'
ZE001 DC X'456245624562'
ZE002 DC X'456240404040'
ZE003 DC X'4562'
ZE004 DC F'4'
ZE005 DC C'@'
ZE006 DC C'1'
G1 DC G'日'
G3 DC 3G'日'
GP DC GL6'日'
GT DC GL2'日日'
G2 DC G'日日'
KLEN DC A(L'G2)
&T SETC T'G2
KTYPE DC C'&T'
KOPT DC C'&SYSOPT_DBCS'

         LTORG
         END

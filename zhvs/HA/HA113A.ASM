* HA113A: GOFF bind-time values. J (and JD) is the length of an
* external dummy section or a class, R (and RD) the address of the
* PSECT that XATTR PSECT associates with a symbol, and CATTR places
* data in a class with its ALIGN and RMODE. The linker fills these
* in. Zeus writes this program only in its own object format
* (manifest object_formats); z/OS assembles it with GOFF.
* Expected values follow the HLASM Language Reference.
HA113A   CSECT
HA113A   XATTR PSECT(PSLBL)
         ZHVSHDR
* J-DXD
         MVC   ACTUAL(4),LJ
         ZHVSCHK J-DXD,ACTUAL,ZE000,4
* JD-DXD
         MVC   ACTUAL(8),LJD
         ZHVSCHK JD-DXD,ACTUAL,ZE001,8
* J-CLASS
         MVC   ACTUAL(4),LJC
         ZHVSCHK J-CLASS,ACTUAL,ZE002,4
* R-PSECT
         MVC   ACTUAL(4),RP
         ZHVSCHK R-PSECT,ACTUAL,APS,4
* RD-PSECT
         MVC   ACTUAL(4),RPD+4
         ZHVSCHK RD-PSECT,ACTUAL,APS,4
* CATTR-ALIGN
         L     2,ACLS
         N     2,=F'15'
         ST    2,ACTUAL
         ZHVSCHK CATTR-ALIGN,ACTUAL,ZE003,4
* CATTR-DATA
         L     1,ACLS
         MVC   ACTUAL(4),0(1)
         ZHVSCHK CATTR-DATA,ACTUAL,ZE004,4
         ZHVSSUM
ACTUAL   DS    2D
ZE000    DC    F'12'
ZE001    DC    FD'12'
ZE002    DC    F'4'
ZE003    DC    F'0'
ZE004    DC    F'31'
LJ       DC    J(DXA)
LJD      DC    JD(DXA)
LJC      DC    J(MYCLS)
RP       DC    R(HA113A)
RPD      DC    RD(HA113A)
APS      DC    A(PSLBL)
ACLS     DC    A(CLSDATA)
         LTORG
DXA      DXD   3F
MYCLS    CATTR RMODE(31),ALIGN(4)
CLSDATA  DC    F'31'
PSCLS    CATTR RMODE(31)
PSLBL    DC    F'9'
         END   HA113A

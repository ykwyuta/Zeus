* HA108A: program-structure instructions: AMODE 31 and RMODE 24
* (a load address keeps bits 1-7 in 31-bit mode) and COPY of a
* library member. Expected values follow the HLASM Language
* Reference.
HA108A   CSECT
HA108A   AMODE 31
HA108A   RMODE 24
         ZHVSHDR
* AMODE31-LA
         L     3,K00
         LA    2,0(,3)
         ST    2,ACTUAL
         ZHVSCHK AMODE31-LA,ACTUAL,ZE000,4
* COPY-MEMBER
         L     2,CPYVAL
         ST    2,ACTUAL
         ZHVSCHK COPY-MEMBER,ACTUAL,ZE001,4
         ZHVSSUM
ACTUAL   DS    2D
ZE000    DC    X'7F000001'
ZE001    DC    F'42'
K00      DC    X'7F000001'
         COPY  HACPY1
         LTORG
         END

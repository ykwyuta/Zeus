HI107A CSECT
         ZHVSHDR
* SIGNED-ZERO
         LGHI 2,0
         AGHI 2,0
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK SIGNED-ZERO,ACTUAL,ZE000,8
* SIGNED-NEG
         LGHI 2,-1
         AGHI 2,0
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK SIGNED-NEG,ACTUAL,ZE001,8
* SIGNED-POS
         LGHI 2,1
         AGHI 2,0
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK SIGNED-POS,ACTUAL,ZE002,8
* SIGNED-OVERFLOW
         LG 2,=XL8'7FFFFFFFFFFFFFFF'
         AGHI 2,1
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK SIGNED-OVERFLOW,ACTUAL,ZE003,8
* LOGICAL-ZERO
         LGHI 2,0
         LGHI 3,0
         ALGR 2,3
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK LOGICAL-ZERO,ACTUAL,ZE004,8
* LOGICAL-CARRY
         LG 2,=XL8'FFFFFFFFFFFFFFFF'
         LGHI 3,1
         ALGR 2,3
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK LOGICAL-CARRY,ACTUAL,ZE005,8
* COMPARE-LOW
         LGHI 2,-1
         LGHI 3,1
         CGR 2,3
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK COMPARE-LOW,ACTUAL,ZE006,8
* COMPARE-HIGH
         LGHI 2,1
         LGHI 3,-1
         CGR 2,3
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK COMPARE-HIGH,ACTUAL,ZE007,8
* COMPARE-EQUAL
         LGHI 2,1
         CGR 2,2
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK COMPARE-EQUAL,ACTUAL,ZE008,8
         ZHVSSUM
ACTUAL DS D
ZE000 DC XL8'0000000000000000'
ZE001 DC XL8'0000000000000001'
ZE002 DC XL8'0000000000000002'
ZE003 DC XL8'0000000000000003'
ZE004 DC XL8'0000000000000000'
ZE005 DC XL8'0000000000000002'
ZE006 DC XL8'0000000000000001'
ZE007 DC XL8'0000000000000002'
ZE008 DC XL8'0000000000000000'

         LTORG
         END

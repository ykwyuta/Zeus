* HI132A: TEST DATA GROUP (TDGET, TDGDT, TDGXT). The second-operand
* address bits 52-63 select data group and sign: safe zero (52/53),
* zero with extreme exponent (54/55), nonzero with extreme exponent
* (56/57), safe nonzero (58/59), nonzero leftmost significand digit
* with nonextreme exponent (60/61), special (62/63); condition code
* 1 when the selected bit is one. All-zero bits are +0 with the
* minimum exponent (extreme), X'..01' is 1 at the minimum exponent,
* and a 16-digit (long) or 7-digit (short) value has a nonzero
* leftmost digit. Expected values follow the Principles of
* Operation.
HI132A CSECT
         ZHVSHDR
* TDGDT-SAFE-ZERO
         XC ACTUAL(4),ACTUAL
         LD 0,DZ
         TDGDT 0,X'800'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-SAFE-ZERO,ACTUAL,ZE000,4
* TDGDT-ZERO-SIGN
         XC ACTUAL(4),ACTUAL
         LD 0,DZ
         TDGDT 0,X'400'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-ZERO-SIGN,ACTUAL,ZE001,4
* TDGDT-ZERO-EXTREME
         XC ACTUAL(4),ACTUAL
         LD 0,DZX
         TDGDT 0,X'200'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-ZERO-EXTREME,ACTUAL,ZE002,4
* TDGDT-NZ-EXTREME
         XC ACTUAL(4),ACTUAL
         LD 0,DNX
         TDGDT 0,X'080'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-NZ-EXTREME,ACTUAL,ZE003,4
* TDGDT-SAFE-NONZERO
         XC ACTUAL(4),ACTUAL
         LD 0,DONE
         TDGDT 0,X'020'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-SAFE-NONZERO,ACTUAL,ZE004,4
* TDGDT-MINUS-SAFE
         XC ACTUAL(4),ACTUAL
         LD 0,DMONE
         TDGDT 0,X'010'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-MINUS-SAFE,ACTUAL,ZE005,4
* TDGDT-LMD-NONZERO
         XC ACTUAL(4),ACTUAL
         LD 0,DFULL
         TDGDT 0,X'008'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-LMD-NONZERO,ACTUAL,ZE006,4
* TDGDT-LMD-NOT-SAFE
         XC ACTUAL(4),ACTUAL
         LD 0,DFULL
         TDGDT 0,X'020'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-LMD-NOT-SAFE,ACTUAL,ZE007,4
* TDGDT-SPECIAL-INF
         XC ACTUAL(4),ACTUAL
         LD 0,DINF
         TDGDT 0,X'002'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-SPECIAL-INF,ACTUAL,ZE008,4
* TDGDT-SAFE-PAIR
         XC ACTUAL(4),ACTUAL
         LD 0,DONE
         TDGDT 0,X'030'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGDT-SAFE-PAIR,ACTUAL,ZE009,4
* TDGET-SAFE-NONZERO
         XC ACTUAL(4),ACTUAL
         LE 0,EONE
         TDGET 0,X'020'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGET-SAFE-NONZERO,ACTUAL,ZE010,4
* TDGET-LMD-NONZERO
         XC ACTUAL(4),ACTUAL
         LE 0,EFULL
         TDGET 0,X'008'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGET-LMD-NONZERO,ACTUAL,ZE011,4
* TDGXT-SAFE-NONZERO
         XC ACTUAL(4),ACTUAL
         LD 0,XONE
         LD 2,XONE+8
         TDGXT 0,X'020'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGXT-SAFE-NONZERO,ACTUAL,ZE012,4
* TDGXT-SPECIAL-NAN
         XC ACTUAL(4),ACTUAL
         LD 0,XNAN
         LD 2,XNAN+8
         TDGXT 0,X'002'
         IPM 0
         SRL 0,28
         ST 0,ACTUAL
         ZHVSCHK TDGXT-SPECIAL-NAN,ACTUAL,ZE013,4
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC F'1'
ZE001 DC F'0'
ZE002 DC F'1'
ZE003 DC F'1'
ZE004 DC F'1'
ZE005 DC F'1'
ZE006 DC F'1'
ZE007 DC F'0'
ZE008 DC F'1'
ZE009 DC F'1'
ZE010 DC F'1'
ZE011 DC F'1'
ZE012 DC F'1'
ZE013 DC F'1'
DZ DC DD'0'
DZX DC X'0000000000000000'
DNX DC X'0000000000000001'
DONE DC DD'1'
DMONE DC DD'-1'
DFULL DC DD'1234567890123456'
DINF DC X'7800000000000000'
EONE DC ED'1'
EFULL DC ED'1234567'
         DS 0D
XONE DC LD'1'
XNAN DC X'7C000000000000000000000000000000'

         LTORG
         END

* HD108A: the LQ-type hexadecimal floating-point constant. It has
* the implicit length 16 and the value of the L-type constant
* (extended HFP: the second doubleword has the characteristic of the
* first minus 14) but quadword alignment, where L is doubleword
* aligned; DC 0LQ forces quadword alignment (Language Reference,
* hexadecimal floating-point constants).
HD108A CSECT
         ZHVSHDR
* LQ-QUADWORD-ALIGN
         MVC ACTUAL(4),KQOFF
         ZHVSCHK LQ-QUADWORD-ALIGN,ACTUAL,ZE000,4
* L-DOUBLEWORD-ALIGN
         MVC ACTUAL(4),KLOFF
         ZHVSCHK L-DOUBLEWORD-ALIGN,ACTUAL,ZE001,4
* LQ-IMPLICIT-LENGTH
         MVC ACTUAL(4),KQLEN
         ZHVSCHK LQ-IMPLICIT-LENGTH,ACTUAL,ZE002,4
* LQ-VALUE-ONE
         MVC ACTUAL(16),LQV
         ZHVSCHK LQ-VALUE-ONE,ACTUAL,ZE003,16
* LQ-EQUALS-L
         MVC ACTUAL(16),LQH
         ZHVSCHK LQ-EQUALS-L,ACTUAL,ZE004,16
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC F'16'
ZE001 DC F'8'
ZE002 DC F'16'
ZE003 DC X'41100000000000003300000000000000'
ZE004 DC L'0.5'
QB DS 0LQ
         DC X'01'
LQV DC LQ'1'
QB2 DS 0LQ
         DC X'01'
LV DC L'1'
LQH DC LQ'0.5'
KQOFF DC A(LQV-QB)
KLOFF DC A(LV-QB2)
KQLEN DC A(L'LQV)

         LTORG
         END

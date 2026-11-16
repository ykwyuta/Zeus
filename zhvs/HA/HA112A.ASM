* HA112A: COM. A common section is reserved storage that the
* statements in it do not assemble into object code (a DC in it
* leaves zeros). A common section of the same name in another
* assembly (HAS12A) is the same storage, of the length of the
* longest. Expected values follow the HLASM Language Reference.
HA112A   CSECT
         ZHVSHDR
* COM-STORAGE
         L     1,=A(XYZ)
         USING XYZ,1
         MVC   PDQ(4),=C'ABCD'
         MVC   ACTUAL(4),PDQ
         DROP  1
         ZHVSCHK COM-STORAGE,ACTUAL,ZE000,4
* COM-NOT-ASSEMBLED
         L     1,=A(XYZ)
         MVC   ACTUAL(4),0(1)
         ZHVSCHK COM-NOT-ASSEMBLED,ACTUAL,ZE001,4
* COM-SHARED
         ST    13,SAVE13
         LA    13,SAVEAREA
         L     15,=V(HAS12A)
         BASR  14,15
         L     13,SAVE13
         L     1,=A(XYZ)
         MVC   ACTUAL(4),4(1)
         ZHVSCHK COM-SHARED,ACTUAL,ZE002,4
* COM-SYMBOL-ADDRESS
         L     1,=A(XYZ)
         LA    2,PDQ-XYZ(,1)
         C     2,=A(PDQ)
         LA    2,1
         BE    *+6
         SR    2,2
         ST    2,ACTUAL
         ZHVSCHK COM-SYMBOL-ADDRESS,ACTUAL,ZE003,4
         ZHVSSUM
ACTUAL   DS    2D
SAVE13   DS    F
SAVEAREA DS    18F
ZE000    DC    C'ABCD'
ZE001    DC    F'0'
ZE002    DC    F'7'
ZE003    DC    F'1'
         LTORG
XYZ      COM
CVAL     DC    F'5'
PDQ      DS    4C
         END   HA112A

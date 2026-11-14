*PROCESS OPTABLE(UNI,NOLIST)
* HA111A: EXTRN and *PROCESS. An address constant A(HAS11A) of a
* symbol named on EXTRN is resolved by the binder to the entry of the
* separately assembled subprogram HAS11A, which returns 77 in
* register 15. The *PROCESS statement before it selects
* OPTABLE(UNI,NOLIST), which &SYSOPT_OPTABLE reports as UNI.
* Expected values follow the HLASM Language Reference.
HA111A   CSECT
         LCLC  &TX
         EXTRN HAS11A
         ZHVSHDR
* EXTRN-ACON-CALL
         ST    13,SAVE13
         LA    13,SAVEAREA
         L     15,AHAS
         BASR  14,15
         L     13,SAVE13
         ST    15,ACTUAL
         ZHVSCHK EXTRN-ACON-CALL,ACTUAL,ZE000,4
* EXTRN-TYPE-ATTR
         MVC   ACTUAL(1),K01
         ZHVSCHK EXTRN-TYPE-ATTR,ACTUAL,ZE001,1
* PROCESS-OPTABLE
         MVC   ACTUAL(3),K02
         ZHVSCHK PROCESS-OPTABLE,ACTUAL,ZE002,3
         ZHVSSUM
ACTUAL   DS    2D
SAVE13   DS    F
SAVEAREA DS    18F
ZE000    DC    F'77'
ZE001    DC    C'T'
ZE002    DC    C'UNI'
AHAS     DC    A(HAS11A)
&TX      SETC  T'HAS11A
K01      DC    C'&TX'
K02      DC    C'&SYSOPT_OPTABLE'
         LTORG
         END

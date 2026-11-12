* HM108A: the type attribute T' of DS symbols (F H C P X,
* G for an explicit length, A, D), of a control section (J), an
* instruction (I), a WXTRN symbol ($), an undefined symbol (U), a
* SETA value (N), and of macro operands (N for a number, O when
* omitted); the defined attribute D' (1 for a symbol defined before
* the reference, 0 for one defined later; through a SETC symbol);
* SYSATTRA and SYSATTRP (the assembler and program types of EQU).
* Expected values follow the HLASM Language Reference.
         MACRO
         ZTYPE &A,&B
         GBLC  &GTA,&GTB
&GTA     SETC  T'&A
&GTB     SETC  T'&B
         MEND
ZDATA    DSECT
FW       DS    F
HW       DS    H
CH       DS    CL2
PK       DS    PL3
XX       DS    X
FL       DS    FL3
AD       DS    A
LG       DS    D
GRS      EQU   5,,,,GR
PTS      EQU   1,,,C'Box7'
         WXTRN XYZ
HM108A CSECT
         ZHVSHDR
HM108B   LR    0,0
         LCLA  &N
         LCLC  &V,&A,&B,&C
         GBLC  &GTA,&GTB
* T-FULLWORD-HALF
         MVC ACTUAL(2),K00
         ZHVSCHK T-FULLWORD-HALF,ACTUAL,ZE000,2
* T-CHAR-PACKED-HEX
         MVC ACTUAL(3),K01
         ZHVSCHK T-CHAR-PACKED-HEX,ACTUAL,ZE001,3
* T-EXPLICIT-LENGTH
         MVC ACTUAL(1),K02
         ZHVSCHK T-EXPLICIT-LENGTH,ACTUAL,ZE002,1
* T-ADDRESS-LONG
         MVC ACTUAL(2),K03
         ZHVSCHK T-ADDRESS-LONG,ACTUAL,ZE003,2
* T-CSECT-INSTR
         MVC ACTUAL(2),K04
         ZHVSCHK T-CSECT-INSTR,ACTUAL,ZE004,2
* T-WXTRN
         MVC ACTUAL(1),K05
         ZHVSCHK T-WXTRN,ACTUAL,ZE005,1
* T-UNDEFINED
         MVC ACTUAL(1),K06
         ZHVSCHK T-UNDEFINED,ACTUAL,ZE006,1
* T-SETA-NUMERIC
         MVC ACTUAL(1),K07
         ZHVSCHK T-SETA-NUMERIC,ACTUAL,ZE007,1
* T-MACRO-OPERANDS
         MVC ACTUAL(2),K08
         ZHVSCHK T-MACRO-OPERANDS,ACTUAL,ZE008,2
* D-DEFINED
         MVC ACTUAL(4),K09
         ZHVSCHK D-DEFINED,ACTUAL,ZE009,4
* D-SETC-SYMBOL
         MVC ACTUAL(4),K10
         ZHVSCHK D-SETC-SYMBOL,ACTUAL,ZE010,4
* SYSATTRA
         MVC ACTUAL(6),K11
         ZHVSCHK SYSATTRA,ACTUAL,ZE011,6
* SYSATTRA-NONE
         MVC ACTUAL(4),K12
         ZHVSCHK SYSATTRA-NONE,ACTUAL,ZE012,4
* SYSATTRA-DS
         MVC ACTUAL(1),K13
         ZHVSCHK SYSATTRA-DS,ACTUAL,ZE013,1
* SYSATTRP
         MVC ACTUAL(4),K14
         ZHVSCHK SYSATTRP,ACTUAL,ZE014,4
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC C'FH'
ZE001 DC C'CPX'
ZE002 DC C'G'
ZE003 DC C'AD'
ZE004 DC C'JI'
ZE005 DC C'$'
ZE006 DC C'U'
ZE007 DC C'N'
ZE008 DC C'NO'
ZE009 DC F'10'
ZE010 DC F'1'
ZE011 DC C'GR',F'2'
ZE012 DC F'0'
ZE013 DC C'F'
ZE014 DC C'Box7'
&A SETC T'FW
&B SETC T'HW
&V SETC '&A&B'
K00 DC C'&V'
&A SETC T'CH
&B SETC T'PK
&C SETC T'XX
&V SETC '&A&B&C'
K01 DC C'&V'
&V SETC T'FL
K02 DC C'&V'
&A SETC T'AD
&B SETC T'LG
&V SETC '&A&B'
K03 DC C'&V'
&A SETC T'HM108A
&B SETC T'HM108B
&V SETC '&A&B'
K04 DC C'&V'
&V SETC T'XYZ
K05 DC C'&V'
&V SETC T'NOWHERE
K06 DC C'&V'
&N SETA 5
&V SETC T'&N
K07 DC C'&V'
         ZTYPE 12
&V SETC '&GTA&GTB'
K08 DC C'&V'
&N SETA D'FW*10+D'LATER
K09 DC F'&N'
&V SETC 'CH'
&N SETA D'&V
K10 DC F'&N'
&V SETC SYSATTRA('GRS')
&N SETA K'&V
K11 DC C'&V',F'&N'
&V SETC SYSATTRA('PTS')
&N SETA K'&V
K12 DC F'&N'
&V SETC SYSATTRA('FW')
K13 DC C'&V'
&V SETC SYSATTRP('PTS')
K14 DC C'&V'
LATER DC F'0'

         LTORG
         END

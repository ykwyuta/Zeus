* HI110A: translate, execute, long moves/compares, string and edit
* instructions. Expected values follow z/Architecture Principles of
* Operation (SA22-7832): TRT sets R1 to the byte found and R2 bits
* 56-63 to the function byte; MVCL pads with R5 bits 32-39; EDMK sets
* R1 when a nonzero digit starts significance; TM gives CC 0/1/3.
HI110A CSECT
         ZHVSHDR
* TR-TABLE
         MVC WORK(3),=X'000102'
         TR WORK(3),TRTAB
         MVC ACTUAL(3),WORK
         ZHVSCHK TR-TABLE,ACTUAL,ZE000,3
* TRT-FOUND-OFFSET
         LGHI 2,0
         LA 1,0
         TRT S1(5),FTAB
         LA 3,S1
         SR 1,3
         LLGFR 1,1
         STG 1,ACTUAL
         ZHVSCHK TRT-FOUND-OFFSET,ACTUAL,ZE001,8
* TRT-FOUND-FUNC
         LGHI 2,0
         TRT S1(5),FTAB
         N 2,=F'255'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TRT-FOUND-FUNC,ACTUAL,ZE002,8
* TRT-FOUND-CC
         TRT S1(5),FTAB
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TRT-FOUND-CC,ACTUAL,ZE003,8
* TRT-NONE-CC
         TRT S2(4),FTAB
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TRT-NONE-CC,ACTUAL,ZE004,8
* TRT-LAST-CC
         TRT S3(4),FTAB
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TRT-LAST-CC,ACTUAL,ZE005,8
* EX-MVC-LENGTH
         MVC WORK(5),=C'.....'
         LA 3,2
         EX 3,EXMVC
         MVC ACTUAL(5),WORK
         ZHVSCHK EX-MVC-LENGTH,ACTUAL,ZE006,5
* MVCL-PAD
         LA 2,WORK
         LA 3,6
         LA 4,=C'AB'
         L 5,=X'5C000002'
         MVCL 2,4
         MVC ACTUAL(6),WORK
         ZHVSCHK MVCL-PAD,ACTUAL,ZE007,6
* MVCL-PAD-CC
         LA 2,WORK
         LA 3,6
         LA 4,=C'AB'
         L 5,=X'5C000002'
         MVCL 2,4
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK MVCL-PAD-CC,ACTUAL,ZE008,8
* CLCL-PAD-EQUAL
         LA 2,=C'AB  '
         LA 3,4
         LA 4,=C'AB'
         L 5,=X'40000002'
         CLCL 2,4
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK CLCL-PAD-EQUAL,ACTUAL,ZE009,8
* CLCL-LOW-OFFSET
         LA 2,C1
         LA 3,2
         LA 4,C2
         LA 5,2
         CLCL 2,4
         LA 6,C1
         SR 2,6
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK CLCL-LOW-OFFSET,ACTUAL,ZE010,8
* MVST-COPY
         MVC WORK(6),=C'......'
         LGHI 0,0
         LA 1,WORK
         LA 2,Z1
         MVST 1,2
         MVC ACTUAL(5),WORK
         ZHVSCHK MVST-COPY,ACTUAL,ZE011,5
* MVST-END-OFFSET
         LGHI 0,0
         LA 1,WORK
         LA 2,Z1
         MVST 1,2
         LA 3,WORK
         SR 1,3
         LLGFR 1,1
         STG 1,ACTUAL
         ZHVSCHK MVST-END-OFFSET,ACTUAL,ZE012,8
* SRST-FOUND
         LGHI 0,C','
         LA 1,S1+5
         LA 2,S1
         SRST 1,2
         LA 3,S1
         SR 1,3
         LLGFR 1,1
         STG 1,ACTUAL
         ZHVSCHK SRST-FOUND,ACTUAL,ZE013,8
* CLST-EQUAL
         LGHI 0,0
         LA 1,Z1
         LA 2,Z2
         CLST 1,2
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK CLST-EQUAL,ACTUAL,ZE014,8
* EDMK-RESULT
         MVC WORK(6),EDPAT
         LA 1,0
         EDMK WORK(6),=X'00123C'
         MVC ACTUAL(6),WORK
         ZHVSCHK EDMK-RESULT,ACTUAL,ZE015,6
* EDMK-MARK-OFFSET
         MVC WORK(6),EDPAT
         LA 1,0
         EDMK WORK(6),=X'00123C'
         LA 3,WORK
         SR 1,3
         LLGFR 1,1
         STG 1,ACTUAL
         ZHVSCHK EDMK-MARK-OFFSET,ACTUAL,ZE016,8
* MVO-SHIFT
         MVC WORK(3),=X'77777F'
         MVO WORK(3),=X'1234'
         MVC ACTUAL(3),WORK
         ZHVSCHK MVO-SHIFT,ACTUAL,ZE017,3
* TM-ONES-CC
         TM TMB,X'03'
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TM-ONES-CC,ACTUAL,ZE018,8
* TM-ZEROS-CC
         TM TMB,X'30'
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TM-ZEROS-CC,ACTUAL,ZE019,8
* TM-MIXED-CC
         TM TMB,X'11'
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TM-MIXED-CC,ACTUAL,ZE020,8
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC C'ABC'
ZE001 DC XL8'02'
ZE002 DC XL8'01'
ZE003 DC XL8'01'
ZE004 DC XL8'00'
ZE005 DC XL8'02'
ZE006 DC C'XYZ..'
ZE007 DC C'AB****'
ZE008 DC XL8'02'
ZE009 DC XL8'00'
ZE010 DC XL8'01'
ZE011 DC C'ABC',X'00',C'.'
ZE012 DC XL8'03'
ZE013 DC XL8'02'
ZE014 DC XL8'00'
ZE015 DC X'404040F1F2F3'
ZE016 DC XL8'03'
ZE017 DC X'01234F'
ZE018 DC XL8'03'
ZE019 DC XL8'00'
ZE020 DC XL8'01'
WORK DS CL8
TRTAB DC C'ABC'
FTAB DC XL256'00'
         ORG FTAB+X'6B'
         DC X'01'
         ORG
S1 DC C'AB,CD'
S2 DC C'ABCD'
S3 DC C'ABC,'
C1 DC C'AB'
C2 DC C'AC'
Z1 DC C'ABC',X'00'
Z2 DC C'ABC',X'00'
EDPAT DC X'402020212020'
EXMVC MVC WORK(0),=C'XYZ'
TMB DC X'0F'

         LTORG
         END

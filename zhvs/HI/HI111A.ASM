* HI111A: branch on count/index, characters under mask, double
* shifts. Expected values follow z/Architecture Principles of
* Operation (SA22-7832): BXLE/BXH add R3 to R1 and compare with the
* odd register of the R3 pair; CLM/STCM select bytes of bits 32-63;
* SLDA keeps the sign and sets CC 3 when a significant bit is lost.
HI111A CSECT
         ZHVSHDR
* BCT-COUNT
         LGHI 2,0
         LA 3,5
HBC1 DS 0H
         AHI 2,1
         BCT 3,HBC1
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK BCT-COUNT,ACTUAL,ZE000,8
* BXLE-COUNT
         LGHI 2,0
         LA 4,0
         LA 6,2
         LA 7,10
HBX1 DS 0H
         AHI 2,1
         BXLE 4,6,HBX1
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK BXLE-COUNT,ACTUAL,ZE001,8
* BXLE-FINAL-INDEX
         LA 4,0
         LA 6,2
         LA 7,10
HBX2 DS 0H
         BXLE 4,6,HBX2
         LLGFR 4,4
         STG 4,ACTUAL
         ZHVSCHK BXLE-FINAL-INDEX,ACTUAL,ZE002,8
* BXH-COUNT
         LGHI 2,0
         LA 4,10
         LHI 6,-2
         LA 7,0
HBH1 DS 0H
         AHI 2,1
         BXH 4,6,HBH1
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK BXH-COUNT,ACTUAL,ZE003,8
* CLM-MASK-EQUAL
         L 2,=X'11C122C2'
         CLM 2,B'0101',CAB
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK CLM-MASK-EQUAL,ACTUAL,ZE004,8
* STCM-MASK
         L 2,=X'11223344'
         STCM 2,B'1010',ACTUAL
         ZHVSCHK STCM-MASK,ACTUAL,ZE005,2
* IC-PRESERVE
         L 2,=X'AABBCCDD'
         IC 2,ICB
         ST 2,ACTUAL
         ZHVSCHK IC-PRESERVE,ACTUAL,ZE006,4
* SLDA-OVERFLOW-CC
         L 4,=X'40000000'
         L 5,=F'0'
         SLDA 4,1
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK SLDA-OVERFLOW-CC,ACTUAL,ZE007,8
* SLDA-OVERFLOW-VAL
         L 4,=X'40000000'
         L 5,=F'0'
         SLDA 4,1
         STM 4,5,ACTUAL
         ZHVSCHK SLDA-OVERFLOW-VAL,ACTUAL,ZE008,8
* SRDA-NEGATIVE
         L 4,=X'FFFFFFFF'
         L 5,=X'FFFFFFF0'
         SRDA 4,2
         STM 4,5,ACTUAL
         ZHVSCHK SRDA-NEGATIVE,ACTUAL,ZE009,8
* SRDA-NEGATIVE-CC
         L 4,=X'FFFFFFFF'
         L 5,=X'FFFFFFF0'
         SRDA 4,2
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK SRDA-NEGATIVE-CC,ACTUAL,ZE010,8
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC XL8'05'
ZE001 DC XL8'06'
ZE002 DC XL8'0C'
ZE003 DC XL8'05'
ZE004 DC XL8'00'
ZE005 DC X'1133'
ZE006 DC X'AABBCCEE'
ZE007 DC XL8'03'
ZE008 DC XL8'00'
ZE009 DC X'FFFFFFFFFFFFFFFC'
ZE010 DC XL8'01'
CAB DC C'AB'
ICB DC X'EE'

         LTORG
         END

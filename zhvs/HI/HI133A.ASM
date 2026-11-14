* HI133A: COMPARE AND SWAP AND STORE (CSST). General register 0
* holds the store characteristic (bits 48-55) and the function code
* (bits 56-63); general register 1 addresses a quadword-aligned
* parameter list with the replacement value at byte 0 and the store
* value at byte 16. Equal: the replacement value goes to the first
* operand, the store value (1, 2, 4 or 8 bytes for SC 0-3) to the
* second, CC 0. Unequal: the first operand is loaded into R3, the
* second operand is not changed, CC 1. Function code 1 works on
* doublewords. Expected values follow the Principles of Operation.
HI133A CSECT
         ZHVSHDR
* CSST-FC0-EQUAL
         MVC OP1(4),K5
         XC OP2(8),OP2
         LA 1,PLRAW+15
         N 1,=X'7FFFFFF0'
         MVC 0(4,1),K9
         MVC 16(8,1),KSTORE
         L 3,K5
         L 0,=F'512'
         CSST OP1,OP2,3
         IPM 2
         SRL 2,28
         ST 2,ACTUAL
         MVC ACTUAL+4(4),OP1
         MVC ACTUAL+8(4),OP2
         ST 3,ACTUAL+12
         ZHVSCHK CSST-FC0-EQUAL,ACTUAL,ZE000,16
* CSST-FC0-UNEQUAL
         MVC OP1(4),K6
         XC OP2(8),OP2
         LA 1,PLRAW+15
         N 1,=X'7FFFFFF0'
         MVC 0(4,1),K9
         MVC 16(8,1),KSTORE
         L 3,K5
         L 0,=F'512'
         CSST OP1,OP2,3
         IPM 2
         SRL 2,28
         ST 2,ACTUAL
         MVC ACTUAL+4(4),OP1
         MVC ACTUAL+8(4),OP2
         ST 3,ACTUAL+12
         ZHVSCHK CSST-FC0-UNEQUAL,ACTUAL,ZE001,16
* CSST-SC0-ONE-BYTE
         MVC OP1(4),K5
         XC OP2(8),OP2
         LA 1,PLRAW+15
         N 1,=X'7FFFFFF0'
         MVC 0(4,1),K9
         MVC 16(8,1),KSTORE
         L 3,K5
         L 0,=F'0'
         CSST OP1,OP2,3
         IPM 2
         SRL 2,28
         ST 2,ACTUAL
         MVC ACTUAL+4(4),OP1
         MVC ACTUAL+8(4),OP2
         ST 3,ACTUAL+12
         ZHVSCHK CSST-SC0-ONE-BYTE,ACTUAL,ZE002,16
* CSST-SC1-HALFWORD
         MVC OP1(4),K5
         XC OP2(8),OP2
         LA 1,PLRAW+15
         N 1,=X'7FFFFFF0'
         MVC 0(4,1),K9
         MVC 16(8,1),KHALF
         L 3,K5
         L 0,=F'256'
         CSST OP1,OP2,3
         IPM 2
         SRL 2,28
         ST 2,ACTUAL
         MVC ACTUAL+4(4),OP1
         MVC ACTUAL+8(4),OP2
         ST 3,ACTUAL+12
         ZHVSCHK CSST-SC1-HALFWORD,ACTUAL,ZE003,16
* CSST-FC1-SC3
         MVC OPG(8),KG1
         XC OP2(8),OP2
         LA 1,PLRAW+15
         N 1,=X'7FFFFFF0'
         MVC 0(8,1),KG2
         MVC 16(8,1),KG3
         LG 3,KG1
         L 0,=F'769'
         CSST OPG,OP2,3
         MVC ACTUAL(8),OPG
         MVC ACTUAL+8(8),OP2
         ZHVSCHK CSST-FC1-SC3,ACTUAL,ZE004,16
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC F'0',F'9',F'123',F'5'
ZE001 DC F'1',F'6',F'0',F'6'
ZE002 DC F'0',F'9',X'00000000',F'5'
ZE003 DC F'0',F'9',X'ABCD0000',F'5'
ZE004 DC FD'200',FD'300'
         DS 0D
OP1 DS F
         DS 0D
OP2 DS XL8
OPG DS FD
PLRAW DS XL48
K5 DC F'5'
K6 DC F'6'
K9 DC F'9'
KSTORE DC F'123',F'0'
KHALF DC X'ABCD',XL6'00'
KG1 DC FD'100'
KG2 DC FD'200'
KG3 DC FD'300'

         LTORG
         END

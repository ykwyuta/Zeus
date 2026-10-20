HD105A CSECT
         ZHVSHDR
* TP-SIGN-0
         MVC TEST(1),=X'10'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-0,ACTUAL,ZE000,8
* TP-SIGN-1
         MVC TEST(1),=X'11'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-1,ACTUAL,ZE001,8
* TP-SIGN-2
         MVC TEST(1),=X'12'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-2,ACTUAL,ZE002,8
* TP-SIGN-3
         MVC TEST(1),=X'13'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-3,ACTUAL,ZE003,8
* TP-SIGN-4
         MVC TEST(1),=X'14'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-4,ACTUAL,ZE004,8
* TP-SIGN-5
         MVC TEST(1),=X'15'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-5,ACTUAL,ZE005,8
* TP-SIGN-6
         MVC TEST(1),=X'16'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-6,ACTUAL,ZE006,8
* TP-SIGN-7
         MVC TEST(1),=X'17'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-7,ACTUAL,ZE007,8
* TP-SIGN-8
         MVC TEST(1),=X'18'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-8,ACTUAL,ZE008,8
* TP-SIGN-9
         MVC TEST(1),=X'19'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-9,ACTUAL,ZE009,8
* TP-SIGN-A
         MVC TEST(1),=X'1A'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-A,ACTUAL,ZE010,8
* TP-SIGN-B
         MVC TEST(1),=X'1B'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-B,ACTUAL,ZE011,8
* TP-SIGN-C
         MVC TEST(1),=X'1C'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-C,ACTUAL,ZE012,8
* TP-SIGN-D
         MVC TEST(1),=X'1D'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-D,ACTUAL,ZE013,8
* TP-SIGN-E
         MVC TEST(1),=X'1E'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-E,ACTUAL,ZE014,8
* TP-SIGN-F
         MVC TEST(1),=X'1F'
         TP TEST(1)
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-SIGN-F,ACTUAL,ZE015,8
* TP-INPUT-KEPT
         ZHVSCHK TP-INPUT-KEPT,TEST,ZE016,1
         ZHVSSUM
ACTUAL DS D
TEST DS X
ZE000 DC XL8'0000000000000001'
ZE001 DC XL8'0000000000000001'
ZE002 DC XL8'0000000000000001'
ZE003 DC XL8'0000000000000001'
ZE004 DC XL8'0000000000000001'
ZE005 DC XL8'0000000000000001'
ZE006 DC XL8'0000000000000001'
ZE007 DC XL8'0000000000000001'
ZE008 DC XL8'0000000000000001'
ZE009 DC XL8'0000000000000001'
ZE010 DC XL8'0000000000000000'
ZE011 DC XL8'0000000000000000'
ZE012 DC XL8'0000000000000000'
ZE013 DC XL8'0000000000000000'
ZE014 DC XL8'0000000000000000'
ZE015 DC XL8'0000000000000000'
ZE016 DC X'1F'

         LTORG
         END

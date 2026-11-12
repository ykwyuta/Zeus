* HS105A: a data exception (AP on an invalid packed operand) ends
* the step abnormally. TEST DECIMAL classifies the operands first
* (Principles of Operation: CC 1 invalid sign, CC 2 invalid digit,
* basis spec). The report is then closed and AP takes the program
* interruption; the step must not return. Zeus ends with RC 12
* (manifest expect_exit_code); z/OS gives completion code S0C7,
* which the run records (basis zos).
HS105A CSECT
         ZHVSHDR
* TP-BAD-SIGN-CC
         TP BADP
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-BAD-SIGN-CC,ACTUAL,ZE000,8
* TP-BAD-DIGIT-CC
         TP BADD
         IPM 2
         SRL 2,28
         N 2,=F'3'
         LLGFR 2,2
         STG 2,ACTUAL
         ZHVSCHK TP-BAD-DIGIT-CC,ACTUAL,ZE001,8
         MVC ZLINE(120),ZBLANK
         UNPK ZLINE(6),ZPASS
         OI ZLINE+5,X'F0'
         MVC ZLINE+6(4),=C' OF '
         UNPK ZLINE+10(6),ZTOTAL
         OI ZLINE+15,X'F0'
         MVC ZLINE+16(33),=C' TESTS WERE EXECUTED SUCCESSFULLY'
         ZHVSPUT
         MVC ZLINE(120),ZBLANK
         UNPK ZLINE(6),ZFAIL
         OI ZLINE+5,X'F0'
         MVC ZLINE+6(15),=C' TEST(S) FAILED'
         ZHVSPUT
         MVC ZLINE(120),ZBLANK
         MVC ZLINE(18),=C'NO TEST(S) DELETED'
         ZHVSPUT
         MVC ZCMD(4),=F'2'
         LA 1,ZPLIST
         L 15,=V(ZHVRPT)
         BASR 14,15
* THE REPORT IS CLOSED. AP MUST TAKE A DATA EXCEPTION (0C7);
* IF IT RETURNS, THE STEP ENDS WITH RC 0 AND THE MANIFEST
* REJECTS IT.
         AP GOODP,BADP
         L 14,ZRETURN
         XR 15,15
         BR 14
ZRETURN  DS F
ZPASS    DC PL4'0'
ZFAIL    DC PL4'0'
ZTOTAL   DC PL4'0'
ZBLANK   DC CL120' '
ZPLIST   DC A(ZCMD+X'80000000')
ZCMD     DC F'0'
ZLINE    DS CL120
ACTUAL DS 2D
ZE000 DC XL8'01'
ZE001 DC XL8'02'
BADP DC X'1234'
BADD DC X'1A2C'
GOODP DC P'1'

         LTORG
         END

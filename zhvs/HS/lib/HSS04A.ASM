HSS04A CSECT
         USING HSS04A,15
         STM 0,12,SAVE
         ST 14,RET
         LR 11,15
         USING HSS04A,11
         DROP 15
         L 9,0(1)
         N 9,=X'7FFFFFFF'
         LA 8,4(9)
         LINK EP=HSS04P,PARAM=((9),(8)),VL=1
         MVC 8(8,9),0(9)
         LINK EP=HSS04C,PARAM=((9),(8)),VL=1
         L 14,RET
         LM 0,12,SAVE
         XR 15,15
         BR 14
SAVE DS 13F
RET DS F
         LTORG
         END

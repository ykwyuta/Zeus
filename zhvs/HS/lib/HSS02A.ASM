HSS02A CSECT
         USING HSS02A,15
         STM 0,12,SAVE
         ST 14,RET
         LR 11,15
         USING HSS02A,11
         DROP 15
         L 9,0(1)
         N 9,=X'7FFFFFFF'
         GETMAIN RU,LV=16,SP=7
         LR 8,1
         MVC 0(4,8),=X'12345678'
         MVC 0(4,9),0(8)
         FREEMAIN RU,LV=16,A=(8),SP=7
         ST 15,4(9)
         POST ECB
         MVC 8(4,9),ECB
         L 14,RET
         LM 0,12,SAVE
         XR 15,15
         BR 14
SAVE DS 13F
RET DS F
ECB DC F'0'
         LTORG
         END

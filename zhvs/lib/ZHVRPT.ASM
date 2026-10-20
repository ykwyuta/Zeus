* GPL-3.0-or-later. REPORT adapter shared by all object formats.
ZHVRPT CSECT
         USING ZHVRPT,15
         STM 0,12,ZSAVE
         ST 14,ZRET
         LR 11,15
         USING ZHVRPT,11
         DROP 15
         L 2,0(1)
         N 2,=X'7FFFFFFF'
         L 3,0(2)
         LTR 3,3
         BZ ZOPEN
         CH 3,=H'2'
         BE ZCLOSE
         MVC ZBUFFER(120),4(2)
         PUT ZREPORT,ZBUFFER
         B ZDONE
ZOPEN    OPEN (ZREPORT,OUTPUT)
         B ZDONE
ZCLOSE   CLOSE (ZREPORT)
ZDONE    L 14,ZRET
         LM 0,12,ZSAVE
         XR 15,15
         BR 14
ZSAVE    DS 13F
ZRET     DS F
ZBUFFER  DS CL120
ZREPORT  DCB DDNAME=REPORT,MACRF=PM,RECFM=F,LRECL=120
         LTORG
         END

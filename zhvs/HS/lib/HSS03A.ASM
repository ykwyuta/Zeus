HSS03A CSECT
         USING HSS03A,15
         STM 0,12,SAVE
         ST 14,RET
         LR 11,15
         USING HSS03A,11
         DROP 15
         L 9,0(1)
         N 9,=X'7FFFFFFF'
         OPEN (OUT,OUTPUT)
         PUT OUT,REC1
         PUT OUT,REC2
         PUT OUT,REC3
         CLOSE (OUT)
         OPEN (IN,INPUT)
         XR 6,6
         XR 7,7
LOOP     GET IN,BUFFER
         AHI 6,1
         XR 2,2
         IC 2,BUFFER
         AR 7,2
         B LOOP
EOF      CLOSE (IN)
         ST 6,0(9)
         ST 7,4(9)
         L 14,RET
         LM 0,12,SAVE
         XR 15,15
         BR 14
SAVE DS 13F
RET DS F
REC1 DC C'ABCD'
REC2 DC C'WXYZ'
REC3 DC C'1234'
BUFFER DS CL4
IN DCB DDNAME=XXXXX001,MACRF=GM,RECFM=F,LRECL=4,EODAD=EOF
OUT DCB DDNAME=XXXXX001,MACRF=PM,RECFM=F,LRECL=4
         LTORG
         END

PMS02A CSECT
         USING PMS02A,15
         L 2,0(1)
         N 2,=X'7FFFFFFF'
         L 3,0(2)
         AHI 3,11
         ST 3,0(2)
         L 2,4(1)
         N 2,=X'7FFFFFFF'
         MVC 0(4,2),=C'ASML'
         LA 15,9
         BR 14
         LTORG
         END

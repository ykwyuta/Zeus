* HAS12A: subprogram of HA112A. Its common section XYZ is longer
* than the one of HA112A; it stores 7 in the second word of XYZ.
HAS12A   CSECT
         USING HAS12A,15
         L     1,=A(XYZ)
         MVC   4(4,1),=F'7'
         BR    14
         LTORG
XYZ      COM
         DS    F
         DS    F
         DS    16F
         END

* HD107A: assembler constants whose conversion the HLASM Language
* Reference does not pin down to the bit: rounding of decimal
* values into HFP, fixed-point constants with a fraction,
* scale modifiers, the decimal point in P constants, and the
* contents of DS storage. Fixed-point fractions are rounded to a
* whole number, P ignores the decimal point and Z puts the sign in
* the zone (Language Reference, basis spec); the HFP rounding of
* E/D/L and DS storage are basis zos (current Zeus assembler).
HD107A CSECT
         ZHVSHDR
* E-TENTH
         MVC ACTUAL(4),KE
         ZHVSCHK E-TENTH,ACTUAL,ZE000,4
* D-TENTH
         MVC ACTUAL(8),KD
         ZHVSCHK D-TENTH,ACTUAL,ZE001,8
* L-TENTH
         MVC ACTUAL(16),KL
         ZHVSCHK L-TENTH,ACTUAL,ZE002,16
* F-FRACTION
         MVC ACTUAL(4),KF
         ZHVSCHK F-FRACTION,ACTUAL,ZE003,4
* H-NEG-FRACTION
         MVC ACTUAL(2),KH
         ZHVSCHK H-NEG-FRACTION,ACTUAL,ZE004,2
* FS-SCALE
         MVC ACTUAL(4),KFS
         ZHVSCHK FS-SCALE,ACTUAL,ZE005,4
* P-DECIMAL-POINT
         MVC ACTUAL(2),KP
         ZHVSCHK P-DECIMAL-POINT,ACTUAL,ZE006,2
* Z-NEGATIVE
         MVC ACTUAL(2),KZ
         ZHVSCHK Z-NEGATIVE,ACTUAL,ZE007,2
* E-ROUND-HALF
         MVC ACTUAL(4),KEH
         ZHVSCHK E-ROUND-HALF,ACTUAL,ZE008,4
* DS-UNINITIALIZED
         MVC ACTUAL(8),KDS
         ZHVSCHK DS-UNINITIALIZED,ACTUAL,ZE009,8
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC XL4'4019999A'
ZE001 DC XL8'401999999999999A'
ZE002 DC XL16'4019999999999999329999999999999A'
ZE003 DC XL4'00000002'
ZE004 DC XL2'FFFE'
ZE005 DC XL4'00000018'
ZE006 DC XL2'125C'
ZE007 DC XL2'F1D2'
ZE008 DC XL4'40555555'
ZE009 DC XL8'0000000000000000'
KE DC E'0.1'
KD DC D'0.1'
KL DC L'0.1'
KF DC F'1.5'
KH DC H'-1.5'
KFS DC FS4'1.5'
KP DC P'1.25'
KZ DC Z'-12'
KEH DC E'0.33333333'
KDS DS D

         LTORG
         END

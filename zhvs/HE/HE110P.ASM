* invalid hex: single violated rule and repaired control.
HE110P CSECT
 USING HE110P,12
 LR 12,15
 DC X'FF'
 BR 14
A DS 16X
 END

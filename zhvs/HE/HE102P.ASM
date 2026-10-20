* 12 bit displacement: single violated rule and repaired control.
HE102P CSECT
 USING HE102P,12
 LR 12,15
 L 2,4095(3)
 BR 14
A DS 16X
 END

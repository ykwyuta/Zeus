* byte immediate: single violated rule and repaired control.
HE106P CSECT
 USING HE106P,12
 LR 12,15
 MVI 0(3),255
 BR 14
A DS 16X
 END

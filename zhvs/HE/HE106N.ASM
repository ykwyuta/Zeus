* byte immediate: single violated rule and repaired control.
HE106N CSECT
 USING HE106N,12
 LR 12,15
 MVI 0(3),256
 BR 14
A DS 16X
 END

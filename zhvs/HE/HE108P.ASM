* decimal length: single violated rule and repaired control.
HE108P CSECT
 USING HE108P,12
 LR 12,15
 ZAP A(16),A(1)
 BR 14
A DS 16X
 END

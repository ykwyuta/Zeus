* HM111A: AREAD and AINSERT. AREAD in a macro reads the next source
* record after the macro instruction as 80 characters (sequence
* field included), successive AREADs read successive records, and
* the records read are not assembled. AINSERT in a macro queues
* records that enter the input after the outermost macro call (FRONT
* before BACK); in open code the record is the next statement. AREAD
* takes queued AINSERT records first. AREAD CLOCKD gives HHMMSSTH
* (eight digits). Expected values follow the HLASM Language
* Reference.
         MACRO
         ZREAD
         GBLC  &GR1,&GR2,&GSQ
         GBLA  &GLEN
         LCLC  &R
&R       AREAD
&GR1     SETC  '&R'(1,8)
&GLEN    SETA  K'&R
&GSQ     SETC  '&R'(73,8)
&R       AREAD
&GR2     SETC  '&R'(1,8)
         MEND
         MACRO
         ZQUEUE
         AINSERT '&&GORD SETC ''&&GORD.A''',BACK
         AINSERT '&&GORD SETC ''&&GORD.B''',FRONT
         MEND
         MACRO
         ZBUF
         GBLC  &GBUF,&GCLK
         LCLC  &R
         AINSERT 'XYZW',BACK
&R       AREAD
&GBUF    SETC  '&R'(1,4)
&GCLK    AREAD CLOCKD
         MEND
HM111A CSECT
         ZHVSHDR
         GBLC  &GR1,&GR2,&GSQ,&GORD,&GOPEN,&GBUF,&GCLK
         GBLA  &GLEN
         LCLA  &N,&M
         LCLB  &X
         ZREAD
HELLO111 THIS RECORD IS READ BY AREAD AND NOT ASSEMBLED                 HM111SEQ
SECOND11 SO IS THIS ONE
&GORD    SETC  ''
         ZQUEUE
         AINSERT '&&GOPEN SETC ''OPEN''',BACK
         ZBUF
* AREAD-RECORD
         MVC ACTUAL(8),K00
         ZHVSCHK AREAD-RECORD,ACTUAL,ZE000,8
* AREAD-80-COLUMNS
         MVC ACTUAL(12),K01
         ZHVSCHK AREAD-80-COLUMNS,ACTUAL,ZE001,12
* AREAD-SUCCESSIVE
         MVC ACTUAL(8),K02
         ZHVSCHK AREAD-SUCCESSIVE,ACTUAL,ZE002,8
* AINSERT-FRONT-BACK
         MVC ACTUAL(2),K03
         ZHVSCHK AINSERT-FRONT-BACK,ACTUAL,ZE003,2
* AINSERT-OPEN-CODE
         MVC ACTUAL(4),K04
         ZHVSCHK AINSERT-OPEN-CODE,ACTUAL,ZE004,4
* AREAD-FROM-AINSERT
         MVC ACTUAL(4),K05
         ZHVSCHK AREAD-FROM-AINSERT,ACTUAL,ZE005,4
* AREAD-CLOCKD
         MVC ACTUAL(8),K06
         ZHVSCHK AREAD-CLOCKD,ACTUAL,ZE006,8
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC C'HELLO111'
ZE001 DC F'80',C'HM111SEQ'
ZE002 DC C'SECOND11'
ZE003 DC C'BA'
ZE004 DC C'OPEN'
ZE005 DC C'XYZW'
ZE006 DC F'8',F'1'
K00 DC C'&GR1'
K01 DC F'&GLEN',C'&GSQ'
K02 DC C'&GR2'
K03 DC C'&GORD'
K04 DC C'&GOPEN'
K05 DC C'&GBUF'
&N SETA K'&GCLK
&X SETB (ISDEC('&GCLK'))
&M SETA &X
K06 DC F'&N',F'&M'

         LTORG
         END

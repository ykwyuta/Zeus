* HM105A: conditional assembly beyond HM101A-HM104A. Expected values
* follow the HLASM Language Reference (SC26-4940): &SYSNDX increases
* by one per macro call; GBLA/GBLC keep values across calls; MEXIT
* leaves the macro; AGO with a list selects by the arithmetic value;
* N'&SYSLIST counts the positional operands; D2A converts a decimal
* string; SETA division truncates toward zero; character comparison
* of unequal lengths makes the shorter string the lower one; a
* duplication factor repeats a SETC value.
         MACRO
         HMNDX
         GBLA &HMN1,&HMN2,&HMCNT
&HMCNT   SETA &HMCNT+1
         AIF  (&HMCNT NE 1).SECOND
&HMN1    SETA &SYSNDX
         MEXIT
.SECOND  ANOP
&HMN2    SETA &SYSNDX
         MEND
         MACRO
         HMEX
         GBLA &HMEXV
&HMEXV   SETA 1
         MEXIT
&HMEXV   SETA 2
         MEND
         MACRO
         HMCAT &P
         GBLC &HMCATV
&HMCATV  SETC '&HMCATV&P'
         MEND
         MACRO
         HMLIST
         GBLA &HMLN
         GBLC &HML2
&HMLN    SETA N'&SYSLIST
&HML2    SETC '&SYSLIST(2)'
         MEND
         GBLA &HMN1,&HMN2,&HMCNT,&HMEXV,&HMLN
         GBLC &HMCATV,&HML2
         LCLA &I,&Q,&R,&D,&V,&STEP
         LCLB &LT1,&LT2
         LCLC &U
         HMNDX
         HMNDX
         HMEX
         HMCAT A
         HMCAT BC
         HMLIST X,YY,Z
&STEP    SETA &HMN2-&HMN1
&I       SETA 0
         AGO  .SKIP1
&I       SETA 99
.SKIP1   ANOP
&Q       SETA 2
         AGO  (&Q).G1,.G2,.G3
.G1      ANOP
&R       SETA 1
         AGO  .GEND
.G2      ANOP
&R       SETA 2
         AGO  .GEND
.G3      ANOP
&R       SETA 3
.GEND    ANOP
&V       SETA D2A('-12')
&D       SETA -7/2
&LT1     SETB ('ABC' LT 'ABD')
&LT2     SETB ('B' LT 'AAA')
&U       SETC (3)'AB'
HM105A CSECT
         ZHVSHDR
* SYSNDX-STEP
         LGHI 2,&STEP
         STG 2,ACTUAL
         ZHVSCHK SYSNDX-STEP,ACTUAL,ZE000,8
* GBLA-ACROSS-CALLS
         LGHI 2,&HMCNT
         STG 2,ACTUAL
         ZHVSCHK GBLA-ACROSS-CALLS,ACTUAL,ZE001,8
* MEXIT-STOPS
         LGHI 2,&HMEXV
         STG 2,ACTUAL
         ZHVSCHK MEXIT-STOPS,ACTUAL,ZE002,8
* GBLC-APPEND
         ZHVSCHK GBLC-APPEND,ZT003,ZE003,3
* SYSLIST-COUNT
         LGHI 2,&HMLN
         STG 2,ACTUAL
         ZHVSCHK SYSLIST-COUNT,ACTUAL,ZE004,8
* SYSLIST-ELEMENT
         ZHVSCHK SYSLIST-ELEMENT,ZT005,ZE005,2
* AGO-SKIPS
         LGHI 2,&I
         STG 2,ACTUAL
         ZHVSCHK AGO-SKIPS,ACTUAL,ZE006,8
* AGO-COMPUTED
         LGHI 2,&R
         STG 2,ACTUAL
         ZHVSCHK AGO-COMPUTED,ACTUAL,ZE007,8
* D2A-NEGATIVE
         LGHI 2,&V
         STG 2,ACTUAL
         ZHVSCHK D2A-NEGATIVE,ACTUAL,ZE008,8
* SETA-DIVIDE-TRUNC
         LGHI 2,&D
         STG 2,ACTUAL
         ZHVSCHK SETA-DIVIDE-TRUNC,ACTUAL,ZE009,8
* COMPARE-SAME-LENGTH
         LGHI 2,&LT1
         STG 2,ACTUAL
         ZHVSCHK COMPARE-SAME-LENGTH,ACTUAL,ZE010,8
* COMPARE-SHORTER-LOW
         LGHI 2,&LT2
         STG 2,ACTUAL
         ZHVSCHK COMPARE-SHORTER-LOW,ACTUAL,ZE011,8
* DUPLICATION-FACTOR
         ZHVSCHK DUPLICATION-FACTOR,ZT012,ZE012,6
         ZHVSSUM
ACTUAL DS D
ZT003 DC C'&HMCATV'
ZT005 DC C'&HML2'
ZT012 DC C'&U'
ZE000 DC XL8'01'
ZE001 DC XL8'02'
ZE002 DC XL8'01'
ZE003 DC C'ABC'
ZE004 DC XL8'03'
ZE005 DC C'YY'
ZE006 DC XL8'00'
ZE007 DC XL8'02'
ZE008 DC XL8'FFFFFFFFFFFFFFF4'
ZE009 DC XL8'FFFFFFFFFFFFFFFD'
ZE010 DC XL8'01'
ZE011 DC XL8'01'
ZE012 DC C'ABABAB'

         LTORG
         END

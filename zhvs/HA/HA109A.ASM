* HA109A: ENTRY (a V-type constant for an entry symbol of this
* module resolves to its address), an external dummy section (DXD)
* with its Q-type offset and
* the CXD total length (the total covers the section; Q is a fullword
* multiple for a fullword-aligned section), and RSECT (&SYSSTYP is
* RSECT inside it, and its data is read normally). Zeus writes this
* program only in its own object format (manifest object_formats).
* Expected values follow the HLASM Language Reference.
         MACRO
         ZSTYP
         GBLC  &GSTYP
&GSTYP   SETC  '&SYSSTYP'
         MEND
HA109A   CSECT
         ENTRY ALTENT
         GBLC  &GSTYP
         ZHVSHDR
* DXD-CXD-COVERS
         L     2,LCXD
         S     2,QDX
         LA    3,8
         CR    2,3
         LA    2,0
         BL    *+8
         LA    2,1
         ST    2,ACTUAL
         ZHVSCHK DXD-CXD-COVERS,ACTUAL,ZE000,4
* DXD-Q-ALIGNED
         L     2,QDX
         N     2,K01
         ST    2,ACTUAL
         ZHVSCHK DXD-Q-ALIGNED,ACTUAL,ZE001,4
* RSECT-DATA
         L     1,ARSV
         L     2,0(,1)
         ST    2,ACTUAL
         ZHVSCHK RSECT-DATA,ACTUAL,ZE002,4
* RSECT-SYSSTYP
         MVC   ACTUAL(5),K02
         ZHVSCHK RSECT-SYSSTYP,ACTUAL,ZE003,5
* ENTRY-VCON
         L     2,VALT
         S     2,AALT
         ST    2,ACTUAL
         ZHVSCHK ENTRY-VCON,ACTUAL,ZE004,4
         ZHVSSUM
ALTENT   BR    14
ACTUAL   DS    2D
ZE000    DC    F'1'
ZE001    DC    F'0'
ZE002    DC    F'7'
ZE003    DC    C'RSECT'
ZE004    DC    F'0'
K01      DC    F'3'
VALT     DC    V(ALTENT)
AALT     DC    A(ALTENT)
QDX      DC    Q(DXAREA)
LCXD     CXD
ARSV     DC    A(RSV)
DXAREA   DXD   2F
         LTORG
HA109R   RSECT
         ZSTYP
RSV      DC    F'7'
HA109A   CSECT
K02      DC    C'&GSTYP'
         END

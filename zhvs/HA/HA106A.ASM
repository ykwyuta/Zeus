* HA106A: data constants and channel command words the suite did not
* use. Expected bytes follow the HLASM Language Reference and
* z/Architecture Principles of Operation: CA is ASCII, CE EBCDIC and
* CU UTF-16 characters; Y is a halfword, AD a doubleword and AL3 a
* three-byte address constant; CCW (format 0) is command, 24-bit
* address, flags, zero, count, and CCW1 (format 1) is command, flags,
* count, 31-bit address; both are aligned on a doubleword and have
* length attribute 8. Addresses are differences inside the section
* so that the bytes do not depend on where the program is loaded.
HA106A CSECT
         ZHVSHDR
* DC-CA-ASCII
         MVC ACTUAL(2),KCA
         ZHVSCHK DC-CA-ASCII,ACTUAL,ZE000,2
* DC-CE-EBCDIC
         MVC ACTUAL(2),KCE
         ZHVSCHK DC-CE-EBCDIC,ACTUAL,ZE001,2
* DC-CU-UTF16
         MVC ACTUAL(4),KCU
         ZHVSCHK DC-CU-UTF16,ACTUAL,ZE002,4
* DC-Y-HALFWORD
         MVC ACTUAL(2),KY
         ZHVSCHK DC-Y-HALFWORD,ACTUAL,ZE003,2
* DC-AD-DOUBLEWORD
         MVC ACTUAL(8),KAD
         ZHVSCHK DC-AD-DOUBLEWORD,ACTUAL,ZE004,8
* DC-AL3-LENGTH
         MVC ACTUAL(3),KAL3
         ZHVSCHK DC-AL3-LENGTH,ACTUAL,ZE005,3
* CCW0-LAYOUT
         MVC ACTUAL(8),KCCW0
         ZHVSCHK CCW0-LAYOUT,ACTUAL,ZE006,8
* CCW1-LAYOUT
         MVC ACTUAL(8),KCCW1
         ZHVSCHK CCW1-LAYOUT,ACTUAL,ZE007,8
* CCW-ALIGNED
         LA 2,KCCW0
         N 2,=F'7'
         ST 2,ACTUAL
         ZHVSCHK CCW-ALIGNED,ACTUAL,ZE008,4
* CCW-LENGTH-ATTR
         LA 2,L'KCCW0
         ST 2,ACTUAL
         ZHVSCHK CCW-LENGTH-ATTR,ACTUAL,ZE009,4
         ZHVSSUM
ACTUAL DS 2D
ZE000 DC X'4142'
ZE001 DC X'C1C2'
ZE002 DC X'00410042'
ZE003 DC X'0010'
ZE004 DC X'0000000000000010'
ZE005 DC X'000010'
ZE006 DC X'0200001020000050'
ZE007 DC X'0640000800000010'
ZE008 DC F'0'
ZE009 DC F'8'
KCA DC CA'AB'
KCE DC CE'AB'
KCU DC CU'AB'
KY DC Y(TGT-TBASE)
KAD DC AD(TGT-TBASE)
KAL3 DC AL3(TGT-TBASE)
KPAD DC X'FF'
KCCW0 CCW X'02',TGT-TBASE,X'20',80
KCCW1 CCW1 X'06',TGT-TBASE,X'40',8
TBASE DS 0D
         DS XL16
TGT DS XL1

         LTORG
         END

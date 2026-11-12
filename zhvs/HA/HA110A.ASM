         ICTL  2,71,16
 * HA110A: ICTL 2,71,16 makes column 2 the begin column: column 1 of
 * every statement is ignored (here a statement carries a stray 9
 * there), labels and comment asterisks start in column 2, and macros
 * from the library keep the standard format. Expected values follow
 * the HLASM Language Reference.
 HA110A   CSECT
          ZHVSHDR
 * ICTL-COLUMN1-IGNORED
9         LA    2,5
          ST    2,ACTUAL
          ZHVSCHK ICTL-COLUMN1-IGNORED,ACTUAL,ZE000,4
 * ICTL-LABEL-COLUMN2
          L     2,LBL
          ST    2,ACTUAL
          ZHVSCHK ICTL-LABEL-COLUMN2,ACTUAL,ZE001,4
          ZHVSSUM
 ACTUAL   DS    2D
 ZE000    DC    F'5'
 ZE001    DC    F'11'
 LBL      DC    F'11'
          LTORG
          END

#!/bin/sh
# Sets up the optional Db2 module of CardDemo (transaction types) on Zeus
# and Db2 Community Edition, and runs it.
#
# usage: tools/carddemo-db2.sh setup        # after tools/carddemo.sh setup
#        tools/carddemo-db2.sh batch        # MNTTRDB2: COBTUPDT updates the table
#        tools/carddemo-db2.sh test         # setup, batch, online scripts
#
# ZEUS_DB2 names the Db2 server (tools/db2ce.sh start prints it).
set -eu

home=${ZEUS_HOME:-work/carddemo}
app=.cache/carddemo/app
mod=$app/app-transaction-type-db2
hlq=AWS.M2.CARDDEMO
bin=target/release
zjob="$bin/zjob --home $home"
out=$home/online
: "${ZEUS_DB2:?set ZEUS_DB2 (tools/db2ce.sh start)}"
export ZEUS_DB2

submit() { # JCL file, with TYPRUN=SCAN removed as the installer does
    sed 's/,TYPRUN=SCAN//; s/TYPRUN=SCAN,//' "$1" >"$home/$(basename "$1")"
    $zjob submit "$home/$(basename "$1")"
}

reset() { # drops the tables CREADB21 creates, so that it can run again
    cat >"$home/DROPDB2.jcl" <<'EOF'
//DROPDB2  JOB 'DROP CARDDEMO TABLES',CLASS=A,MSGCLASS=H
//DROP     EXEC PGM=IKJEFT01
//SYSTSPRT DD SYSOUT=*
//SYSPRINT DD SYSOUT=*
//SYSTSIN  DD *
  DSN SYSTEM(DAZ1)
  RUN PROGRAM(DSNTIAD) PLAN(DSNTIAD) PARMS('RC0')
  END
/*
//SYSIN    DD *
  DROP TABLE CARDDEMO.TRANSACTION_TYPE_CATEGORY;
  DROP TABLE CARDDEMO.TRANSACTION_TYPE;
/*
EOF
    $zjob submit "$home/DROPDB2.jcl"
}

setup() {
    cargo build --release -q -p zeus-cobol -p zeus-bms -p zeus-jcl -p zeus-cics
    # the Db2 and Language Environment libraries named in the JCL
    for lib in OEM.DB2.DAZ1.SDSNEXIT OEMA.DB2.VERSIONA.SDSNLOAD OEM.DB2.DAZ1.RUNLIB.LOAD \
        OEM.DB2.DAZ1.SDSNLOAD CEE.SCEERUN; do
        $zjob catalog define-pds $lib --recfm U --lrecl 0
    done
    $zjob catalog define-pds "$hlq.DBRMLIB" --recfm FB --lrecl 80
    # input of the batch maintenance job MNTTRDB2: add a type, update one,
    # delete the added one later
    tmp=$(mktemp)
    printf '%s\n' "* sample maintenance input" "A08INTEREST" "U06REVERSAL" "A09FEE" "D09" >"$tmp"
    $zjob catalog import INPFILE "$tmp" --recfm FB --lrecl 53 --text
    rm -f "$tmp"
    for f in "$mod"/ctl/*.ctl; do
        $zjob catalog put "$hlq.CNTL($(basename "$f" .ctl))" "$f"
    done
    tmp=$(mktemp -d)
    $bin/zcobc compile -I "$mod/cpy" -I "$mod/dcl" -I "$app/cpy" -o "$tmp/COBTUPDT.zo" \
        "$mod/cbl/COBTUPDT.cbl"
    $zjob catalog put "$hlq.LOADLIB(COBTUPDT)" "$tmp/COBTUPDT.zo"
    rm -rf "$tmp"
    # online programs and maps (with the base CardDemo ones)
    [ -d "$out/loadlib" ] || tools/carddemo-online.sh build
    for p in COTRTLIC COTRTUPC; do
        $bin/zcobc compile -I "$mod/cpy" -I "$mod/dcl" -I "$mod/cpy-bms" -I "$app/cpy" \
            -I "$app/cpy-bms" -o "$out/loadlib/$p.zo" "$mod/cbl/$p.cbl"
    done
    for f in "$mod"/bms/*.bms; do
        $bin/zbms "$f" --map "$out/maps/$(basename "$f" .bms).json"
    done
    reset
    submit "$mod/jcl/CREADB21.jcl"
}

case ${1:-} in
    setup) setup ;;
    batch) submit "$mod/jcl/MNTTRDB2.jcl" ;;
    *) echo "usage: $0 setup | batch" >&2; exit 2 ;;
esac

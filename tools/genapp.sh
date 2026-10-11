#!/bin/sh
# Sets up the IBM CICS general insurance application (GenApp) on Zeus and
# Db2 Community Edition, and runs its 3270 transactions.
#
# usage: tools/genapp.sh setup          # VSAM files, Db2 tables, programs, maps, CSD
#        tools/genapp.sh run SCRIPT...  # zcics scripts
#        tools/genapp.sh serve [PORT]   # TN3270 server (default 3270)
#        tools/genapp.sh test           # setup, then tools/genapp/*.txt
#
# ZEUS_DB2 names the Db2 server (tools/db2ce.sh start prints it); the data
# sets are in ZEUS_HOME (default work/genapp).
set -eu

home=${ZEUS_HOME:-work/genapp}
g=.cache/genapp/base
bin=target/release
zjob="$bin/zjob --home $home"
hlq=ZEUS.GENAPP
db=GENASA1
: "${ZEUS_DB2:?set ZEUS_DB2 (tools/db2ce.sh start)}"
export ZEUS_DB2
# the programs' unqualified tables, as BIND QUALIFIER(<DB2DBID>) gives them
export ZEUS_DB2_SCHEMA=$db

# The customization the installation's CUST1 exec applies to the CNTL
# members: <name> placeholders replaced by site values.
customize() { # member
    sed -e "s/<USRHLQ>/ZEUS/g; s/<KSDSCUS>/$hlq.KSDSCUST.TXT/g" \
        -e "s/<KSDSPOL>/$hlq.KSDSPOLY.TXT/g; s/<DB2HLQ>/DSN.V12/g" \
        -e "s/<DB2RUN>/DSNV12/g; s/<DB2SSID>/DKM1/g; s/<DB2PLAN>/DSNTIA12/g" \
        -e "s/<DB2DBID>/$db/g; s/<SQLID>/$db/g; s/<CICSHLQ>/CICS.V54/g" \
        -e "s/<CSDNAME>/$hlq.DFHCSD/g; s/<DBRMLIX>/$hlq.DBRMLIB/g" \
        "$g/cntl/$1.jcl"
}

submit() { # member
    customize "$1" >"$home/cntl/$1.jcl"
    $zjob submit "$home/cntl/$1.jcl"
}

setup() {
    tools/fetch-genapp.sh >/dev/null
    cargo build --release -q -p zeus-cobol -p zeus-bms -p zeus-jcl -p zeus-cics
    rm -rf "$home"
    mkdir -p "$home/cntl" "$home/loadlib" "$home/maps" "$home/copy"
    for lib in DSN.V12.SDSNLOAD DSNV12.RUNLIB.LOAD; do
        $zjob catalog define-pds $lib --recfm U --lrecl 0
    done
    # sample data, then the VSAM files loaded from it (ADEF121)
    $zjob catalog import $hlq.KSDSCUST.TXT $g/data/ksdscust.txt --recfm FB --lrecl 225 --text
    $zjob catalog import $hlq.KSDSPOLY.TXT $g/data/ksdspoly.txt --recfm FB --lrecl 64 --text
    submit adef121
    # the Db2 database, its tables and sample rows (DB2DEL first, so that
    # setup can run again)
    submit db2del >/dev/null || true
    submit db2cre
    # mapset and programs (the COBOL job and BIND have nothing to do here)
    $bin/zbms $g/src/ssmap.bms --copy "$home/copy/SSMAP.cpy" --map "$home/maps/SSMAP.json"
    for f in $g/src/lg*.cbl; do
        n=$(basename "$f" .cbl | tr a-z A-Z)
        $bin/zcobc compile -I $g/src -I "$home/copy" -o "$home/loadlib/$n.zo" "$f"
    done
    # resource definitions: the DFHCSDUP input of CDEF121
    customize cdef121 | sed -n '/^\/\/SYSIN/,/^\/\*/p' | sed '1d;$d' >"$home/GENAPP.CSD"
}

zcics() {
    $bin/zcics --home "$home" --csd "$home/GENAPP.CSD" --loadlib "$home/loadlib" \
        --mapsets "$home/maps" "$@"
}

case ${1:-} in
    setup) setup ;;
    run)
        shift
        for s in "$@"; do zcics "$s"; done
        ;;
    serve) zcics --serve "${2:-3270}" ;;
    test)
        setup >"$home.setup.log" 2>&1 || { cat "$home.setup.log"; exit 1; }
        for s in tools/genapp/*.txt; do
            echo "=== $s"
            zcics "$s" >"$home/$(basename "$s" .txt).log" ||
                { cat "$home/$(basename "$s" .txt).log"; exit 1; }
        done
        echo "all GenApp scripts passed"
        ;;
    *) echo "usage: $0 setup | run SCRIPT... | serve [PORT] | test" >&2; exit 2 ;;
esac

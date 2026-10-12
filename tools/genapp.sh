#!/bin/sh
# Sets up the IBM CICS general insurance application (GenApp) on Zeus and
# Db2 Community Edition, and runs its 3270 transactions.
#
# usage: tools/genapp.sh setup          # VSAM files, Db2 tables, programs, maps, CSD
#        tools/genapp.sh run SCRIPT...  # zcics scripts
#        tools/genapp.sh serve [PORT]   # TN3270 server (default 3270)
#        tools/genapp.sh test           # setup, then tools/genapp/*.txt
#        tools/genapp.sh tn3270         # s3270 through the TN3270 server (after test)
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
: "${ZEUS_DB2:?set ZEUS_DB2 (tools/db2ce.sh start, or tools/pg.sh start for PostgreSQL)}"
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
    tn3270)
        # a 3270 emulator (s3270) works the conversational screens on Db2:
        # TN3270E with an LU name, then plain TN3270, then two terminals
        port=13271
        (exec $bin/zcics --home "$home" --csd "$home/GENAPP.CSD" --loadlib "$home/loadlib" \
            --mapsets "$home/maps" --serve $port >"$home/tn3270-server.log" 2>&1) &
        server=$!
        sleep 1
        sed "s/localhost:3270/localhost:$port/" tools/genapp/tn3270.s3270 |
            timeout 120 s3270 >"$home/tn3270.log" 2>&1 || true
        # two terminals: A waits in a conversational update while B's
        # inquiry runs; B ends first, then A's update goes through
        a='Connect(localhost:'$port')\nWait(10,InputField)\nClear()\nString("SSC1")\nEnter()\nWait(10,Output)\nString("0000000005")\nMoveCursor(21,24)\nString("4")\nEnter()\nWait(10,Output)\n'
        a2='MoveCursor(12,50)\nEraseEOF()\nString("graham@example.com")\nEnter()\nWait(10,Output)\nAscii(23,0,1,80)\nQuit()\n'
        b='Connect(localhost:'$port')\nWait(10,InputField)\nClear()\nString("SSC1")\nEnter()\nWait(10,Output)\nString("0000000006")\nMoveCursor(21,24)\nString("1")\nEnter()\nWait(10,Output)\nAscii(4,0,1,80)\nQuit()\n'
        ( (printf "$a"; sleep 4; printf "$a2") | timeout 60 s3270 >"$home/tn3270-a.log" 2>&1
            date +%s%N >"$home/tn3270-a.end" ) &
        session_a=$!
        sleep 2
        printf "$b" | timeout 30 s3270 >"$home/tn3270-b.log" 2>&1
        date +%s%N >"$home/tn3270-b.end"
        wait $session_a
        kill $server
        grep -q "Brian" "$home/tn3270-b.log" && grep -q "Customer details updated" "$home/tn3270-a.log" &&
            [ "$(cat "$home/tn3270-b.end")" -lt "$(cat "$home/tn3270-a.end")" ] || {
            cat "$home/tn3270-a.log" "$home/tn3270-b.log" "$home/tn3270-server.log"
            echo "TN3270 test failed: terminal B did not run while A waited" >&2
            exit 1
        }
        for want in "HIGH CUSTOMER=0000000011" "Andrew" "Customer details updated" \
            "07700 900004" "Transaction ended" "Louis"; do
            if ! grep -q "$want" "$home/tn3270.log"; then
                cat "$home/tn3270.log" "$home/tn3270-server.log"
                echo "TN3270 test failed: '$want' not shown" >&2
                exit 1
            fi
        done
        for want in "TN3270E, LU ZEUSLU01" "T002 connected (IBM-327[89]-[2-5]-E, LU ZEUS0002)" \
            "T001 query reply"; do
            if ! grep -q -- "$want" "$home/tn3270-server.log"; then
                cat "$home/tn3270-server.log"
                echo "TN3270 test failed: '$want' not logged" >&2
                exit 1
            fi
        done
        echo "GenApp through s3270 (TN3270E, TN3270, two terminals) passed"
        ;;
    *) echo "usage: $0 setup | run SCRIPT... | serve [PORT] | test | tn3270" >&2; exit 2 ;;
esac

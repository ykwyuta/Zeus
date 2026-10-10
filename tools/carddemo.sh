#!/bin/sh
# Sets up AWS CardDemo on Zeus and runs its batch jobs.
#
# usage: tools/carddemo.sh setup    # catalog the sample data, libraries,
#                                   # compile the batch programs
#        tools/carddemo.sh run JOB... # submit jobs from app/jcl
#        tools/carddemo.sh all      # setup, initialization and batch jobs
#
# The Zeus system (catalog and job output) is in $ZEUS_HOME
# (default work/carddemo); CardDemo itself is fetched into .cache/carddemo.
set -eu

home=${ZEUS_HOME:-work/carddemo}
src=.cache/carddemo/app
bin=target/release
hlq=AWS.M2.CARDDEMO
zjob="$bin/zjob --home $home"

setup() {
    tools/fetch-carddemo.sh
    cargo build --release -q -p zeus-cobol -p zeus-jcl
    rm -rf "$home"
    # sample data: name and record length
    for spec in ACCTDATA:300 CARDDATA:150 CARDXREF:50 CUSTDATA:500 DALYTRAN:350 \
        DISCGRP:50 TCATBALF:50 TRANCATG:60 TRANTYPE:60 USRSEC:80 EXPORT.DATA:500; do
        name=${spec%%:*}
        lrecl=${spec##*:}
        $zjob catalog import "$hlq.$name.PS" "$src/data/EBCDIC/$hlq.$name.PS" \
            --recfm FB --lrecl "$lrecl"
    done
    $zjob catalog import "$hlq.DALYTRAN.PS.INIT" "$src/data/EBCDIC/$hlq.DALYTRAN.PS.INIT" \
        --recfm FB --lrecl 350
    # libraries
    $zjob catalog define-pds "$hlq.PROC" --recfm FB --lrecl 80
    for f in "$src"/proc/*; do
        m=$(basename "$f" | sed 's/\..*//')
        $zjob catalog put "$hlq.PROC($m)" "$f"
    done
    $zjob catalog define-pds "$hlq.CNTL" --recfm FB --lrecl 80
    for f in "$src"/ctl/*; do
        m=$(basename "$f" | sed 's/\..*//')
        $zjob catalog put "$hlq.CNTL($m)" "$f"
    done
    $zjob catalog define-pds "$hlq.LOADLIB" --recfm U --lrecl 0
    tmp=$(mktemp -d)
    for f in "$src"/cbl/CB*.cbl "$src"/cbl/CB*.CBL "$src"/cbl/CSUTLDTC.cbl "$src"/cbl/COBSWAIT.cbl; do
        m=$(basename "$f" | sed 's/\..*//')
        $bin/zcobc compile -I "$src/cpy" -o "$tmp/$m.zo" "$f"
        $zjob catalog put "$hlq.LOADLIB($m)" "$tmp/$m.zo"
    done
    rm -rf "$tmp"
    echo "CardDemo set up in $home"
}

run() {
    for job in "$@"; do
        f=$(ls "$src/jcl/$job".* 2>/dev/null | head -1)
        if [ -z "$f" ]; then
            echo "no JCL for $job" >&2
            exit 2
        fi
        echo "=== $job"
        $zjob submit "$f" || echo "=== $job ended with exit status $?"
    done
}

# Environment initialization and the batch process, in the order of the
# CardDemo README (without the optional Db2 / IMS / MQ modules); DALYREJS
# defines the GDG base POSTTRAN writes its rejects to.
init_jobs="DUSRSECJ CLOSEFIL ACCTFILE CARDFILE CUSTFILE XREFFILE TRANFILE DISCGRP
    TCATBALF TRANCATG TRANTYPE OPENFIL DEFGDGB DALYREJS"
batch_jobs="CLOSEFIL TRANBKP POSTTRAN INTCALC TRANBKP COMBTRAN CREASTMT TRANIDX OPENFIL"

case ${1:-} in
    setup) setup ;;
    run) shift; run "$@" ;;
    all)
        setup
        # shellcheck disable=SC2086
        run $init_jobs $batch_jobs
        ;;
    *) echo "usage: $0 setup | run JOB... | all" >&2; exit 2 ;;
esac

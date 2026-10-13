#!/bin/sh
# Sets up AWS CardDemo on Zeus and runs its batch jobs.
#
# usage: tools/carddemo.sh setup    # catalog the sample data, libraries,
#                                   # compile the batch programs
#        tools/carddemo.sh init     # setup and the initialization jobs
#                                   # (VSAM files such as USRSEC)
#        tools/carddemo.sh run JOB... # submit jobs from app/jcl
#        tools/carddemo.sh all      # setup, initialization, batch and
#                                   # report jobs; fails unless all end RC 0
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
    # report date range read by CBTRN03C (TRANREPT), created by the
    # installation; the range of the SORT step in TRANREPT
    tmp=$(mktemp)
    echo "2022-01-01 2022-07-06" >"$tmp"
    $zjob catalog import "$hlq.DATEPARM" "$tmp" --recfm FB --lrecl 80 --text
    rm -f "$tmp"
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
        rc=0
        $zjob submit "$f" || rc=$?
        # POSTTRAN ends with RC 4 when it rejects transactions (CBTRN02C
        # sets RETURN-CODE); the sample data has rejects
        expected=0
        [ "$job" = POSTTRAN ] && expected=4
        if [ "$rc" -ne "$expected" ]; then
            echo "=== $job ended with exit status $rc"
            failed="$failed $job"
        fi
    done
}

# Environment initialization and the batch process, in the order of the
# CardDemo README (without the optional Db2 / IMS / MQ modules); DALYREJS
# defines the GDG base POSTTRAN writes its rejects to.
init_jobs="DUSRSECJ CLOSEFIL ACCTFILE CARDFILE CUSTFILE XREFFILE TRANFILE DISCGRP
    TCATBALF TRANCATG TRANTYPE OPENFIL DEFGDGB DALYREJS"
batch_jobs="CLOSEFIL TRANBKP POSTTRAN INTCALC TRANBKP COMBTRAN CREASTMT TRANIDX OPENFIL"
# Reports and utilities. Not run: CBADMCDJ (CICS CSD update), TXT2PDF1
# (TXT2PDF product), INTRDRJ1/2 (need FTP test data), FTPJCL, DEFCUST and
# REPTFILE (they end with RC 8 / 12 on a set-up system by design), CBIMPORT
# (its JCL has no CARDOUT DD: the OPEN fails with status 35 as on z/OS).
report_jobs="READACCT READCARD READCUST READXREF PRTCATBL TRANREPT CBEXPORT
    ESDSRRDS WAITSTEP"
failed=

case ${1:-} in
    setup) setup ;;
    run) shift; run "$@" ;;
    init)
        setup
        # shellcheck disable=SC2086
        run $init_jobs
        if [ -n "$failed" ]; then
            echo "failed jobs:$failed" >&2
            exit 1
        fi
        ;;
    all)
        setup
        # shellcheck disable=SC2086
        run $init_jobs $batch_jobs $report_jobs
        if [ -n "$failed" ]; then
            echo "failed jobs:$failed" >&2
            exit 1
        fi
        echo "all CardDemo batch jobs ended with their expected return codes"
        ;;
    *) echo "usage: $0 setup | init | run JOB... | all" >&2; exit 2 ;;
esac

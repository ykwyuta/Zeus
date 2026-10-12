#!/bin/sh
# The MQ extension of CardDemo (app-vsam-mq): CODATE01 (CDRD) and COACCT01
# (CDRA) answer request messages; the queue manager's trigger messages on
# CARD.DEMO.INITQ start them through zcics's trigger monitor (CKTI).
# The authorization module's COPAUA0C (CP00) answers authorization
# requests the same way, with the pending authorizations in IMS DB.
#
# usage: tools/carddemo-mq.sh setup   # programs; MQ objects (runmqsc in the
#                                      # container of tools/mq.sh)
#        tools/carddemo-mq.sh test    # setup, requests, the region, replies
#
# Needs the CardDemo data sets of tools/carddemo.sh and the IMS databases
# of tools/carddemo-ims.sh (ZEUS_HOME, default work/carddemo), and the
# queue manager of tools/mq.sh (ZEUS_MQ, ZEUS_MQ_LIB).
set -eu

home=${ZEUS_HOME:-work/carddemo}
app=.cache/carddemo/app
mod=$app/app-vsam-mq
auth=$app/app-authorization-ims-db2-mq
out=$home/mq
bin=target/release
mq=${ZEUS_MQ_CONTAINER:-zeus-mq}
: "${ZEUS_MQ:?set ZEUS_MQ (tools/mq.sh start)}"

mqsc() {
    docker exec -i "$mq" runmqsc QM1 >"$out/mqsc.log"
}

setup() {
    cargo build --release -q -p zeus-cobol -p zeus-cics
    rm -rf "$out"
    mkdir -p "$out/loadlib"
    for f in "$mod"/cbl/*.cbl; do
        $bin/zcobc compile --cics -I "$app/cpy" -o "$out/loadlib/$(basename "$f" .cbl).zo" "$f"
    done
    $bin/zcobc compile --cics -I "$auth/cpy" -I "$app/cpy" -o "$out/loadlib/COPAUA0C.zo" \
        "$auth/cbl/COPAUA0C.cbl"
    # the request queues trigger CDRD and CDRA through the initiation queue
    mqsc <<'MQSC'
DEFINE QLOCAL(CARD.DEMO.INITQ) REPLACE
DEFINE PROCESS(CARD.DEMO.DATE.PROC) APPLICID('CDRD') REPLACE
DEFINE PROCESS(CARD.DEMO.ACCT.PROC) APPLICID('CDRA') REPLACE
DEFINE QLOCAL(CARD.DEMO.REQUEST.DATE) TRIGGER TRIGTYPE(FIRST) INITQ(CARD.DEMO.INITQ) PROCESS(CARD.DEMO.DATE.PROC) REPLACE
DEFINE QLOCAL(CARD.DEMO.REQUEST.ACCT) TRIGGER TRIGTYPE(FIRST) INITQ(CARD.DEMO.INITQ) PROCESS(CARD.DEMO.ACCT.PROC) REPLACE
DEFINE QLOCAL(CARD.DEMO.REPLY.DATE) REPLACE
DEFINE QLOCAL(CARD.DEMO.REPLY.ACCT) REPLACE
DEFINE QLOCAL(CARD.DEMO.ERROR) REPLACE
DEFINE PROCESS(AWS.M2.CARDDEMO.PAUTH.PROC) APPLICID('CP00') REPLACE
DEFINE QLOCAL(AWS.M2.CARDDEMO.PAUTH.REQUEST) TRIGGER TRIGTYPE(FIRST) INITQ(CARD.DEMO.INITQ) PROCESS(AWS.M2.CARDDEMO.PAUTH.PROC) REPLACE
DEFINE QLOCAL(AWS.M2.CARDDEMO.PAUTH.REPLY) REPLACE
SET AUTHREC PROFILE('CARD.**') OBJTYPE(QUEUE) PRINCIPAL('app') AUTHADD(ALLMQI)
SET AUTHREC PROFILE('CARD.**') OBJTYPE(PROCESS) PRINCIPAL('app') AUTHADD(INQ,DSP)
SET AUTHREC PROFILE('AWS.M2.CARDDEMO.**') OBJTYPE(QUEUE) PRINCIPAL('app') AUTHADD(ALLMQI)
SET AUTHREC PROFILE('AWS.M2.CARDDEMO.**') OBJTYPE(PROCESS) PRINCIPAL('app') AUTHADD(INQ,DSP)
MQSC
}

case ${1:-} in
    setup) setup ;;
    test)
        setup
        cargo build --release -q -p zeus-mq
        # left over replies of an earlier run
        [ -f "$home/catalog/ims/ims.wal" ] || tools/carddemo-ims.sh setup >/dev/null
        for q in CARD.DEMO.REPLY.DATE CARD.DEMO.REPLY.ACCT CARD.DEMO.ERROR AWS.M2.CARDDEMO.PAUTH.REPLY; do
            $bin/zmq get $q >/dev/null
        done
        $bin/zmq put CARD.DEMO.REQUEST.DATE DATE00000001
        $bin/zmq put CARD.DEMO.REQUEST.ACCT INQA00000000001
        # an authorization of card 0500024453765740 (account 50)
        $bin/zmq put --reply-to AWS.M2.CARDDEMO.PAUTH.REPLY AWS.M2.CARDDEMO.PAUTH.REQUEST \
            "261012,124500,0500024453765740,0100,1228,0100,POS,000000,100.00,5411,840,01,MERCH0000000001,ZEUS GROCERY,SAN JOSE,CA,95101,TXN000000000001"
        # COPAUA0C's replies expire after 5 seconds and it waits 5 seconds for
        # more requests: the reply is read while the region runs
        $bin/zmq get --wait 15000 AWS.M2.CARDDEMO.PAUTH.REPLY >"$out/reply-auth.txt" &
        reader=$!
        printf 'wait 2\n' >"$out/region.txt"
        $bin/zcics --home "$home" --csd "$app/csd/CARDDEMO.CSD" --csd "$mod/csd/CRDDEMOM.csd" \
            --csd "$auth/csd/CRDDEMO2.csd" --loadlib "$out/loadlib" \
            --ims-lib OEM.IMS.IMSP.PSBLIB --ims-lib OEM.IMS.IMSP.DBDLIB \
            --mq-initq CARD.DEMO.INITQ "$out/region.txt" >"$out/region.log" 2>&1 ||
            { cat "$out/region.log"; exit 1; }
        wait $reader
        $bin/zmq get CARD.DEMO.REPLY.DATE >"$out/reply-date.txt"
        $bin/zmq get CARD.DEMO.REPLY.ACCT >"$out/reply-acct.txt"
        $bin/zmq get CARD.DEMO.ERROR >"$out/error.txt"
        cat "$out/region.log" "$out/reply-date.txt" "$out/reply-acct.txt" "$out/reply-auth.txt" "$out/error.txt"
        grep -q 'SYSTEM DATE' "$out/reply-date.txt" || { echo "no date reply" >&2; exit 1; }
        grep -q 'ACCOUNT ID' "$out/reply-acct.txt" || { echo "no account reply" >&2; exit 1; }
        grep -q '^0500024453765740,TXN000000000001,' "$out/reply-auth.txt" ||
            { echo "no authorization reply" >&2; exit 1; }
        [ ! -s "$out/error.txt" ] || { echo "error messages" >&2; exit 1; }
        echo "carddemo-mq: ok"
        ;;
    *) echo "usage: $0 setup | test" >&2; exit 2 ;;
esac

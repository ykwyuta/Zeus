#!/bin/sh
# Sets up the IMS authorization module of CardDemo on Zeus (IMS DB) and
# runs its batch jobs and online transactions.
#
# usage: tools/carddemo-ims.sh setup          # after tools/carddemo.sh setup:
#                                             # IMS libraries, programs, data
#        tools/carddemo-ims.sh batch          # the IMS batch jobs
#        tools/carddemo-ims.sh run SCRIPT...  # zcics with the IMS transactions
#        tools/carddemo-ims.sh test           # setup, batch (checked), online scripts
#
# The DBDs and PSBs are members of OEM.IMS.IMSP.DBDLIB / PSBLIB (and
# IMS.DBDLIB / PSBLIB, as CBPAUP0J names them); the databases are in
# $ZEUS_HOME/catalog/ims/ims.wal.
set -eu

home=${ZEUS_HOME:-work/carddemo}
app=.cache/carddemo/app
mod=$app/app-authorization-ims-db2-mq
hlq=AWS.M2.CARDDEMO
bin=target/release
zjob="$bin/zjob --home $home"
out=$home/online

setup() {
    cargo build --release -q -p zeus-cobol -p zeus-bms -p zeus-jcl -p zeus-cics -p zeus-ims
    tmp=$(mktemp -d)
    : >"$tmp/empty"
    # the IMS libraries: DBD and PSB sources as members
    for lib in OEM.IMS.IMSP.DBDLIB OEM.IMS.IMSP.PSBLIB IMS.DBDLIB IMS.PSBLIB OEMPP.IMS.V15R01MB.PROCLIB; do
        $zjob catalog define-pds $lib --recfm FB --lrecl 80 >/dev/null
    done
    for f in "$mod"/ims/*; do
        [ -f "$f" ] || continue
        m=$(basename "$f" | sed 's/\..*//' | tr a-z A-Z)
        case $f in
            *.psb | *.PSB) lib=PSBLIB ;;
            *) lib=DBDLIB ;;
        esac
        $zjob catalog put "OEM.IMS.IMSP.$lib($m)" "$f"
        $zjob catalog put "IMS.$lib($m)" "$f"
    done
    echo "* VSAM buffer pools" >"$tmp/vsmdb"
    $zjob catalog put "OEMPP.IMS.V15R01MB.PROCLIB(DFSVSMDB)" "$tmp/vsmdb"
    # the IMS and site libraries the JCL names
    for lib in IMS.SDFSRESL IMS.PROCLIB OEMA.IMS.IMSP.SDFSRESL OEMA.IMS.IMSP.SDFSRESL.V151 \
        XXXXXXXX.PROD.LOADLIB; do
        $zjob catalog define-pds $lib --recfm U --lrecl 0 >/dev/null
    done
    # database data sets (the data is in the Zeus store) and the files the
    # jobs delete first
    for ds in OEM.IMS.IMSP.PAUTHDB OEM.IMS.IMSP.PAUTHDBX; do
        $zjob catalog import $ds "$tmp/empty" --recfm FB --lrecl 80
    done
    $zjob catalog import $hlq.PAUTDB.ROOT.FILEO "$tmp/empty" --recfm FB --lrecl 100
    $zjob catalog import $hlq.PAUTDB.CHILD.FILEO "$tmp/empty" --recfm FB --lrecl 206
    $zjob catalog import $hlq.PAUTDB.ROOT.GSAM "$tmp/empty" --recfm FB --lrecl 100
    $zjob catalog import $hlq.PAUTDB.CHILD.GSAM "$tmp/empty" --recfm FB --lrecl 200
    # the unload of the pending authorization database (DFSURGU0)
    $zjob catalog import $hlq.IMSDATA.DBPAUTP0 "$mod/data/EBCDIC/$hlq.IMSDATA.DBPAUTP0.dat" \
        --recfm VB --lrecl 27994
    # batch programs
    for p in CBPAUP0C PAUDBUNL PAUDBLOD DBUNLDGS; do
        f=$(ls "$mod"/cbl/$p.* | head -1)
        $bin/zcobc compile -I "$mod/cpy" -I "$app/cpy" -o "$tmp/$p.zo" "$f"
        $zjob catalog put "$hlq.LOADLIB($p)" "$tmp/$p.zo"
        $zjob catalog put "XXXXXXXX.PROD.LOADLIB($p)" "$tmp/$p.zo"
    done
    # online programs and maps (with the base CardDemo ones)
    [ -d "$out/loadlib" ] || tools/carddemo-online.sh build
    for p in COPAUS0C COPAUS1C COPAUS2C; do
        $bin/zcobc compile -I "$mod/cpy" -I "$mod/cpy-bms" -I "$mod/dcl" -I "$app/cpy" \
            -I "$app/cpy-bms" -o "$out/loadlib/$p.zo" "$mod/cbl/$p.cbl"
    done
    for f in "$mod"/bms/*.bms; do
        $bin/zbms "$f" --map "$out/maps/$(basename "$f" .bms).json"
    done
    rm -rf "$tmp"
    # a new store, reloaded from the unload (DFSURGL0)
    rm -f "$home/catalog/ims/ims.wal"
    cat >"$home/RELOADPA.jcl" <<'JCL'
//RELOADPA JOB 'RELOAD DBPAUTP0'
//RELOAD   EXEC PGM=DFSRRC00,PARM=(ULR,DFSURGL0,DBPAUTP0)
//IMS      DD DSN=OEM.IMS.IMSP.PSBLIB,DISP=SHR
//         DD DSN=OEM.IMS.IMSP.DBDLIB,DISP=SHR
//SYSPRINT DD SYSOUT=*
//DFSUINPT DD DSN=AWS.M2.CARDDEMO.IMSDATA.DBPAUTP0,DISP=SHR
JCL
    $zjob submit "$home/RELOADPA.jcl"
}

count() { # segments in the database
    $bin/zims count "$home/catalog/ims/ims.wal" "$mod/ims/DBPAUTP0.dbd"
}

batch() {
    # unload to sequential files (CBLTDLI) and to GSAM, delete the expired
    # authorizations (EXEC DLI, BMP) and load the unloaded ones again
    for job in UNLDPADB UNLDGSAM CBPAUP0J LOADPADB; do
        echo "=== $job"
        $zjob submit "$(ls "$mod"/jcl/$job.* | head -1)"
        echo "segments: $(count)"
    done
}

zcics() {
    $bin/zcics --home "$home" --csd "$app/csd/CARDDEMO.CSD" --csd "$mod/csd/CRDDEMO2.csd" \
        --loadlib "$out/loadlib" --mapsets "$out/maps" \
        --ims-lib OEM.IMS.IMSP.PSBLIB --ims-lib OEM.IMS.IMSP.DBDLIB "$@"
}

case ${1:-} in
    setup) setup ;;
    batch) batch ;;
    run)
        shift
        for s in "$@"; do zcics "$s"; done
        ;;
    test)
        setup >"$home/carddemo-ims-setup.log"
        batch >"$home/carddemo-ims-batch.log"
        if grep -hE "RC=[0-9]{4}|ABEND|JCL ERROR" "$home"/carddemo-ims-*.log | grep -vq "RC=0000"; then
            cat "$home"/carddemo-ims-*.log
            echo "an IMS job step did not end with RC 0" >&2
            exit 1
        fi
        # 22 summaries and 202 details; CBPAUP0J deletes the expired ones
        # (all but one summary), LOADPADB inserts the 21 summaries (the one
        # with a blank key is not unloaded) and 202 details again
        expected="segments: 224
segments: 224
segments: 1
segments: 224"
        got=$(grep '^segments:' "$home/carddemo-ims-batch.log")
        if [ "$got" != "$expected" ]; then
            cat "$home/carddemo-ims-batch.log"
            echo "unexpected segment counts" >&2
            exit 1
        fi
        for s in tools/carddemo-ims/*.txt; do
            echo "=== $s"
            zcics "$s" >"$out/ims-$(basename "$s" .txt).log" ||
                { cat "$out/ims-$(basename "$s" .txt).log"; exit 1; }
        done
        echo "all CardDemo IMS tests passed"
        ;;
    *) echo "usage: $0 setup | batch | run SCRIPT... | test" >&2; exit 2 ;;
esac

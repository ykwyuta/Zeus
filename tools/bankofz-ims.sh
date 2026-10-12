#!/bin/sh
# Sets up the IMS path of Bank of Z (IBM/Bank-of-Z) on Zeus: IMS DB with
# its DBDs, PSBs and sample data, the message processing programs (COBOL
# and the PL/I IBLOGIN) and their transactions, and runs input messages
# through a message processing region.
#
# usage: tools/bankofz-ims.sh setup              # libraries, programs, databases
#        tools/bankofz-ims.sh tran TRAN TEXT     # one message: send, run, receive
#        tools/bankofz-ims.sh test               # setup, then tools/bankofz-ims/*.txt
#
# A test file has lines `send TRAN TEXT`, `sendhex TRAN HEX` (the message
# text in hexadecimal) and `expect TEXT` (the output of the last message
# contains TEXT); `#` starts a comment. The data sets
# are in ZEUS_HOME (default work/bankofz-ims). IBTRAN (object-oriented
# COBOL calling Java) and the Java programs are not part of it.
set -eu

home=${ZEUS_HOME:-work/bankofz-ims}
b=.cache/bankofz
i=$b/src/base/ims
bin=target/release
zjob="$bin/zjob --home $home"
hlq=BANKZ.IMS

# The values the installation's Ansible templates fill in.
customize() { # template
    sed -e '/{{ *ims\.jobcard/,/{%- *endif *%}/c\
//BANKZIMS JOB (ZEUS),CLASS=A' \
        -e "s/{{ *ims\.ims_hlq *}}/$hlq/g; s/{{ *ims\.dfs_imsplex *}}/PLEX1/g" \
        -e "s/{{ *ims\.datastore *}}/IMS1/g" "$1"
}

submit() { # template
    customize "$b/.setup/jcl/ims/$1" >"$home/cntl/$1.jcl"
    $zjob submit "$home/cntl/$1.jcl"
}

setup() {
    tools/fetch-bankofz.sh >/dev/null
    cargo build --release -q -p zeus-cobol -p zeus-pli -p zeus-jcl -p zeus-ims
    rm -rf "$home"
    mkdir -p "$home/cntl"
    for lib in DBDLIB PSBLIB; do
        $zjob catalog define-pds $hlq.$lib --recfm FB --lrecl 80 >/dev/null
    done
    for lib in PGMLIB SDFSRESL; do
        $zjob catalog define-pds $hlq.$lib --recfm U --lrecl 0 >/dev/null
    done
    for f in $i/DBD/*.asm; do
        $zjob catalog put "$hlq.DBDLIB($(basename "$f" .asm))" "$f"
    done
    for f in $i/PSB/*.asm; do
        $zjob catalog put "$hlq.PSBLIB($(basename "$f" .asm))" "$f"
    done
    # the load programs' input (CSV lines) and the database data sets
    # (the data is in the Zeus store)
    : >"$home/empty"
    for db in CUSTOMER ACCOUNT CUSTACCS HISTORY TSTAT; do
        $zjob catalog import $hlq.$db.INPUT $i/LoadData/$db.data --recfm FB --lrecl 200 --text
        $zjob catalog import $hlq.$db.DB "$home/empty" --recfm FB --lrecl 80
    done
    # the programs, into the program library
    tmp=$(mktemp -d)
    for f in $i/cobol/*.cbl; do
        n=$(basename "$f" .cbl)
        [ "$n" = IBTRAN ] && continue
        $bin/zcobc compile -I $i/copy -o "$tmp/$n.zo" "$f"
        $zjob catalog put "$hlq.PGMLIB($n)" "$tmp/$n.zo"
    done
    $bin/zpli compile -o "$tmp/IBLOGIN.zo" $i/pli/IBLOGIN.pli
    $zjob catalog put "$hlq.PGMLIB(IBLOGIN)" "$tmp/IBLOGIN.zo"
    rm -rf "$tmp"
    for j in Ims-load-cust Ims-load-acct Ims-load-cusa Ims-load-hist Ims-load-tsta; do
        submit $j.j2
    done
    # the transactions (type-2 commands)
    submit Ims-spoc.jcl.j2
}

# A message processing region: runs the queued messages.
region() {
    cat >"$home/cntl/MPP.jcl" <<JCL
//BANKZMPP JOB (ZEUS),CLASS=A
//MPP      EXEC PGM=DFSRRC00,PARM='MSG'
//STEPLIB  DD DSN=$hlq.PGMLIB,DISP=SHR
//IMS      DD DSN=$hlq.PSBLIB,DISP=SHR
//         DD DSN=$hlq.DBDLIB,DISP=SHR
//SYSPRINT DD SYSOUT=*
//SYSOUT   DD SYSOUT=*
JCL
    $zjob submit "$home/cntl/MPP.jcl" >/dev/null
}

tran() { # code text [--hex]
    $zjob ims send "$1" "$2" ${3:-}
    region
    $zjob ims receive ZEUSTERM
}

case ${1:-} in
    setup) setup ;;
    tran) tran "$2" "$3" ;;
    test)
        setup >"$home.setup.log" 2>&1 || { cat "$home.setup.log"; exit 1; }
        for s in tools/bankofz-ims/*.txt; do
            echo "=== $s"
            out=""
            while IFS= read -r line; do
                case $line in
                    "" | "#"*) ;;
                    "send "*)
                        rest=${line#send }
                        out=$(tran "${rest%% *}" "${rest#* }")
                        echo "$out"
                        ;;
                    "sendhex "*)
                        rest=${line#sendhex }
                        out=$(tran "${rest%% *}" "${rest#* }" --hex)
                        echo "$out"
                        ;;
                    "expect "*)
                        want=${line#expect }
                        if ! printf '%s\n' "$out" | grep -qF -- "$want"; then
                            echo "$s: the output does not contain '$want'" >&2
                            exit 1
                        fi
                        ;;
                    *) echo "$s: bad line: $line" >&2; exit 1 ;;
                esac
            done <"$s"
        done
        echo "all Bank of Z IMS tests passed"
        ;;
    *) echo "usage: $0 setup | tran TRAN TEXT | test" >&2; exit 2 ;;
esac

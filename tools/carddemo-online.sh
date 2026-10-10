#!/bin/sh
# Builds the CardDemo online programs and mapsets and runs zcics scripts.
#
# usage: tools/carddemo-online.sh build
#        tools/carddemo-online.sh run SCRIPT...
#        tools/carddemo-online.sh test   # build, run tools/carddemo-online/*.txt
#                                        # (on freshly set up data)
#        tools/carddemo-online.sh serve [PORT]  # TN3270 server (default 3270)
#        tools/carddemo-online.sh tn3270 # sign on through s3270 (TN3270)
#
# Needs the CardDemo data sets set up by tools/carddemo.sh (ZEUS_HOME,
# default work/carddemo).
set -eu

home=${ZEUS_HOME:-work/carddemo}
app=.cache/carddemo/app
out=$home/online
bin=target/release

build() {
    cargo build --release -q -p zeus-cobol -p zeus-bms -p zeus-cics
    rm -rf "$out"
    mkdir -p "$out/loadlib" "$out/maps"
    for f in "$app"/cbl/CO*.cbl "$app"/cbl/CS*.cbl; do
        n=$(basename "$f" .cbl)
        $bin/zcobc compile -I "$app/cpy" -I "$app/cpy-bms" -o "$out/loadlib/$n.zo" "$f"
    done
    for f in "$app"/bms/*.bms; do
        $bin/zbms "$f" --map "$out/maps/$(basename "$f" .bms).json"
    done
    echo "CardDemo online programs built in $out"
}

case ${1:-} in
    build) build ;;
    run)
        shift
        [ -d "$out/loadlib" ] || build
        for s in "$@"; do
            $bin/zcics --home "$home" --csd "$app/csd/CARDDEMO.CSD" \
                --loadlib "$out/loadlib" --mapsets "$out/maps" "$s"
        done
        ;;
    test)
        build
        for s in tools/carddemo-online/*.txt; do
            echo "=== $s"
            $bin/zcics --home "$home" --csd "$app/csd/CARDDEMO.CSD" \
                --loadlib "$out/loadlib" --mapsets "$out/maps" "$s" >"$out/$(basename "$s" .txt).log" ||
                { cat "$out/$(basename "$s" .txt).log"; exit 1; }
        done
        echo "all CardDemo online scripts passed"
        ;;
    serve)
        [ -d "$out/loadlib" ] || build
        exec $bin/zcics --home "$home" --csd "$app/csd/CARDDEMO.CSD" \
            --loadlib "$out/loadlib" --mapsets "$out/maps" --serve "${2:-3270}"
        ;;
    tn3270)
        # a 3270 emulator (s3270) signs on through the TN3270 server
        [ -d "$out/loadlib" ] || build
        port=13270
        $bin/zcics --home "$home" --csd "$app/csd/CARDDEMO.CSD" \
            --loadlib "$out/loadlib" --mapsets "$out/maps" --serve $port \
            >"$out/tn3270-server.log" 2>&1 &
        server=$!
        sleep 1
        sed "s/localhost:3270/localhost:$port/" tools/carddemo-online/tn3270.s3270 |
            timeout 60 s3270 >"$out/tn3270.log" 2>&1 || true
        kill $server
        for want in "Admin Menu" "List Users" "ADMIN005    GRANVILLE"; do
            if ! grep -q "$want" "$out/tn3270.log"; then
                cat "$out/tn3270.log" "$out/tn3270-server.log"
                echo "TN3270 test failed: '$want' not shown" >&2
                exit 1
            fi
        done
        echo "TN3270 sign-on through s3270 passed"
        ;;
    *) echo "usage: $0 build | run SCRIPT... | test | serve [PORT] | tn3270" >&2; exit 2 ;;
esac

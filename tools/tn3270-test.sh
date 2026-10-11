#!/bin/sh
# Terminal facilities through real emulators: s3270 as a 3279 model 5
# display (TN3270E, LU ZEUSLU01) and pr3287 as its associated printer
# (TN3270E ASSOCIATE). Covers the alternate screen size (Erase/Write
# Alternate), START to a printer with RETRIEVE (SCS print data), structured
# fields (Read Partition Query from a program, the reply with AID X'88')
# and SYSREQ / LOGOFF (SSCP-LU data, UNBIND).
#
# usage: tools/tn3270-test.sh      (needs s3270 and pr3287)
set -eu

work=${ZEUS_HOME:-work/tn3270}
src=crates/zeus-cics/tests/terminal
bin=target/release
port=${ZEUS_TN3270_PORT:-13273}

cargo build --release -q -p zeus-cobol -p zeus-cics
rm -rf "$work"
mkdir -p "$work/loadlib"
for p in ZPRT ZPR2P ZALT ZSF ZBUF; do
    $bin/zcobc compile -o "$work/loadlib/$p.zo" "$src/$p.cbl"
done
(exec $bin/zcics --home "$work" --csd "$src/TEST.CSD" --loadlib "$work/loadlib" \
    --serve $port >"$work/server.log" 2>&1) &
server=$!
trap 'kill $server $printer 2>/dev/null || true' EXIT
sleep 1
cat >"$work/display.s3270" <<EOS
Connect(ZEUSLU01@localhost:$port)
Wait(10,InputField)
Clear()
String("ZALT")
Enter()
Wait(10,Output)
Ascii(0,0,1,132)
Query(ScreenCurSize)
Clear()
String("ZSF")
Enter()
Wait(10,Output)
Wait(2,Seconds)
Ascii(0,0,1,80)
Clear()
String("ZPRT")
Enter()
Wait(10,Output)
Ascii(0,0,1,80)
Wait(3,Seconds)
SysReq()
Wait(10,Output)
Wait(1,Seconds)
Ascii()
String("LOGOFF")
Enter()
Wait(10,Disconnect)
Quit()
EOS
# the display connects first; the printer associates with it, then the
# display's script goes on
(cat "$work/display.s3270" | sed -n 1,2p; sleep 4; sed 1,2d "$work/display.s3270"; sleep 2) |
    timeout 60 s3270 -model 3279-5-E >"$work/display.log" 2>&1 &
display=$!
sleep 2
(exec pr3287 -assoc ZEUSLU01 -command "cat >>$work/printed.txt" localhost:$port \
    >"$work/printer.log" 2>&1) &
printer=$!
wait $display || true
sleep 1
fail() {
    cat "$work/display.log" "$work/server.log" "$work/printer.log" 2>/dev/null
    [ -f "$work/printed.txt" ] && cat "$work/printed.txt"
    echo "TN3270 terminal test failed: $1" >&2
    exit 1
}
for want in "SIZE 027X132 ALT 027X132 COLOR YES" "data: 27 132" \
    "SF BYTES:" "AID: SF" "PRINT REQUESTED, TERMIDERR OK" "ZEUS SSCP"; do
    grep -q "$want" "$work/display.log" || fail "display: '$want' not shown"
done
for want in "TN3270E, LU ZEUSLU01" "printer PRT1 connected (IBM-3287-1, TN3270E, LU ZEUSPRT1)" \
    "printer PRT1 printed" "terminal T001 disconnected"; do
    grep -q "$want" "$work/server.log" || fail "server: '$want' not logged"
done
for want in "ZPR2: HELLO PRINTER" "ZPR2: PRINTED LATER"; do
    grep -q "$want" "$work/printed.txt" 2>/dev/null || fail "printer: '$want' not printed"
done
echo "TN3270E display (model 5), printer, structured fields and SYSREQ passed"

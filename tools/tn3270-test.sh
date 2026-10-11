#!/bin/sh
# Terminal facilities through real emulators: s3270 as a 3279 model 5
# display (TN3270E, LU ZEUSLU01) and pr3287 as its associated printer
# (TN3270E ASSOCIATE). Covers the alternate screen size (Erase/Write
# Alternate), START to a printer with RETRIEVE (SCS print data), structured
# fields (Read Partition Query from a program, the reply with AID X'88')
# and SYSREQ / LOGOFF (SSCP-LU data, UNBIND); then an oversize screen
# (160 x 62, 14-bit addresses, a new BIND after the Query Reply) and a
# Japanese screen (CCSID 939 on both sides: double-byte characters with SO /
# SI, a SOSI input field).
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
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true

# an oversize screen
(exec $bin/zcics --home "$work" --csd "$src/TEST.CSD" --loadlib "$work/loadlib" \
    --serve $port >"$work/server2.log" 2>&1) &
server=$!
sleep 1
printf 'Connect(localhost:%s)\nWait(10,InputField)\nClear()\nString("ZALT")\nEnter()\nWait(10,Output)\nWait(1,Seconds)\nQuery(ScreenCurSize)\nAscii(0,0,1,60)\nQuit()\n' $port |
    timeout 30 s3270 -model 3279-4-E -oversize 160x62 >"$work/oversize.log" 2>&1 || true
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true
for want in "data: 62 160" "SIZE 062X160 ALT 062X160"; do
    grep -q "$want" "$work/oversize.log" || { cat "$work/oversize.log" "$work/server2.log"; fail "oversize: '$want' not shown"; }
done

# a Japanese screen (CCSID 939)
jp=crates/zeus-cics/tests/japanese
mkdir -p "$work/jp/maps" "$work/jp/copy" "$work/jp/loadlib"
ZEUS_CCSID=939 $bin/zbms "$jp/JPMAP.bms" --copy "$work/jp/copy/JPMAP.cpy" \
    --map "$work/jp/maps/JPMAP.json"
ZEUS_CCSID=939 $bin/zcobc compile --cics -I "$work/jp/copy" -o "$work/jp/loadlib/JPGM.zo" \
    "$jp/JPGM.cbl"
(ZEUS_CCSID=939 exec $bin/zcics --home "$work/jp" --csd "$jp/TEST.CSD" \
    --loadlib "$work/jp/loadlib" --mapsets "$work/jp/maps" --serve $port \
    >"$work/server3.log" 2>&1) &
server=$!
sleep 1
printf 'Connect(localhost:%s)\nWait(10,InputField)\nClear()\nString("JPGM")\nEnter()\nWait(10,Output)\nString("山田")\nEnter()\nWait(10,Output)\nAscii(0,0,5,80)\nQuit()\n' $port |
    LANG=C.UTF-8 LC_ALL=C.UTF-8 timeout 30 s3270 -codepage cp939 >"$work/japanese.log" 2>&1 || true
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true
for want in "日本語の画面" "お名前：   山田" "こんにちは、  山田  さん"; do
    grep -q "$want" "$work/japanese.log" || { cat "$work/japanese.log" "$work/server3.log"; fail "Japanese: '$want' not shown"; }
done
grep -q "dbcs: true" "$work/server3.log" || fail "Japanese: DBCS not in the Query Reply"
echo "TN3270E display (model 5, oversize, Japanese), printer, structured fields and SYSREQ passed"

#!/bin/sh
# Terminal facilities through real emulators: s3270 as a 3279 model 5
# display (TN3270E, LU ZEUSLU01) and pr3287 as its associated printer
# (TN3270E ASSOCIATE). Covers the alternate screen size (Erase/Write
# Alternate), START to a printer with RETRIEVE (SCS print data), structured
# fields (Read Partition Query from a program, the reply with AID X'88')
# and SYSREQ / LOGOFF (SSCP-LU data, UNBIND); then an oversize screen
# (160 x 62, 14-bit addresses, a new BIND after the Query Reply) and a
# Japanese screen (CCSID 939 on both sides: double-byte characters with SO /
# SI, a SOSI input field), file transfers with IND$FILE, and tasks of two
# displays running at the same time.
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
for p in ZPRT ZPR2P ZALT ZSF ZBUF ZSA ZLONG; do
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
# the printer associates once the display is connected
for _ in $(seq 50); do
    grep -q "terminal T001 connected" "$work/server.log" && break
    sleep 0.1
done
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

# a program's own data stream: field and character attributes (SA)
(exec $bin/zcics --home "$work" --csd "$src/TEST.CSD" --loadlib "$work/loadlib" \
    --serve $port >"$work/server5.log" 2>&1) &
server=$!
sleep 1
printf 'Connect(localhost:%s)\nWait(10,InputField)\nClear()\nString("ZSA")\nEnter()\nWait(10,Output)\nReadBuffer(Ascii)\nQuit()\n' $port |
    timeout 30 s3270 -model 3279-2-E >"$work/sa.log" 2>&1 || true
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true
for want in "SF(c0=e0,42=f2) 52 45 44" "SA(42=f6) 59 45 4c SA(41=f2) 52 45 56" "SF(c0=c0,43=f1) 2a"; do
    grep -qF "$want" "$work/sa.log" || { cat "$work/sa.log"; fail "attributes: '$want' not in the buffer"; }
done

# concurrent tasks: ZLNG on one display enqueues a resource, computes
# (8 million additions) and waits two seconds (DELAY); another display's
# transactions answer meanwhile, and its ZLNG finds the resource busy
(exec $bin/zcics --home "$work" --csd "$src/TEST.CSD" --loadlib "$work/loadlib" \
    --serve $port >"$work/server7.log" 2>&1) &
server=$!
sleep 1
printf 'Connect(localhost:%s)\nWait(10,InputField)\nClear()\nString("ZLNG")\nEnter()\nWait(20,Output)\nAscii(0,0,1,40)\nQuit()\n' $port |
    timeout 30 s3270 >"$work/long.log" 2>&1 &
long=$!
sleep 1.5
start=$(date +%s%N)
printf 'Connect(localhost:%s)\nWait(10,InputField)\nClear()\nString("ZALT")\nEnter()\nWait(10,Output)\nAscii(0,0,1,40)\nClear()\nString("ZLNG")\nEnter()\nWait(10,Output)\nAscii(0,0,1,40)\nQuit()\n' $port |
    timeout 30 s3270 >"$work/short.log" 2>&1 || true
elapsed=$((($(date +%s%N) - start) / 1000000))
wait $long || true
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true
grep -q "data: ZLONG: DONE 032000004000000" "$work/long.log" &&
    grep -q "data: SIZE " "$work/short.log" && grep -q "data: ZLONG: BUSY" "$work/short.log" ||
    { cat "$work/long.log" "$work/short.log" "$work/server7.log"; fail "concurrent tasks"; }
# ZLNG takes more than four seconds; the other display's work, not two
[ "$elapsed" -lt 2000 ] ||
    { cat "$work/short.log" "$work/server7.log"; fail "concurrent tasks: the second display waited ${elapsed} ms"; }

# file transfer (IND$FILE, DFT): text to a data set and back, binary
# data to a TS queue and back
printf 'first record\nsecond, a longer record\nthird\n' >"$work/up.txt"
head -c 300 /dev/urandom >"$work/up.bin"
(exec $bin/zcics --home "$work" --csd "$src/TEST.CSD" --loadlib "$work/loadlib" \
    --serve $port >"$work/server4.log" 2>&1) &
server=$!
sleep 1
cat <<EOS | timeout 60 s3270 >"$work/transfer.log" 2>&1 || true
Connect(localhost:$port)
Wait(10,InputField)
Clear()
Transfer(Direction=send,HostFile=ZEUS.FT.TEXT,LocalFile=$work/up.txt,Host=cics,Mode=ascii,Cr=remove,Lrecl=40)
Clear()
Transfer(Direction=receive,HostFile=ZEUS.FT.TEXT,LocalFile=$work/down.txt,Host=cics,Mode=ascii,Cr=add)
Clear()
Transfer(Direction=send,HostFile=BINQ,LocalFile=$work/up.bin,Host=cics,Mode=binary)
Clear()
Transfer(Direction=receive,HostFile=BINQ,LocalFile=$work/down.bin,Host=cics,Mode=binary)
Quit()
EOS
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true
cmp -s "$work/up.txt" "$work/down.txt" || { cat "$work/transfer.log" "$work/server4.log"; fail "text transfer differs"; }
cmp -s "$work/up.bin" "$work/down.bin" || { cat "$work/transfer.log" "$work/server4.log"; fail "binary transfer differs"; }
$bin/zjob --home "$work" catalog print ZEUS.FT.TEXT | grep -q "second, a longer record" ||
    fail "the uploaded data set lacks the record"
[ "$(grep -c 'TRANS03 File transfer complete' "$work/server4.log")" = 4 ] ||
    { cat "$work/server4.log"; fail "four transfers did not complete"; }

# TLS: TN3270 over TLS on its own port, STARTTLS on the plain one (and
# a client that refuses it goes on in plain)
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$work/key.pem" -out "$work/cert.pem" \
    -days 2 -subj "/CN=localhost" 2>/dev/null
tport=$((port + 1))
(exec $bin/zcics --home "$work" --csd "$src/TEST.CSD" --loadlib "$work/loadlib" \
    --serve $port --serve-tls $tport --tls-cert "$work/cert.pem" --tls-key "$work/key.pem" \
    >"$work/server6.log" 2>&1) &
server=$!
sleep 1
for target in "L:Y:localhost:$tport" "Y:localhost:$port" "localhost:$port -set startTls=false"; do
    host=${target%% *}
    opts=${target#"$host"}
    # (s3270 4.1 may crash on leaving when STARTTLS is off; what it read
    # before is what the test checks)
    # shellcheck disable=SC2086
    (printf 'Connect(%s)\nWait(10,InputField)\nQuery(Tls)\nClear()\nString("ZALT")\nEnter()\nWait(10,Output)\nWait(1,Seconds)\nAscii(0,0,1,40)\nQuit()\n' "$host" |
        timeout 30 s3270 $opts >>"$work/tls.log" 2>&1) 2>/dev/null || true
done
kill $server 2>/dev/null || true
wait $server 2>/dev/null || true
[ "$(grep -c 'data: secure host-unverified' "$work/tls.log")" = 2 ] &&
    [ "$(grep -c 'data: not secure' "$work/tls.log")" = 1 ] &&
    [ "$(grep -c 'data: SIZE ' "$work/tls.log")" = 3 ] ||
    { cat "$work/tls.log" "$work/server6.log"; fail "TLS sessions"; }

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
echo "TN3270E display (model 5, oversize, Japanese), printer, structured fields, SYSREQ, concurrent tasks, IND\$FILE and TLS passed"

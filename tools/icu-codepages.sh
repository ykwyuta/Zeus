#!/bin/sh
# Downloads ICU's mapping files (UCM) of the Japanese EBCDIC code pages
# (CCSIDs 930, 939, 1390, 1399) at a pinned commit of unicode-org/icu-data
# into .cache/icu and regenerates crates/zeus-cobol/codepages from them.
set -eu

commit=${ICU_DATA_COMMIT:-bae5ca647fa41703393dbf00240ec1b807a9c353}
base=https://raw.githubusercontent.com/unicode-org/icu-data/$commit
cache=.cache/icu
out=crates/zeus-cobol/codepages
mkdir -p "$cache"
for f in ibm-930_P120-1999 ibm-939_P120-1999 ibm-1390_P110-2003 ibm-1399_P110-2003; do
    [ -f "$cache/$f.ucm" ] || curl -sSfL -o "$cache/$f.ucm" "$base/charset/data/ucm/$f.ucm"
done
[ -f "$cache/LICENSE" ] || curl -sSfL -o "$cache/LICENSE" "$base/LICENSE"
# the generator is built with the current tables; the new ones replace
# them when it has written all of them
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cargo run -q -p zeus-cobol --example icu_codepages -- "$cache" "$tmp"
rm -f "$out"/*.sbcs "$out"/*.dbcs
cp "$tmp"/*.sbcs "$tmp"/*.dbcs "$out"/
cp "$cache/LICENSE" "$out/LICENSE-ICU"

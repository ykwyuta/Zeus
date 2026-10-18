#!/bin/sh
# Downloads the EBCDIC mapping files in codepages/SOURCES (UCM)
# at a pinned commit of unicode-org/icu-data
# into .cache/icu and regenerates crates/zeus-cobol/codepages from them.
set -eu

commit=${ICU_DATA_COMMIT:-bae5ca647fa41703393dbf00240ec1b807a9c353}
base=https://raw.githubusercontent.com/unicode-org/icu-data/$commit
cache=.cache/icu
out=crates/zeus-cobol/codepages
mkdir -p "$cache"
while read -r f ccsid dbcs; do
    [ -f "$cache/$f" ] || curl -sSfL -o "$cache/$f" "$base/charset/data/ucm/$f"
done < "$out/SOURCES"
[ -f "$cache/LICENSE" ] || curl -sSfL -o "$cache/LICENSE" "$base/LICENSE"
# the generator is built with the current tables; the new ones replace
# them when it has written all of them
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cargo run -q -p zeus-cobol --example icu_codepages -- "$cache" "$tmp"
rm -f "$out"/*.sbcs "$out"/*.dbcs
cp "$tmp"/*.sbcs "$tmp"/*.dbcs "$out"/
cp "$tmp"/tables.rs "$out"/
cp "$cache/LICENSE" "$out/LICENSE-ICU"

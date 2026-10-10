#!/bin/sh
# Downloads the NIST CCVS85 COBOL test suite (newcob.val) into .cache/ccvs85
# and verifies its checksum. The suite is not stored in the repository.
#
# usage: tools/fetch-ccvs85.sh [cache-dir]
set -eu

dir=${1:-.cache/ccvs85}
url=https://sourceforge.net/projects/gnucobol/files/nist/newcob.val.tar.gz/download
archive_sha=e4513f26a9b38911f7bf882fe3d3339a80b45cabc2caf85eb3055b0f5ce87ee0
newcob_sha=744a04982095a3abea29a9df5faf63d226083edecc7b5bf34bc412eae0d53274

check() {
    echo "$1  $2" | sha256sum -c --status
}

mkdir -p "$dir"
if [ -f "$dir/newcob.val" ] && check "$newcob_sha" "$dir/newcob.val"; then
    echo "newcob.val already present in $dir"
    exit 0
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL --retry 4 -o "$tmp/newcob.val.tar.gz" "$url"
if ! check "$archive_sha" "$tmp/newcob.val.tar.gz"; then
    echo "checksum mismatch for downloaded archive" >&2
    exit 1
fi
tar -xzf "$tmp/newcob.val.tar.gz" -C "$tmp" newcob.val
if ! check "$newcob_sha" "$tmp/newcob.val"; then
    echo "checksum mismatch for newcob.val" >&2
    exit 1
fi
mv "$tmp/newcob.val" "$dir/newcob.val"
echo "newcob.val stored in $dir"

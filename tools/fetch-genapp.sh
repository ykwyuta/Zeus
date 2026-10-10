#!/bin/sh
# Downloads the CICS General Insurance Application (GenApp, cicsdev/cics-genapp)
# at a pinned commit into .cache/genapp. Not stored in the repository.
#
# usage: tools/fetch-genapp.sh [cache-dir]
set -eu

dir=${1:-.cache/genapp}
url=https://github.com/cicsdev/cics-genapp
commit=f6f3f4b2580d31b7d8dcc31ce3e3676f4cceaaaa

if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ]; then
    echo "GenApp $commit already present in $dir"
    exit 0
fi
rm -rf "$dir"
git init -q "$dir"
git -C "$dir" fetch -q --depth 1 "$url" "$commit"
git -C "$dir" checkout -q FETCH_HEAD
echo "GenApp $commit stored in $dir"

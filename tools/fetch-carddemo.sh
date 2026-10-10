#!/bin/sh
# Downloads AWS CardDemo (Apache-2.0) at a pinned commit into
# .cache/carddemo. The application is not stored in the repository.
#
# usage: tools/fetch-carddemo.sh [cache-dir]
set -eu

dir=${1:-.cache/carddemo}
url=https://github.com/aws-samples/aws-mainframe-modernization-carddemo
commit=59cc6c2fd7ebd7ef7925cad552a01a4b8b6e4d5e

if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ]; then
    echo "CardDemo $commit already present in $dir"
    exit 0
fi
rm -rf "$dir"
git init -q "$dir"
git -C "$dir" fetch -q --depth 1 "$url" "$commit"
git -C "$dir" checkout -q FETCH_HEAD
echo "CardDemo $commit stored in $dir"

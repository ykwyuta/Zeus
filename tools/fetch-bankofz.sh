#!/bin/sh
# Downloads Bank of Z (IBM/Bank-of-Z)
# at a pinned commit into .cache/bankofz. Not stored in the repository.
#
# usage: tools/fetch-bankofz.sh [cache-dir]
set -eu

dir=${1:-.cache/bankofz}
url=https://github.com/IBM/Bank-of-Z
commit=977924a39449178efd69b652765ed5b73da93baa

if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ]; then
    echo "Bank of Z $commit already present in $dir"
    exit 0
fi
rm -rf "$dir"
git init -q "$dir"
git -C "$dir" fetch -q --depth 1 "$url" "$commit"
git -C "$dir" checkout -q FETCH_HEAD
echo "Bank of Z $commit stored in $dir"

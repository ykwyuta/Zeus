#!/bin/sh
# Downloads the additional COBOL corpora of docs/proposal/0022 at pinned
# commits into .cache/corpora/<name>. They are not stored in the repository.
#
# usage: tools/fetch-corpora.sh [name...]   (default: all)
#
# names: zfam zecs zuid taxe-fonciere mortgage zopeneditor cobol-course
#        cobol-check acif
set -eu

root=.cache/corpora

# name url commit [sparse-path...]
corpus() {
    case $1 in
    zfam) echo https://github.com/walmartlabs/zFAM f57e36adec60c0c03d2fb86cd944b2ce0c43f8f5 ;;
    zecs) echo https://github.com/walmartlabs/zECS 6d6bcbbc89c9be086a58cb7ad2ff4d702e873d02 ;;
    zuid) echo https://github.com/walmartlabs/zUID 4bf25a2eed97c7e0c2f586cdcfb5fdb8e5575121 ;;
    taxe-fonciere) echo https://github.com/etalab/taxe-fonciere 6bd40b2d78eaeb3f2635fbd7a9eb1f952d0739ba ;;
    mortgage) echo https://github.com/IBM/dbb ce2c1be58eb049e000c6627bd8273263fef0c606 zBuilder/MortgageApplication ;;
    zopeneditor) echo https://github.com/IBM/zopeneditor-sample 8f9835308de6159eb54f226041d5f81a780cb8eb ;;
    cobol-course) echo https://github.com/openmainframeproject/cobol-programming-course 780b1c9624e066b9d784a738896e493282c5fbf4 ;;
    cobol-check) echo https://github.com/openmainframeproject/cobol-check c79624bd286d5f08f156ccce755bc5d4fffd1909 ;;
    acif) echo https://github.com/IBM/IBM-Z-zOS 083802e04cbb3e9da7e69d0c3fd435da228907a7 zOS-Print/ACIF-User-Exit-samples ;;
    *) return 1 ;;
    esac
}

fetch() { # name
    spec=$(corpus "$1") || {
        echo "unknown corpus $1" >&2
        exit 2
    }
    # shellcheck disable=SC2086
    set -- "$1" $spec
    name=$1 url=$2 commit=$3
    shift 3
    dir=$root/$name
    if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ]; then
        echo "$name $commit already present in $dir"
        return
    fi
    rm -rf "$dir"
    git init -q "$dir"
    if [ $# -gt 0 ]; then
        # large repositories: only the corpus directory
        git -C "$dir" sparse-checkout set --no-cone "$@"
        git -C "$dir" fetch -q --depth 1 --filter=blob:none "$url" "$commit"
    else
        git -C "$dir" fetch -q --depth 1 "$url" "$commit"
    fi
    git -C "$dir" checkout -q FETCH_HEAD
    case $name in
    taxe-fonciere)
        # EFITAUX2 is Latin-1; the other members are ASCII
        f=$dir/src/EFITAUX2.cob
        iconv -f ISO-8859-1 -t UTF-8 "$f" >"$f.utf8"
        mv "$f.utf8" "$f"
        ;;
    esac
    echo "$name $commit stored in $dir"
}

if [ $# -eq 0 ]; then
    set -- zfam zecs zuid taxe-fonciere mortgage zopeneditor cobol-course cobol-check acif
fi
for name in "$@"; do
    fetch "$name"
done

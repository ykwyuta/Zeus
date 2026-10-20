#!/bin/sh
# Lists the COBOL programs of the additional corpora (docs/proposal/0022)
# with the zcobc / ztestgen options they compile with, one per line,
# tab-separated (paths contain spaces):
#
#   P <corpus> <source> <option>...      a program
#   X <corpus> <program> <reason>        a program left out (reasons outside Zeus)
#
# Fetches the corpora and writes the BMS symbolic maps to work/corpora.
#
# usage: tools/corpora-programs.sh
set -eu

c=.cache/corpora
work=work/corpora
bin=target/release

tools/fetch-corpora.sh >/dev/null

# corpus program reason
excluded='taxe-fonciere	FMSTAU2	its COPY members XB30 and XB35 are not published
cobol-course	CBL0002	lab exercise: the compile error is to be corrected
cobol-course	CBL0009	lab exercise: the compile error is to be corrected
cobol-course	CBL0012	lab exercise: the compile error is to be corrected
cobol-check	MOCK	BY VALUE of a PIC X(80) item (test input of the mock parser)
cobol-check	MOCKTEST	BY VALUE of a PIC X(80) item (test input of the mock parser)
acif	ACIFOTX	COPY member AFPAGOT is part of the Print Services product
acif	STRFLDS	a COPY member, not a program'
printf '%s\n' "$excluded" | sed 's/^/X	/'

# programs: corpus option... -- source...
programs() {
    corpus=$1
    shift
    opts=
    while [ "$1" != "--" ]; do
        opts="$opts	$1"
        shift
    done
    shift
    for f in "$@"; do
        name=$(basename "$f" | sed 's/\.[^.]*$//' | tr a-z A-Z)
        if printf '%s\n' "$excluded" | awk -F'\t' -v c="$corpus" -v n="$name" \
            '$1 == c && $2 == n { found = 1 } END { exit !found }'; then
            continue
        fi
        printf 'P\t%s\t%s%s\n' "$corpus" "$f" "$opts"
    done
}

programs zfam -I "$c/zfam/Source" -- "$c"/zfam/Source/*.cbl
programs zecs -I "$c/zecs/Source" -- "$c"/zecs/Source/*.cbl
programs zuid --source-encoding ebcdic --ccsid 37 -I "$c/zuid/cbl" -- "$c"/zuid/cbl/*.cbl
programs taxe-fonciere -I "$c/taxe-fonciere/src" -- "$c"/taxe-fonciere/src/*.cob
m=$c/mortgage/zBuilder/MortgageApplication
mkdir -p "$work/bms-mortgage"
for f in "$m"/bms/*.bms; do
    $bin/zbms "$f" --copy "$work/bms-mortgage/$(basename "$f" .bms | tr a-z A-Z).cpy" >/dev/null
done
programs mortgage -I "$m/copybook" -I "$work/bms-mortgage" -- "$m"/cobol/*.cbl
z=$c/zopeneditor
programs zopeneditor -I "$z/COPYBOOK" -I "$z/COPYLIB" -I "$z/COPYLIB-MVS" -- "$z"/COBOL/*
programs cobol-course -- "$c"/cobol-course/*/Labs/cbl/* "$c"/cobol-course/*/Challenges/*/cbl/*
k=$c/cobol-check
programs cobol-check -I "$k/src/main/cobol/copy" -I "$k/src/main/cobol/copy/Outrec" -I "$k/copybooks" \
    -- "$k"/src/main/cobol/*.CBL "$k"/src/main/cobol/*.cbl
a="$c/acif/zOS-Print/ACIF-User-Exit-samples/Cobol Source"
programs acif -I "$a" -- "$a"/*

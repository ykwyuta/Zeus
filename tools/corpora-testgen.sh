#!/bin/sh
# Generates C1 test cases with ztestgen for every program of the additional
# corpora (docs/proposal/0022), runs them again with ztest and prints the
# branch coverage of each program.
#
# usage: tools/corpora-testgen.sh [--time SECONDS] [--jobs N] [corpus...]
#
# Cases and reports go to work/corpora-testgen/<corpus>/<PROGRAM>; the
# programs of each corpus are compiled to work/corpora-testgen/lib/<corpus>
# so that the programs they call run (-L) instead of stubs. Seeds (initial
# inputs, --seeds) are in tools/corpora-seeds/<corpus>/<PROGRAM>. The
# summary is work/corpora-testgen/summary.tsv, with index.html linking the
# report.html of every program.
set -eu

work=work/corpora-testgen
bin=target/release
time=60
jobs=4

if [ "${1:-}" = "--one" ]; then
    # one program: --one corpus source option...
    corpus=$2 source=$3
    shift 3
    name=$(basename "$source" | sed 's/\.[^.]*$//' | tr a-z A-Z)
    out=$work/$corpus/$name
    rm -rf "$out"
    mkdir -p "$out"
    # options of ztestgen only: seeds (valid requests written by hand) and
    # programs that run long by design
    extra=
    seeds=tools/corpora-seeds/$corpus/$name
    [ -d "$seeds" ] && extra="--seeds $seeds"
    case $corpus/$name in
    # a loop of a million iterations
    cobol-course/CBLC2) extra="$extra --limit 10000000" ;;
    # an endless loop (the lab is about a time limit)
    cobol-course/CBL0015)
        printf '%s\t%s\t-\t-\t-\t0\tendless loop by design\n' "$corpus" "$name" >"$out/summary.tsv"
        exit 0
        ;;
    esac
    # shellcheck disable=SC2086
    if "$bin/ztestgen" "$@" $extra -L "$work/lib/$corpus" --time "$TESTGEN_TIME" --out "$out" \
        "$source" >"$out/ztestgen.log" 2>&1; then
        if "$bin/ztest" "$@" -L "$work/lib/$corpus" "$source" "$out" >"$out/ztest.log" 2>&1; then
            replay=ok
        else
            replay=failed
        fi
        c1=$(sed -n 's/^C1: \([0-9]*\) of \([0-9]*\) .*, \([0-9.]*\)% of the feasible.*/\1\t\2\t\3/p' "$out/report.txt")
        cases=$(ls "$out" | grep -c '^case-' || true)
    else
        replay=- c1="-	-	-" cases=0
        sed -n '1p' "$out/ztestgen.log" >"$out/error.txt"
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$corpus" "$name" "$c1" "$cases" "$replay" >"$out/summary.tsv"
    exit 0
fi

while [ $# -gt 0 ]; do
    case $1 in
    --time) time=$2; shift 2 ;;
    --jobs) jobs=$2; shift 2 ;;
    *) break ;;
    esac
done

cargo build --release -q -p zeus-cobol -p zeus-bms -p zeus-testgen
mkdir -p "$work"
tools/corpora-programs.sh >"$work/programs.tsv"
corpora=${*:-$(awk -F'\t' '$1 == "P" && !seen[$2]++ { print $2 }' "$work/programs.tsv")}

# the called programs: each program compiled, NUL-separated arguments of
# zcobc compile in a file per program
for corpus in $corpora; do
    rm -rf "${work:?}/lib/$corpus"
    mkdir -p "$work/lib/$corpus"
    awk -F'\t' -v c="$corpus" -v lib="$work/lib/$corpus" '
        $1 == "P" && $2 == c {
            name = $3
            sub(/.*\//, "", name)
            sub(/\.[^.]*$/, "", name)
            f = sprintf("%s/%04d.args", lib, ++n)
            printf "compile" >f
            for (i = 4; i <= NF; i++) printf "%c%s", 0, $i >f
            printf "%c-o%c%s/%s.zo%c%s", 0, 0, lib, toupper(name), 0, $3 >f
            close(f)
        }' "$work/programs.tsv"
    for f in "$work/lib/$corpus"/*.args; do
        xargs -0 "$bin/zcobc" <"$f" >/dev/null 2>&1 || true
    done
    rm -f "$work/lib/$corpus"/*.args
done

# one job per program: the arguments of --one, NUL-separated, in a file
rm -rf "$work/jobs"
mkdir -p "$work/jobs"
for corpus in $corpora; do
    awk -F'\t' -v c="$corpus" -v dir="$work/jobs" '
        $1 == "P" && $2 == c {
            f = sprintf("%s/%s.%04d", dir, c, ++n)
            printf "--one%c%s%c%s", 0, $2, 0, $3 >f
            for (i = 4; i <= NF; i++) printf "%c%s", 0, $i >f
            close(f)
        }' "$work/programs.tsv"
done
export TESTGEN_TIME=$time
ls "$work/jobs"/* | xargs -P "$jobs" -I{} sh -c 'xargs -0 sh tools/corpora-testgen.sh <"$1"' _ {}

for corpus in $corpora; do
    cat "$work/$corpus"/*/summary.tsv
done >"$work/summary.tsv"
printf 'corpus\tprogram\tcovered\tdirections\tfeasible%%\tcases\tztest\n'
cat "$work/summary.tsv"

# index.html: every program with its coverage and its report.html
awk -F'\t' '
    BEGIN {
        print "<!doctype html><html lang=en><meta charset=utf-8>"
        print "<meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Corpora coverage</title>"
        print "<style>:root{--bg:#fff;--fg:#1d1d1f;--muted:#6b6b70;--line:#e3e3e6;--bar:#2e7d32;--bar-bg:#e5e5e8;--accent:#2457c5}"
        print "@media (prefers-color-scheme:dark){:root:not([data-theme=\"light\"]){--bg:#17171a;--fg:#e6e6e9;--muted:#9a9aa2;--line:#2c2c31;--bar:#66bb6a;--bar-bg:#2c2c31;--accent:#8fb0ff}}"
        print ":root[data-theme=\"dark\"]{--bg:#17171a;--fg:#e6e6e9;--muted:#9a9aa2;--line:#2c2c31;--bar:#66bb6a;--bar-bg:#2c2c31;--accent:#8fb0ff}"
        print "body{margin:0;padding:16px 24px;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif}"
        print "a{color:var(--accent)}table{border-collapse:collapse}td,th{padding:3px 10px;border-bottom:1px solid var(--line);text-align:left}"
        print "td.r{text-align:right}.bar{width:160px;height:8px;background:var(--bar-bg);border-radius:4px;overflow:hidden}"
        print ".bar span{display:block;height:100%;background:var(--bar)}.muted{color:var(--muted)}"
        print "@media (max-width:700px){body{padding:12px 16px}}</style><body><h1>C1 coverage of the corpora</h1>"
        print "<p class=muted>Generated by tools/corpora-testgen.sh. Feasible: without the directions proven impossible or infeasible.</p>"
        print "<div style=\"overflow-x:auto\"><table><tr><th>Corpus<th>Program<th>Covered<th><th>Feasible<th>Cases<th>ztest</tr>"
    }
    {
        pct = ($5 == "-") ? 0 : $5
        name = ($3 == "-") ? $2 : "<a href=\"" $1 "/" $2 "/report.html\">" $2 "</a>"
        cov = ($3 == "-") ? "-" : $3 " / " $4
        printf "<tr><td>%s<td>%s<td class=r>%s<td><div class=bar><span style=\"width:%s%%\"></span></div><td class=r>%s%%<td class=r>%s<td>%s</tr>\n", $1, name, cov, pct, $5, $6, $7
        if ($3 != "-") { c += $3; t += $4 }
    }
    END {
        printf "</table></div><p><b>%d of %d branch directions (%.1f%%)</b></p></body></html>\n", c, t, t ? 100 * c / t : 100
    }' "$work/summary.tsv" >"$work/index.html"
echo "report: $work/index.html"

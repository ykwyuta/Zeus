#!/bin/sh
# Runs ZHVS (HLASM Validation Suite) and ZPVS (PL/I Validation Suite) on
# Hercules inside a reproducible Docker container (or directly on the host
# when ZEUS_HERCULES_LOCAL=1).
#
# usage:
#   tools/hercules-suites.sh build
#   tools/hercules-suites.sh zhvs [zeus-hercdiff zhvs options...]
#   tools/hercules-suites.sh zpvs [--mvs-dasd DIR] [zeus-hercdiff zpvs options...]
#   tools/hercules-suites.sh all  [--mvs-dasd DIR]
#
# Environment variables:
#   ZEUS_HERCULES_IMAGE   Docker image tag (default: zeus-hercules-oracle:latest)
#   ZEUS_MVS38_DASD       Host directory containing MVS 3.8j CCKD DASDs
#                         (mvsres.140.cckd, mvs001.141.cckd, ...) for ZPVS
#   ZEUS_HERCULES_LOCAL   Set to 1 to run directly on the host without Docker
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
image=${ZEUS_HERCULES_IMAGE:-zeus-hercules-oracle:latest}
local_run=${ZEUS_HERCULES_LOCAL:-0}

ensure_image() {
    if [ "$local_run" = "1" ]; then
        return 0
    fi
    if ! docker image inspect "$image" >/dev/null 2>&1; then
        docker build -t "$image" -f "$root/tools/hercules/Dockerfile" "$root/tools/hercules"
    fi
}

run_zhvs() {
    ensure_image
    mkdir -p "$root/work"
    if [ "$local_run" = "1" ]; then
        cargo run --release -p zeus-hercdiff -- zhvs \
            --out "$root/work/zhvs-hercules.json" "$@"
    else
        docker run --rm \
            -v "$root:/workspace" \
            -w /workspace \
            "$image" \
            cargo run --release -p zeus-hercdiff -- zhvs \
                --out work/zhvs-hercules.json "$@"
    fi
}

run_zpvs() {
    mvs_dasd=${ZEUS_MVS38_DASD:-}
    # Check if --mvs-dasd was passed on the command line
    prev=""
    for arg in "$@"; do
        if [ "$prev" = "--mvs-dasd" ]; then
            mvs_dasd=$arg
            break
        fi
        prev=$arg
    done
    if [ -z "$mvs_dasd" ]; then
        echo "error: ZPVS requires MVS 3.8j DASD images; pass --mvs-dasd DIR or set ZEUS_MVS38_DASD" >&2
        exit 2
    fi
    ensure_image
    mkdir -p "$root/work"
    if [ "$local_run" = "1" ]; then
        if [ -n "${ZEUS_MVS38_DASD:-}" ]; then
            cargo run --release -p zeus-hercdiff -- zpvs \
                --mvs-dasd "$mvs_dasd" \
                --out "$root/work/zpvs-hercules.json" "$@"
        else
            cargo run --release -p zeus-hercdiff -- zpvs \
                --out "$root/work/zpvs-hercules.json" "$@"
        fi
    else
        dasd_abs=$(cd "$mvs_dasd" && pwd)
        # Strip --mvs-dasd <dir> from "$@" if present so we can pass /mvs-dasd inside the container
        set -- $(
            skip=0
            for a in "$@"; do
                if [ $skip -eq 1 ]; then
                    skip=0
                    continue
                fi
                if [ "$a" = "--mvs-dasd" ]; then
                    skip=1
                    continue
                fi
                printf '%s\n' "$a"
            done
        )
        docker run --rm \
            -v "$root:/workspace" \
            -v "$dasd_abs:/mvs-dasd:ro" \
            -w /workspace \
            "$image" \
            cargo run --release -p zeus-hercdiff -- zpvs \
                --mvs-dasd /mvs-dasd \
                --out work/zpvs-hercules.json "$@"
    fi
}

cmd=${1:-}
case $cmd in
    build)
        docker build -t "$image" -f "$root/tools/hercules/Dockerfile" "$root/tools/hercules"
        ;;
    zhvs)
        shift
        run_zhvs "$@"
        ;;
    zpvs)
        shift
        run_zpvs "$@"
        ;;
    all)
        shift
        run_zhvs
        run_zpvs "$@"
        ;;
    *)
        echo "usage: $0 build | zhvs [options] | zpvs [--mvs-dasd DIR] [options] | all [--mvs-dasd DIR]" >&2
        exit 2
        ;;
esac

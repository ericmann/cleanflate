#!/usr/bin/env bash
# Usage: fuzz-milestone.sh SECONDS TARGET...
#        fuzz-milestone.sh --summarize LOG
# Runs the given cargo-fuzz targets concurrently and prints one summary line each.
set -u

summarize() {
    local log="$1"
    if [ ! -f "$log" ]; then
        echo 0
        return 0
    fi
    grep -oE '^#[0-9]+' "$log" | tr -d '#' | sort -n | tail -n 1 | grep . || echo 0
}

if [ "${1:-}" = "--summarize" ]; then
    summarize "${2:-/dev/null}"
    exit 0
fi

if [ "$#" -lt 2 ]; then
    echo "usage: $0 SECONDS TARGET... | --summarize LOG" >&2
    exit 2
fi

seconds="$1"
shift
mkdir -p target/fuzz-logs
marker="target/fuzz-logs/.start.$$"
: > "$marker"
declare -A pids starts
for t in "$@"; do
    mkdir -p "fuzz/corpus/$t" "fuzz/artifacts/$t"
    starts[$t]=$(date +%s)
    cargo +nightly fuzz run "$t" "fuzz/corpus/$t" "fuzz/seeds/$t" -- \
        -max_total_time="$seconds" -rss_limit_mb=2048 > "target/fuzz-logs/$t.log" 2>&1 &
    pids[$t]=$!
done

status=0
for t in "$@"; do
    code=0
    wait "${pids[$t]}" || code=$?
    wall=$(( $(date +%s) - starts[$t] ))
    execs=$(summarize "target/fuzz-logs/$t.log")
    arts=$(find "fuzz/artifacts/$t" -type f -newer "$marker" 2>/dev/null | wc -l)
    echo "fuzz-milestone: $t seconds=$wall execs=$execs exit=$code artifacts=$arts"
    if [ "$code" -ne 0 ] || [ "$arts" -ne 0 ]; then
        status=1
    fi
done
rm -f "$marker"
exit $status

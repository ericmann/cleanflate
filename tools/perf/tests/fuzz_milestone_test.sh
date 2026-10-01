#!/usr/bin/env bash
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
script="$here/../fuzz-milestone.sh"

summarize_reads_largest_exec_count() {
    got=$(bash "$script" --summarize "$here/fixtures/fuzz-several.log")
    [ "$got" = "4096" ] || { echo "FAIL summarize_reads_largest_exec_count: got $got"; exit 1; }
    echo "ok summarize_reads_largest_exec_count"
}

summarize_empty_log_is_zero() {
    got=$(bash "$script" --summarize "$here/fixtures/fuzz-empty.log")
    [ "$got" = "0" ] || { echo "FAIL summarize_empty_log_is_zero: got $got"; exit 1; }
    echo "ok summarize_empty_log_is_zero"
}

summarize_reads_largest_exec_count
summarize_empty_log_is_zero

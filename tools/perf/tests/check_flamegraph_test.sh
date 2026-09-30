#!/usr/bin/env bash
set -u
here="$(cd "$(dirname "$0")" && pwd)"
script="$here/../check-flamegraph.sh"
fail=0

check() {
    name="$1"
    expect="$2"
    file="$3"
    bash "$script" "$file" >/dev/null 2>&1
    code=$?
    if [ "$expect" = pass ] && [ "$code" -eq 0 ]; then
        echo "ok $name"
    elif [ "$expect" = fail ] && [ "$code" -ne 0 ]; then
        echo "ok $name"
    else
        echo "FAIL $name: exit $code"
        fail=1
    fi
}

check accepts_cleanflate_svg pass "$here/fixtures/flame-ok.svg"
check rejects_svg_without_cleanflate_frames fail "$here/fixtures/flame-no-cleanflate.svg"
check rejects_svg_with_oracle_frame fail "$here/fixtures/flame-oracle.svg"
check rejects_missing_file fail "$here/fixtures/does-not-exist.svg"
exit "$fail"

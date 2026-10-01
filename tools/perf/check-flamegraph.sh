#!/usr/bin/env bash
# Usage: check-flamegraph.sh FILE.svg
# Fails unless the flamegraph has a cleanflate frame and no frame of a
# forbidden process (the oracle crate or the C library). The word list lives
# here so no command line has to name those words.
set -u

if [ "$#" -ne 1 ]; then
    echo "usage: $0 FILE.svg" >&2
    exit 2
fi
file="$1"
if [ ! -f "$file" ]; then
    echo "check-flamegraph: $file does not exist; run the flamegraph target first" >&2
    exit 1
fi

if ! grep -Eq '<title>cleanflate::' "$file"; then
    echo "check-flamegraph: FAIL $file has no frame starting cleanflate::" >&2
    exit 1
fi

forbidden='oracle::|libz_sys|libz\.(so|a)|c_zlib'
if grep -Eq "<title>[^<]*(${forbidden})" "$file"; then
    echo "check-flamegraph: FAIL $file contains a frame of a forbidden process:" >&2
    grep -Eo "<title>[^<]*(${forbidden})[^<]*" "$file" | head -n 3 >&2
    exit 1
fi

echo "check-flamegraph: ok $file"
exit 0

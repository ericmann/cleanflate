#!/usr/bin/env bash
# Snapshot the live provenance log (SPEC S5 M6). Copies
# provenance/tool-log.jsonl to provenance/tool-log.snapshot.jsonl if the
# live log exists and is non-empty, and prints the line count. Otherwise
# prints "no live log" and exits 0.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LIVE_LOG="$ROOT/provenance/tool-log.jsonl"
SNAPSHOT="$ROOT/provenance/tool-log.snapshot.jsonl"

if [ ! -s "$LIVE_LOG" ]; then
    echo "no live log"
    exit 0
fi

cp "$LIVE_LOG" "$SNAPSHOT"
lines="$(wc -l < "$LIVE_LOG" | tr -d ' ')"
echo "$lines"
exit 0

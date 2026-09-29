#!/usr/bin/env bash
# Vendor RFC 1950/1951/1952 plain text into docs/rfc/, recording URL and
# SHA-256 in docs/rfc/SOURCES.json (SPEC S2.1, S5 M0). Refuses to run once
# the post-M0 lockdown flag exists. Fetches only from the three
# rfc-editor.org URLs in tools/cleanroom/allowlist.txt -- no other host.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LOCKDOWN="${CLEANROOM_LOCKDOWN_FILE:-$ROOT/tools/cleanroom/LOCKDOWN}"

if [ -f "$LOCKDOWN" ]; then
    echo "fetch-rfcs.sh: refusing to run, lockdown flag present: $LOCKDOWN" >&2
    exit 1
fi

OUT_DIR="$ROOT/docs/rfc"
mkdir -p "$OUT_DIR"

fetch_with_retry() {
    url="$1"
    dest="$2"
    attempt=1
    while [ "$attempt" -le 3 ]; do
        if curl -fsSL "$url" -o "$dest" 2>"$dest.err"; then
            rm -f "$dest.err"
            return 0
        fi
        attempt=$((attempt + 1))
    done
    echo "fetch-rfcs.sh: failed to fetch $url after 3 attempts: $(cat "$dest.err" 2>/dev/null)" >&2
    rm -f "$dest.err"
    return 1
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

now_iso() {
    date -u +"%Y-%m-%dT%H:%M:%SZ"
}

entries=()
for n in 1950 1951 1952; do
    url="https://www.rfc-editor.org/rfc/rfc${n}.txt"
    dest="$OUT_DIR/rfc${n}.txt"
    if ! fetch_with_retry "$url" "$dest"; then
        exit 1
    fi
    sha="$(sha256_of "$dest")"
    bytes="$(wc -c < "$dest" | tr -d ' ')"
    fetched_at="$(now_iso)"
    entries+=("{\"file\":\"rfc${n}.txt\",\"url\":\"${url}\",\"sha256\":\"${sha}\",\"bytes\":${bytes},\"fetched_at\":\"${fetched_at}\"}")
done

{
    echo "["
    for i in "${!entries[@]}"; do
        sep=","
        if [ "$i" -eq $((${#entries[@]} - 1)) ]; then
            sep=""
        fi
        echo "  ${entries[$i]}${sep}"
    done
    echo "]"
} > "$OUT_DIR/SOURCES.json"

echo "fetch-rfcs.sh: wrote ${#entries[@]} RFC texts and $OUT_DIR/SOURCES.json"

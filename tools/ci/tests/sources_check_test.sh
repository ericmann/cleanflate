#!/usr/bin/env bash
# Acceptance tests for tools/ci/sources-check.sh.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCES_CHECK="$SCRIPT_DIR/../sources-check.sh"
REAL_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

TOTAL_FAIL=0

report() {
    name="$1"
    ok="$2"
    detail="${3:-}"
    if [ "$ok" -eq 0 ]; then
        echo "PASS $name"
    else
        echo "FAIL $name: $detail"
        TOTAL_FAIL=1
    fi
}

# case_real_sources_pass
out="$(SOURCES_CHECK_ROOT="$REAL_ROOT" bash "$SOURCES_CHECK" 2>&1)"
code=$?
if [ "$code" -eq 0 ]; then
    report case_real_sources_pass 0
else
    report case_real_sources_pass 1 "$out"
fi

# case_tampered_file_fails (on a temp copy of docs/rfc)
d="$(mktemp -d)"
mkdir -p "$d/docs/rfc"
cp "$REAL_ROOT/docs/rfc/SOURCES.json" "$d/docs/rfc/SOURCES.json"
cp "$REAL_ROOT/docs/rfc/rfc1950.txt" "$d/docs/rfc/rfc1950.txt"
cp "$REAL_ROOT/docs/rfc/rfc1951.txt" "$d/docs/rfc/rfc1951.txt"
cp "$REAL_ROOT/docs/rfc/rfc1952.txt" "$d/docs/rfc/rfc1952.txt"
echo "TAMPERED" >> "$d/docs/rfc/rfc1951.txt"
out="$(SOURCES_CHECK_ROOT="$d" bash "$SOURCES_CHECK" 2>&1)"
code=$?
if [ "$code" -ne 0 ] && echo "$out" | grep -q "sha-mismatch"; then
    report case_tampered_file_fails 0
else
    report case_tampered_file_fails 1 "expected sha-mismatch failure, got exit $code: $out"
fi
rm -rf "$d"

# case_missing_file_fails
d="$(mktemp -d)"
mkdir -p "$d/docs/rfc"
cp "$REAL_ROOT/docs/rfc/SOURCES.json" "$d/docs/rfc/SOURCES.json"
cp "$REAL_ROOT/docs/rfc/rfc1950.txt" "$d/docs/rfc/rfc1950.txt"
cp "$REAL_ROOT/docs/rfc/rfc1952.txt" "$d/docs/rfc/rfc1952.txt"
# rfc1951.txt intentionally missing
out="$(SOURCES_CHECK_ROOT="$d" bash "$SOURCES_CHECK" 2>&1)"
code=$?
if [ "$code" -ne 0 ] && echo "$out" | grep -q "missing-file"; then
    report case_missing_file_fails 0
else
    report case_missing_file_fails 1 "expected missing-file failure, got exit $code: $out"
fi
rm -rf "$d"

# case_corpus_tamper_fails (P0-08 adds testdata/corpora/SOURCES.json; run
# only if it exists, since a missing corpus is legitimate before P0-08).
if [ -f "$REAL_ROOT/testdata/corpora/SOURCES.json" ]; then
    d="$(mktemp -d)"
    mkdir -p "$d/docs/rfc" "$d/testdata/corpora"
    cp "$REAL_ROOT/docs/rfc/SOURCES.json" "$d/docs/rfc/SOURCES.json"
    cp "$REAL_ROOT/docs/rfc/rfc1950.txt" "$d/docs/rfc/rfc1950.txt"
    cp "$REAL_ROOT/docs/rfc/rfc1951.txt" "$d/docs/rfc/rfc1951.txt"
    cp "$REAL_ROOT/docs/rfc/rfc1952.txt" "$d/docs/rfc/rfc1952.txt"
    cp "$REAL_ROOT/testdata/corpora/SOURCES.json" "$d/testdata/corpora/SOURCES.json"
    first_file="$(python3 -c "
import json
with open('$REAL_ROOT/testdata/corpora/SOURCES.json') as f:
    data = json.load(f)
for corpus_name, info in data.items():
    for entry in info.get('files', []):
        print(f\"{corpus_name}/{entry.get('name','')}\")
        raise SystemExit
")"
    if [ -n "$first_file" ] && [ -f "$REAL_ROOT/testdata/corpora/$first_file" ]; then
        mkdir -p "$d/testdata/corpora/$(dirname "$first_file")"
        cp "$REAL_ROOT/testdata/corpora/$first_file" "$d/testdata/corpora/$first_file"
        echo "TAMPERED" >> "$d/testdata/corpora/$first_file"
        out="$(SOURCES_CHECK_ROOT="$d" bash "$SOURCES_CHECK" 2>&1)"
        code=$?
        if [ "$code" -ne 0 ] && echo "$out" | grep -q "corpus-sha-mismatch"; then
            report case_corpus_tamper_fails 0
        else
            report case_corpus_tamper_fails 1 "expected corpus-sha-mismatch, got exit $code: $out"
        fi
    else
        echo "SKIP case_corpus_tamper_fails: no local corpus file to tamper with"
    fi
    rm -rf "$d"
else
    echo "SKIP case_corpus_tamper_fails: testdata/corpora/SOURCES.json not present yet (P0-08)"
fi

# case_manual_fetch_requires_intervention_row: a SOURCES.json entry
# fetched_by a human with no matching provenance/human-interventions.md
# row fails; adding the row makes it pass (R1-08).
d="$(mktemp -d)"
mkdir -p "$d/docs/rfc" "$d/testdata/corpora" "$d/provenance"
cp "$REAL_ROOT/docs/rfc/SOURCES.json" "$d/docs/rfc/SOURCES.json"
cp "$REAL_ROOT/docs/rfc/rfc1950.txt" "$d/docs/rfc/rfc1950.txt"
cp "$REAL_ROOT/docs/rfc/rfc1951.txt" "$d/docs/rfc/rfc1951.txt"
cp "$REAL_ROOT/docs/rfc/rfc1952.txt" "$d/docs/rfc/rfc1952.txt"
manual_sha="deadbeef00000000000000000000000000000000000000000000000000000000"
cat > "$d/testdata/corpora/SOURCES.json" <<JSON
{
  "canterbury": {
    "url": "https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz",
    "archive_sha256": "$manual_sha",
    "archive_bytes": 1,
    "status": "ok",
    "fetched_by": "human-operator-browser",
    "files": []
  }
}
JSON
echo "| when | who | stage/task | what | why |" > "$d/provenance/human-interventions.md"
echo "|---|---|---|---|---|" >> "$d/provenance/human-interventions.md"

out_before="$(SOURCES_CHECK_ROOT="$d" bash "$SOURCES_CHECK" 2>&1)"
code_before=$?

echo "| now | operator | test | downloaded archive sha256 $manual_sha | testing |" \
    >> "$d/provenance/human-interventions.md"
out_after="$(SOURCES_CHECK_ROOT="$d" bash "$SOURCES_CHECK" 2>&1)"
code_after=$?

if [ "$code_before" -ne 0 ] \
    && echo "$out_before" | grep -q "manual-fetch-missing-intervention-row" \
    && [ "$code_after" -eq 0 ]; then
    report case_manual_fetch_requires_intervention_row 0
else
    report case_manual_fetch_requires_intervention_row 1 \
        "before: exit $code_before: $out_before / after: exit $code_after: $out_after"
fi
rm -rf "$d"

if [ "$TOTAL_FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

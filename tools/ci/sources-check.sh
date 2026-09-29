#!/usr/bin/env bash
# Verify docs/rfc/SOURCES.json (and testdata/corpora/SOURCES.json, if
# present) against the files on disk: every listed file exists with a
# matching SHA-256, and each RFC text contains its "Request for
# Comments: 195N" marker.
set -u

if [ -n "${SOURCES_CHECK_ROOT:-}" ]; then
    ROOT="$SOURCES_CHECK_ROOT"
else
    ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi

FAIL=0

fail() {
    echo "FAIL $1: $2"
    FAIL=1
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

check_sources_file() {
    sources_json="$1"
    base_dir="$2"
    label="$3"
    local_only="$4" # "1" to skip files that are not present locally

    if [ ! -f "$sources_json" ]; then
        if [ "$label" = "rfc" ]; then
            fail "$label-missing-manifest" "$sources_json does not exist"
        fi
        return
    fi

    count="$(python3 -c "
import json
with open('$sources_json') as f:
    data = json.load(f)
print(len(data) if isinstance(data, list) else 0)
")"

    i=0
    while [ "$i" -lt "$count" ]; do
        file="$(python3 -c "
import json
with open('$sources_json') as f:
    data = json.load(f)
print(data[$i].get('file',''))
")"
        expected_sha="$(python3 -c "
import json
with open('$sources_json') as f:
    data = json.load(f)
print(data[$i].get('sha256',''))
")"
        path="$base_dir/$file"
        if [ ! -f "$path" ]; then
            if [ "$local_only" = "1" ]; then
                i=$((i + 1))
                continue
            fi
            fail "$label-missing-file" "$path does not exist"
            i=$((i + 1))
            continue
        fi
        actual_sha="$(sha256_of "$path")"
        if [ "$actual_sha" != "$expected_sha" ]; then
            fail "$label-sha-mismatch" "$path: expected $expected_sha, got $actual_sha"
        fi
        if [ "$label" = "rfc" ]; then
            rfc_num="$(echo "$file" | grep -oE '[0-9]{4}' | head -1)"
            if [ -n "$rfc_num" ]; then
                if ! grep -q "Request for Comments: $rfc_num" "$path"; then
                    fail "rfc-marker-missing" "$path does not contain 'Request for Comments: $rfc_num'"
                fi
            fi
        fi
        i=$((i + 1))
    done
}

check_sources_file "$ROOT/docs/rfc/SOURCES.json" "$ROOT/docs/rfc" "rfc" "0"

if [ -f "$ROOT/testdata/corpora/SOURCES.json" ]; then
    python3 -c "
import json, sys
with open('$ROOT/testdata/corpora/SOURCES.json') as f:
    data = json.load(f)
for corpus_name, info in data.items():
    for entry in info.get('files', []):
        print(f\"{corpus_name}:{entry.get('name','')}:{entry.get('sha256','')}\")
" | while IFS=':' read -r corpus_name name expected_sha; do
        [ -n "$name" ] || continue
        path="$ROOT/testdata/corpora/$corpus_name/$name"
        if [ ! -f "$path" ]; then
            continue
        fi
        actual_sha="$(sha256_of "$path")"
        if [ "$actual_sha" != "$expected_sha" ]; then
            echo "FAIL corpus-sha-mismatch: $path: expected $expected_sha, got $actual_sha"
            exit 1
        fi
    done || FAIL=1
fi

# R1-08: a corpus fetched by a human (fetched_by set to anything other
# than the automated default) must have a matching row in
# provenance/human-interventions.md naming its archive_sha256, so the
# manual step is recorded, not just the file on disk.
if [ -f "$ROOT/testdata/corpora/SOURCES.json" ]; then
    interventions_file="$ROOT/provenance/human-interventions.md"
    if [ -f "$interventions_file" ]; then
        interventions_content="$(cat "$interventions_file")"
    else
        interventions_content=""
    fi
    manual_check_output="$(INTERVENTIONS_CONTENT="$interventions_content" python3 -c "
import json
import os

with open('$ROOT/testdata/corpora/SOURCES.json') as f:
    data = json.load(f)
interventions = os.environ.get('INTERVENTIONS_CONTENT', '')

missing = []
for corpus_name, info in data.items():
    fetched_by = info.get('fetched_by')
    if not fetched_by or fetched_by in ('automated', 'cached'):
        continue
    sha = info.get('archive_sha256', '')
    if not sha or sha not in interventions:
        missing.append((corpus_name, fetched_by, sha))

for corpus_name, fetched_by, sha in missing:
    print(
        f'FAIL manual-fetch-missing-intervention-row: {corpus_name} '
        f'fetched_by={fetched_by!r} archive_sha256={sha} has no matching '
        'row in provenance/human-interventions.md'
    )
import sys
sys.exit(1 if missing else 0)
")"
    manual_check_status=$?
    if [ -n "$manual_check_output" ]; then
        echo "$manual_check_output"
    fi
    if [ "$manual_check_status" -ne 0 ]; then
        FAIL=1
    fi
fi

if [ "$FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

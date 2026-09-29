#!/usr/bin/env bash
# Download and extract the Canterbury and Silesia corpora into
# testdata/corpora/ (SPEC S2.1: corpora are opaque binary data -- this
# script never reads or summarises their contents, only hashes/sizes
# them). Usage: fetch-corpora.sh [canterbury|silesia|all] [--record]
#                                [--from-file PATH]
#
# Without --record: verifies local files against the committed
# testdata/corpora/SOURCES.json, downloading first if the archive is
# missing. Exits 1 on any mismatch.
# With --record: (re)downloads (or copies, with --from-file), (re)extracts,
# and rewrites SOURCES.json with freshly computed hashes for the
# requested corpus/corpora.
# --from-file PATH: use a pre-downloaded Canterbury archive instead of
# fetching it over the network (for when the allowlisted host blocks
# this machine's requests, e.g. Cloudflare bot protection, but a human
# operator downloaded it in a browser). PATH must match the known-good
# SHA-256 for the standard Canterbury archive; SOURCES.json still
# records the original allowlisted URL, plus a note that a human fetched
# it by hand, so provenance stays honest.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LOCKDOWN="${CLEANROOM_LOCKDOWN_FILE:-$ROOT/tools/cleanroom/LOCKDOWN}"
CORPORA_DIR="$ROOT/testdata/corpora"
ARCHIVES_DIR="$CORPORA_DIR/archives"
SOURCES_JSON="$CORPORA_DIR/SOURCES.json"

# The standard Canterbury corpus archive's known SHA-256, used only to
# sanity-check a --from-file copy (defence against pointing at the wrong
# file); it is not required for the normal network-fetch path.
CANTERBURY_KNOWN_SHA256="f140e8a5b73d3f53198555a63bfb827889394a42f20825df33c810c3d5e3f8fb"

WHICH="${1:-all}"
shift || true
RECORD=0
FROM_FILE=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --record)
            RECORD=1
            shift
            ;;
        --from-file)
            FROM_FILE="${2:-}"
            shift 2
            ;;
        *)
            echo "usage: fetch-corpora.sh [canterbury|silesia|all] [--record] [--from-file PATH]" >&2
            exit 1
            ;;
    esac
done

if [ -f "$LOCKDOWN" ] && [ "${CLEANROOM_ALLOW_CORPUS_FETCH:-0}" != "1" ]; then
    echo "fetch-corpora.sh: refusing to run, lockdown flag present and CLEANROOM_ALLOW_CORPUS_FETCH!=1" >&2
    exit 1
fi

mkdir -p "$ARCHIVES_DIR"

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

fetch_url() {
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
    return 1
}

# fetch_first_working DEST URL...: tries each URL, up to 3 passes over
# the whole list. Sets FETCHED_URL on success.
fetch_first_working() {
    dest="$1"
    shift
    urls=("$@")
    pass=1
    while [ "$pass" -le 3 ]; do
        for url in "${urls[@]}"; do
            if curl -fsSL "$url" -o "$dest" 2>"$dest.err"; then
                rm -f "$dest.err"
                FETCHED_URL="$url"
                return 0
            fi
        done
        pass=$((pass + 1))
    done
    LAST_ERR="$(cat "$dest.err" 2>/dev/null || true)"
    rm -f "$dest.err"
    return 1
}

do_canterbury() {
    archive="$ARCHIVES_DIR/cantrbry.tar.gz"
    url="https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz"
    extract_dir="$CORPORA_DIR/canterbury"

    if [ -n "$FROM_FILE" ]; then
        if [ ! -f "$FROM_FILE" ]; then
            echo "fetch-corpora.sh: --from-file $FROM_FILE does not exist" >&2
            return 1
        fi
        actual_sha="$(sha256_of "$FROM_FILE")"
        if [ "$actual_sha" != "$CANTERBURY_KNOWN_SHA256" ]; then
            echo "fetch-corpora.sh: --from-file $FROM_FILE sha256 $actual_sha does not match the known-good Canterbury archive hash $CANTERBURY_KNOWN_SHA256" >&2
            return 1
        fi
        cp "$FROM_FILE" "$archive"
        CANTERBURY_FETCHED_BY="human-operator-browser"
    elif [ "$RECORD" -eq 1 ] || [ ! -f "$archive" ]; then
        if ! fetch_url "$url" "$archive"; then
            echo "fetch-corpora.sh: Canterbury download failed after 3 attempts: $(cat "$archive.err" 2>/dev/null)" >&2
            return 1
        fi
        CANTERBURY_FETCHED_BY="automated"
    else
        CANTERBURY_FETCHED_BY="${CANTERBURY_FETCHED_BY:-cached}"
    fi

    mkdir -p "$extract_dir"
    if ! tar -xzf "$archive" -C "$extract_dir"; then
        echo "fetch-corpora.sh: failed to extract Canterbury archive" >&2
        return 1
    fi
    # The archive preserves its original (sometimes read-only) 1990s
    # permissions; normalise to u+rw so later tooling (tests, re-runs)
    # can always overwrite these files.
    chmod -R u+rw "$extract_dir"

    CANTERBURY_URL="$url"
    CANTERBURY_ARCHIVE="$archive"
    CANTERBURY_DIR="$extract_dir"
    CANTERBURY_STATUS="ok"
    return 0
}

do_silesia() {
    archive="$ARCHIVES_DIR/silesia.zip"
    extract_dir="$CORPORA_DIR/silesia"
    urls=(
        "https://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip"
        "https://sun.aei.polsl.pl//~sdeor/corpus/silesia.zip"
        "http://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip"
        "http://sun.aei.polsl.pl//~sdeor/corpus/silesia.zip"
    )

    if [ "$RECORD" -eq 1 ] || [ ! -f "$archive" ]; then
        if ! fetch_first_working "$archive" "${urls[@]}"; then
            echo "fetch-corpora.sh: Silesia NOT DOWNLOADED (all mirrors failed): $LAST_ERR" >&2
            SILESIA_STATUS="missing"
            SILESIA_ERROR="$LAST_ERR"
            return 0
        fi
        SILESIA_URL="$FETCHED_URL"
    else
        SILESIA_URL="${urls[0]}"
    fi

    mkdir -p "$extract_dir"
    if ! unzip -o -j "$archive" -d "$extract_dir" >/dev/null; then
        echo "fetch-corpora.sh: failed to extract Silesia archive" >&2
        SILESIA_STATUS="missing"
        SILESIA_ERROR="extract failed"
        return 0
    fi
    chmod -R u+rw "$extract_dir"

    SILESIA_ARCHIVE="$archive"
    SILESIA_DIR="$extract_dir"
    SILESIA_STATUS="ok"
    return 0
}

verify_against_sources() {
    if [ ! -f "$SOURCES_JSON" ]; then
        echo "fetch-corpora.sh: $SOURCES_JSON does not exist, nothing to verify" >&2
        return 1
    fi
    python3 - "$SOURCES_JSON" "$CORPORA_DIR" <<'PYEOF'
import hashlib
import json
import os
import sys

sources_json, corpora_dir = sys.argv[1], sys.argv[2]
with open(sources_json) as f:
    data = json.load(f)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


ok = True
for corpus_name, info in data.items():
    if info.get("status") != "ok":
        continue
    for entry in info.get("files", []):
        path = os.path.join(corpora_dir, corpus_name, entry["name"])
        if not os.path.isfile(path):
            print(f"FAIL missing: {path}")
            ok = False
            continue
        actual = sha256_of(path)
        if actual != entry["sha256"]:
            print(f"FAIL mismatch: {path}: expected {entry['sha256']}, got {actual}")
            ok = False

sys.exit(0 if ok else 1)
PYEOF
}

write_sources_json() {
    python3 - \
        "$SOURCES_JSON" \
        "${CANTERBURY_STATUS:-}" "${CANTERBURY_URL:-}" "${CANTERBURY_ARCHIVE:-}" "${CANTERBURY_DIR:-}" "${CANTERBURY_FETCHED_BY:-}" \
        "${SILESIA_STATUS:-}" "${SILESIA_URL:-}" "${SILESIA_ARCHIVE:-}" "${SILESIA_DIR:-}" "${SILESIA_ERROR:-}" \
        <<'PYEOF'
import hashlib
import json
import os
import sys

(sources_json, c_status, c_url, c_archive, c_dir, c_fetched_by,
 s_status, s_url, s_archive, s_dir, s_error) = sys.argv[1:12]


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def files_of(d):
    entries = []
    if d and os.path.isdir(d):
        for name in sorted(os.listdir(d)):
            p = os.path.join(d, name)
            if not os.path.isfile(p):
                continue
            entries.append({"name": name, "sha256": sha256_of(p), "bytes": os.path.getsize(p)})
    return entries


# Preserve the entry for whichever corpus wasn't touched this run.
data = {}
if os.path.exists(sources_json):
    try:
        with open(sources_json) as f:
            data = json.load(f)
    except Exception:
        data = {}

if c_status:
    entry = {
        "url": c_url,
        "archive_sha256": sha256_of(c_archive) if c_archive else "",
        "archive_bytes": os.path.getsize(c_archive) if c_archive else 0,
        "status": c_status,
        "files": files_of(c_dir),
    }
    if c_fetched_by and c_fetched_by != "automated" and c_fetched_by != "cached":
        entry["fetched_by"] = c_fetched_by
        entry["note"] = (
            "The allowlisted host blocked automated requests from the build "
            "sandbox (Cloudflare bot protection, HTTP 403). A human operator "
            "downloaded this file from the same allowlisted URL in a "
            "browser and it was verified against the known-good SHA-256 "
            "before use."
        )
    data["canterbury"] = entry

if s_status:
    if s_status == "ok":
        data["silesia"] = {
            "url": s_url,
            "archive_sha256": sha256_of(s_archive) if s_archive else "",
            "archive_bytes": os.path.getsize(s_archive) if s_archive else 0,
            "status": "ok",
            "files": files_of(s_dir),
        }
    else:
        data["silesia"] = {
            "url": "",
            "archive_sha256": "",
            "archive_bytes": 0,
            "status": "missing",
            "error": s_error,
            "files": [],
        }

with open(sources_json, "w") as f:
    json.dump(data, f, indent=2, sort_keys=True)
    f.write("\n")
PYEOF
}

main() {
    case "$WHICH" in
        canterbury|silesia|all) ;;
        *)
            echo "usage: fetch-corpora.sh [canterbury|silesia|all] [--record] [--from-file PATH]" >&2
            exit 1
            ;;
    esac

    # In both modes: download (if the archive is missing, or --record was
    # given) and (re-)extract the requested corpus/corpora. In verify
    # mode this makes a fresh checkout self-heal; --record then also
    # rewrites SOURCES.json with freshly computed hashes.
    fail=0
    if [ "$WHICH" = "canterbury" ] || [ "$WHICH" = "all" ]; then
        if ! do_canterbury; then
            fail=1
        fi
    fi
    if [ "$WHICH" = "silesia" ] || [ "$WHICH" = "all" ]; then
        do_silesia
    fi
    if [ "$fail" -ne 0 ]; then
        exit 1
    fi

    if [ "$RECORD" -eq 1 ]; then
        write_sources_json
        exit 0
    fi

    verify_against_sources
    exit $?
}

main

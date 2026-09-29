#!/usr/bin/env bash
# Mechanically enforce CLAUDE.md C1 (no unsafe token), C2 (crate attrs),
# C3 (zero dependencies, no build.rs) and C11 (forbidden path names)
# for crates/cleanflate.
set -u

if [ -n "${CRATE_CHECKS_ROOT:-}" ]; then
    ROOT="$CRATE_CHECKS_ROOT"
else
    ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi

FAIL=0

fail() {
    echo "FAIL $1: $2"
    FAIL=1
}

# (a) deps: zero dependencies (normal, dev, build) and no build.rs.
if command -v cargo >/dev/null 2>&1 && [ -f "$ROOT/Cargo.toml" ]; then
    deps_len="$(cargo metadata --format-version 1 --no-deps --manifest-path "$ROOT/Cargo.toml" 2>/dev/null \
        | python3 -c 'import json,sys
try:
    data = json.load(sys.stdin)
except Exception:
    print("-1")
    sys.exit(0)
for pkg in data.get("packages", []):
    if pkg.get("name") == "cleanflate":
        print(len(pkg.get("dependencies", [])))
        sys.exit(0)
print("-1")')"
    if [ "$deps_len" = "-1" ]; then
        fail "deps" "could not find cleanflate package in cargo metadata"
    elif [ "$deps_len" != "0" ]; then
        fail "deps" "cleanflate has $deps_len dependencies, expected 0"
    fi
else
    fail "deps" "cargo not available or $ROOT/Cargo.toml missing"
fi

if [ -f "$ROOT/crates/cleanflate/build.rs" ]; then
    fail "deps" "crates/cleanflate/build.rs must not exist"
fi

# (b) unsafe-token: no bare `unsafe` word anywhere under crates/cleanflate.
if [ -d "$ROOT/crates/cleanflate" ]; then
    hits="$(grep -rnw unsafe "$ROOT/crates/cleanflate" 2>/dev/null || true)"
    if [ -n "$hits" ]; then
        fail "unsafe-token" "found 'unsafe' token: $(echo "$hits" | head -1)"
    fi
else
    fail "unsafe-token" "$ROOT/crates/cleanflate does not exist"
fi

# (c) crate-attrs: lib.rs has #![forbid(unsafe_code)] and the deny line.
LIBRS="$ROOT/crates/cleanflate/src/lib.rs"
if [ -f "$LIBRS" ]; then
    if ! grep -qF '#![forbid(unsafe_code)]' "$LIBRS"; then
        fail "crate-attrs" "missing literal line #![forbid(unsafe_code)]"
    fi
    deny_block="$(tr '\n' ' ' < "$LIBRS")"
    for needle in 'clippy::unwrap_used' 'clippy::expect_used' 'clippy::panic' 'clippy::indexing_slicing'; do
        if ! printf '%s' "$deny_block" | grep -qE "#!\[deny\([^]]*${needle}"; then
            fail "crate-attrs" "missing $needle in a #![deny(...)] line"
        fi
    done
else
    fail "crate-attrs" "$LIBRS does not exist"
fi

# (d) forbidden-paths: no tracked path outside crates/cleanflate or
# crates/oracle contains a forbidden word.
if command -v git >/dev/null 2>&1 && git -C "$ROOT" rev-parse --show-toplevel >/dev/null 2>&1; then
    forbidden_hits="$(git -C "$ROOT" ls-files \
        | grep -v '^crates/cleanflate/' \
        | grep -v '^crates/oracle/' \
        | grep -iE 'zlib|miniz|flate2|libdeflate|libz-sys|libz_sys|zopfli' || true)"
    if [ -n "$forbidden_hits" ]; then
        fail "forbidden-paths" "$(echo "$forbidden_hits" | head -1)"
    fi
else
    fail "forbidden-paths" "$ROOT is not a git repository"
fi

if [ "$FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

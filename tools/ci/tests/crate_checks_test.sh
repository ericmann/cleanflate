#!/usr/bin/env bash
# Acceptance tests for tools/ci/crate-checks.sh. Builds throwaway fixture
# repos under mktemp -d and prints PASS/FAIL per case.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_CHECKS="$SCRIPT_DIR/../crate-checks.sh"

TOTAL_FAIL=0

# make_fixture DIR: writes a minimal clean workspace (workspace root,
# crates/cleanflate with no deps and correct attrs, crates/dummy for local
# path-dependency fixtures) into DIR, and git-inits it so ls-files works.
make_fixture() {
    dir="$1"
    mkdir -p "$dir/crates/cleanflate/src" "$dir/crates/oracle/src" "$dir/crates/dummy/src"

    cat > "$dir/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/cleanflate", "crates/oracle", "crates/dummy"]
resolver = "3"
EOF

    cat > "$dir/crates/cleanflate/Cargo.toml" <<'EOF'
[package]
name = "cleanflate"
version = "0.1.0"
edition = "2021"
EOF

    cat > "$dir/crates/cleanflate/src/lib.rs" <<'EOF'
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

pub fn add(a: u32, b: u32) -> u32 {
    a + b
}
EOF

    cat > "$dir/crates/oracle/Cargo.toml" <<'EOF'
[package]
name = "oracle"
version = "0.1.0"
edition = "2021"
EOF
    echo "// oracle stub" > "$dir/crates/oracle/src/lib.rs"

    cat > "$dir/crates/dummy/Cargo.toml" <<'EOF'
[package]
name = "dummy"
version = "0.1.0"
edition = "2021"
EOF
    echo "// dummy stub" > "$dir/crates/dummy/src/lib.rs"

    git -C "$dir" init -q
    git -C "$dir" add -A
    git -C "$dir" -c user.email=t@example.com -c user.name=t commit -q -m init
}

run_case() {
    name="$1"
    expect="$2" # "pass" or "fail"
    dir="$3"

    out="$(CRATE_CHECKS_ROOT="$dir" bash "$CRATE_CHECKS" 2>&1)"
    code=$?

    if [ "$expect" = "pass" ]; then
        if [ "$code" -eq 0 ]; then
            echo "PASS $name"
        else
            echo "FAIL $name: expected pass, got exit $code: $out"
            TOTAL_FAIL=1
        fi
    else
        if [ "$code" -ne 0 ]; then
            echo "PASS $name"
        else
            echo "FAIL $name: expected fail, got exit 0"
            TOTAL_FAIL=1
        fi
    fi
}

# case_clean_passes
d="$(mktemp -d)"
make_fixture "$d"
run_case case_clean_passes pass "$d"
rm -rf "$d"

# case_unsafe_block_fails
d="$(mktemp -d)"
make_fixture "$d"
cat >> "$d/crates/cleanflate/src/lib.rs" <<'EOF'
pub unsafe fn bad(p: *const u8) -> u8 { 0 }
EOF
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -am upd
run_case case_unsafe_block_fails fail "$d"
rm -rf "$d"

# case_unsafe_in_comment_fails (the word still appears; the rule is a raw
# token search, so even a comment mention fails it)
d="$(mktemp -d)"
make_fixture "$d"
cat >> "$d/crates/cleanflate/src/lib.rs" <<'EOF'
// this path is unsafe for short inputs
EOF
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -am upd
run_case case_unsafe_in_comment_fails fail "$d"
rm -rf "$d"

# case_dependency_fails
d="$(mktemp -d)"
make_fixture "$d"
cat >> "$d/crates/cleanflate/Cargo.toml" <<'EOF'

[dependencies]
dummy = { path = "../dummy" }
EOF
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -am upd
run_case case_dependency_fails fail "$d"
rm -rf "$d"

# case_dev_dependency_fails
d="$(mktemp -d)"
make_fixture "$d"
cat >> "$d/crates/cleanflate/Cargo.toml" <<'EOF'

[dev-dependencies]
dummy = { path = "../dummy" }
EOF
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -am upd
run_case case_dev_dependency_fails fail "$d"
rm -rf "$d"

# case_missing_forbid_fails
d="$(mktemp -d)"
make_fixture "$d"
cat > "$d/crates/cleanflate/src/lib.rs" <<'EOF'
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

pub fn add(a: u32, b: u32) -> u32 {
    a + b
}
EOF
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -am upd
run_case case_missing_forbid_fails fail "$d"
rm -rf "$d"

# case_forbidden_path_fails
d="$(mktemp -d)"
make_fixture "$d"
mkdir -p "$d/tools/miniz-helpers"
echo "stub" > "$d/tools/miniz-helpers/notes.txt"
git -C "$d" add -A
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -m upd
run_case case_forbidden_path_fails fail "$d"
rm -rf "$d"

# case_forbidden_word_inside_crates_ok: a forbidden word inside
# crates/cleanflate or crates/oracle paths is fine (exempt dirs).
d="$(mktemp -d)"
make_fixture "$d"
mkdir -p "$d/crates/oracle/src/zlib_ffi"
echo "// stub" > "$d/crates/oracle/src/zlib_ffi/notes.rs"
git -C "$d" add -A
git -C "$d" -c user.email=t@example.com -c user.name=t commit -q -m upd
run_case case_forbidden_word_inside_crates_ok pass "$d"
rm -rf "$d"

if [ "$TOTAL_FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

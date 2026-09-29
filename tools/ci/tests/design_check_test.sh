#!/usr/bin/env bash
# Acceptance tests for tools/ci/design-check.sh.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESIGN_CHECK="$SCRIPT_DIR/../design-check.sh"

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

# case_ok
d="$(mktemp -d)"
cat > "$d/DESIGN.md" <<'EOF'
# design

### D-01: A decision

RFC: RFC 1951 S3.2.5
Decision: something.
Rationale: because.

### D-02: Another decision

RFC: n/a (API)
Decision: something else.
Rationale: because also.
EOF
out="$(DESIGN_CHECK_FILE="$d/DESIGN.md" bash "$DESIGN_CHECK" 2>&1)"
code=$?
if [ "$code" -eq 0 ]; then
    report case_ok 0
else
    report case_ok 1 "$out"
fi
rm -rf "$d"

# case_missing_rfc_line_fails
d="$(mktemp -d)"
cat > "$d/DESIGN.md" <<'EOF'
# design

### D-01: A decision

Decision: something with no RFC line.
Rationale: oops.

### D-02: Another decision

RFC: n/a (API)
Decision: this one is fine.
Rationale: because.
EOF
out="$(DESIGN_CHECK_FILE="$d/DESIGN.md" bash "$DESIGN_CHECK" 2>&1)"
code=$?
if [ "$code" -ne 0 ] && echo "$out" | grep -q "D-01"; then
    report case_missing_rfc_line_fails 0
else
    report case_missing_rfc_line_fails 1 "expected D-01 failure, got exit $code: $out"
fi
rm -rf "$d"

if [ "$TOTAL_FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

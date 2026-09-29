#!/usr/bin/env bash
# Verify every "### D-" heading in docs/DESIGN.md is followed, before the
# next heading, by a line starting "RFC:" (SPEC S3.4).
set -u

if [ -n "${DESIGN_CHECK_FILE:-}" ]; then
    FILE="$DESIGN_CHECK_FILE"
else
    ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
    FILE="$ROOT/docs/DESIGN.md"
fi

if [ ! -f "$FILE" ]; then
    echo "FAIL design-check: $FILE does not exist"
    exit 1
fi

python3 - "$FILE" <<'PYEOF'
import re
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    lines = f.readlines()

heading_re = re.compile(r"^### (D-\S+):")
perf_re = re.compile(r"^### (D-\S+): \[perf\]")
PHRASE = "general technique; not derived from any specific implementation"
ok = True
current_heading = None
current_has_rfc = False
current_perf = False
has_measurement = False
has_phrase = False

def check_close(heading, has_rfc):
    global ok
    if heading is not None and not has_rfc:
        print(f"FAIL design-check: {heading} has no RFC: line before the next heading")
        ok = False
    if heading is not None and current_perf:
        if not has_measurement:
            print(f"FAIL design-check: {heading} [perf] block has no Measurement: line")
            ok = False
        if not has_phrase:
            print(f"FAIL design-check: {heading} [perf] block lacks the phrase '{PHRASE}'")
            ok = False

for line in lines:
    m = heading_re.match(line)
    if m:
        check_close(current_heading, current_has_rfc)
        current_heading = m.group(1)
        current_has_rfc = False
        current_perf = bool(perf_re.match(line))
        has_measurement = False
        has_phrase = False
        continue
    if current_heading is not None:
        if line.startswith("Measurement:"):
            has_measurement = True
        if PHRASE in line:
            has_phrase = True
    if current_heading is not None and line.startswith("RFC:"):
        current_has_rfc = True

check_close(current_heading, current_has_rfc)

if current_heading is None:
    print("FAIL design-check: no '### D-' headings found")
    ok = False

sys.exit(0 if ok else 1)
PYEOF

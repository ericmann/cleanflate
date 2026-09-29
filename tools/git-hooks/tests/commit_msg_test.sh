#!/usr/bin/env bash
# Acceptance tests for tools/git-hooks/commit-msg.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK="$SCRIPT_DIR/../commit-msg"

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

run_hook() {
    # run_hook REPO_ROOT MSG_FILE
    (cd "$1" && bash "$HOOK" "$2")
}

# case_task_subject_tagged_implement
d="$(mktemp -d)"
git -C "$d" init -q
f="$d/msg.txt"
echo "P1-04: Adler-32 and CRC-32 in cleanflate" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ] && grep -q '\[stage:implement\] \[task:P1-04\]' "$f"; then
    report case_task_subject_tagged_implement 0
else
    report case_task_subject_tagged_implement 1 "$(cat "$f")"
fi
rm -rf "$d"

# case_round1_tagged_fix
d="$(mktemp -d)"
git -C "$d" init -q
mkdir -p "$d/.foundry"
echo '{"round": 1}' > "$d/.foundry/state.json"
f="$d/msg.txt"
echo "P1-04: fix something" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ] && grep -q '\[stage:fix\] \[task:P1-04\]' "$f"; then
    report case_round1_tagged_fix 0
else
    report case_round1_tagged_fix 1 "$(cat "$f")"
fi
rm -rf "$d"

# case_plan_subject
d="$(mktemp -d)"
git -C "$d" init -q
f="$d/msg.txt"
echo "plan: derive build plan from SPEC" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ] && grep -q '\[stage:plan\] \[task:plan\]' "$f"; then
    report case_plan_subject 0
else
    report case_plan_subject 1 "$(cat "$f")"
fi
rm -rf "$d"

# case_review_subject
d="$(mktemp -d)"
git -C "$d" init -q
f="$d/msg.txt"
echo "review: round 1 findings" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ] && grep -q '\[stage:review\] \[task:review\]' "$f"; then
    report case_review_subject 0
else
    report case_review_subject 1 "$(cat "$f")"
fi
rm -rf "$d"

# case_progress_subject_with_id
d="$(mktemp -d)"
git -C "$d" init -q
f="$d/msg.txt"
echo "progress: P0-02 done" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ] && grep -q '\[stage:implement\] \[task:P0-02\]' "$f"; then
    report case_progress_subject_with_id 0
else
    report case_progress_subject_with_id 1 "$(cat "$f")"
fi
rm -rf "$d"

# case_already_tagged_unchanged
d="$(mktemp -d)"
git -C "$d" init -q
f="$d/msg.txt"
printf 'P0-01: something\n\n[stage:implement] [task:P0-01]\n' > "$f"
before="$(cat "$f")"
run_hook "$d" "$f"
code=$?
after="$(cat "$f")"
if [ "$code" -eq 0 ] && [ "$before" = "$after" ]; then
    report case_already_tagged_unchanged 0
else
    report case_already_tagged_unchanged 1 "message changed"
fi
rm -rf "$d"

# case_no_state_file_ok
d="$(mktemp -d)"
git -C "$d" init -q
f="$d/msg.txt"
echo "P2-01: something" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ] && grep -q '\[stage:implement\] \[task:P2-01\]' "$f"; then
    report case_no_state_file_ok 0
else
    report case_no_state_file_ok 1 "$(cat "$f")"
fi
rm -rf "$d"

# case_hook_exit_zero_on_unreadable_state
d="$(mktemp -d)"
git -C "$d" init -q
mkdir -p "$d/.foundry"
echo "not json at all {{{" > "$d/.foundry/state.json"
f="$d/msg.txt"
echo "P2-02: something" > "$f"
run_hook "$d" "$f"
code=$?
if [ "$code" -eq 0 ]; then
    report case_hook_exit_zero_on_unreadable_state 0
else
    report case_hook_exit_zero_on_unreadable_state 1 "exit $code"
fi
rm -rf "$d"

if [ "$TOTAL_FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

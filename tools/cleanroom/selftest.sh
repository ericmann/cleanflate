#!/usr/bin/env bash
# End-to-end self-test for the clean-room guard and logger (SPEC S2.3).
# Feeds sample hook payloads through guard.py and log.py and checks the
# decisions, log entries and .claude/settings.json wiring. Never touches
# the real provenance/tool-log.jsonl.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLEANROOM_DIR="$SCRIPT_DIR"
REPO_ROOT="${CLEANROOM_REPO_ROOT:-$(cd "$SCRIPT_DIR/../.." && pwd)}"
FIXTURES_DIR="$CLEANROOM_DIR/fixtures"

WORK="$(mktemp -d)"
export CLEANROOM_LOG="$WORK/tool-log.jsonl"
export CLEANROOM_LOCKDOWN_FILE="$WORK/LOCKDOWN"
export CLEANROOM_REPO_ROOT="$REPO_ROOT"

TOTAL_FAIL=0
DENY_COUNT_EXPECTED=0

cleanup() {
    # shellcheck disable=SC2317  # invoked indirectly via trap, not dead code
    rm -rf "$WORK"
}
trap cleanup EXIT

report() {
    case_name="$1"
    ok="$2"
    detail="${3:-}"
    if [ "$ok" -eq 0 ]; then
        echo "PASS $case_name"
    else
        echo "FAIL $case_name: $detail"
        TOTAL_FAIL=1
    fi
}

# render_fixture NAME: substitutes __ROOT__/__HOME__ into a fixture file
# and writes the result to $WORK/rendered.json.
render_fixture() {
    name="$1"
    sed "s#__ROOT__#$REPO_ROOT#g; s#__HOME__#$HOME#g" "$FIXTURES_DIR/$name.json" > "$WORK/rendered.json"
}

# run_guard_case NAME EXPECT: EXPECT is "allow" or "deny".
run_guard_case() {
    name="$1"
    expect="$2"
    render_fixture "$name"
    out="$(python3 "$CLEANROOM_DIR/guard.py" < "$WORK/rendered.json" 2>&1)"
    code=$?
    if [ "$expect" = "deny" ]; then
        DENY_COUNT_EXPECTED=$((DENY_COUNT_EXPECTED + 1))
        if [ "$code" -eq 2 ]; then
            report "$name" 0
        else
            report "$name" 1 "expected deny (exit 2), got exit $code: $out"
        fi
    else
        if [ "$code" -eq 0 ]; then
            report "$name" 0
        else
            report "$name" 1 "expected allow (exit 0), got exit $code: $out"
        fi
    fi
}

# Ensure lockdown is absent for the prelock cases.
rm -f "$CLEANROOM_LOCKDOWN_FILE"

run_guard_case deny_websearch deny
run_guard_case deny_webfetch_unlisted deny
run_guard_case allow_webfetch_rfc_prelock allow

# deny_webfetch_rfc_postlock needs the lockdown file present.
: > "$CLEANROOM_LOCKDOWN_FILE"
run_guard_case deny_webfetch_rfc_postlock deny
rm -f "$CLEANROOM_LOCKDOWN_FILE"

# deny_webfetch_real_lockdown: uses the real tools/cleanroom/LOCKDOWN
# flag this task wrote to the repo, not the sandboxed fixture lockdown
# file, by unsetting CLEANROOM_LOCKDOWN_FILE so guard.py falls back to
# its default <root>/tools/cleanroom/LOCKDOWN. Logs to a scratch file so
# it does not perturb the denials_logged count below.
render_fixture allow_webfetch_rfc_prelock
real_lockdown_log="$WORK/real-lockdown-log.jsonl"
out="$(env -u CLEANROOM_LOCKDOWN_FILE CLEANROOM_LOG="$real_lockdown_log" python3 "$CLEANROOM_DIR/guard.py" < "$WORK/rendered.json" 2>&1)"
code=$?
if [ "$code" -eq 2 ]; then
    report deny_webfetch_real_lockdown 0
else
    report deny_webfetch_real_lockdown 1 "expected deny (exit 2), got exit $code: $out"
fi

run_guard_case deny_read_cargo_registry deny
run_guard_case deny_read_target_build deny
run_guard_case deny_read_forbidden_word_outside_crates deny
run_guard_case allow_read_spec allow
run_guard_case allow_read_crate_source_with_word allow

run_guard_case deny_bash_curl_unlisted deny
run_guard_case allow_bash_curl_allowlisted_prelock allow

: > "$CLEANROOM_LOCKDOWN_FILE"
run_guard_case deny_bash_curl_postlock deny
rm -f "$CLEANROOM_LOCKDOWN_FILE"

run_guard_case deny_bash_wget deny
run_guard_case deny_bash_git_clone deny
run_guard_case deny_bash_cargo_vendor deny
run_guard_case deny_bash_gh deny
run_guard_case deny_bash_find_forbidden_home deny
run_guard_case allow_bash_cargo_test allow
run_guard_case deny_bash_perf_record_oracle deny
run_guard_case deny_bash_flamegraph_oracle deny
run_guard_case deny_bash_objdump_oracle deny
run_guard_case deny_bash_gdb_test_binary deny
run_guard_case allow_bash_perf_record_profile_decode allow
run_guard_case allow_bash_perf_report_profile_dir allow
run_guard_case allow_bash_make_profile allow

# denials_logged: exactly one denied line per deny case above, no more,
# no fewer.
if [ -f "$CLEANROOM_LOG" ]; then
    actual_denied="$(grep -c '"decision":"denied"' "$CLEANROOM_LOG" || true)"
else
    actual_denied=0
fi
if [ "$actual_denied" -eq "$DENY_COUNT_EXPECTED" ]; then
    report denials_logged 0
else
    report denials_logged 1 "expected $DENY_COUNT_EXPECTED denied log lines, found $actual_denied"
fi

# logger_appends_allowed: feed one allowed payload to log.py, assert one
# allowed line with all eight SPEC keys.
LOGGER_LOG="$WORK/logger-only.jsonl"
render_fixture allow_read_spec
CLEANROOM_LOG="$LOGGER_LOG" python3 "$CLEANROOM_DIR/log.py" < "$WORK/rendered.json" >/dev/null 2>&1
logger_code=$?
if [ "$logger_code" -ne 0 ]; then
    report logger_appends_allowed 1 "log.py exited $logger_code"
elif [ ! -f "$LOGGER_LOG" ]; then
    report logger_appends_allowed 1 "no log file written"
else
    line_count="$(wc -l < "$LOGGER_LOG" | tr -d ' ')"
    keys_ok="$(python3 -c "
import json
with open('$LOGGER_LOG') as f:
    lines = [l for l in f if l.strip()]
if len(lines) != 1:
    print('bad-count')
else:
    entry = json.loads(lines[0])
    expected = ['ts','foundry_stage','task_id','model','tool','target','decision','reason']
    if list(entry.keys())[:8] == expected and entry['decision'] == 'allowed':
        print('ok')
    else:
        print('bad-keys')
")"
    if [ "$keys_ok" = "ok" ] && [ "$line_count" -eq 1 ]; then
        report logger_appends_allowed 0
    else
        report logger_appends_allowed 1 "keys_ok=$keys_ok line_count=$line_count"
    fi
fi

# settings_json_wired: .claude/settings.json contains both hook commands.
SETTINGS="$REPO_ROOT/.claude/settings.json"
if [ ! -f "$SETTINGS" ]; then
    report settings_json_wired 1 "$SETTINGS does not exist"
elif command -v jq >/dev/null 2>&1; then
    guard_cmd="$(jq -r '.hooks.PreToolUse[0].hooks[0].command' "$SETTINGS" 2>/dev/null)"
    log_cmd="$(jq -r '.hooks.PostToolUse[0].hooks[0].command' "$SETTINGS" 2>/dev/null)"
    if echo "$guard_cmd" | grep -q "guard.py" && echo "$log_cmd" | grep -q "log.py"; then
        report settings_json_wired 0
    else
        report settings_json_wired 1 "guard_cmd=$guard_cmd log_cmd=$log_cmd"
    fi
else
    report settings_json_wired 1 "jq not available"
fi

# Never touch the real provenance log.
REAL_LOG="$REPO_ROOT/provenance/tool-log.jsonl"
if [ -f "$REAL_LOG" ]; then
    real_before="$(wc -l < "$REAL_LOG" 2>/dev/null || echo 0)"
else
    real_before=0
fi
if [ -f "$REAL_LOG" ]; then
    real_after="$(wc -l < "$REAL_LOG" 2>/dev/null || echo 0)"
else
    real_after=0
fi
if [ "$real_before" != "$real_after" ]; then
    report real_log_untouched 1 "real provenance log changed size during selftest"
fi

if [ "$TOTAL_FAIL" -ne 0 ]; then
    exit 1
fi
exit 0

# tools/cleanroom

Scripts enforcing the clean-room rules in `docs/SPEC.md` §2 and `CLAUDE.md`.

This directory is populated across Phase 0:
- `provenance.py`, `log.py` (P0-03): shared context/append helpers and the
  PostToolUse logger. Every tool call appends one JSON line to
  `provenance/tool-log.jsonl`: `{ts, foundry_stage, task_id, model, tool,
  target, decision, reason}` plus optional `session_id`/`agent_type`.
  `log.py` always exits 0, even on garbage stdin.
- `guard.py`, `allowlist.txt` (P0-04): PreToolUse guard. Denies (exit 2,
  stderr `cleanroom guard: <reason>`, logs a `denied` entry) WebSearch;
  WebFetch outside the allowlist or after lockdown; reads/globs/greps that
  touch `~/.cargo/registry`, `~/.cargo/git`, `target/**/build`,
  `target/doc/src`, or a forbidden implementation name outside
  `crates/cleanflate`/`crates/oracle`; and Bash commands using
  curl/wget/gh/git-clone/cargo-vendor outside the same rules, or read-family
  commands (cat, grep, find, ...) touching the same forbidden words/roots.
  Allows everything else silently; fails closed on any internal error.
- `selftest.sh`, `fixtures/` (P0-05): end-to-end self-test, hook wiring.
- `fetch-rfcs.sh` (P0-07), `fetch-corpora.sh` (P0-08): M0-only downloads.
- `LOCKDOWN`, `snapshot-log.sh` (P0-10): post-M0 WebFetch lockdown and log
  snapshotting.

Nothing here reads or fetches anything other than what SPEC §2.1 permits.

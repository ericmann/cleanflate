# provenance

Machine-generated record of every tool call the pipeline made, proving what
was and was not consulted while building `cleanflate`. Populated starting in
Phase 0 (P0-03 onward); see that section of `docs/PLAN.md` for the schema.

## Entry schema

Every line of `tool-log.jsonl` is one JSON object with exactly these keys,
in this order (`tools/cleanroom/provenance.py::ENTRY_KEYS`):

| key            | source |
|----------------|--------|
| `ts`           | `provenance.now_iso()` — UTC ISO-8601, millisecond precision, `Z` suffix, at the moment the hook ran. |
| `foundry_stage`| `provenance.derive_stage()` — first match of: the calling agent's `agent_type` (planner/implementer/reviewer/summarizer, with round >= 1 implementer calls becoming `fix`), else the `FOUNDRY_STAGE` env var, else `fix`/`implement` inferred from `.foundry/implement.lock` + the round recorded in `.foundry/state.json`, else `null`. |
| `task_id`      | `provenance.derive_task_id()` — the `FOUNDRY_TASK_ID` env var (implement/fix stages only), else the first `- [~]` task line in `docs/PROGRESS.md`, else `null`. |
| `model`        | `provenance.derive_model()` — the hook payload's `model` field, else `CLEANROOM_MODEL` env, else a best-effort tail scan of the session transcript for the last `message.model`, else `null`. |
| `tool`         | the hook's `tool_name` (`Bash`, `Read`, `WebFetch`, ...) verbatim. |
| `target`       | `provenance.target_of()` — the command string for `Bash`, the path/URL/pattern for `Read`/`Glob`/`Grep`/`WebFetch`, truncated to a bounded length. |
| `decision`     | `"allowed"` or `"denied"`. |
| `reason`       | `null` for allowed calls; a short string for denied calls (why `guard.py` refused). |

`guard.py` (PreToolUse) only ever appends `"denied"` entries, at the point
it refuses a call. `log.py` (PostToolUse) appends one `"allowed"` entry for
every tool call that was not denied, after it ran.

## Live log vs snapshot

- `tool-log.jsonl` (gitignored) is the live, append-only log every hook
  invocation writes to during a session. It grows for as long as the
  session runs.
- `tool-log.snapshot.jsonl` (committed) is a point-in-time copy made by
  `make provenance-snapshot` (`tools/cleanroom/snapshot-log.sh`), taken
  deliberately at milestones so the provenance record survives even
  though the live log itself is gitignored. It never edits or filters
  entries, only copies them verbatim.

## Guard rules summary

`tools/cleanroom/guard.py` runs as the PreToolUse hook and denies, before
the tool executes:

- R1: `WebSearch`, always.
- R2: `WebFetch` to any URL not in `tools/cleanroom/allowlist.txt`, and
  (once `tools/cleanroom/LOCKDOWN` exists) `WebFetch` entirely.
- R3: `Read`/`Glob`/`Grep` under `~/.cargo/registry`, `~/.cargo/git`,
  `$CARGO_HOME/registry`, `target/**/build` or `target/doc/src`, or
  whose path/pattern names a forbidden reference implementation, outside
  `crates/cleanflate` and `crates/oracle`.
- R4: `Bash` commands that `git clone`, `cargo vendor`/`download`, call
  `gh`, `curl`/`wget` to a non-allowlisted URL (or at all, once
  `LOCKDOWN` exists), invoke `fetch-rfcs.sh`/`fetch-corpora.sh` post-lockdown,
  reference a forbidden root directly, or run a read-family command
  (`cat`, `grep`, `find`, ...) whose arguments name a forbidden
  implementation or recursively `grep` the repo root without
  `--exclude-dir=target`.
- R5: any exception inside the guard denies the call and logs
  `"guard internal error: <exc>"` — fail closed.

See `docs/SPEC.md` §2.2–§2.3 and `CLAUDE.md`'s clean-room rules (CR1–CR6)
for the normative versions; this is a summary for readers of the log.

## Known gaps

- The plan stage ran before this harness existed, so no plan-stage tool
  calls appear in the log.
- Hooks are snapshotted by Claude Code at session start. A session that
  installs or edits `.claude/settings.json` is not itself covered by the
  new hooks until the *next* session/restart.
- Commands run inside MCP servers, or as subprocesses of a `make` target
  (e.g. a script a Makefile target shells out to, once it's already past
  the PreToolUse boundary), are not individually seen by the guard —
  only the top-level `Bash` invocation is.
- Bypassing the guard via an interpreter escape hatch (e.g. `python3 -c
  "import urllib.request; ..."` to fetch a URL) is not blocked; the guard
  pattern-matches `curl`/`wget`/`gh` and known read-family commands, not
  arbitrary interpreter code.
- Commits made with `git commit --no-verify` skip the commit-msg hook
  that tags commits with `[stage:...] [task:...]`.
- `model` can be `null` when none of `derive_model()`'s sources have a
  value (e.g. a hook payload without `model` and no readable transcript).

## Harness status during build

`provenance/tool-log.jsonl` exists and has entries with `ts` timestamps
after the P0-05 commit (`0593469`, 2026-09-29T07:47:10-07:00) that
installed `.claude/settings.json`. Status: **LIVE**.

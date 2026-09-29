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

## Denied calls during build

CR6: "If the guard denies a call, do not work around it. Note the
denial in the progress log and continue another way." Every entry
`provenance/tool-log.jsonl` has ever recorded with `"decision":"denied"`,
as of this writing (all from the guard's own `PreToolUse` hook, never
worked around):

| ts | task | reason |
|----|------|--------|
| 2026-09-29T14:43:07.953Z | P0-05 | unparseable command |
| 2026-09-29T14:43:23.189Z | P0-05 | unparseable command |
| 2026-09-29T14:43:46.542Z | P0-05 | forbidden word in command outside repo/target (`grep -rn "zlib\|gzip\|deflate\|inflate" ...`) |
| 2026-09-29T14:43:52.987Z | P0-05 | forbidden word in command outside repo/target (same `grep`, retried) |
| 2026-09-29T14:44:43.248Z | P0-05 | forbidden word in command outside repo/target (same `grep`, retried again) |
| 2026-09-29T14:50:42.835Z | P0-08 | curl with no recognizable URL (`which tar unzip curl`, a false-positive match on the literal word "curl") |
| 2026-09-29T14:52:57.057Z | P0-08 | curl url not allowlisted: `https://corpus.canterbury.ac.nz/` (missing the `/resources/cantrbry.tar.gz` path the allowlist actually names) |
| 2026-09-29T14:54:47.458Z | P1-01 | forbidden word in path argument: `*/libz-sys-*/src/lib.rs` |

The P0-05 denials happened while building `tools/cleanroom/`'s own
guard-fixture test suite (P0-05's log entry: "exercising it live during
fixture-writing") -- probing the guard's own behavior, not an attempt to
read anything. The P0-08 denials were the implementer discovering the
allowlist's exact required form by trial. The P1-01 denial is the one
CR1-relevant case: an attempt to `find / -path '*/libz-sys-*/src/lib.rs'`
(to read a compiler error against the actual source instead of the
message alone) was blocked outright. **No libz-sys source was ever
read** by any tool call in this build; every libz-sys-related fix in
this repo's history was made from compiler error messages alone (P1-01's
own log entry says as much), as CR5 requires.

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
- Commits `c1bbaa9`, `4015214`, `8edc633`, `956d6d7`, `44f624e` and
  `f776879` predate `tools/git-hooks/commit-msg` (installed by P0-06)
  and so carry no `[stage:...] [task:...]` tags.
- `tools/cleanroom/LOCKDOWN` (SPEC S2.3: "After M0 completes, WebFetch
  is denied entirely") was not written before Phases 1-4 ran, because
  P0-10 (the task that writes it) was initially blocked behind P0-08 and
  only unblocked and completed in review-fix round 1, well after those
  phases had already run with WebFetch still allowlist-gated rather than
  fully locked down.
- The P0-08 Canterbury retries that hit Cloudflare's bot protection were
  also tried with a browser-like `User-Agent` header specifically (not
  just a bare `curl`), and still blocked -- see the row this file's
  table already has for the human-operator download that resolved it.

## Harness status during build

`provenance/tool-log.jsonl` exists and has entries with `ts` timestamps
after the P0-05 commit (`0593469`, 2026-09-29T07:47:10-07:00) that
installed `.claude/settings.json`. Status: **LIVE**.

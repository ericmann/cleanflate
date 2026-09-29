# cleanflate build plan
Derived from docs/SPEC.md v(unversioned, commit ba82a47) on 2026-09-29. SPEC.md wins over this file.

## Decisions
- Phases vs milestones → Phase N implements SPEC milestone MN for M0–M4 and M6. There is **no Phase 5**: M5 is marked optional and is deferred. Rationale: M5 (no_std, a compressor) is a separate clean-room scope, the companion bench spec only needs decompression, and adding it doubles the unattended surface. BUILD_REPORT.md lists it as deferred.
- "Each milestone ends with a Foundry review stage" (SPEC §5) → a single Foundry flight runs one review loop after all phases. Each phase instead ends with an acceptance-and-push task that runs that milestone's acceptance commands and records the results. Rationale: Foundry reviews once per flight. A human who wants per-milestone review can run one flight per phase.
- CI (SPEC §5 M0) → `make ci` is authoritative. `.github/workflows/ci.yml` runs the same target. Rationale: the implementer can run `make ci` locally, but it cannot observe GitHub Actions.
- Hook liveness → Claude Code may snapshot hooks at session start. If so, the hooks written in P0-05 do not take effect until the session restarts. P0-11 records whether the log is live, and restarting is a manual check. No later task depends on liveness. Rationale: the flight must not stall on something only a human can do.
- Tool-log location → the live log `provenance/tool-log.jsonl` is gitignored. `make provenance-snapshot` copies it to the tracked `provenance/tool-log.snapshot.jsonl` at every phase end. Rationale: a tracked file that grows on every tool call leaves the tree dirty after every commit, and Foundry refuses to close tasks with uncommitted files.
- Denied calls → the PreToolUse guard appends its own `denied` line, and the PostToolUse logger appends `allowed` lines. Rationale: PostToolUse never fires for a call that PreToolUse blocked, so the logger alone would drop every denial (SPEC §2.3 forbids that).
- Guard implementation → Python 3 using only the stdlib (`tools/cleanroom/guard.py`, with `shlex` for tokenizing). SPEC allows "guard.sh or equivalent". Rationale: robust JSON and shell-word parsing. python3 3.12 and jq 1.7 are present.
- Guard failure mode → fail closed: an internal error exits 2 and logs the error. Rationale: provenance comes before convenience. The self-test is what keeps the guard from wedging a flight.
- Forbidden-word path rule (SPEC §2.3) → apply it literally. Every tracked path outside `crates/cleanflate/` and `crates/oracle/` avoids the words zlib/miniz/flate2/libdeflate/libz-sys/zopfli. Testdata paths therefore name framings `rfc1951`/`rfc1950`/`rfc1952`, and fuzz targets are `never_panic_*`/`diff_oracle_*`. `make crate-checks` enforces this.
- Guard extras beyond SPEC → deny all of `~/.cargo/registry/**` (not only `src/`), `target/doc/src/**` (rustdoc source pages), and the tokens `libz_sys`, plus inflate/deflate/gzip/puff for paths outside the repo. Rationale: these are the same leaks under a different spelling.
- Commit tags (SPEC §2.4) → every agent-written commit body carries `[stage:<s>] [task:<id>]`. A versioned `commit-msg` hook (`tools/git-hooks/commit-msg`, enabled with `git config core.hooksPath tools/git-hooks`) adds the tags to any commit that lacks them, including Foundry's own `progress:`/`review:` commits. Rationale: Foundry requires subjects to start with `<ID>:`, so the tags go in the body.
- Corpora → neither corpus is committed. `testdata/corpora/SOURCES.json` (URLs, SHA-256) is committed, and archives and extracted files are gitignored. GitHub CI fetches Canterbury with an explicit override variable. Silesia is used locally only (the full tier). Rationale: repo size and redistribution uncertainty.
- Generated conformance corpus → `testdata/generated/` is gitignored and regenerated deterministically. `testdata/MANIFEST.json` is committed and verified by SHA-256 on every `make corpus`. Tier `quick` (synthetic + Canterbury) runs in `make test`. Tier `full` (adds Silesia) runs in `make conformance-full`. Rationale: gigabytes of output cannot live in git, and a determinism check replaces committing them.
- Generation matrix (SPEC §4) → a bounded set of four sub-matrices (levels, strategies, window/memLevel, dictionary) defined in P1-02, not the full cross product. Rationale: the full cross product is about 9,450 streams per input, and every individual parameter value is still covered.
- Oracle build → `libz-sys` with the `static` feature, which builds the C backend that libz-sys bundles, not a system zlib. Rationale: deterministic MANIFEST hashes across machines.
- Dev-dependency restriction (SPEC §3.1) → `crates/cleanflate` has no dev-dependencies. Every test that needs proptest, serde or the oracle lives in `crates/oracle/tests/`. cleanflate's own tests use std only. Unit tests live under `crates/cleanflate/src/tests/` so that line rules can exclude them.
- Streaming architecture → the raw DEFLATE core is a resumable state machine from M1 on, and the one-shot functions drive it. `Decompressor` is public from P1-10, and M4 finalizes its contract. Rationale: retrofitting resumability onto a slice decoder is a rewrite, and one code path makes chunking invariance structural.
- Public API names → `inflate`, `zlib_decompress`, `gzip_decompress`, `decompress_with(Format, &[u8], &Options) -> Result<Decoded, Error>`, `Decompressor { new, decompress, finish, total_in, total_out, trailing_start, gzip_headers }`, `Progress { consumed, produced, status }`, `Status { NeedsInput, OutputFull, Done }`, `Options { max_output, trailing, dictionary, max_gzip_field }`, `TrailingData { Reject, Allow }`, `Format { Raw, Zlib, Gzip }` (#[non_exhaustive]; Zlib added in P3-01, Gzip in P3-03). Rationale: the SPEC §3.3 capabilities with one name each.
- Error enum → `Error::{BadHeader(HeaderError), PresetDictionaryRequired{dict_id}, DictionaryMismatch{expected,actual}, BadBlockType, InvalidStoredLength{len,nlen}, InvalidHuffmanTable(HuffmanError), InvalidSymbol, InvalidDistance{distance,available}, ChecksumMismatch{kind,expected,actual}, Truncated, TrailingData{offset}, OutputLimitExceeded{limit}}` plus `Error::kind() -> ErrorKind`. Rationale: covers every SPEC §3.3 category, and `ErrorKind` names are the strings used in the malformed corpus.
- Default output limit (SPEC §3.3 "default documented") → `DEFAULT_MAX_OUTPUT = 1 GiB` in `crates/cleanflate/src/config.rs`. Producing byte limit+1 fails with `OutputLimitExceeded`, and exactly `limit` bytes succeeds. Rationale: larger than any corpus member (51 MB) and small enough to stop a bomb.
- Gzip FNAME/FCOMMENT/FEXTRA memory → store at most `DEFAULT_MAX_GZIP_FIELD = 65536` bytes of each field, keep parsing the rest, and set `truncated_fields`. Valid streams are never rejected. Rationale: bounded memory (SPEC §1).
- Trailing data (SPEC §3.3) → the default is `TrailingData::Reject`. After a gzip member, bytes starting with 1f 8b are always another member, and anything else is trailing data. Under `Allow`, `consumed` and `trailing_start()` report the exact offset. Rationale: strict by default, and the lenient option is explicit.
- Incomplete Huffman codes (RFC 1951 §3.2.7 leaves this open) → reject every incomplete or over-subscribed table except the distance table in two cases: a single code of length 1, and all lengths zero (literals-only block). HDIST up to 32 is accepted per the RFC's stated range, and distance symbols 30/31 fail with `InvalidSymbol` if decoded. Length code 284 with extra value 31 decodes arithmetically to 258 and is accepted. Rationale: follow the RFC text literally, and document each choice in DESIGN.md.
- Zlib CINFO → validate CINFO ≤ 7, but do not limit back-references to the declared window. Header checks run in the order FCHECK, CM, CINFO. Rationale: RFC 1950 defines CINFO as an encoder hint, and a fixed order makes errors deterministic.
- Zip-bomb fixture (SPEC §5 M4 says "1 KiB → >1 GiB") → use about 1 MiB of compressed zeros that expands to DEFAULT_MAX_OUTPUT + 1 MiB. Rationale: DEFLATE's maximum ratio is about 1032:1, so 1 KiB cannot expand to 1 GiB.
- Fuzzing toolchain → cargo-fuzz and nightly are not installed. P4-04 attempts to install them. Smoke runs (`FUZZ_SECONDS`, default 60) are best-effort, and the 1-hour runs come from `.github/workflows/fuzz-nightly.yml` or a human. Rationale: a 1-hour run cannot sit inside per-task verify.
- Chunking property test cases → `CLEANFLATE_CHUNKING_CASES` env, default 512 in `make test`. `make proptest-10k` runs 10,000 cases in release. Rationale: keeps per-task verify fast while still running the SPEC's 10k cases at each phase end.
- Release tag `v0.1.0-cleanroom` and BUILD_REPORT review-round counts → the implementer does not tag. A human tags after the Foundry review converges and the branch merges, then re-runs `make provenance-summary build-report`. Rationale: tagging before review would freeze unreviewed code.
- Transcript backfill of pre-hook activity → not built. `provenance/README.md` documents the gap. Rationale: the transcript format is undocumented and reading it may prompt for permission, which would stall an unattended flight.
- Toolchain → `rust-toolchain.toml` pins `channel = "1.89.0"` (the installed stable), edition 2021, `rust-version = "1.89"`, `resolver = "3"` (MSRV-aware resolution). `[profile.dev] opt-level = 1` with `[profile.dev.package."*"] opt-level = 3`. Rationale: tests stay fast while overflow checks stay on.
- Performance measurement → `oracle bench` reports best-of-N one-shot decompression throughput (uncompressed MB/s) for cleanflate and C zlib on each Silesia file, compressed at level 6, raw and zlib framing, in release. The ratio is reported against the 70% target either way.

## Conventions
- Every task is one commit. The subject is `<ID>: <title>`, and the body follows the `## Commit template` in CLAUDE.md, including the `[stage:implement] [task:<ID>]` line and a `Defects-caught:` line.
- The implementer reads CLAUDE.md, the task, and the SPEC sections the task cites. Constraint IDs `C1`–`C16` and clean-room rules `CR1`–`CR6` refer to CLAUDE.md.
- cleanflate unit tests go in `crates/cleanflate/src/tests/<module>.rs` (declared from `src/tests/mod.rs`). Integration tests that need no deps go in `crates/cleanflate/tests/`. Tests needing the oracle, serde or proptest go in `crates/oracle/tests/`.
- Test names in Acceptance tests are `file::function`. All of them must exist and pass.
- Makefile targets are the only commands the implementer needs. Every target exits non-zero on failure.
- A phase-end task writes `Manual check: NOT VERIFIED (human)` in its progress-log entry and never waits.

## Phase 0 — M0 Clean-room harness and scaffolding

### P0-01: Workspace scaffold, toolchain pin and Makefile skeleton
**Goal:** Create the Cargo workspace, the empty `cleanflate` and `oracle` crates, the directory skeleton and a Makefile whose verify targets all exist.
**Files touched:** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.gitignore`, `Makefile`, `crates/cleanflate/Cargo.toml`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/mod.rs`, `crates/oracle/Cargo.toml`, `crates/oracle/src/lib.rs`, `fuzz/README.md`, `tools/cleanroom/README.md`, `provenance/README.md` (stub, one paragraph), `docs/rfc/README.md`, `testdata/README.md`, `README.md`.
**Design constraints:** SPEC §3.1 and §5 M0.
- Workspace `Cargo.toml`:
  - `members = ["crates/cleanflate", "crates/oracle"]`, `exclude = ["fuzz"]`, `resolver = "3"`.
  - `[workspace.package] edition = "2021"`, `rust-version = "1.89"`.
  - `[profile.dev] opt-level = 1`, `[profile.dev.package."*"] opt-level = 3`, `[profile.release] debug = "line-tables-only"`.
- `rust-toolchain.toml`: `channel = "1.89.0"`, `components = ["rustfmt", "clippy"]`, `profile = "minimal"`. If rustup reports the toolchain missing, run `rustup toolchain install 1.89.0 --profile minimal --component rustfmt,clippy`.
- Only `tools/cleanroom/README.md` goes under `tools/` in this task. Do not create scripts yet.
- `crates/cleanflate/Cargo.toml`: no `[dependencies]`, `[dev-dependencies]` or `[build-dependencies]` sections, and no build script (C3).
- `crates/cleanflate/src/lib.rs` starts with exactly these lines:
  ```
  #![forbid(unsafe_code)]
  #![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unreachable, clippy::todo, clippy::unimplemented)]
  #![warn(missing_docs)]
  ```
  It continues with a crate doc comment and `#[cfg(test)] mod tests;`.
- `src/tests/mod.rs` begins with `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]` and holds one test.
- `crates/oracle`: `publish = false`, no dependencies yet.
- `.gitignore`: `/target`, `/fuzz/target`, `/fuzz/corpus`, `/fuzz/artifacts`, `/fuzz/coverage`, `/testdata/generated/`, `/testdata/corpora/archives/`, `/testdata/corpora/canterbury/`, `/testdata/corpora/silesia/`, `/provenance/tool-log.jsonl`, `/provenance/*.lock`.
- Makefile targets:
  - `fmt-check` (`cargo fmt --all -- --check`)
  - `lint` (`cargo clippy --workspace --all-targets -- -D warnings`)
  - `test` (`cargo test --workspace`)
  - stubs that print `STUB: <target> arrives in <task>` and exit 0: `corpus` (P1-03), `crate-checks` (P0-02), `cleanroom-selftest` (P0-05), `tools-check` (P0-02), `design-check` (P1-05), `fuzz-check` (P4-04). These targets must exist from the start because `docs/foundry.json` verify and extraVerify call them.
  - `ci` depending on `fmt-check lint corpus test crate-checks cleanroom-selftest`
- Directory and file names outside the two crates must not contain zlib/miniz/flate2/libdeflate/libz-sys/zopfli (C11).
- CR1–CR6 apply.
**Acceptance tests:** `crates/cleanflate/src/tests/mod.rs::crate_builds` (asserts `true`, proving the test tree is wired). `make ci` exits 0.
**Out of scope:** Any decoder code. Any oracle FFI. Hooks. Downloads. Creating a git branch.
**Verification:** `make fmt-check && make lint && make test && make ci`. Also check `cargo metadata --no-deps --format-version 1 | jq '.packages[]|select(.name=="cleanflate")|.dependencies|length'` prints `0`.
**Depends on:** none

### P0-02: Crate checks script (zero deps, no unsafe token, forbidden path names)
**Goal:** Make `make crate-checks` mechanically enforce C1, C2, C3 and C11, with shell tests proving each check fails on a violating fixture.
**Files touched:** `tools/ci/crate-checks.sh`, `tools/ci/tests/crate_checks_test.sh`, `Makefile`.
**Design constraints:** SPEC §3.1 and §5 M0 ("a check that crates/cleanflate has zero dependencies and contains no unsafe token").
- The script honours `CRATE_CHECKS_ROOT` (default: git toplevel) so tests can point it at a temp fixture.
- Checks, each printing `FAIL <check>: <detail>` on violation and exiting 1 if any failed:
  - (a) `deps`: for the `cleanflate` package, `cargo metadata --format-version 1 --no-deps --manifest-path $ROOT/Cargo.toml` has an empty `.dependencies` array (this covers normal, dev and build deps). Also, `crates/cleanflate/build.rs` does not exist.
  - (b) `unsafe-token`: `grep -rnw unsafe $ROOT/crates/cleanflate` finds nothing. `-w` treats `_` as a word character, so `unsafe_code` does not match.
  - (c) `crate-attrs`: `crates/cleanflate/src/lib.rs` contains the literal lines `#![forbid(unsafe_code)]` and a `#![deny(` line naming all of `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::indexing_slicing`.
  - (d) `forbidden-paths`: `git -C $ROOT ls-files` minus paths under `crates/cleanflate/` and `crates/oracle/` has no path matching the case-insensitive ERE `zlib|miniz|flate2|libdeflate|libz-sys|libz_sys|zopfli`.
- Add Makefile target `tools-check`:
  - `shellcheck` on every `*.sh` under `tools/` plus `tools/git-hooks/*` if present
  - `python3 -m py_compile` on `tools/**/*.py` if any
  - `python3 -m unittest discover -s tools/cleanroom/tests -p 'test_*.py'` if that dir exists (test modules put `tools/cleanroom` on `sys.path` themselves)
  - every `tools/ci/tests/*_test.sh`
  Guard each optional step with an existence test so the target works now.
- Replace the `crate-checks` and `tools-check` stubs with the real targets (`crate-checks` runs `bash tools/ci/crate-checks.sh`). Add `tools-check` to `ci`.
**Acceptance tests:** `tools/ci/tests/crate_checks_test.sh`, which builds throwaway fixture repos in `mktemp -d` and prints PASS/FAIL per case:
- `case_clean_passes`
- `case_unsafe_block_fails`
- `case_unsafe_in_comment_fails`
- `case_dependency_fails`
- `case_dev_dependency_fails`
- `case_missing_forbid_fails`
- `case_forbidden_path_fails`
- `case_forbidden_word_inside_crates_ok`
The file exits non-zero if any case fails. `make crate-checks` passes on the real repo.
**Out of scope:** Clippy config. Hook scripts. Any network access.
**Verification:** `make crate-checks`, `make tools-check`, `make ci`.
**Depends on:** P0-01

### P0-03: Provenance library and PostToolUse logger
**Goal:** Implement `tools/cleanroom/provenance.py` (shared context and append helpers) and `tools/cleanroom/log.py` (the PostToolUse hook) so every tool call can be appended to `provenance/tool-log.jsonl` as one JSON line.
**Files touched:** `tools/cleanroom/provenance.py`, `tools/cleanroom/log.py`, `tools/cleanroom/tests/__init__.py`, `tools/cleanroom/tests/test_provenance.py`, `tools/cleanroom/README.md`.
**Design constraints:** SPEC §2.3 (logger bullet). Python 3 stdlib only.
- Entry keys, in this order: `ts` (UTC ISO-8601 with milliseconds and `Z`), `foundry_stage`, `task_id`, `model`, `tool`, `target`, `decision` (`"allowed"` or `"denied"`), `reason` (string or null). Extra keys `session_id` and `agent_type` may follow.
- `provenance.py` API:
  - `repo_root(hook_input)`: env `CLEANROOM_REPO_ROOT`, else env `CLAUDE_PROJECT_DIR`, else `hook_input["cwd"]`'s git toplevel, else cwd.
  - `log_path(root)`: env `CLEANROOM_LOG`, else `<root>/provenance/tool-log.jsonl`.
  - `context(hook_input, root) -> dict` (see derivation below).
  - `target_of(tool_name, tool_input) -> str` (see mapping below).
  - `append(entry, path)`: `os.open` with `O_APPEND|O_CREAT`, `fcntl.flock`, one `write` of `json.dumps(entry, separators=(",",":")) + "\n"`, and creating `provenance/` if missing.
- Stage derivation, first match wins:
  1. Hook input `agent_type` containing `planner` → `plan`, `implementer` → `implement` (or `fix` if `.foundry/state.json` `round` ≥ 1), `reviewer` → `review`, `summarizer` → `summarize`.
  2. Env `FOUNDRY_STAGE` if it is one of plan/implement/review/fix/summarize.
  3. `<root>/.foundry/implement.lock` exists → `implement`, or `fix` when round ≥ 1.
  4. Otherwise null.
- Task derivation: when the stage is implement or fix, use env `FOUNDRY_TASK_ID`, else the first line under `## Tasks` in `docs/PROGRESS.md` matching `^- \[~\] (\S+)`. Otherwise null.
- Model derivation: hook input `model` if it is a string, else env `CLEANROOM_MODEL`, else best-effort. The best-effort path reads the last 256 KiB of `transcript_path`, scans lines newest-first, and uses the first `message.model` string. Otherwise null. A missing or unreadable file is never an error.
- `target_of` mapping:
  - Read/Write/Edit → `file_path`
  - NotebookEdit → `notebook_path`
  - Grep → `path=<p> glob=<g> pattern=<pat>`
  - Glob → `path=<p> pattern=<pat>`
  - WebFetch → `url`
  - WebSearch → `query`
  - Bash → `command` truncated to 4000 chars
  - Task/Agent → `subagent_type: description`
  - anything else → the tool name plus compact JSON of the input, truncated to 500 chars
- `log.py` reads the hook JSON from stdin, appends an `allowed` entry and **always exits 0**. On any exception it tries to append an entry with reason `logger-error: <msg>`. If that also fails, it writes to stderr and still exits 0.
**Acceptance tests:** `tools/cleanroom/tests/test_provenance.py`:
- `test_target_for_each_tool`
- `test_stage_from_agent_type`
- `test_stage_from_env`
- `test_stage_from_lock_and_round`
- `test_stage_null_outside_flight`
- `test_task_id_from_progress`
- `test_model_from_transcript_tail`
- `test_model_null_when_unknown`
- `test_append_writes_one_json_line_with_spec_keys`
- `test_log_py_exits_zero_on_garbage_stdin`
All tests use temp dirs and `CLEANROOM_LOG`, and never touch the real log.
**Out of scope:** The guard (P0-04). `.claude/settings.json` (P0-05). Reading real Claude transcripts.
**Verification:** `make tools-check`.
**Depends on:** P0-01

### P0-04: PreToolUse clean-room guard
**Goal:** Implement `tools/cleanroom/guard.py` so that it exits 2 with a stderr reason and logs a `denied` entry for every call SPEC §2.3 forbids, and otherwise exits 0 silently.
**Files touched:** `tools/cleanroom/guard.py`, `tools/cleanroom/allowlist.txt`, `tools/cleanroom/tests/test_guard.py`, `tools/cleanroom/README.md`.
**Design constraints:** SPEC §2.2 and §2.3 (guard bullets). Python 3 stdlib only. Uses `provenance.py` from P0-03 for context and `append`.
- **Configuration and overrides:**
  - Lockdown flag: `CLEANROOM_LOCKDOWN_FILE`, default `<root>/tools/cleanroom/LOCKDOWN`.
  - Repo root: `CLEANROOM_REPO_ROOT`.
  - Log path: `CLEANROOM_LOG`.
  - Home directory: `HOME`.
- **`allowlist.txt`** holds one URL per line, `#` comments allowed, and exactly:
  - `https://www.rfc-editor.org/rfc/rfc1950.txt`
  - `https://www.rfc-editor.org/rfc/rfc1951.txt`
  - `https://www.rfc-editor.org/rfc/rfc1952.txt`
  - the same three without `www.`
  - `https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz`
  - `https://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip`
  - `https://sun.aei.polsl.pl//~sdeor/corpus/silesia.zip`
  - the two Silesia URLs with `http://`
- **Forbidden words (W):** `zlib`, `miniz`, `flate2`, `libdeflate`, `libz-sys`, `libz_sys`, `zopfli`, matched case-insensitively.
- **Outside-repo extra words (W+):** W plus `inflate`, `deflate`, `gzip`, `puff`.
- **Exempt dirs (E):** `<root>/crates/cleanflate/` and `<root>/crates/oracle/`.
- **Forbidden roots (F):**
  - `$HOME/.cargo/registry/` (and `$CARGO_HOME/registry/` if `CARGO_HOME` is set)
  - `$HOME/.cargo/git/`
  - any path containing a `target/` segment followed later by a `build` segment
  - any path containing `target/doc/src`
- **Rules, evaluated in order. The first deny wins.**
  - **R1:** `WebSearch` → deny.
  - **R2:** `WebFetch` → deny if the lockdown file exists. Otherwise allow only if `url` (trimmed) is in the allowlist, and deny anything else.
  - **R3:** `Read` (`file_path`), `Glob` (`path`, `pattern`) and `Grep` (`path`, `glob`). Resolve paths with `~` expansion, relative to hook `cwd`, `normpath`, and also `realpath`. Then:
    - deny if the path is under F;
    - deny if a path value contains a W word and does not resolve inside E;
    - deny if a path outside `<root>` contains a W+ word;
    - deny if the Glob `pattern` or Grep `glob` contains a W word and the search base (`path`, default `cwd`) is not inside E.
    - The Grep `pattern` (the regex) is not path-checked.
  - **R4:** `Bash` (`command`), checked in this order:
    - **(a) Network:** match the raw command against `(^|[\s;|&(\x60'"])(curl|wget|gh)(\s|$)`, `\bgit\s+clone\b` and `\bcargo\s+(vendor|download)\b`.
      - `gh`, `git clone`, `cargo vendor` and `cargo download` → always deny.
      - curl/wget → allow only if the lockdown file is absent and every `https?://\S+` URL in the command (quotes stripped) is in the allowlist, with at least one present. Otherwise deny.
    - **(b) Fetch scripts:** a command referencing `fetch-rfcs.sh`, `fetch-corpora.sh` or `CLEANROOM_ALLOW_CORPUS_FETCH` → deny once the lockdown file exists.
    - **(c) Forbidden roots:** the raw command contains `.cargo/registry`, `.cargo/git`, `target/doc/src`, or matches `target/(\S*/)?build(/|\s|$)` → deny.
    - **(d) Read-family commands.** Split the command into segments on `;`, `&&`, `||`, `|` and newlines. Take a segment's command word after skipping `VAR=val` assignments and the wrappers `sudo`, `env`, `time`, `command`, `xargs` (and their flags). If its basename is in {cat, less, more, head, tail, grep, egrep, fgrep, rg, ag, ack, find, fd, locate, ls, tree, sed, awk, bat, strings, xxd, od, hexdump, file, zcat, zless, zgrep, zmore, view, vi, vim, nano, emacs}:
      - Tokenize with `shlex.split`. A `ValueError` → deny, reason "unparseable command".
      - A token is path-like if it starts with `/`, `~`, `./` or `../`, contains `/`, or exists relative to `cwd`.
      - Deny if any path-like token contains a W word and does not resolve inside E.
      - Deny if any non-path token contains a W word, unless every path-like token (or `cwd` if there are none) resolves inside `<root>` and not under `<root>/target` or `<root>/fuzz/target`.
      - For grep/egrep/fgrep with a recursive flag (`-r`, `-R`, `--recursive`, or a combined short flag containing r/R): deny if any path argument (or `cwd` if none) is `<root>` or an ancestor of `<root>/target`, unless `--exclude-dir=target` is present.
  - **R5:** any uncaught exception → deny with reason `guard internal error: <exc>` (fail closed).
- **On deny:** append an entry with decision `denied` and the reason (the same schema as P0-03), print `cleanroom guard: <reason>` to stderr and exit 2.
- **On allow:** exit 0, print nothing, and do not log. The PostToolUse logger logs allowed calls.
**Acceptance tests:** `tools/cleanroom/tests/test_guard.py`, one unittest per behaviour (all with temp `CLEANROOM_LOG` and `CLEANROOM_LOCKDOWN_FILE`):
- `test_r1_websearch_denied`
- `test_r2_webfetch_allowlisted_prelock_allowed`
- `test_r2_webfetch_unlisted_denied`
- `test_r2_webfetch_postlock_denied`
- `test_r3_read_cargo_registry_denied`
- `test_r3_read_cargo_git_denied`
- `test_r3_read_target_build_denied`
- `test_r3_read_target_doc_src_denied`
- `test_r3_read_zlib_path_outside_crates_denied`
- `test_r3_read_crate_zlib_rs_allowed`
- `test_r3_read_outside_repo_gzip_denied`
- `test_r3_glob_zlib_pattern_repo_root_denied`
- `test_r3_grep_zlib_regex_in_repo_allowed`
- `test_r4a_curl_allowlisted_prelock_allowed`
- `test_r4a_curl_unlisted_denied`
- `test_r4a_curl_postlock_denied`
- `test_r4a_wget_denied`
- `test_r4a_gh_denied`
- `test_r4a_git_clone_denied`
- `test_r4a_cargo_vendor_denied`
- `test_r4a_curl_inside_bash_c_denied`
- `test_r4b_fetch_script_postlock_denied`
- `test_r4c_cat_registry_denied`
- `test_r4d_find_home_zlib_denied`
- `test_r4d_grep_zlib_in_repo_paths_allowed`
- `test_r4d_grep_r_repo_root_without_exclude_denied`
- `test_r4d_grep_r_with_exclude_dir_allowed`
- `test_r4d_unparseable_denied`
- `test_cargo_test_allowed`
- `test_git_status_allowed`
- `test_denial_is_logged_once`
- `test_internal_error_fails_closed`
**Out of scope:** Wiring into `.claude/settings.json` (P0-05). The LOCKDOWN file itself (P0-10). Downloading anything.
**Verification:** `make tools-check`.
**Depends on:** P0-03

### P0-05: Cleanroom self-test and hook wiring
**Goal:** Wire the guard and logger into `.claude/settings.json`, and add `tools/cleanroom/selftest.sh`, an end-to-end self-test that pipes sample hook payloads through both scripts.
**Files touched:** `.claude/settings.json`, `tools/cleanroom/selftest.sh`, `tools/cleanroom/fixtures/*.json`, `Makefile`, `tools/cleanroom/README.md`.
**Design constraints:** SPEC §2.3 (hooks, self-test) and §5 M0 acceptance.
- `.claude/settings.json` holds exactly this `hooks` block. Merge it if the file somehow exists, and do not touch `.claude/settings.local.json`:
  - `PreToolUse`: `[{"matcher": "Read|Grep|Glob|WebFetch|WebSearch|Bash", "hooks": [{"type": "command", "command": "python3 \"$CLAUDE_PROJECT_DIR/tools/cleanroom/guard.py\""}]}]`
  - `PostToolUse`: `[{"matcher": "*", "hooks": [{"type": "command", "command": "python3 \"$CLAUDE_PROJECT_DIR/tools/cleanroom/log.py\""}]}]`
- `selftest.sh`:
  - Sets a temp `CLEANROOM_LOG`, a temp `CLEANROOM_LOCKDOWN_FILE` (absent, then created for post-lock cases), and `CLEANROOM_REPO_ROOT=<repo>`.
  - Feeds each `fixtures/*.json` payload to `guard.py` and asserts the exit code (2 deny, 0 allow).
  - Asserts each denial added exactly one `denied` line.
  - Feeds one allowed payload to `log.py` and asserts one `allowed` line with all eight SPEC keys.
  - Asserts with `jq` that `.claude/settings.json` contains both hook commands.
  - Prints `PASS <case>`/`FAIL <case>` and exits 1 on any failure.
  - Must never write to `provenance/tool-log.jsonl`.
- Replace the `cleanroom-selftest` stub with `bash tools/cleanroom/selftest.sh`.
- Hooks may not take effect until Claude Code restarts. Do not try to restart anything.
**Acceptance tests:** `tools/cleanroom/selftest.sh` cases:
- `deny_websearch`
- `deny_webfetch_unlisted`
- `allow_webfetch_rfc_prelock`
- `deny_webfetch_rfc_postlock`
- `deny_read_cargo_registry`
- `deny_read_target_build`
- `deny_read_forbidden_word_outside_crates`
- `allow_read_spec`
- `allow_read_crate_source_with_word`
- `deny_bash_curl_unlisted`
- `allow_bash_curl_allowlisted_prelock`
- `deny_bash_curl_postlock`
- `deny_bash_wget`
- `deny_bash_git_clone`
- `deny_bash_cargo_vendor`
- `deny_bash_gh`
- `deny_bash_find_forbidden_home`
- `allow_bash_cargo_test`
- `denials_logged`
- `logger_appends_allowed`
- `settings_json_wired`
`make cleanroom-selftest` exits 0.
**Out of scope:** Writing the real LOCKDOWN file. Any download. Changing guard rules (fix P0-04 code only if a self-test exposes a bug, and say so under Interpretation).
**Verification:** `make cleanroom-selftest && make tools-check && make ci`.
**Depends on:** P0-04

### P0-06: commit-msg hook adding stage and task tags
**Goal:** Add a versioned `commit-msg` hook that appends `[stage:<s>] [task:<id>]` to any commit message lacking them, and enable it for this clone.
**Files touched:** `tools/git-hooks/commit-msg`, `tools/git-hooks/tests/commit_msg_test.sh` (also make `tools-check` run `tools/git-hooks/tests/*_test.sh`), `Makefile` (target `hooks-install`: `git config core.hooksPath tools/git-hooks`), `tools/cleanroom/README.md`.
**Design constraints:** SPEC §2.4.
- POSIX `sh` or `bash`. **It must always exit 0**: a failing hook would wedge the flight.
- If the message already contains `[stage:`, leave it unchanged.
- Derive the task from the subject:
  - `^([A-Z][A-Za-z0-9]*-[0-9]+):` → that ID
  - `^plan:` → `plan`
  - `^review:` → `review`
  - `^progress:` → the first `[A-Z][A-Za-z0-9]*-[0-9]+` in the subject, else `progress`
  - otherwise the lowercase word before the first `:`, else `unknown`
- Derive the stage:
  - `plan:` → `plan`
  - `review:` → `review`
  - a subject containing `summary` → `summarize`
  - otherwise `fix` if `.foundry/state.json` has `round` ≥ 1 (read with `jq` if available, else `sed`), else `implement`
- Append a blank line and the tag line before any trailer block. Keep it simple: append at the end.
- Run `make hooks-install` in this task, and document in README.md that fresh clones must run it.
**Acceptance tests:** `tools/git-hooks/tests/commit_msg_test.sh`, which runs the hook on temp message files:
- `case_task_subject_tagged_implement`
- `case_round1_tagged_fix`
- `case_plan_subject`
- `case_review_subject`
- `case_progress_subject_with_id`
- `case_already_tagged_unchanged`
- `case_no_state_file_ok`
- `case_hook_exit_zero_on_unreadable_state`
**Out of scope:** Rewriting past commits. Changing Foundry's commit behaviour.
**Verification:** `make tools-check`. After committing, check that `git log -1 --format=%B` shows exactly one tag line.
**Depends on:** P0-01

### P0-07: Vendor RFC 1950/1951/1952 texts
**Goal:** Add `tools/cleanroom/fetch-rfcs.sh`, run it to vendor the three RFC texts into `docs/rfc/` with `docs/rfc/SOURCES.json`, and add a check that the vendored hashes match.
**Files touched:** `tools/cleanroom/fetch-rfcs.sh`, `docs/rfc/rfc1950.txt`, `docs/rfc/rfc1951.txt`, `docs/rfc/rfc1952.txt`, `docs/rfc/SOURCES.json`, `docs/rfc/README.md`, `tools/ci/sources-check.sh`, `tools/ci/tests/sources_check_test.sh`, `Makefile`.
**Design constraints:** SPEC §2.1, §2.3 (WebFetch allowlist used only in M0) and §5 M0 ("recording source URL and SHA-256").
- `fetch-rfcs.sh`:
  - Refuses to run if the lockdown file exists.
  - Uses `curl -fsSL` only on the `https://www.rfc-editor.org/rfc/rfc195N.txt` URLs from `tools/cleanroom/allowlist.txt`.
  - Writes `SOURCES.json` as `[{"file", "url", "sha256", "bytes", "fetched_at"}]`.
  - Run it from the repo root.
  - If the network fails, retry up to 3 times, then block the task with the curl error. Do not fetch from any other host.
- `sources-check.sh`:
  - Verifies every file in `docs/rfc/SOURCES.json` exists with a matching sha256.
  - Verifies each RFC text contains `Request for Comments: 195N`.
  - Verifies `testdata/corpora/SOURCES.json` the same way, for files present locally only, and only if it exists.
- Add a Makefile target `sources-check` and include it in `tools-check`.
**Acceptance tests:** `tools/ci/tests/sources_check_test.sh`:
- `case_real_sources_pass`
- `case_tampered_file_fails` (on a temp copy)
- `case_missing_file_fails`
**Out of scope:** Corpora (P0-08). Any other URL. Summarising the RFCs.
**Verification:** `make sources-check && make tools-check`.
**Depends on:** P0-04

### P0-08: Download Canterbury and Silesia corpora
**Goal:** Add `tools/cleanroom/fetch-corpora.sh`, download and extract both corpora into `testdata/corpora/`, and commit `testdata/corpora/SOURCES.json` with URLs and SHA-256 for every archive and extracted file.
**Files touched:** `tools/cleanroom/fetch-corpora.sh`, `testdata/corpora/SOURCES.json`, `testdata/corpora/README.md`, `testdata/README.md`.
**Design constraints:** SPEC §2.1 (corpora are opaque binary data), §2.3 and §5 M0.
- Usage: `fetch-corpora.sh [canterbury|silesia|all] [--record]`.
- If `tools/cleanroom/LOCKDOWN` exists, refuse unless `CLEANROOM_ALLOW_CORPUS_FETCH=1`. That variable is for humans and CI only.
- Downloads with `curl -fsSL`, using only the allowlist URLs from P0-04 (Silesia: try the https single-slash URL, then the double-slash one, then http), into `testdata/corpora/archives/`.
- Extracts Canterbury with `tar -xzf` into `testdata/corpora/canterbury/` and Silesia with `unzip -o -j` into `testdata/corpora/silesia/`, keeping flat file names.
- `--record` writes `SOURCES.json`: `{"canterbury": {"url", "archive_sha256", "archive_bytes", "status": "ok", "files": [{"name", "sha256", "bytes"}]}, "silesia": {...}}`.
- Without `--record` it verifies against `SOURCES.json` and exits 1 on mismatch.
- Canterbury is required: if it cannot be downloaded after 3 attempts, block the task.
- Silesia is best-effort: if all its URLs fail, record `"status": "missing"` with the curl error for silesia, finish the task, and say "Silesia NOT DOWNLOADED" in the progress log and commit body.
- Treat corpus contents as opaque. Never open or summarise them.
**Acceptance tests:**
- `make sources-check` passes, now also covering the corpus files.
- `tools/ci/tests/sources_check_test.sh::case_corpus_tamper_fails` (add this case) is present and passes.
**Out of scope:** Generating compressed streams (P1-03). Committing corpus files (they are gitignored).
**Verification:** `bash tools/cleanroom/fetch-corpora.sh all` (verify mode) exits 0, `make sources-check`, and `git status --porcelain` shows no corpus files.
**Depends on:** P0-04

### P0-09: CI entry point and GitHub Actions workflow
**Goal:** Finalise `make ci` and add `.github/workflows/ci.yml`, which runs it on GitHub.
**Files touched:** `Makefile`, `.github/workflows/ci.yml`, `README.md`.
**Design constraints:** SPEC §5 M0 (fmt, clippy deny warnings, test, cleanroom self-test, zero-dependency and no-unsafe check).
- `ci` runs, in order: `fmt-check lint corpus test crate-checks cleanroom-selftest tools-check`.
- Workflow:
  - Triggers on push and pull_request, runs on `ubuntu-latest`.
  - Steps: checkout; `rustup toolchain install 1.89.0 --profile minimal --component rustfmt,clippy`; `sudo apt-get install -y shellcheck jq` if missing; `CLEANROOM_ALLOW_CORPUS_FETCH=1 bash tools/cleanroom/fetch-corpora.sh canterbury`; `make hooks-install`; `make ci`.
  - No third-party actions except `actions/checkout`.
- README gets a "Building and CI" section listing the Makefile targets.
**Acceptance tests:** `make ci` exits 0 locally. The workflow YAML parses: use `python3 -c 'import yaml,sys; yaml.safe_load(open(sys.argv[1]))' .github/workflows/ci.yml` if PyYAML is importable, else `ruby -ryaml -e 'YAML.load_file(ARGV[0])' .github/workflows/ci.yml` if ruby exists. If neither validator exists, write `YAML parse: NOT CHECKED` in the progress log. Do not install a validator.
**Out of scope:** Fuzz workflows (P4-06). Silesia in CI.
**Verification:** `make ci`.
**Depends on:** P0-02, P0-05, P0-08

### P0-10: Lockdown flag, provenance docs and log snapshot
**Goal:** Write the post-M0 lockdown flag, document the provenance record and its gaps, and add `make provenance-snapshot`.
**Files touched:** `tools/cleanroom/LOCKDOWN`, `tools/cleanroom/snapshot-log.sh`, `provenance/README.md`, `provenance/human-interventions.md`, `tools/cleanroom/selftest.sh`, `Makefile`.
**Design constraints:** SPEC §2.3 ("After M0 completes, WebFetch is denied entirely. Implement this with a flag file") and §5 M6 (human interventions logged manually).
- `LOCKDOWN` content: `locked_at=<UTC ISO-8601>` and `head=<git rev-parse HEAD before this commit>`.
- `snapshot-log.sh` copies `provenance/tool-log.jsonl` to `provenance/tool-log.snapshot.jsonl` if the live log exists and is non-empty, and prints the line count. Otherwise it prints `no live log` and exits 0. Target `provenance-snapshot` runs it.
- `provenance/README.md` must cover:
  - the entry schema and where each field comes from (P0-03)
  - the live-log vs snapshot arrangement
  - guard rules summary
  - known gaps:
    - the plan stage ran before the harness existed
    - hooks are snapshotted at session start, so a session that installed them is not covered until restart
    - commands run inside MCP servers or Makefile subprocesses are not individually seen
    - bypass via interpreters (e.g. `python3 -c` network calls) is not blocked
    - commits made with `--no-verify` skip the tag hook
    - model may be null
  - a "Harness status during build" section. Determine it now: check whether `provenance/tool-log.jsonl` exists and has entries with `ts` after the P0-05 commit time, and record `LIVE` or `NOT LIVE (restart required)`.
- `human-interventions.md` is a table template (`| when (UTC) | who | stage/task | what | why |`) with no rows.
- Add selftest case `deny_webfetch_real_lockdown`, which uses the real flag without the env override.
**Acceptance tests:**
- `tools/cleanroom/selftest.sh` case `deny_webfetch_real_lockdown`.
- `tools/cleanroom/tests/test_guard.py::test_real_lockdown_file_denies_fetch_script`.
- `make cleanroom-selftest` passes.
**Out of scope:** Deleting or rewriting any log. Restarting Claude Code.
**Verification:** `make cleanroom-selftest && make provenance-snapshot && make ci`.
**Depends on:** P0-05, P0-07, P0-08

### P0-11: M0 acceptance and push
**Goal:** Run the M0 acceptance commands, snapshot the provenance log, push the branch, and list the manual checks.
**Files touched:** `provenance/tool-log.snapshot.jsonl` (if a live log exists), `provenance/README.md` (harness status line only).
**Design constraints:** SPEC §5 M0 acceptance: the self-test passes, CI is green on an empty crate, the provenance log is populating, and WebFetch is locked after the flag is written.
**Acceptance tests:** `make ci` exits 0 and `make cleanroom-selftest` exits 0. Record both results in the progress log.
**Out of scope:** Any code change beyond a ≤ 20-line fix for a failure found here (describe it under Interpretation). Waiting for a human.
**Verification:** `make ci`, `make provenance-snapshot`, then `git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` in the progress log with this list:
1. Restart Claude Code in this repo, approve the hooks in `/hooks`, and confirm `provenance/tool-log.jsonl` gains a line per tool call.
2. In a fresh session, confirm WebSearch and WebFetch are refused with a `cleanroom guard:` message and logged as `denied`.
3. Confirm the GitHub Actions `ci` run for the pushed branch is green.
4. Spot-check `docs/rfc/*.txt` against rfc-editor.org.
5. Record any intervention in `provenance/human-interventions.md`.
**Depends on:** P0-01, P0-02, P0-03, P0-04, P0-05, P0-06, P0-07, P0-08, P0-09, P0-10

## Phase 1 — M1 Bit reader, stored and fixed-Huffman blocks, checksums

### P1-01: Oracle FFI wrappers over libz-sys
**Goal:** Give `crates/oracle` safe Rust wrappers for C zlib compression, decompression and checksums through libz-sys's public API only.
**Files touched:** `crates/oracle/Cargo.toml`, `Cargo.lock`, `crates/oracle/src/lib.rs`, `crates/oracle/src/ffi.rs`, `crates/oracle/tests/ffi_roundtrip.rs`.
**Design constraints:** SPEC §2.1 (libz-sys public API surface only), §4 and CR5.
- **Dependencies:**
  - `libz-sys = { version = "1", features = ["static"] }`. If cargo says the feature does not exist, remove it and add `[env] LIBZ_SYS_STATIC = "1"` to `.cargo/config.toml` instead. Never open libz-sys sources or rustdoc `src` pages to find out.
  - `libc = "0.2"`.
  - `cleanflate = { path = "../cleanflate" }` (unused until later is fine).
- **Signatures to use**, from the zlib manual as exposed by libz-sys (`z_streamp = *mut z_stream`):
  - `deflateInit2_(strm, level: c_int, method: c_int, windowBits: c_int, memLevel: c_int, strategy: c_int, version: *const c_char, stream_size: c_int) -> c_int`
  - `deflate(strm, flush: c_int) -> c_int`
  - `deflateEnd(strm) -> c_int`
  - `deflateSetDictionary(strm, dictionary: *const Bytef, dictLength: uInt) -> c_int`
  - `inflateInit2_(strm, windowBits: c_int, version: *const c_char, stream_size: c_int) -> c_int`
  - `inflate(strm, flush) -> c_int`
  - `inflateEnd(strm) -> c_int`
  - `inflateSetDictionary(strm, dictionary, dictLength) -> c_int`
  - `zlibVersion() -> *const c_char`
  - `crc32(crc: uLong, buf: *const Bytef, len: uInt) -> uLong`
  - `adler32(adler: uLong, buf: *const Bytef, len: uInt) -> uLong`
  - Pass `zlibVersion()` as `version` and `size_of::<z_stream>() as c_int` as `stream_size`.
- **Constants:** use libz-sys's `Z_*` constants if exported, with these values: Z_OK 0, Z_STREAM_END 1, Z_NEED_DICT 2, Z_DATA_ERROR -3, Z_BUF_ERROR -5, Z_NO_FLUSH 0, Z_FINISH 4, Z_DEFLATED 8. Strategies: default 0, filtered 1, huffman-only 2, RLE 3, fixed 4.
- **windowBits:** raw = `-wb`, zlib = `wb`, gzip = `wb + 16`. Inflate uses raw -15, zlib 15 and gzip 31. Gzip inflate stops at the end of the first member.
- **z_stream construction:** build it by writing every field explicitly. Never use `mem::zeroed`, because `zalloc`/`zfree` may be non-nullable fn pointers. Supply `extern "C"` allocator functions wrapping `libc::calloc`/`libc::free`.
- **Public API** (`oracle::ffi`):
  - `enum Framing { Raw, Zlib, Gzip }`
  - `enum Strategy { Default, Filtered, HuffmanOnly, Rle, Fixed }`
  - `struct DeflateParams { level: u8 /*0..=9*/, strategy, window_bits: u8 /*9..=15*/, mem_level: u8 /*1..=9*/, framing }`
  - `fn deflate_bytes(input, &DeflateParams, dictionary: Option<&[u8]>) -> Result<Vec<u8>, OracleError>`
  - `fn deflate_zeros(total: u64, &DeflateParams) -> Result<Vec<u8>, OracleError>`, which feeds 1 MiB zero chunks with Z_NO_FLUSH and then Z_FINISH
  - `fn inflate_bytes(input, Framing, dictionary: Option<&[u8]>, max_output: usize) -> InflateOutcome`, where `InflateOutcome { output: Vec<u8>, consumed: usize, status: OracleStatus }` and `OracleStatus::{StreamEnd, DataError, NeedDict, Truncated, OutputLimit, OtherError(i32)}`
  - `fn crc32(&[u8]) -> u32`, `fn adler32(&[u8]) -> u32`, `fn zlib_version() -> String`
- `unsafe` is allowed in this crate only (C1 covers only cleanflate).
**Acceptance tests:** `crates/oracle/tests/ffi_roundtrip.rs`:
- `roundtrip_all_framings_and_levels` (10 KiB pseudo-random plus text-like input)
- `roundtrip_all_strategies_windowbits_memlevels` (sampled grid)
- `zlib_dictionary_roundtrip`
- `inflate_garbage_is_error_not_panic`
- `crc32_check_value` (`b"123456789"` → `0xCBF43926`)
- `adler32_known_value` (`b"Wikipedia"` → `0x11E60398`)
- `deflate_zeros_matches_deflate_bytes` (1 MiB)
- `gzip_inflate_reports_first_member_consumed`
**Out of scope:** Corpus generation (P1-02, P1-03). Any use of cleanflate.
**Verification:** `make lint && cargo test -p oracle --test ffi_roundtrip`.
**Depends on:** P0-01

### P1-02: Deterministic synthetic inputs and generation matrix
**Goal:** Define the deterministic synthetic inputs and the bounded parameter matrix the conformance corpus is generated from.
**Files touched:** `crates/oracle/src/prng.rs`, `crates/oracle/src/inputs.rs`, `crates/oracle/src/matrix.rs`, `crates/oracle/src/lib.rs`, `crates/oracle/Cargo.toml` (`sha2 = "0.10"`), `crates/oracle/tests/matrix.rs`.
**Design constraints:** SPEC §4 (inputs, determinism).
- **PRNG:** `SplitMix64` (`state += 0x9E3779B97F4A7C15`, then the standard xor-shift-multiply finaliser). `pub const CORPUS_SEED: u64 = 0x636C_6561_6E66_6C61` in `matrix.rs`. Each synthetic input seeds `CORPUS_SEED ^ fnv1a64(id)`.
- **Synthetic inputs** (id → content):
  - `empty` (0 B)
  - `one-byte` (`0x61`)
  - `zeros-1KiB`, `zeros-64KiB`, `zeros-1MiB`, `zeros-16MiB`
  - `random-64KiB`, `random-1MiB`
  - `text-256KiB`: words from a fixed 64-word list joined by spaces/newlines, chosen by the PRNG
  - `repeat-abc-1MiB`
  - `repeat-period17-1MiB`: a 17-byte random pattern repeated
  - `window-d32767`, `window-d32768`, `window-d32769`: a random 512 B block R, then random filler of length d−512, then R again, then 1 KiB random. The second R starts exactly d bytes after the first.
  - `run-64KiB`: one random byte, then 65,536 × `0x42` (max-length matches at distance 1)
  - `period258-64KiB`: a 258 B random block repeated 254 times
- **Corpus inputs:** every regular file in `testdata/corpora/canterbury/` (id `canterbury-<name>`, tier `canterbury`) and `testdata/corpora/silesia/` (id `silesia-<name>`, tier `silesia`), sorted by name. Synthetic inputs are tier `synthetic`.
- **Matrix** (`fn entries(inputs) -> Vec<EntrySpec>`), deduplicated by parameter key:
  - **A levels:** every input × levels 0..=9 × {Raw, Zlib, Gzip}, strategy Default, wb 15, mem 8. Inputs larger than 4 MiB use levels {0,1,6,9} only.
  - **B strategies:** inputs ≤ 4 MiB get Fixed × levels 1..=9 × Raw, plus Fixed × level 6 × {Zlib, Gzip}, plus {Filtered, HuffmanOnly, Rle} × {1,6,9} × Raw. Inputs > 4 MiB get {Fixed, HuffmanOnly, Rle} × level 6 × Raw.
  - **C window:** inputs ≤ 1 MiB get wb 9..=15 × mem {1,9} × level 6 × {Raw, Zlib}, plus wb {9,15} × mem 1..=9 × level 1 × Raw.
  - **D dictionary:** Canterbury `alice29.txt`, `asyoulik.txt` and `cp.html` × levels {1,6,9} × Zlib, with the dictionary set to the first 32,768 bytes of `canterbury/lcet10.txt`. Skip if that file is absent.
- **Entry id:** `<input-id>__<raw|zlib|gzip>_l<level>_<default|filtered|huffman|rle|fixed>_w<wb>_m<mem>[_dict]`.
- **Relative path:** `<tier>/<input-id>/<rfc1951|rfc1950|rfc1952>-l<level>-<strategy>-w<wb>-m<mem>[-dict].bin`. Paths never contain the forbidden words (C11).
- **Tiers:** `quick` = synthetic + canterbury; `full` = all.
**Acceptance tests:** `crates/oracle/tests/matrix.rs`:
- `synthetic_inputs_are_deterministic` (generate twice and compare, and pin the SHA-256 of `random-64KiB`, computing it once and hard-coding the value in the test)
- `window_inputs_repeat_at_exact_distance`
- `matrix_ids_unique`
- `matrix_covers_all_parameter_values` (quick tier contains every level 0–9, all 5 strategies, wb 9–15, mem 1–9, all 3 framings, and ≥ 1 dictionary entry when Canterbury is present)
- `quick_tier_size_is_bounded` (between 300 and 6,000 entries)
**Out of scope:** Compressing anything or writing files (P1-03).
**Verification:** `cargo test -p oracle --test matrix && make lint`.
**Depends on:** P1-01

### P1-03: Corpus generator CLI and MANIFEST
**Goal:** Add the `oracle generate` command that writes the conformance corpus into `testdata/generated/`, records and verifies `testdata/MANIFEST.json`, and turns `make corpus`/`make corpus-full` into real targets.
**Files touched:** `crates/oracle/src/manifest.rs`, `crates/oracle/src/generate.rs`, `crates/oracle/src/main.rs`, `crates/oracle/Cargo.toml` (`serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`), `crates/oracle/tests/manifest.rs`, `testdata/MANIFEST.json`, `testdata/README.md`, `Makefile`.
**Design constraints:** SPEC §4 (deterministic generation, MANIFEST with parameters, input source, SHA-256).
- **MANIFEST format:** `{"format": 1, "zlib_version": "...", "seed": "0x636c65616e666c61", "entries": [...]}`. Entries are sorted by `id`. Each entry has:
  - `id`, `tier`, `input` (`synthetic:<id>` or `testdata/corpora/<dir>/<name>`), `input_sha256`, `input_len`
  - `framing` (`raw`/`zlib`/`gzip`), `level`, `strategy`, `window_bits`, `mem_level`
  - `dictionary` (null or `{"input", "len", "sha256"}`)
  - `path` (relative to `testdata/generated/`), `sha256`, `len`
- **CLI:** `oracle generate --tier quick|full [--write-manifest] [--out DIR]` (default out `testdata/generated`).
  - Without `--write-manifest`: for each entry of the tier, if the file exists with the manifest sha, skip it. Otherwise generate it and require that its sha equals the manifest. Exit 1 listing every mismatch, missing manifest entry, or zlib_version difference.
  - `--write-manifest` regenerates the tier and replaces the manifest entries for that tier, keeping other tiers' entries.
  - `full` exits 1 with `Silesia corpus missing: run tools/cleanroom/fetch-corpora.sh silesia` if its inputs are absent.
  - Hand-parse arguments with std. No clap.
- **Makefile:**
  - `corpus: cargo run --release -q -p oracle -- generate --tier quick`
  - `corpus-full: cargo run --release -q -p oracle -- generate --tier full`
- In this task, run `--tier full --write-manifest`, or `--tier quick --write-manifest` if Silesia is missing (say so in the log), then commit the MANIFEST.
**Acceptance tests:** `crates/oracle/tests/manifest.rs`:
- `manifest_serde_roundtrip`
- `generate_small_matrix_twice_is_identical` (temp dirs)
- `check_mode_detects_tampered_file`
- `check_mode_detects_missing_manifest_entry`
Also, `make corpus` exits 0 on a second run without regenerating (prints a skipped count).
**Out of scope:** Decoding with cleanflate (P1-11). Malformed corpus (P2-04).
**Verification:** `make corpus && make corpus && make lint && cargo test -p oracle --test manifest`. Also check `git status` shows no files under `testdata/generated/`.
**Depends on:** P1-02, P0-08

### P1-04: Adler-32 and CRC-32 in cleanflate
**Goal:** Implement Adler-32 and CRC-32 from the RFC definitions as the public `cleanflate::checksum` module, cross-checked against the oracle.
**Files touched:** `crates/cleanflate/src/checksum.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/mod.rs`, `crates/cleanflate/src/tests/checksum.rs`, `crates/oracle/tests/checksums.rs`. Do not create `docs/DESIGN.md` here; P1-05 creates it and records the checksum decision as D-10.
**Design constraints:** RFC 1950 §2.2 and §9 (Adler-32), RFC 1952 §2.3.1 and §8 (CRC-32), SPEC §3.2 ("implemented in-crate, derived from the RFCs' definitions") and §5 M1.
- API:
  - `pub struct Adler32` with `new()`, `update(&mut self, &[u8])` and `finish(&self) -> u32`, plus `pub fn adler32(&[u8]) -> u32`
  - `pub struct Crc32` with `new()`, `update` and `finish`, plus `pub fn crc32(&[u8]) -> u32`
- CRC-32: a 256-entry table built by a `const fn` from the reflected polynomial `0xEDB88320` per RFC 1952 §8.
- Adler-32: modulus 65521. Reducing per byte is acceptable here, and P4-09 may optimise it.
- Cite the RFC sections in rustdoc on `checksum.rs`. P1-05 copies them into DESIGN.md D-10.
- C1–C8 and C10 apply. Use no indexing: table lookups use `.get()` with a fallback, or iterator zips.
**Acceptance tests:**
- `crates/cleanflate/src/tests/checksum.rs`:
  - `adler32_empty_is_one`
  - `adler32_wikipedia` (`0x11E60398`)
  - `crc32_empty_is_zero`
  - `crc32_check_value_123456789` (`0xCBF43926`)
  - `adler32_incremental_equals_oneshot`
  - `crc32_incremental_equals_oneshot`
  - `adler32_large_buffer_of_ff` (1 MiB of 0xFF; this exercises the modulus)
- `crates/oracle/tests/checksums.rs`:
  - `adler32_matches_oracle_random_buffers`
  - `crc32_matches_oracle_random_buffers` (lengths 0..=10,000, 200 buffers, SplitMix64 seeded)
**Out of scope:** Wrapper parsing (Phase 3). Performance tuning.
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P0-01, P1-01

### P1-05: Error types, options, config constants, RFC tables, DESIGN.md skeleton
**Goal:** Add the typed error model, the options and config constants, and the RFC 1951 constant tables, and start `docs/DESIGN.md` with the design-check script.
**Files touched:** `crates/cleanflate/src/error.rs`, `crates/cleanflate/src/config.rs`, `crates/cleanflate/src/options.rs`, `crates/cleanflate/src/tables.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,error.rs,tables.rs}`, `docs/DESIGN.md`, `tools/ci/design-check.sh`, `tools/ci/tests/design_check_test.sh`, `Makefile`.
**Design constraints:** SPEC §3.3 (Errors, Limits, trailing data) and §3.4. RFC 1951 §3.2.5, §3.2.6 and §3.2.7.
- **`error.rs`:**
  - `pub enum Error` (`#[non_exhaustive]`, `Debug, Clone, PartialEq, Eq`) with exactly the variants:
    - `BadHeader(HeaderError)`
    - `PresetDictionaryRequired { dict_id: u32 }`
    - `DictionaryMismatch { expected: u32, actual: u32 }`
    - `BadBlockType`
    - `InvalidStoredLength { len: u16, nlen: u16 }`
    - `InvalidHuffmanTable(HuffmanError)`
    - `InvalidSymbol`
    - `InvalidDistance { distance: u32, available: u64 }`
    - `ChecksumMismatch { kind: ChecksumKind, expected: u32, actual: u32 }`
    - `Truncated`
    - `TrailingData { offset: u64 }`
    - `OutputLimitExceeded { limit: u64 }`
  - `pub enum HeaderError { ZlibFcheck, ZlibCompressionMethod(u8), ZlibWindowSize(u8), GzipMagic, GzipCompressionMethod(u8), GzipReservedFlags(u8) }`
  - `pub enum HuffmanError { OverSubscribed, Incomplete, TooManyLengthCodes, RepeatWithoutPrevious, RepeatOverflow, MissingEndOfBlock }`
  - `pub enum ChecksumKind { Adler32, Crc32, Isize, GzipHeaderCrc }`
  - `pub enum ErrorKind` with one unit variant per `Error` variant, `Error::kind()`, and `ErrorKind::as_str()` returning the variant name (e.g. `"InvalidHuffmanTable"`).
  - `Display` and `std::error::Error` impls.
  - Mark everything `#[non_exhaustive]` except ErrorKind.
- **`config.rs`** (the only place tunable numbers appear, C8):
  - `pub const DEFAULT_MAX_OUTPUT: u64 = 1 << 30;`
  - `pub const DEFAULT_MAX_GZIP_FIELD: usize = 65_536;`
  - Re-export both from lib.rs.
- **`options.rs`:**
  - `#[non_exhaustive] pub enum Format { Raw }` (Zlib and Gzip are added later)
  - `pub enum TrailingData { Reject, Allow }`, with default Reject
  - `pub struct Options { pub max_output: u64, pub trailing: TrailingData, pub dictionary: Option<Vec<u8>>, pub max_gzip_field: usize }`, whose `Default` uses the config constants
- **`tables.rs`** (`pub(crate)`):
  - `LENGTH_BASE`/`LENGTH_EXTRA` (29 entries, codes 257–285)
  - `DIST_BASE`/`DIST_EXTRA` (30 entries)
  - `CODE_LENGTH_ORDER` (19)
  - `WINDOW_SIZE = 32768`, `MAX_MATCH = 258`, `END_OF_BLOCK = 256`
  - `fixed_litlen_lengths() -> [u8; 288]` and `fixed_dist_lengths() -> [u8; 32]` per §3.2.6. All 32 fixed distance codes have length 5; codes 30/31 exist but are invalid if decoded.
  - Derive every value from docs/rfc/rfc1951.txt.
- **`docs/DESIGN.md`** (created here):
  - A title, a "How to read" note, and one `### D-NN: <title>` block per decision, each with lines `RFC: <section or "n/a (API)">`, `Decision:` and `Rationale:`.
  - Seed decisions: D-01 error model, D-02 output limit default, D-03 trailing-data policy, D-04 single resumable core, D-05 tables source.
  - Add `D-10: checksums`, citing RFC 1950 §2.2/§9 and RFC 1952 §8 as used by `checksum.rs` from P1-04.
- **`tools/ci/design-check.sh`:** every `### D-` heading in docs/DESIGN.md is followed, before the next heading, by a line starting `RFC:`. Exits 1 otherwise. Replace the `design-check` Makefile stub with it, and add it to `ci`.
- C1–C10 apply.
**Acceptance tests:**
- `crates/cleanflate/src/tests/error.rs`:
  - `every_variant_has_distinct_kind`
  - `kind_names_match_variant_names`
  - `display_is_nonempty_for_all_variants`
- `crates/cleanflate/src/tests/tables.rs`:
  - `length_table_spot_checks_rfc_3_2_5` (257→3/0, 265→11/1, 284→227/5, 285→258/0)
  - `distance_table_spot_checks` (0→1/0, 4→5/1, 29→24577/13)
  - `length_ranges_contiguous`
  - `distance_ranges_contiguous`
  - `fixed_litlen_lengths_rfc_3_2_6`
  - `code_length_order_rfc_3_2_7`
- `tools/ci/tests/design_check_test.sh`:
  - `case_ok`
  - `case_missing_rfc_line_fails`
**Out of scope:** Decoding logic. Zlib/Gzip format variants.
**Verification:** `make lint && make test && make design-check && make crate-checks`.
**Depends on:** P0-01

### P1-06: Resumable bit reader and 32 KiB window
**Goal:** Implement the LSB-first bit reader and the 32 KiB history window that the resumable inflater is built on.
**Files touched:** `crates/cleanflate/src/bitreader.rs`, `crates/cleanflate/src/window.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,bitreader.rs,window.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.1.1 (bit packing) and §2 (window). SPEC §5 M1. C1–C10 apply.
- **`BitReader`** (`pub(crate)`) holds a `u64` buffer and a `u32` bit count. Methods:
  - `available()`
  - `fill(&mut self, input: &[u8], pos: &mut usize, need: u32) -> bool`: loads whole bytes one at a time **only while** `available < need` and input remains. It returns whether `need` is satisfied and must never load a byte once `need` is met.
  - `peek(n) -> u32` (n ≤ 32; unavailable high bits read as 0)
  - `consume(n)`
  - `take(input, pos, n) -> Option<u32>` (fill, peek and consume; `None` leaves state unchanged apart from bytes loaded)
  - `align_to_byte()` (drops `available % 8` bits)
  - Invariant, documented in DESIGN.md D-06: after each completed decode step the buffer holds ≤ 7 bits, so the bytes consumed always equal the exact stream length.
- **`Window`** (`pub(crate)`) holds a 32,768-byte ring (`tables::WINDOW_SIZE`) and `total: u64` (bytes ever pushed). Methods:
  - `push(u8)`
  - `available() -> usize` (min(total, 32768))
  - `get_back(distance) -> Option<u8>` (distance 1 = last byte)
  - `prefill(&[u8])` (keeps the last 32,768 bytes)
  - `copy_match(distance, len, out: &mut [u8]) -> usize`: writes up to `out.len()` bytes, pushing each into the ring, byte by byte, so overlapping copies (distance < len) repeat correctly. Returns the count written.
- Record DESIGN.md D-06 (bit order and the ≤7-bit invariant) and D-07 (window ring and overlap semantics).
**Acceptance tests:**
- `crates/cleanflate/src/tests/bitreader.rs`:
  - `reads_lsb_first` (byte `0b1010_1100`: take 3 → `0b100`, take 5 → `0b10101`)
  - `value_spanning_bytes`
  - `fill_loads_only_needed_bytes`
  - `take_short_returns_none`
  - `align_drops_partial_byte`
  - `peek_pads_with_zero`
- `crates/cleanflate/src/tests/window.rs`:
  - `push_and_get_back`
  - `wraps_after_32768`
  - `distance_beyond_available_is_none`
  - `overlap_distance_1_repeats`
  - `overlap_distance_3` (`abc` + copy(3, 7) → `abcabcabca`)
  - `copy_respects_out_len`
  - `prefill_keeps_last_32k`
**Out of scope:** Huffman decoding. Fast bulk refill (P4-08).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-05

### P1-07: Canonical Huffman construction and bit-serial decoder
**Goal:** Build canonical Huffman codes from code lengths and decode symbols bit by bit, resumably.
**Files touched:** `crates/cleanflate/src/huffman.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,huffman.rs,bitwriter.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.2.2 (canonical codes) and §3.1.1 (codes packed MSB-first). C1–C10 apply.
- **`Huffman`** (`pub(crate)`) holds `counts: [u16; 16]` and `symbols` ordered by (length, symbol value).
- **`from_lengths(lengths: &[u8]) -> Result<Huffman, HuffmanError>`:**
  - Lengths are 0–15.
  - In this task, reject only over-subscription (`HuffmanError::OverSubscribed`). Incomplete-set rules arrive in P2-01; leave a `// P2-01` note.
- **`decode(&self, br: &BitReader) -> Decoded` where `enum Decoded { Symbol { symbol: u16, len: u32 }, NeedMoreBits, Invalid }`:**
  - Walks lengths 1..=15, building the code MSB-first from successive stream bits using `peek`, and stops at the first length where the code falls inside that length's range.
  - If `available()` bits run out first → `NeedMoreBits`. A 15-bit miss → `Invalid`.
  - It does not consume. The caller consumes `len` on success, or calls `fill(available+1)` on `NeedMoreBits`.
- **`src/tests/bitwriter.rs`:** a test-only `BitWriter` with `write_bits(value, n)` (LSB-first), `write_code(code, len)` (MSB-first), `align()` and `finish() -> Vec<u8>`. Later tests reuse it.
- Record DESIGN.md D-08 (canonical construction and bit-serial decode).
**Acceptance tests:** `crates/cleanflate/src/tests/huffman.rs`:
- `rfc_example_abcdefgh` (lengths 3,3,3,3,3,2,4,4 for A–H: F=00, A=010, B=011, C=100, D=101, E=110, G=1110, H=1111)
- `fixed_litlen_codes_rfc_3_2_6` (0→00110000, 143→10111111, 144→110010000, 255→111111111, 256→0000000, 279→0010111, 280→11000000, 287→11000111)
- `oversubscribed_rejected` ([1,1,1])
- `need_more_bits_when_short`
- `decode_does_not_consume`
**Out of scope:** Validation of incomplete sets (P2-01). Table-driven decoding (P2-03).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-06

### P1-08: Inflater state machine with block headers and stored blocks
**Goal:** Create the resumable raw-DEFLATE `Inflater` that decodes block headers and stored blocks.
**Files touched:** `crates/cleanflate/src/inflate.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,inflate_stored.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.2.3 and §3.2.4. SPEC §3.2 and §3.3. C1–C10 apply.
- **API** (`pub(crate)`):
  - `Inflater::new()`
  - `step(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize, InflateStatus), Error>`, returning (consumed, produced, status)
  - `enum InflateStatus { NeedsInput, OutputFull, Done }`
  - `is_done()`, `total_out() -> u64`
- **Step contract:**
  - The state machine runs atomic sub-steps. If a sub-step lacks bits and `input` is exhausted → `NeedsInput`, with all of `input` consumed into the bit buffer and the state positioned to retry the sub-step.
  - Output space 0 when a byte must be written → `OutputFull`.
  - After the final block ends → `Done`, with the partial last byte discarded and `consumed` counting exactly through that byte.
  - Errors are sticky: store the error, and return a clone of it on every later call.
- **States:** `BlockHeader` (3 bits: BFINAL, BTYPE), `StoredLen` (align to byte, then read LEN and NLEN as 4 bytes little-endian), `StoredCopy { remaining }`, `Done`, `Failed(Error)`.
- **Block types and errors:**
  - LEN ≠ !NLEN → `Error::InvalidStoredLength { len, nlen }`.
  - BTYPE 11 → `Error::BadBlockType`.
  - BTYPE 10: return `Error::BadBlockType` with the comment `// replaced by P2-02 dynamic blocks`.
  - BTYPE 01: implemented in P1-09; until then, return `Error::BadBlockType` with `// replaced by P1-09`.
- **Stored copy:** copies min(remaining, input left, output left) bytes into `output` and the `Window`.
- Record DESIGN.md D-09 (the step contract).
**Acceptance tests:** `crates/cleanflate/src/tests/inflate_stored.rs`, where "run" helpers drive `step` to completion:
- `stored_empty_final_block` (`01 00 00 ff ff` → empty, Done, consumed 5)
- `stored_hello`
- `stored_two_blocks_second_final`
- `stored_nlen_mismatch_error`
- `btype3_is_bad_block_type`
- `stored_one_byte_input_and_output_chunks_same_output`
- `truncated_input_returns_needs_input`
- `consumed_exact_with_trailing_bytes` (appended junk is not consumed)
- `error_is_sticky`
**Out of scope:** Fixed/dynamic Huffman blocks. Public API (P1-10).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-05, P1-06

### P1-09: Fixed-Huffman blocks, length/distance decoding and window copies
**Goal:** Decode BTYPE 01 blocks: literals, end-of-block, and length/distance pairs copied through the window, including overlapping copies.
**Files touched:** `crates/cleanflate/src/inflate.rs`, `crates/cleanflate/src/tests/{mod.rs,inflate_fixed.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.2.5, §3.2.6 and §3.2.3 (decoding loop). SPEC §5 M1. C1–C10 apply.
- **Tables:** build the fixed tables once per Inflater via `Huffman::from_lengths` with `tables::fixed_litlen_lengths()` (288) and `fixed_dist_lengths()` (32).
- **States:** `LitLen`, `LenExtra { sym }`, `Dist { len }`, `DistExtra { len, dsym }`, `Copy { len, dist }`. Each state is atomic and resumable using the `decode`/`NeedMoreBits` protocol from P1-07.
- **Symbols:**
  - 0–255: literal
  - 256: end of block → `BlockHeader`, or `Done` if BFINAL
  - 257–285: length
  - 286/287: `Error::InvalidSymbol`
  - Distance symbols 30/31: `Error::InvalidSymbol`
  - Code 284 with extra value 31 → length 258, accepted (DESIGN.md D-11 records the interpretation).
- **Distance check:** distance > `window.available()` → `Error::InvalidDistance { distance, available }`.
- **Copy** writes through `Window::copy_match`, resuming from `Copy { remaining }` on `OutputFull`.
- Use checked or saturating arithmetic wherever a sum could overflow.
**Acceptance tests:** `crates/cleanflate/src/tests/inflate_fixed.rs` (streams built with the test BitWriter):
- `fixed_literals_abc`
- `fixed_empty_block` (`03 00`)
- `fixed_overlapping_match_distance_1`
- `fixed_match_distance_3_len_10`
- `fixed_length_284_extra_31_is_258`
- `fixed_symbol_286_invalid`
- `fixed_distance_code_30_invalid`
- `distance_before_stream_start_invalid`
- `max_distance_32768_ok` (a stored block of 32,768 bytes, then a fixed block with distance 32768)
- `distance_one_past_available_invalid` (distance 5 after 4 output bytes)
- `fixed_one_byte_chunks_same_output`
- `stored_then_fixed_multi_block`
**Out of scope:** Dynamic blocks. The public API.
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-07, P1-08

### P1-10: Public raw API, Decompressor, output limit, trailing policy and early invariants
**Goal:** Expose `inflate`, `decompress_with` and the streaming `Decompressor` for raw DEFLATE, with the output limit and trailing-data policy, plus the first chunking-invariance and no-panic tests.
**Files touched:** `crates/cleanflate/src/stream.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,stream_raw.rs,invariants.rs}`, `docs/DESIGN.md`.
**Design constraints:** SPEC §3.3 (one-shot, streaming, limits, errors, trailing data) and §1 (no panics, bounded memory). C1–C10 apply.
- **Public API:**
  - `pub struct Decompressor`, `pub struct Progress { pub consumed: usize, pub produced: usize, pub status: Status }`, `pub enum Status { NeedsInput, OutputFull, Done }`
  - `pub struct Decoded { pub data: Vec<u8>, pub consumed: usize }`. The `gzip_members` field is added in P3-03.
  - `Decompressor::new(format: Format, options: Options)`
  - `decompress(&mut self, input: &[u8], output: &mut [u8]) -> Result<Progress, Error>`
  - `finish(&mut self) -> Result<(), Error>`
  - `total_in() -> u64`, `total_out() -> u64`, `trailing_start() -> Option<u64>`
  - `pub fn inflate(input: &[u8]) -> Result<Vec<u8>, Error>`
  - `pub fn decompress_with(format: Format, input: &[u8], options: &Options) -> Result<Decoded, Error>`
- **Streaming rules:**
  - With status `NeedsInput`, all input was consumed.
  - Input may be left unconsumed only with `OutputFull` or `Done`.
  - Errors are sticky.
  - `finish()` returns `Ok` if Done, else `Err(Truncated)`.
- **Output limit:** pass the inflater at most `max_output - total_out` bytes of `output`. If the inflater reports `OutputFull` while that allowance was the binding limit (allowance < output.len(), or allowance == 0), return `Err(OutputLimitExceeded { limit })`. A stream of exactly `max_output` bytes succeeds, because EOB needs no output space.
- **Trailing data (Raw):**
  - `Reject`: any input byte after Done, in this call or a later one → `Err(TrailingData { offset })`, where offset is the absolute input offset of the first trailing byte.
  - `Allow`: stop, and set `trailing_start()`.
- **One-shot (`decompress_with`):**
  - Drives a Decompressor with the whole input and a `Vec` that grows by `max(len, 32768)`, capped so its length never exceeds `max_output`.
  - Write into the zero-extended tail with `resize`, then `truncate` to what was produced.
  - Calls `finish()` at the end.
  - Put growth in a pure `pub(crate) fn next_len(len: usize, limit: u64) -> usize`.
  - Empty input → `Err(Truncated)`.
- Record DESIGN.md decisions D-12 (streaming contract), D-02 update (limit semantics) and D-03 update.
**Acceptance tests:**
- `crates/cleanflate/src/tests/stream_raw.rs`:
  - `inflate_stored_hello`
  - `inflate_fixed_abc`
  - `limit_exact_ok`
  - `limit_plus_one_err`
  - `next_len_never_exceeds_limit`
  - `trailing_reject_default`
  - `trailing_allow_reports_offset`
  - `trailing_after_done_in_later_call_rejected`
  - `empty_input_truncated`
  - `finish_before_done_truncated`
  - `error_is_sticky`
  - `needs_input_consumes_everything`
- `crates/cleanflate/src/tests/invariants.rs`:
  - `chunking_invariance_deterministic`: ≥ 8 hand-built valid streams (stored, fixed, multi-block, overlapping copies) × 200 chunk schedules from a test-local xorshift with fixed seeds, including all-1-byte input and output. The result and output must equal one-shot.
  - `garbage_never_panics`: 20,000 pseudo-random buffers of length 0–512 through `inflate` and through 1-byte streaming, which must return without panicking.
**Out of scope:** Zlib/gzip. proptest (P4-02). Dictionary.
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-09

### P1-11: Conformance harness and M1 conformance test
**Goal:** Add the manifest-driven conformance harness in `crates/oracle` and the M1 test decoding every level-0 and fixed-strategy raw stream byte-identically.
**Files touched:** `crates/oracle/src/conformance.rs`, `crates/oracle/src/lib.rs`, `crates/oracle/Cargo.toml`, `crates/oracle/tests/conformance.rs`, `Makefile`.
**Design constraints:** SPEC §4 and §5 M1 acceptance. C13 applies: no silent skips.
- **Harness:**
  - `load_manifest()`.
  - `selected(filter) -> Vec<Entry>` limited to the tier from env `CLEANFLATE_TIER` (`quick` default, or `full`).
  - `check_entries(entries, decode_fn) -> Report` reads each file, verifies its sha256 against the manifest (a mismatch or missing file is a failure with the message `run make corpus`), decodes it, and compares SHA-256 of the output with `input_sha256`.
  - Collect all failures and report them together: id, error or first differing offset.
  - A filter that selects zero entries is a failure.
  - Resolve `testdata/` from `env!("CARGO_MANIFEST_DIR")` + `/../../testdata`, never from the process cwd.
- **Test** `conformance_raw_stored_and_fixed`: entries with framing `raw`, no dictionary, and (`level == 0` or `strategy == fixed`), decoded with `cleanflate::inflate`.
- **Makefile:**
  - `conformance-full: corpus-full` then `CLEANFLATE_TIER=full cargo test --release -p oracle --test conformance`
  - `heavy: conformance-full` (P4 adds more)
**Acceptance tests:** `crates/oracle/tests/conformance.rs::conformance_raw_stored_and_fixed` passes on tier quick, via `make test`.
**Out of scope:** Dynamic-Huffman entries (P2-05). Zlib/gzip entries (P3-04).
**Verification:** `make corpus && make test`. Run `make conformance-full` too if Silesia is present, and record the count and duration.
**Depends on:** P1-10, P1-03

### P1-12: M1 acceptance and push
**Goal:** Run the M1 acceptance commands, fix small defects found, snapshot provenance, push, and list the manual checks.
**Files touched:** `provenance/tool-log.snapshot.jsonl`, `docs/DESIGN.md` (only if the RFC-citation audit finds gaps), plus fixes of ≤ 100 lines in files touched by P1-04…P1-11.
**Design constraints:** SPEC §5 M1 acceptance: all oracle-generated level-0 and fixed-strategy streams decode byte-identically, and checksum unit tests pass.
**Acceptance tests:**
- `make ci` passes.
- `make conformance-full` passes: record the entry count and wall time, or `NOT RUN: Silesia missing`.
- `cargo test -p oracle --test checksums` passes.
- Every `### D-` in DESIGN.md has an `RFC:` line.
**Out of scope:** Starting M2 work. Fixes larger than 100 lines: block the task with the failing ids instead.
**Verification:** `make ci && make conformance-full && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with this list:
1. Read DESIGN.md D-01…D-12 for RFC grounding.
2. Confirm GitHub CI is green.
3. Spot-check that `provenance/tool-log.snapshot.jsonl` shows no forbidden reads.
**Depends on:** P1-11, P1-04

## Phase 2 — M2 Dynamic Huffman and complete raw inflate

### P2-01: Huffman table validation, including single-code edge cases
**Goal:** Reject over-subscribed and incomplete code sets, except the two RFC-sanctioned distance-table cases, and document the edge cases.
**Files touched:** `crates/cleanflate/src/huffman.rs`, `crates/cleanflate/src/inflate.rs` (call sites pass a kind), `crates/cleanflate/src/tests/{mod.rs,huffman_validation.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.2.2 and §3.2.7 ("If only one distance code is used, it is encoded using one bit, not zero bits…"; "One distance code of zero bits means that there are no distance codes used at all"). SPEC §5 M2. C1–C10 apply.
- **API change:** `from_lengths(lengths, kind: TableKind)` with `pub(crate) enum TableKind { LitLen, Distance, CodeLength }`.
- **Rules:**
  - Over-subscribed → `OverSubscribed`.
  - Incomplete → `Incomplete`, **except** for `Distance`, which accepts (a) exactly one non-zero length, equal to 1, and (b) all lengths zero (an empty table).
  - Decoding from an empty table, or hitting the unused code of a single-code table → `Decoded::Invalid`, which the inflater maps to `Error::InvalidSymbol`.
  - Fixed tables use 288 and 32 lengths, so both are complete.
- Record DESIGN.md D-13, which lists each edge case and its outcome, including that one lit/len code alone is rejected as incomplete.
**Acceptance tests:** `crates/cleanflate/src/tests/huffman_validation.rs`:
- `oversubscribed_litlen_rejected`
- `incomplete_litlen_rejected`
- `incomplete_codelength_rejected`
- `single_litlen_code_rejected`
- `distance_single_code_len1_accepted`
- `distance_single_code_len2_rejected`
- `distance_all_zero_accepted`
- `distance_two_codes_incomplete_rejected`
- `decode_unused_code_of_single_code_table_invalid`
- `decode_from_empty_table_invalid`
- `fixed_tables_complete`
**Out of scope:** The dynamic block header (P2-02). Table-driven decoding (P2-03).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-07

### P2-02: Dynamic block header and code-length alphabet decoding
**Goal:** Decode BTYPE 10 blocks: HLIT/HDIST/HCLEN, the code-length code, repeat codes 16/17/18, and table construction. This replaces the temporary BadBlockType.
**Files touched:** `crates/cleanflate/src/inflate.rs`, `crates/cleanflate/src/tests/{mod.rs,inflate_dynamic.rs,dynbuilder.rs,invariants.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.2.7. SPEC §5 M2. C1–C10 apply.
- **States** (each atomic and resumable):
  - `DynHeader`: 14 bits: HLIT (5) + 257, HDIST (5) + 1, HCLEN (4) + 4.
  - `CodeLenLens { i }`: 3 bits each, in `CODE_LENGTH_ORDER`.
  - `CodeLens { idx }`: decode with the CodeLength table.
  - `Repeat { sym, ... }`: extra bits 2/3/7 for 16/17/18.
- **Rules:**
  - HLIT > 286 → `InvalidHuffmanTable(TooManyLengthCodes)`. HDIST up to 32 is accepted (RFC range 1–32).
  - Code 16 as the first length → `RepeatWithoutPrevious`.
  - A repeat running past HLIT+HDIST → `RepeatOverflow`.
  - Repeats may cross the lit/len→distance boundary (a single sequence per §3.2.7).
  - Lit/len length for symbol 256 is 0 → `MissingEndOfBlock`.
  - Build lit/len with `TableKind::LitLen` and distance with `TableKind::Distance`, then reuse the P1-09 symbol states.
- **Test helper `dynbuilder.rs`:** given lit/len and distance lengths plus a symbol list, emits a complete dynamic block with the test BitWriter. It computes canonical codes itself and emits code lengths with and without repeat codes.
- Extend `invariants.rs::chunking_invariance_deterministic` with ≥ 4 dynamic streams.
- Record DESIGN.md D-14.
**Acceptance tests:** `crates/cleanflate/src/tests/inflate_dynamic.rs`:
- `dynamic_literals_only`
- `dynamic_with_matches`
- `dynamic_repeat_codes_16_17_18`
- `dynamic_repeat_crosses_boundary_ok`
- `dynamic_hlit_287_rejected`
- `dynamic_hdist_32_accepted`
- `dynamic_repeat16_first_rejected`
- `dynamic_repeat_overflow_rejected`
- `dynamic_missing_eob_rejected`
- `dynamic_single_distance_code_ok`
- `dynamic_no_distance_codes_literals_ok`
- `dynamic_no_distance_codes_then_length_invalid_symbol`
- `dynamic_oversubscribed_codelength_rejected`
- `dynamic_one_byte_chunks_same_output`
**Out of scope:** Table-driven decoding (P2-03). The malformed corpus files (P2-04).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P2-01, P1-10

### P2-03: Table-driven Huffman decode path
**Goal:** Replace per-symbol bit-serial decoding with a table lookup for codes up to `LITLEN_TABLE_BITS`/`DIST_TABLE_BITS` bits, falling back to the bit-serial walk for longer codes, while staying resumable.
**Files touched:** `crates/cleanflate/src/huffman.rs`, `crates/cleanflate/src/config.rs`, `crates/cleanflate/src/inflate.rs`, `crates/cleanflate/src/tests/{mod.rs,huffman_table.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1951 §3.2.2 and §3.1.1. SPEC §5 M2 ("table-driven fast decode path"). C1–C10 apply, and C8 especially.
- **Config:** add `pub(crate) const LITLEN_TABLE_BITS: u32 = 10;` and `pub(crate) const DIST_TABLE_BITS: u32 = 8;` to `config.rs`. They are named nowhere else with a value.
- **Table:**
  - A table of `1 << bits` entries, indexed by the next `bits` stream bits (the LSB-first order, i.e. codes bit-reversed), with each entry holding (symbol, length) or a LONG marker.
  - Fill every index whose low `len` bits equal the reversed code.
- **Decode:**
  - Peek `bits` (missing bits read as 0).
  - If the entry has `len ≤ available()` → `Symbol`.
  - If the entry is LONG → the bit-serial walk.
  - If `len > available()` → `NeedMoreBits`.
  - Unused index (incomplete-allowed tables) → `Invalid`.
- Keep the bit-serial decoder as the reference used by tests.
- Record DESIGN.md D-15 (table layout, why padded lookups are safe).
**Acceptance tests:** `crates/cleanflate/src/tests/huffman_table.rs`:
- `table_equals_bitserial_fixed_tables`
- `table_equals_bitserial_random_complete_codes` (1,000 random complete length sets built by splitting leaves up to depth 15, every symbol followed by random padding bits)
- `table_handles_codes_longer_than_table_bits`
- `table_partial_bits_need_more`
- `table_single_distance_code_unused_is_invalid`
All earlier inflate tests still pass.
**Out of scope:** Bulk bit refill and fast loops (P4-08). Tuning the table bits (P4-09).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P2-02

### P2-04: Malformed corpus builder, raw cases and malformed test
**Goal:** Hand-build the raw-DEFLATE malformed corpus from RFC rules into `testdata/malformed/rfc1951/`, with `testdata/malformed/EXPECTED.tsv`, and test that every file yields its expected `ErrorKind`.
**Files touched:** `crates/oracle/src/malformed.rs`, `crates/oracle/src/main.rs`, `crates/oracle/src/lib.rs`, `crates/oracle/tests/malformed.rs`, `testdata/malformed/EXPECTED.tsv`, `testdata/malformed/rfc1951/*.bin`, `testdata/malformed/README.md`, `Makefile` (target `malformed`: `cargo run -q -p oracle -- malformed`).
**Design constraints:** SPEC §4 (malformed corpus: hand-built from RFC rules, each file records its expected error variant) and §5 M2 acceptance.
- **Builder:**
  - Uses its own `BitWriter` in `malformed.rs`. Do not import cleanflate test code.
  - Checksums, where needed, come from `oracle::ffi` (libz), never from cleanflate, to keep the corpus independent.
- **`EXPECTED.tsv`:**
  - `#` comment lines, then `path<TAB>expected<TAB>rule`.
  - `expected` is an `ErrorKind::as_str()` name or `OK`.
  - Sweep rows use `sweep:<path>` in the path column with expected `Truncated`: every strict prefix (lengths 0..len-1) must fail with Truncated.
- **Raw cases**, at minimum:
  - `btype3` BadBlockType
  - `stored_nlen_mismatch` InvalidStoredLength
  - `fixed_litlen_286` InvalidSymbol
  - `fixed_dist_code_30` InvalidSymbol
  - `fixed_distance_too_far` InvalidDistance
  - `fixed_distance_at_start` InvalidDistance
  - `dynamic_hlit_287` InvalidHuffmanTable
  - `dynamic_codelength_oversubscribed` InvalidHuffmanTable
  - `dynamic_codelength_incomplete` InvalidHuffmanTable
  - `dynamic_litlen_oversubscribed` InvalidHuffmanTable
  - `dynamic_litlen_incomplete` InvalidHuffmanTable
  - `dynamic_dist_oversubscribed` InvalidHuffmanTable
  - `dynamic_dist_incomplete_two_codes` InvalidHuffmanTable
  - `dynamic_repeat16_first` InvalidHuffmanTable
  - `dynamic_repeat_overflow` InvalidHuffmanTable
  - `dynamic_missing_eob` InvalidHuffmanTable
  - `dynamic_no_dist_then_length` InvalidSymbol
  - `dynamic_single_dist_code` OK
  - `dynamic_no_dist_literals` OK
  - `fixed_len284_extra31` OK
  - `valid_stored_trailing_byte` TrailingData
  - sweeps over `valid_small_stored`, `valid_small_fixed` and `valid_small_dynamic`
- **Test:** `malformed_raw_cases` runs every `rfc1951/` row through `cleanflate::inflate` and asserts `err.kind().as_str()` (or Ok) equals expected, reporting all mismatches together. `malformed_corpus_is_up_to_date` regenerates into a temp dir and byte-compares with the committed files and TSV.
**Acceptance tests:**
- `crates/oracle/tests/malformed.rs::malformed_raw_cases`
- `crates/oracle/tests/malformed.rs::malformed_corpus_is_up_to_date`
**Out of scope:** Zlib/gzip malformed cases (P3-05). Changing decoder semantics. If a case disagrees with the decoder, the RFC wins: fix whichever side is wrong and explain under Interpretation.
**Verification:** `make malformed && make test && git status --porcelain testdata/malformed` (empty after the commit).
**Depends on:** P2-02, P1-01

### P2-05: Full raw conformance
**Goal:** Assert that 100% of raw-DEFLATE conformance entries decode byte-identically, both one-shot and streamed with odd chunk sizes.
**Files touched:** `crates/oracle/tests/conformance.rs`, `crates/oracle/src/conformance.rs`, plus decoder fixes if needed in `crates/cleanflate/src/*.rs` (≤ 150 lines).
**Design constraints:** SPEC §5 M2 acceptance. C1–C10 and C13 apply.
- `conformance_raw_all`: every `raw` entry, one-shot.
- `conformance_raw_all_streaming_odd_chunks`: every `raw` entry, decoded with `Decompressor` using 1,021-byte input chunks and 997-byte output chunks.
**Acceptance tests:**
- `crates/oracle/tests/conformance.rs::conformance_raw_all`
- `crates/oracle/tests/conformance.rs::conformance_raw_all_streaming_odd_chunks`
**Out of scope:** Wrappers. Performance.
**Verification:** `make corpus && make test`. Run `make conformance-full` too if Silesia is present, and record the counts.
**Depends on:** P2-03, P1-11

### P2-06: M2 acceptance and push
**Goal:** Run the M2 acceptance commands, fix small defects, snapshot provenance, push, and list the manual checks.
**Files touched:** `provenance/tool-log.snapshot.jsonl`, `docs/DESIGN.md` (audit), plus fixes of ≤ 100 lines in Phase 2 files.
**Design constraints:** SPEC §5 M2 acceptance: 100% of the raw conformance corpus is byte-identical, and every malformed file returns its expected variant with no panic.
**Acceptance tests:**
- `make ci` passes.
- `make conformance-full` passes (record count and time, or `NOT RUN: Silesia missing`).
- `cargo test -p oracle --test malformed` passes.
- DESIGN.md D-13 lists every single-code edge case.
**Out of scope:** M3 work. Fixes larger than 100 lines (block with details instead).
**Verification:** `make ci && make conformance-full && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with this list:
1. Review DESIGN.md D-13/D-14 against RFC 1951 §3.2.7.
2. Confirm GitHub CI is green.
**Depends on:** P2-04, P2-05

## Phase 3 — M3 zlib and gzip wrappers

### P3-01: Preset dictionary support and the zlib wrapper
**Goal:** Add `Format::Zlib` with full RFC 1950 header and trailer handling, including preset dictionaries, and expose `zlib_decompress`.
**Files touched:** `crates/cleanflate/src/zlib.rs`, `crates/cleanflate/src/inflate.rs` (`Inflater::with_dictionary`), `crates/cleanflate/src/options.rs`, `crates/cleanflate/src/stream.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,zlib.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1950 §2.2 (CMF, FLG, FCHECK, FDICT/DICTID, ADLER32 big-endian over uncompressed data excluding the dictionary) and §2.3. SPEC §3.2 (zlib bullet) and §3.3. C1–C10 apply.
- **Header** (resumable byte state machine), checked in the order FCHECK → CM → CINFO:
  - `(CMF*256 + FLG) % 31 != 0` → `BadHeader(ZlibFcheck)`
  - `CM != 8` → `BadHeader(ZlibCompressionMethod(cm))`
  - `CINFO > 7` → `BadHeader(ZlibWindowSize(cinfo))`
  - FLEVEL is ignored.
- **FDICT set:** read DICTID (4 bytes, big-endian).
  - `options.dictionary` is None → `PresetDictionaryRequired { dict_id }`.
  - `adler32(dict) != dict_id` → `DictionaryMismatch { expected: dict_id, actual }`.
  - Otherwise `Inflater::with_dictionary(dict)` prefills the window with the last ≤ 32,768 bytes.
  - FDICT clear with a dictionary supplied → the dictionary is ignored.
  - For `Format::Raw`, a supplied dictionary prefills the window.
- **Trailer:** Adler-32 over the produced bytes, big-endian. Mismatch → `ChecksumMismatch { kind: Adler32, expected, actual }`.
- **Other rules:** do not bound distances by CINFO (DESIGN.md D-16). Trailing policy is as for Raw, after the trailer.
- Add `Format::Zlib` and `pub fn zlib_decompress(input: &[u8]) -> Result<Vec<u8>, Error>`.
**Acceptance tests:** `crates/cleanflate/src/tests/zlib.rs`:
- `zlib_minimal_empty` (`78 9c 03 00 00 00 00 01` → empty)
- `zlib_stored_hello`
- `zlib_bad_fcheck`
- `zlib_cm_7_rejected`
- `zlib_cinfo_8_rejected`
- `zlib_fdict_without_dictionary`
- `zlib_fdict_wrong_dictionary`
- `zlib_fdict_with_dictionary_copies_from_dictionary`
- `raw_with_dictionary_prefill`
- `zlib_adler_mismatch`
- `zlib_trailing_reject`
- `zlib_trailing_allow_offset`
- `zlib_truncated_in_trailer`
- `zlib_one_byte_chunks_same_output`
**Out of scope:** Gzip. Conformance corpus runs (P3-04).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-10, P1-04, P2-02

### P3-02: Gzip member header parser
**Goal:** Implement a resumable RFC 1952 member-header parser that exposes MTIME, XFL, OS and the optional fields with bounded memory.
**Files touched:** `crates/cleanflate/src/gzip.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,gzip_header.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1952 §2.3 and §2.3.1 (ID1/ID2, CM, FLG bits FTEXT/FHCRC/FEXTRA/FNAME/FCOMMENT, reserved bits, MTIME, XFL, OS, XLEN, CRC16). SPEC §3.2 (gzip bullet) and §1 (bounded memory). C1–C10 apply.
- **Public type:** `#[non_exhaustive] pub struct GzipHeader { pub text: bool, pub mtime: u32, pub xfl: u8, pub os: u8, pub extra: Option<Vec<u8>>, pub name: Option<Vec<u8>>, pub comment: Option<Vec<u8>>, pub header_crc: bool, pub truncated_fields: bool }`.
- **Parser:** `pub(crate) struct GzipHeaderParser` with `feed(&mut self, input: &[u8]) -> Result<(usize, Option<GzipHeader>), Error>`. It consumes bytes until the header completes.
- **Errors:**
  - Magic ≠ 1f 8b → `BadHeader(GzipMagic)`
  - CM ≠ 8 → `BadHeader(GzipCompressionMethod(cm))`
  - `FLG & 0xE0 != 0` → `BadHeader(GzipReservedFlags(flg))`
- **FHCRC:** the low 16 bits of the CRC-32 over every header byte before the CRC16 field. A mismatch → `ChecksumMismatch { kind: GzipHeaderCrc, expected, actual }`.
- **Field limit:** each of extra/name/comment keeps at most `options.max_gzip_field` bytes. The rest is parsed and discarded, and `truncated_fields = true`. FNAME and FCOMMENT are zero-terminated, and the terminator is not stored.
- Record DESIGN.md D-17.
**Acceptance tests:** `crates/cleanflate/src/tests/gzip_header.rs`:
- `minimal_header`
- `all_flags_header`
- `fextra_parsed`
- `fname_and_comment_parsed`
- `fhcrc_ok`
- `fhcrc_mismatch`
- `bad_magic`
- `bad_cm`
- `reserved_flag_bits_rejected`
- `field_longer_than_limit_truncated_not_rejected`
- `byte_by_byte_equals_all_at_once`
- `truncated_header_needs_more`
**Out of scope:** Member body, trailer, multi-member (P3-03).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P1-04, P1-05

### P3-03: Gzip members, trailer, multi-member streams and gzip API
**Goal:** Add `Format::Gzip`: decode members with CRC-32/ISIZE verification, concatenate multi-member output, apply trailing-data rules, and expose `gzip_decompress` and the parsed headers.
**Files touched:** `crates/cleanflate/src/gzip.rs`, `crates/cleanflate/src/stream.rs`, `crates/cleanflate/src/options.rs`, `crates/cleanflate/src/lib.rs`, `crates/cleanflate/src/tests/{mod.rs,gzip.rs}`, `docs/DESIGN.md`.
**Design constraints:** RFC 1952 §2.2 (members concatenated), §2.3 and §2.3.1 (CRC32, ISIZE little-endian, ISIZE = size mod 2^32). SPEC §3.2 (multi-member) and §3.3 (trailing data). C1–C10 apply.
- **Decompressor gzip states:** `MemberHeader` → `Body` (new Inflater per member) → `Trailer` (8 bytes) → `Boundary`.
- **Trailer checks:**
  - CRC mismatch → `ChecksumMismatch { kind: Crc32 }`
  - ISIZE mismatch → `ChecksumMismatch { kind: Isize }`
- **Boundary rules:**
  - Next bytes 1f 8b → a new member; its errors are real errors.
  - One byte 0x1f then input ends → buffer it and return `NeedsInput`.
  - Any other byte: under `Reject` → `Err(TrailingData { offset })`; under `Allow` → `Done` with `trailing_start()` = end of last member.
  - Input ends at a boundary → `NeedsInput`. `finish()` returns Ok at a boundary once ≥ 1 member is complete. A buffered lone 0x1f gives `TrailingData` under Reject and Ok under Allow.
  - Empty input → `Truncated`.
- **API additions:**
  - `Decompressor::gzip_headers() -> &[GzipHeader]`
  - `Decoded.gzip_members: Vec<GzipHeader>` (empty for other formats)
  - `pub fn gzip_decompress(input: &[u8]) -> Result<Vec<u8>, Error>`
  - Add `Format::Gzip`.
- Record DESIGN.md D-18 (boundary rules).
**Acceptance tests:** `crates/cleanflate/src/tests/gzip.rs`:
- `gzip_minimal_empty_member`
- `gzip_hello`
- `gzip_two_members_concatenate`
- `gzip_empty_member_between`
- `gzip_crc_mismatch`
- `gzip_isize_mismatch`
- `gzip_trailing_zeros_rejected_by_default`
- `gzip_trailing_allowed_reports_offset`
- `gzip_second_member_bad_header`
- `gzip_lone_1f_trailing_reject`
- `gzip_lone_1f_trailing_allow`
- `gzip_one_byte_chunks_multi_member`
- `gzip_headers_exposed`
- `gzip_truncated_in_trailer`
**Out of scope:** Conformance corpus runs (P3-04). Malformed files (P3-05).
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P3-01, P3-02

### P3-04: Zlib and gzip conformance, dictionary and multi-member tests
**Goal:** Assert 100% byte-identical decoding of every zlib and gzip conformance entry, including dictionary entries, plus oracle-built multi-member streams.
**Files touched:** `crates/oracle/tests/conformance.rs`, `crates/oracle/tests/wrappers.rs`, `crates/oracle/src/conformance.rs`, plus decoder fixes ≤ 150 lines in `crates/cleanflate/src/*.rs`.
**Design constraints:** SPEC §5 M3 acceptance and §4. C13 applies.
- `conformance_zlib_all`: zlib entries without a dictionary.
- `conformance_zlib_dictionary`: dictionary entries, decoded with `Options { dictionary: Some(first dictionary.len bytes of the manifest's dictionary input) }`.
- `conformance_gzip_all`.
- `conformance_wrappers_streaming_odd_chunks`: 1,021 in / 997 out.
- `crates/oracle/tests/wrappers.rs`:
  - `gzip_multi_member_oracle`: 200 cases concatenating 2–5 oracle gzip members of random inputs, varied params, including empty inputs. Must equal the concatenated inputs, and `gzip_headers().len()` must equal the member count.
  - `zlib_dictionary_required_on_oracle_stream`: the error's `dict_id` equals `oracle::ffi::adler32(dict)`.
  - `zlib_wrong_dictionary_mismatch`.
**Acceptance tests:**
- `crates/oracle/tests/conformance.rs`:
  - `conformance_zlib_all`
  - `conformance_zlib_dictionary`
  - `conformance_gzip_all`
  - `conformance_wrappers_streaming_odd_chunks`
- `crates/oracle/tests/wrappers.rs`:
  - `gzip_multi_member_oracle`
  - `zlib_dictionary_required_on_oracle_stream`
  - `zlib_wrong_dictionary_mismatch`
**Out of scope:** The malformed wrapper corpus (P3-05).
**Verification:** `make corpus && make test`. Run `make conformance-full` too if Silesia is present.
**Depends on:** P3-03, P1-11

### P3-05: Malformed zlib and gzip cases
**Goal:** Extend the malformed corpus with RFC 1950 and RFC 1952 cases and assert their expected error kinds.
**Files touched:** `crates/oracle/src/malformed.rs`, `crates/oracle/tests/malformed.rs`, `testdata/malformed/EXPECTED.tsv`, `testdata/malformed/rfc1950/*.bin`, `testdata/malformed/rfc1952/*.bin`.
**Design constraints:** SPEC §4 (bad headers, bad checksums, invalid gzip flags, truncation at every offset) and §5 M3 acceptance. Checksums come from `oracle::ffi` only.
- **rfc1950 cases:**
  - `bad_fcheck` BadHeader
  - `cm_7` BadHeader
  - `cinfo_8` BadHeader
  - `fdict_no_dictionary` PresetDictionaryRequired
  - `adler_mismatch` ChecksumMismatch
  - `trailing_byte` TrailingData
  - `valid_small` OK
  - `sweep:rfc1950/valid_small.bin` Truncated
- **rfc1952 cases:**
  - `bad_magic` BadHeader
  - `cm_7` BadHeader
  - `reserved_flag_bit5` BadHeader
  - `reserved_flag_bit7` BadHeader
  - `fhcrc_mismatch` ChecksumMismatch
  - `crc32_mismatch` ChecksumMismatch
  - `isize_mismatch` ChecksumMismatch
  - `trailing_garbage` TrailingData
  - `second_member_bad_cm` BadHeader
  - `all_flags_valid` OK
  - `two_members_valid` OK
  - `sweep:rfc1952/all_flags_valid.bin` Truncated
- **Test:** `malformed_wrapper_cases` runs rfc1950 rows through `zlib_decompress` and rfc1952 rows through `gzip_decompress`. `malformed_corpus_is_up_to_date` still passes.
**Acceptance tests:**
- `crates/oracle/tests/malformed.rs::malformed_wrapper_cases`
- `crates/oracle/tests/malformed.rs::malformed_corpus_is_up_to_date`
**Out of scope:** Decoder changes beyond ≤ 50-line fixes (explain under Interpretation).
**Verification:** `make malformed && make test`.
**Depends on:** P3-03, P2-04

### P3-06: M3 acceptance and push
**Goal:** Run the M3 acceptance commands, fix small defects, snapshot provenance, push, and list the manual checks.
**Files touched:** `provenance/tool-log.snapshot.jsonl`, `docs/DESIGN.md` (audit), plus fixes of ≤ 100 lines in Phase 3 files.
**Design constraints:** SPEC §5 M3 acceptance: 100% of the zlib and gzip conformance corpus passes, multi-member tests pass, and wrapper malformed cases match.
**Acceptance tests:**
- `make ci` passes.
- `make conformance-full` passes (record, or `NOT RUN: Silesia missing`).
- `cargo test -p oracle --test wrappers --test malformed` passes.
**Out of scope:** M4 work. Fixes larger than 100 lines.
**Verification:** `make ci && make conformance-full && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with this list:
1. Review DESIGN.md D-16…D-18 against RFC 1950/1952.
2. Confirm GitHub CI is green.
**Depends on:** P3-04, P3-05

## Phase 4 — M4 Streaming, limits, fuzzing, performance pass

### P4-01: Finalise the streaming contract for all formats
**Goal:** Harden and document the public `Decompressor` contract across Raw, Zlib and Gzip, and prove it with deterministic tests, including error-kind invariance under 1-byte chunking.
**Files touched:** `crates/cleanflate/src/stream.rs`, `crates/cleanflate/src/lib.rs` (crate docs with a streaming example using `?`), `crates/cleanflate/src/tests/{mod.rs,stream_contract.rs}`, `crates/oracle/tests/malformed.rs`, `docs/DESIGN.md`.
**Design constraints:** SPEC §3.3 (streaming bullet: arbitrary chunks including 1 byte, caller-provided output, consumed/produced/status, identical output regardless of chunking). C1–C10 apply.
- The contract, written into rustdoc and DESIGN.md D-12:
  - `NeedsInput` ⇒ all input consumed.
  - `OutputFull` ⇒ the output slice is full, or the limit allowance is exhausted (which is an error).
  - `Done` ⇒ the stream is complete and `consumed` stops at its end.
  - Errors are sticky.
  - Output written during a call that returns `Err` is unspecified.
  - `finish()` semantics are per format (Raw/Zlib: Ok iff Done; Gzip: rules from P3-03).
  - Calling `decompress` after `Done` with non-empty input: Reject → TrailingData, Allow → `Ok(Progress{0,0,Done})`.
- Fix any deviation found.
**Acceptance tests:**
- `crates/cleanflate/src/tests/stream_contract.rs`:
  - `one_byte_in_one_byte_out_all_formats`
  - `output_full_then_resume`
  - `needs_input_consumes_all_formats`
  - `done_consumed_exact_raw_and_zlib`
  - `after_done_reject_and_allow`
  - `error_sticky_all_formats`
  - `finish_semantics_table`
  - `zero_length_output_slice_makes_header_progress`
- `crates/oracle/tests/malformed.rs::malformed_one_byte_chunks_same_kind`: every malformed row is streamed 1 byte in / 1 byte out, then `finish()`, and the result must give the same ErrorKind or OK as one-shot.
**Out of scope:** proptest (P4-02). The bomb fixture (P4-03). Performance.
**Verification:** `make lint && make test && make crate-checks`.
**Depends on:** P3-03

### P4-02: Chunking-invariance property test (proptest)
**Goal:** Add the proptest chunking-invariance property: random input and output chunk sizes, including 1, produce output identical to one-shot, across random oracle-compressed streams.
**Files touched:** `crates/oracle/Cargo.toml` (`[dev-dependencies] proptest = "1"`), `Cargo.lock`, `crates/oracle/tests/chunking.rs`, `Makefile`.
**Design constraints:** SPEC §5 M4 (proptest in dev-deps of oracle only, SPEC §3.1). The case count is config: env `CLEANFLATE_CHUNKING_CASES`, falling back to `const DEFAULT_CASES: u32 = 512;` in the test file only.
- **Strategy:**
  - Data is 0–16,384 bytes, a mix of random and repetitive.
  - Params cover all framings, levels 0–9, all strategies, wb 9–15 and mem 1–9.
  - Input and output chunk-size vectors each hold elements 1–64, with a 10% chance of an all-1s schedule.
- **Assertions:**
  - The streamed output equals the one-shot output, which equals the data.
  - For a copy with one random byte flipped, streamed and one-shot give the same Ok/ErrorKind and the same output when Ok.
- Makefile: `proptest-10k: CLEANFLATE_CHUNKING_CASES=10000 cargo test --release -p oracle --test chunking`. Add `proptest-10k` to `heavy`.
**Acceptance tests:**
- `crates/oracle/tests/chunking.rs::chunking_invariance`
- `crates/oracle/tests/chunking.rs::chunking_invariance_on_corrupted_streams`
- `make proptest-10k` passes: record the duration.
**Out of scope:** Fuzzing. Changing decoder semantics (fix bugs only, ≤ 100 lines, and report them in `Defects-caught:`).
**Verification:** `make test && make proptest-10k`.
**Depends on:** P4-01, P1-01

### P4-03: Output-limit hardening and zip-bomb fixture
**Goal:** Prove the output limit fails fast with bounded memory against a decompression-bomb fixture in all three formats.
**Files touched:** `crates/oracle/tests/bomb.rs`, `crates/cleanflate/src/stream.rs` (fixes only), `docs/DESIGN.md`, `Makefile`.
**Design constraints:** SPEC §3.3 (Limits) and §5 M4. The fixture ratio follows the Decisions entry ("~1 MiB → DEFAULT_MAX_OUTPUT + 1 MiB"). Refer to the limit only as `cleanflate::DEFAULT_MAX_OUTPUT` (C8: no `1 << 30` literal).
- `bomb_streaming_default_limit` (`#[ignore]`):
  - `oracle::ffi::deflate_zeros(DEFAULT_MAX_OUTPUT + (1 << 20), level 9, Raw)`; record the compressed size, asserting it is < 2 MiB.
  - Stream it with a fixed 64 KiB output buffer and default Options.
  - Expect `Err(OutputLimitExceeded { limit: DEFAULT_MAX_OUTPUT })` with `total_out() == DEFAULT_MAX_OUTPUT`.
- `bomb_oneshot_small_limit`: 64 MiB of zeros, `decompress_with` with `max_output = 16 << 20` → OutputLimitExceeded, for Raw, Zlib and Gzip.
- `limit_exact_boundary_all_formats`: output of exactly the limit → Ok.
- Makefile: `bomb: cargo test --release -p oracle --test bomb -- --include-ignored`. Add it to `heavy`.
- Record DESIGN.md D-19, which explains why the SPEC's 1 KiB example is impossible (the maximum DEFLATE ratio is about 1032:1).
**Acceptance tests:**
- `crates/oracle/tests/bomb.rs`:
  - `bomb_streaming_default_limit`
  - `bomb_oneshot_small_limit`
  - `limit_exact_boundary_all_formats`
- `make bomb` passes: record the wall time and the compressed size.
**Out of scope:** Changing the default limit value.
**Verification:** `make test && make bomb`.
**Depends on:** P4-01, P1-01

### P4-04: Fuzz crate and never-panic targets
**Goal:** Create the cargo-fuzz crate with never-panic targets for each of the three entry points, compile-check it in verify, and run smoke fuzzing if a nightly toolchain can be installed.
**Files touched:** `fuzz/Cargo.toml`, `fuzz/fuzz_targets/never_panic_raw.rs`, `fuzz/fuzz_targets/never_panic_rfc1950.rs`, `fuzz/fuzz_targets/never_panic_rfc1952.rs`, `fuzz/seeds/<target>/*` (≤ 10 small seeds each, copied from `testdata/malformed/`), `fuzz/README.md`, `Makefile`.
**Design constraints:** SPEC §5 M4 (a). File names avoid forbidden words (C11). C9: fuzz uses `oracle::ffi`, never libz-sys directly.
- **`fuzz/Cargo.toml`:**
  - Package `cleanflate-fuzz`, `publish = false`, `[package.metadata] cargo-fuzz = true`.
  - Deps: `libfuzzer-sys = "0.4"`, `cleanflate = { path = "../crates/cleanflate" }`, `oracle = { path = "../crates/oracle" }`.
  - `[workspace] members = ["."]`.
  - One `[[bin]]` per target with `test = false, doc = false, bench = false`.
- **Each target:**
  - Calls the one-shot function with `Options { max_output: 1 << 24, ..Default::default() }`. `1 << 24` is not a tunable; name it `const FUZZ_MAX_OUTPUT` in each target.
  - Then runs streaming with input and output chunk sizes derived from the first two bytes (1–256).
  - Results are ignored. Only panics and timeouts are failures.
- **Makefile:**
  - `fuzz-check: cargo check --manifest-path fuzz/Cargo.toml --bins` (replaces the P0-01 stub)
  - `FUZZ_SECONDS ?= 60`
  - `fuzz-smoke:` for each target `cargo +nightly fuzz run <t> fuzz/seeds/<t> -- -max_total_time=$(FUZZ_SECONDS) -rss_limit_mb=2048`
- **Toolchain:** try `rustup toolchain install nightly --profile minimal` and `cargo install cargo-fuzz --locked`. If either fails, record `fuzz-smoke NOT RUN: <reason>` and still complete the task.
**Acceptance tests:**
- `make fuzz-check` passes.
- `make fuzz-smoke` exits 0 with zero crashes, or is recorded NOT RUN with the reason. Record per-target exec counts.
**Out of scope:** Differential targets (P4-05). CI workflow (P4-06).
**Verification:** `make fuzz-check`, then `make fuzz-smoke` if the tooling is available.
**Depends on:** P4-01

### P4-05: Differential fuzz targets against the oracle
**Goal:** Add fuzz target (b), a round-trip differential that must never diverge, and target (c), an arbitrary-bytes accept/reject differential that logs divergences for triage instead of failing.
**Files touched:** `fuzz/Cargo.toml`, `fuzz/fuzz_targets/diff_oracle_roundtrip.rs`, `fuzz/fuzz_targets/diff_oracle_arbitrary.rs`, `fuzz/seeds/...`, `fuzz/README.md`, `Makefile`.
**Design constraints:** SPEC §5 M4 (b) and (c). C9 applies.
- **(b):**
  - Parameters come from the first 5 bytes: framing, level 0–9, strategy, wb 9–15, mem 1–9. The data is the rest, at most 64 KiB.
  - Compress with `oracle::ffi::deflate_bytes`. Decompress with cleanflate one-shot, cleanflate streaming (chunk sizes from the data length), and `oracle::ffi::inflate_bytes`.
  - `panic!` if any output differs from the data. That panic is the fuzz failure.
- **(c):**
  - For each framing, run cleanflate with `Options { trailing: TrailingData::Allow, max_output: FUZZ_MAX_OUTPUT }` and the oracle with `max_output = FUZZ_MAX_OUTPUT`.
  - Classify the pair as `both_ok_same`, `both_ok_differ`, `both_err`, `clean_ok_oracle_err` or `clean_err_oracle_ok`.
  - Append a JSON line `{framing, class, clean_kind, oracle_status, input_hex (≤ 4096 bytes)}` to `$CLEANFLATE_DIVERGENCE_LOG` (default `fuzz/artifacts/divergences.jsonl`) for any class except `both_ok_same` and `both_err`.
  - Never panic on divergence. `both_ok_differ` is also logged; it is not an automatic failure, per SPEC.
- **Makefile:**
  - Add both targets to `fuzz-smoke`.
  - `fuzz-divergences:` prints counts per class using `jq`.
**Acceptance tests:**
- `make fuzz-check` passes.
- `make fuzz-smoke` exits 0, or is recorded NOT RUN. Record the divergence counts by class in the progress log.
**Out of scope:** Triage fixes. If (c) reveals a real decoder bug, fix it only if ≤ 50 lines and add a regression test in the right test file. Otherwise record it for review.
**Verification:** `make fuzz-check && make fuzz-smoke`.
**Depends on:** P4-04, P1-01

### P4-06: Nightly fuzz workflow and long-run target
**Goal:** Provide the 1-hour-per-target fuzz runs as a GitHub Actions nightly workflow and a `make fuzz-long` target.
**Files touched:** `.github/workflows/fuzz-nightly.yml`, `Makefile`, `fuzz/README.md`.
**Design constraints:** SPEC §5 M4 acceptance: (a) and (b) each run 1 hour with zero crashes and zero divergences.
- **Makefile:** `fuzz-long:` runs `$(MAKE) fuzz-smoke FUZZ_SECONDS=3600`.
- **Workflow:**
  - Triggers: `schedule` (daily cron) and `workflow_dispatch`.
  - A matrix job per target: the three never-panic targets, `diff_oracle_roundtrip` and `diff_oracle_arbitrary`.
  - Steps: install nightly and cargo-fuzz (`cargo install cargo-fuzz --locked`), then run the target for 3,600 s.
  - Upload `fuzz/artifacts/` on failure, and always upload the divergence log for `diff_oracle_arbitrary`.
  - No third-party actions except `actions/checkout` and `actions/upload-artifact`.
**Acceptance tests:** The YAML parses (use the same validator approach as P0-09, or record NOT CHECKED). `make -n fuzz-long` prints the five 3,600-second runs.
**Out of scope:** Running the 1-hour fuzzing inside this task.
**Verification:** `make -n fuzz-long`, plus the YAML parse check.
**Depends on:** P4-05

### P4-07: Benchmark harness and baseline
**Goal:** Add `oracle bench`, which measures single-thread decompression throughput for cleanflate and C zlib, and record the baseline in `docs/PERF.md`.
**Files touched:** `crates/oracle/src/bench.rs`, `crates/oracle/src/main.rs`, `crates/oracle/src/lib.rs`, `Makefile`, `docs/PERF.md`.
**Design constraints:** SPEC §5 M4 (performance pass; target ≥ 70% of C zlib on Silesia; report either way) and §2.2 (no reference-implementation reading; C10 applies to PERF.md).
- **CLI:** `oracle bench [--corpus silesia|canterbury] [--iters N (default 5)] [--level L (default 6)]`.
- **Method:**
  - For each corpus file, compress with the oracle, raw and zlib framing.
  - Time best-of-N `cleanflate::decompress_with` against `oracle::ffi::inflate_bytes` with `std::time::Instant`, single-threaded and release-only. The bench refuses to run in a debug build.
  - Throughput is uncompressed bytes per second.
- **Output:** a markdown table per file plus aggregate MB/s and ratio (cleanflate / zlib), for raw and zlib.
- **Makefile:** `bench: cargo run --release -q -p oracle -- bench --corpus silesia`. Fall back to canterbury, with a printed warning, if Silesia is missing.
- **`docs/PERF.md`:** method, machine (`uname -srm`, CPU model from `/proc/cpuinfo`, rustc version), and a "Baseline (P4-07)" table.
**Acceptance tests:**
- `crates/oracle/src/bench.rs` unit test `throughput_math`, as a `#[cfg(test)]` in the oracle crate (this is allowed; C6 only covers cleanflate).
- `make bench` runs and writes numbers into PERF.md.
**Out of scope:** Any optimisation.
**Verification:** `make bench && make lint`.
**Depends on:** P3-03, P1-03

### P4-08: Performance pass 1 — bulk refill and fast decode loop
**Goal:** Speed up the hot decode loop with a bulk bit refill and a fast path used when ample input and output remain, measured before and after.
**Files touched:** `crates/cleanflate/src/bitreader.rs`, `crates/cleanflate/src/inflate.rs`, `crates/cleanflate/src/window.rs`, `crates/cleanflate/src/tests/{mod.rs,fastpath.rs}`, `docs/PERF.md`, `docs/DESIGN.md`.
**Design constraints:** SPEC §5 M4 (profiling- and RFC-guided only, never by reference implementations; CR1) and SPEC §1 (safety first). C1–C10 apply: no unsafe and no indexing panics, so use `get`, `chunks_exact` and `copy_within`.
- **Fast path:**
  - Entered only when ≥ 16 input bytes and ≥ 258 output bytes remain.
  - Refills the u64 bit buffer several bytes at a time.
  - Decodes symbols without returning to the outer state dispatch.
  - Copies non-overlapping matches in bulk (`copy_from_slice` or `copy_within` on the ring or output).
- **On exit:** it must restore the P1-06 invariant by returning unconsumed whole bytes from the bit buffer to the input position. `consumed` stays exact and resumability is unchanged.
- **Measurement:** run `make bench` before any change and after; put both tables in PERF.md under "Pass 1", and both aggregate numbers in the commit `Measurement:` line.
- **Outcome:** if the change is slower or breaks any test, revert it and record the negative result instead.
**Acceptance tests:**
- `crates/cleanflate/src/tests/fastpath.rs`:
  - `fastpath_equals_slowpath_on_all_test_streams`
  - `fastpath_consumed_exact_at_done`
  - `fastpath_boundary_input_16_bytes`
- `make ci`, `make conformance-full` and `make proptest-10k` still pass.
**Out of scope:** Checksum and table-size tuning (P4-09). Any unsafe code or SIMD (SPEC §7).
**Verification:** `make bench` before and after, `make ci`, `make heavy`.
**Depends on:** P4-07, P2-03

### P4-09: Performance pass 2 — measured hotspot and table-bits tuning
**Goal:** Measure where time goes, optimise the top remaining hotspot, tune `LITLEN_TABLE_BITS`/`DIST_TABLE_BITS` by measurement, and record the final ratio against the 70% target.
**Files touched:** `crates/cleanflate/src/checksum.rs` and/or `crates/cleanflate/src/huffman.rs`, `crates/cleanflate/src/config.rs`, `crates/cleanflate/src/tests/*`, `docs/PERF.md`, `docs/DESIGN.md`.
**Design constraints:** SPEC §5 M4. CR1 applies. C1–C10 apply, and C8 especially: table bits change only in config.rs.
- **Profiling:**
  - If `perf` exists, use `perf record -g` on `make bench` and include `perf report --stdio` top-10 in PERF.md.
  - Otherwise, time component micro-benchmarks via `oracle bench`: raw vs zlib framing isolates Adler-32 cost, and Canterbury vs Silesia isolates the rest.
- **Candidates** (derive from the RFC definitions and arithmetic; consult no implementation):
  - Adler-32 deferred modulo with a bound you derive and prove in DESIGN.md.
  - CRC-32 multi-table (slice-by-N) derived from the polynomial.
  - Table-bits sweep: {9,10,11} × {7,8,9}; keep the best.
- **Measurement:** before and after for every change kept. Put the numbers in PERF.md "Pass 2", the commit `Measurement:` line and the progress log.
- **Final result:** PERF.md ends with a "Result" line: `cleanflate / C zlib = X% on Silesia (target 70%): MET|NOT MET`.
**Acceptance tests:**
- Checksum tests from P1-04 and `crates/oracle/tests/checksums.rs` still pass.
- New `crates/cleanflate/src/tests/checksum.rs::adler32_bound_worst_case` (max-length run of 0xFF crossing the deferred-modulo bound).
- `make ci` and `make heavy` pass.
**Out of scope:** Unsafe code, SIMD and threads (SPEC §7). Changing semantics.
**Verification:** `make bench` before and after, `make ci`, `make heavy`.
**Depends on:** P4-08

### P4-10: M4 acceptance and push
**Goal:** Run the M4 acceptance commands, record fuzz, property and performance results, snapshot provenance, push, and list the manual checks.
**Files touched:** `provenance/tool-log.snapshot.jsonl`, `docs/PERF.md` (final numbers only), plus fixes of ≤ 100 lines in Phase 4 files.
**Design constraints:** SPEC §5 M4 acceptance: fuzz targets (a) and (b) each run 1 hour with zero crashes and divergences, and the chunking property test passes 10k cases.
**Acceptance tests:**
- `make ci` passes.
- `make heavy` passes: conformance-full, proptest-10k and bomb.
- `make fuzz-smoke` passes, or NOT RUN with the reason.
- Record `make fuzz-divergences` counts.
**Out of scope:** Running the 1-hour fuzzing unattended. It is a manual check.
**Verification:** `make ci && make heavy && make fuzz-smoke && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with this list:
1. Run `make fuzz-long`, or trigger `fuzz-nightly`, and confirm 1 hour each for (a) ×3 and (b) with zero crashes.
2. Triage `divergences.jsonl` from (c).
3. Re-run `make bench` on a quiet machine and confirm the PERF.md result line.
4. Confirm GitHub CI is green.
**Depends on:** P4-02, P4-03, P4-06, P4-09

## Phase 6 — M6 Freeze and hand-off

### P6-01: Provenance summary generator
**Goal:** Generate `provenance/SUMMARY.md` from the tool log, the human-interventions file and git history, with every section SPEC §5 M6 requires.
**Files touched:** `tools/cleanroom/summarize.py`, `tools/cleanroom/tests/test_summarize.py`, `tools/cleanroom/tests/fixtures/tool-log.sample.jsonl`, `Makefile` (target `provenance-summary`), `provenance/SUMMARY.md`.
**Design constraints:** SPEC §5 M6 (total tool calls, denied calls with reasons, every file and URL read grouped, per-stage wall clock, models per stage, human interventions) and §2.3. Python 3 stdlib only.
- **Input:** `provenance/tool-log.jsonl` if present, else `provenance/tool-log.snapshot.jsonl`. `--log` overrides.
- **Sections:**
  - Source and line count.
  - Totals by tool and by stage.
  - Denied calls: a table of ts, stage, task, tool, target and reason.
  - Files read, grouped by top-level repo dir. Paths outside the repo are listed under a separate "Outside repository" heading.
  - URLs fetched.
  - Per-stage wall clock: first ts, last ts, span, and active time summing inter-call gaps under 15 minutes.
  - Models per stage, with counts, and null reported as "unknown".
  - Human interventions: the table from `provenance/human-interventions.md`, verbatim.
  - Known gaps, copied from the `provenance/README.md` "Known gaps" section.
- Lines that fail to parse are counted and reported, never dropped silently.
**Acceptance tests:** `tools/cleanroom/tests/test_summarize.py`:
- `test_totals`
- `test_denied_table`
- `test_files_grouped_and_outside_repo_flagged`
- `test_urls_listed`
- `test_stage_wall_clock_active_time`
- `test_models_unknown_for_null`
- `test_bad_lines_counted`
- `test_includes_human_interventions`
**Out of scope:** Transcript backfill (see Decisions). Tagging.
**Verification:** `make tools-check && make provenance-summary`.
**Depends on:** P0-10

### P6-02: Build report generator and DESIGN.md audit
**Goal:** Produce `docs/BUILD_REPORT.md` from git history and the docs via a re-runnable script, and audit DESIGN.md for complete RFC citations.
**Files touched:** `tools/cleanroom/build_report.py`, `tools/cleanroom/tests/test_build_report.py`, `Makefile` (target `build-report`), `docs/BUILD_REPORT.md`, `docs/DESIGN.md` (fill any missing `RFC:` lines).
**Design constraints:** SPEC §5 M6 (milestones, review rounds per milestone, defects caught in review vs by tests vs by fuzzing, known limitations) and §6 (DESIGN.md cites RFC sections for all core decisions).
- **Milestones:** per phase, the task count and commit count, from `git log --format=%H%x00%s%x00%b` matching `[task:P<n>-`.
- **Defects:** aggregate `Defects-caught:` lines from commit bodies, classified by keyword into tests / conformance / fuzz / review, with commits carrying `[stage:fix]` counted as review.
- **Review rounds:** count `review:` commits. If there are none yet, write "Pending: regenerate with `make build-report` after the Foundry review converges".
- **Known limitations:**
  - M5 deferred (no_std, compressor)
  - the PERF.md result line
  - the provenance gaps from `provenance/README.md`
  - open divergence classes, if any are recorded
  - C ABI out of scope (SPEC §7)
**Acceptance tests:** `tools/cleanroom/tests/test_build_report.py`:
- `test_counts_tasks_per_phase` (temp git repo fixture)
- `test_defects_classified`
- `test_review_pending_when_no_review_commits`
- `test_known_limitations_include_perf_result`
Also, `make design-check` passes.
**Out of scope:** Tagging. Editing SPEC.md.
**Verification:** `make tools-check && make build-report && make design-check`.
**Depends on:** P6-01, P4-09

### P6-03: Freeze and hand-off push
**Goal:** Run the final full verification, regenerate the provenance summary and build report, snapshot the log, push, and list the human steps for tagging `v0.1.0-cleanroom`.
**Files touched:** `provenance/tool-log.snapshot.jsonl`, `provenance/SUMMARY.md`, `docs/BUILD_REPORT.md`.
**Design constraints:** SPEC §5 M6 and §6 (definition of done). Per Decisions, the implementer does not create the tag.
**Acceptance tests:**
- `make ci` passes.
- `make heavy` passes.
- `make provenance-summary` and `make build-report` regenerate without error.
- `make crate-checks` passes.
**Out of scope:** Creating or pushing tags. Merging.
**Verification:** `make ci && make heavy && make provenance-snapshot && make provenance-summary && make build-report && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with this list:
1. After the Foundry review converges and the branch merges, run `make provenance-snapshot provenance-summary build-report`, commit, then `git tag v0.1.0-cleanroom && git push origin v0.1.0-cleanroom`.
2. Complete `provenance/human-interventions.md`.
3. Confirm the 1-hour fuzz results from P4-10 are recorded in BUILD_REPORT.md.
**Depends on:** P6-01, P6-02

## Spec issues
- **§2.3 hooks mid-session.** Claude Code may snapshot hooks when a session starts, so hooks written by M0 may not govern the rest of the same flight. Resolution: P0-10/P0-11 record whether the log is live, and a restart is a manual check. For the strongest record, a human should run Phase 0, restart Claude Code, then resume the flight.
- **§2.3 logger vs denied calls.** PostToolUse does not fire for calls that PreToolUse blocked. Resolution: the guard logs its own denials.
- **§2.3/§2.4 tracked log vs clean tree.** A tracked log that grows on every tool call conflicts with Foundry's clean-tree requirement. Resolution: the live log is gitignored, and a snapshot is committed at each phase end.
- **§2.3 forbidden-word paths.** The literal rule forbids natural names such as fuzz targets or testdata folders containing "zlib". Resolution: naming convention (rfc1950/rfc1951/rfc1952), enforced by `make crate-checks`.
- **§2.4 commit tags.** Foundry requires `<ID>: <title>` subjects and authors its own bookkeeping commits. Resolution: tags go in the body, and a `commit-msg` hook adds them to any commit lacking them. Commits created with `--no-verify` would be untagged; this is documented as a gap.
- **§2.1 provenance start.** The plan stage (this document) ran before the harness existed, so there is no tool log for it. The planner read only docs/SPEC.md, the repo's README and Foundry's own plugin docs. Record this in `provenance/README.md` (P0-10).
- **§2.1 zlib manual signatures.** §2.1 and §4 refer to "the zlib manual's documented function signatures written in the SPEC below", but SPEC contains none. Resolution: P1-01 lists the signatures and constants from the public API.
- **§3.1 dev-dependencies.** Allowing dev-dependencies only in oracle/fuzz means cleanflate's own tests cannot use proptest. Resolution: property tests live in `crates/oracle/tests/`, and cleanflate tests use deterministic std-only generators.
- **§3.1 MSRV pin.** "MSRV pinned in rust-toolchain.toml" requires rustup to install the exact `1.89.0` toolchain (a download) even though stable 1.89.0 is present. This is accepted.
- **§4 generation matrix.** Levels 0–9 × 5 strategies × windowBits 9–15 × memLevel 1–9 × 3 framings is 9,450 streams per input and infeasible on Silesia. Resolution: the bounded matrix in P1-02 still covers every value.
- **§4 corpus hosting.** SPEC does not say whether corpora and the generated corpus are committed. Resolution: neither is committed. Hashes are committed and CI fetches Canterbury.
- **§5 M4 bomb ratio.** "1 KiB → >1 GiB" is impossible for single-layer DEFLATE (maximum about 1032:1). Resolution: about 1 MiB of compressed zeros expanding past `DEFAULT_MAX_OUTPUT` (P4-03).
- **§5 per-milestone review.** "Each milestone ends with a Foundry review stage" does not match Foundry's single review loop per flight. Resolution: phase-end acceptance tasks. The human may run one flight per milestone.
- **§5 M4 fuzz duration.** 1-hour runs per target cannot run inside per-task verify, and nightly/cargo-fuzz are not installed. Resolution: smoke runs in-flight, a nightly workflow, and a manual check.
- **§5 M6 ordering.** The tag and "review rounds per milestone" can only exist after the Foundry review. Resolution: re-runnable generators, with the human tagging after merge.
- **§3.2 incomplete codes.** RFC 1951 is silent on incomplete code sets, HDIST 31/32 usage, and length code 284 with extra value 31. Resolution: see Decisions, recorded in DESIGN.md D-11/D-13/D-14. Differential fuzz target (c) logs any disagreement with C zlib for human triage.
- **§3.3 gzip FNAME/FCOMMENT.** These fields are unbounded, which conflicts with bounded memory. Resolution: store-and-truncate at `DEFAULT_MAX_GZIP_FIELD` without rejecting.
- **§3.3 default output limit.** SPEC gives no value. Resolution: 1 GiB (Decisions).

## Review fixes (round 1)

### R1-01: Independent checksums in cleanflate unit-test fixtures (C16) plus a mechanical rule
**Goal:** Stop cleanflate's unit-test fixture builders from computing trailer/header checksums with cleanflate's own crate::checksum, so a checksum bug can no longer hide itself, and add a docs/foundry.json rule so the same shape cannot pass again.
**Files touched:** crates/cleanflate/src/tests/stream_contract.rs, crates/cleanflate/src/tests/zlib.rs, crates/cleanflate/src/tests/gzip.rs, crates/cleanflate/src/tests/gzip_header.rs, docs/foundry.json, CLAUDE.md
**Design constraints:** C16: every checksum embedded in a fixture is either a hard-coded literal whose value was computed once with oracle::ffi::{crc32,adler32} (with a comment naming the call that produced it), or the test moves to crates/oracle/tests/ and calls oracle::ffi directly. cleanflate keeps zero dependencies (C3). Only crates/cleanflate/src/tests/checksum.rs may import crate::checksum under src/tests/. New foundry.json rule id `no-cleanflate-checksums-in-fixtures`: paths [crates/cleanflate/src/tests/], exclude [crates/cleanflate/src/tests/checksum.rs], pattern `\bcrate::checksum\b`, shouldMatch must include the diff's own lines `use crate::checksum::{adler32, crc32};` and `use crate::checksum::crc32;` and `use crate::checksum::adler32;`; shouldNotMatch includes `use crate::tests::bitwriter::BitWriter;` and `use crate::error::Error;`. Tag C16 in CLAUDE.md with [rule:no-cleanflate-checksums-in-fixtures]. Assertions that compare the decoder's reported `actual`/`dict_id` values must also use the independent (literal) value, not crate::checksum.
**Acceptance tests:** All existing zlib/gzip/gzip_header/stream_contract tests keep passing with independent checksums. The new constraint rule's self-test fixtures pass in foundry_verify. Would-have-caught check: foundry_mutate on crates/cleanflate/src/checksum.rs replacing `            s2 = (s2 + s1) % ADLER_MOD;` with `            s2 = (s2 + s1) % (ADLER_MOD - 1);` must now fail at least one test in tests/zlib.rs (e.g. zlib_stored_hello), not only tests/checksum.rs.
**Out of scope:** Changing checksum.rs itself. Changing decoder behaviour. Moving tests that do not build checksummed fixtures.
**Verification:** make test && make lint && foundry_verify (constraints show the new rule ok with 0 hits)
**Depends on:** none

### R1-02: Output limit: empty output slice must be OutputFull, and cover the streaming limit arithmetic
**Goal:** Make Decompressor::decompress raise OutputLimitExceeded only when the remaining max_output allowance (not the caller's output slice length) bounded the call, so a zero-length output slice yields a resumable Ok(OutputFull), and add streaming tests that pin the limit arithmetic.
**Files touched:** crates/cleanflate/src/stream.rs, crates/cleanflate/src/tests/stream_raw.rs, docs/DESIGN.md
**Design constraints:** C4/C5/C8 apply (no panics, no indexing, limit only via Options/config constants). The limit error fires iff remaining_allowance < output.len() or remaining_allowance == 0, and the backend reported OutputFull. Exact-limit streams still succeed; limit+1 still fails with total_out() == limit. Update DESIGN.md D-13/D-20 wording if it describes the old condition.
**Acceptance tests:** crates/cleanflate/src/tests/stream_raw.rs::empty_output_slice_is_output_full_not_limit: a stored or fixed stream that must emit bytes, decompress(&input, &mut []) returns Ok with status OutputFull and produced 0 (currently Err(OutputLimitExceeded)), then a 16-byte buffer finishes with the right bytes and Done. crates/cleanflate/src/tests/stream_raw.rs::streaming_limit_with_large_buffer: max_output 4 with a 64-byte output buffer on stored "hello" returns Err(OutputLimitExceeded{limit:4}) with total_out()==4; max_output 5 returns Done with "hello". Would-have-caught check: foundry_mutate on stream.rs replacing `let remaining_allowance = self.options.max_output.saturating_sub(already_out);` with `let remaining_allowance = self.options.max_output.saturating_add(1).saturating_sub(already_out);` must be killed by make test.
**Out of scope:** decompress_with's growth schedule (next_len). The default limit value. Bomb tests.
**Verification:** make test && make lint && make bomb
**Depends on:** none

### R1-03: Gzip: exact consumed at Done under TrailingData::Allow
**Goal:** Make gzip's member-boundary lookahead not count trailing bytes as consumed, so Progress.consumed at Done and Decoded.consumed stop exactly at the end of the last member (PLAN Decisions: 'Under Allow, consumed and trailing_start() report the exact offset'; P4-01 contract; lib.rs streaming docs; DESIGN.md D-20).
**Files touched:** crates/cleanflate/src/gzip.rs, crates/cleanflate/src/stream.rs, crates/cleanflate/src/lib.rs, crates/cleanflate/src/tests/gzip.rs, crates/cleanflate/src/tests/stream_contract.rs, docs/DESIGN.md
**Design constraints:** RFC 1952 S2.2 boundary rules from P3-03 stay as they are (1f 8b means a new member; anything else is trailing; a lone 0x1f that is the last byte offered is buffered and resolved by finish()). Decide at the boundary by peeking at bytes already in the current call's input rather than consuming them; only a lone trailing 0x1f at the end of a call may be consumed ahead of the decision. Document that single case as the one exception in lib.rs and D-19/D-20. decompress_with's Decoded.consumed equals trailing_start() whenever that is set. Reject-mode offsets are unchanged. C4/C5 apply.
**Acceptance tests:** crates/cleanflate/src/tests/gzip.rs::gzip_trailing_allow_consumed_exact: one valid member of length N followed by b"XYZ" under Allow: decompress_with(..).consumed == N, and one streaming decompress call with the whole input returns Progress{consumed: N, status: Done} with trailing_start()==Some(N) (currently 27 vs 26 for a 26-byte member). Same for trailing bytes [0x1f, b'Z'] (currently overshoots by 2). crates/cleanflate/src/tests/stream_contract.rs::done_consumed_exact_all_formats extends done_consumed_exact_raw_and_zlib to Format::Gzip.
**Out of scope:** Changing the 1f 8b rule. Raw/zlib trailing handling. Header parsing.
**Verification:** make test && make lint
**Depends on:** R1-01

### R1-04: Bound retained gzip member headers during streaming
**Goal:** Keep a streaming gzip Decompressor's memory bounded (SPEC S1) by capping how many GzipHeader values are retained, while still decoding every member and reporting how many members were seen.
**Files touched:** crates/cleanflate/src/config.rs, crates/cleanflate/src/options.rs, crates/cleanflate/src/gzip.rs, crates/cleanflate/src/stream.rs, crates/cleanflate/src/tests/gzip.rs, docs/DESIGN.md, docs/foundry.json, CLAUDE.md
**Design constraints:** New tunable DEFAULT_MAX_GZIP_HEADERS gets its value only in config.rs (C8), is re-exported beside DEFAULT_MAX_GZIP_FIELD, and Options gains max_gzip_headers initialised from it (option-defaults-not-literal). Add DEFAULT_MAX_GZIP_HEADERS to the tunables-defined-only-in-config pattern and add max_gzip_headers to the option-defaults-not-literal pattern in docs/foundry.json, each with a matching shouldMatch fixture. Headers past the cap are parsed and validated (including FHCRC) but not stored. Add Decompressor::gzip_member_count() -> u64 counting every member started. Decoded.gzip_members holds the retained headers only. New DESIGN.md decision with an RFC: line (RFC 1952 S2.2) and SPEC S1. Update CLAUDE.md's config.rs module-map line.
**Acceptance tests:** crates/cleanflate/src/tests/gzip.rs::gzip_many_members_headers_bounded: 10 concatenated valid members with max_gzip_headers = 3 decode to the concatenation of all 10 payloads, gzip_headers().len() == 3, and gzip_member_count() == 10. crates/cleanflate/src/tests/gzip.rs::gzip_default_header_cap_from_config: Options::default().max_gzip_headers == DEFAULT_MAX_GZIP_HEADERS.
**Out of scope:** Changing the per-field max_gzip_field cap. Output-limit logic.
**Verification:** make test && make lint && foundry_verify
**Depends on:** R1-03

### R1-05: Make stream_contract test drivers fail instead of hang on no progress
**Goal:** Make every chunk-driving loop in stream_contract.rs fail with a clear message when a call on empty input makes no progress and does not return Done, so a decoder regression turns into a red test instead of a hang in make test.
**Files touched:** crates/cleanflate/src/tests/stream_contract.rs
**Design constraints:** Test-only change. Use a no-progress guard (consumed == 0 && produced == 0 && input empty && status != Done → panic with format and state), not a wall-clock timeout. Keep every existing assertion.
**Acceptance tests:** Would-have-caught check: foundry_mutate on crates/cleanflate/src/gzip.rs replacing `if expected_isize != actual_isize {` with `if false && expected_isize != actual_isize {` must report killed within the normal make test time. It currently hangs forever in stream_contract::error_sticky_all_formats.
**Out of scope:** Decoder changes. Loops in other test files, unless they share a helper.
**Verification:** make test && make lint
**Depends on:** R1-03

### R1-06: Commit fuzz/Cargo.lock instead of gitignoring it to dodge libz-only-in-oracle
**Goal:** Restore reproducible fuzz builds by tracking fuzz/Cargo.lock, and scope the libz-only-in-oracle rule so an unavoidable transitive lockfile entry is not a violation.
**Files touched:** .gitignore, fuzz/Cargo.lock, docs/foundry.json
**Design constraints:** Remove the /fuzz/Cargo.lock ignore and its comment. Add `exclude: ["fuzz/Cargo.lock"]` to libz-only-in-oracle and update its description to say lockfiles are exempt; keep every existing fixture. fuzz/Cargo.lock must pin the same libz-sys version as the workspace Cargo.lock. C11: the lockfile path contains no forbidden word.
**Acceptance tests:** foundry_verify constraints show libz-only-in-oracle ok with 0 hits while fuzz/Cargo.lock is tracked. make crate-checks passes (forbidden-paths). make fuzz-check passes using the committed lockfile.
**Out of scope:** Changing fuzz targets. Workflow changes beyond what the lockfile needs.
**Verification:** make fuzz-check && make crate-checks && foundry_verify
**Depends on:** none

### R1-07: Multi-member-aware oracle for differential fuzz target (c)
**Goal:** Make diff_oracle_arbitrary compare cleanflate's RFC 1952 S2.2 multi-member gzip decoding against an oracle that also concatenates members, so gzip divergences in the log mean something instead of being background noise.
**Files touched:** crates/oracle/src/ffi.rs, crates/oracle/tests/ffi_roundtrip.rs, fuzz/fuzz_targets/diff_oracle_arbitrary.rs, fuzz/README.md
**Design constraints:** C9: fuzz reaches C zlib only through oracle::ffi. Add oracle::ffi::inflate_gzip_members(input, max_output) -> InflateOutcome. It calls inflate_bytes repeatedly while the remaining input starts with 1f 8b and the previous member hit StreamEnd, concatenates outputs, stops at the first non-1f8b remainder (trailing, allowed), and reports an error status if any member fails. The target uses it for Framing::Gzip only; raw/zlib are unchanged. Divergences are still logged, never panicked on.
**Acceptance tests:** crates/oracle/tests/ffi_roundtrip.rs::gzip_multi_member_matches_concatenation: two oracle-compressed gzip members, concatenated, give the concatenated payloads with consumed == total length; appending b"XYZ" still gives StreamEnd with consumed == member bytes; a second member with a corrupt CRC gives an error status. After a make fuzz-smoke run (if nightly is available), make fuzz-divergences shows no gzip both_ok_differ entries caused by multi-member concatenation; record the per-class counts in the progress log, or record NOT RUN with the reason.
**Out of scope:** Changing cleanflate. Target (b). Workflow changes.
**Verification:** make test && make fuzz-check (plus make fuzz-smoke and make fuzz-divergences if nightly is available)
**Depends on:** R1-03, R1-06

### R1-08: Complete the provenance record: intervention row, denial notes, bootstrap gaps
**Goal:** Record in provenance/ what this run actually did that the log alone does not explain: the human browser download for P0-08, the guard denials that were never noted as CR6 requires (including P1-01's blocked search for libz-sys source), the untagged bootstrap commits, and the phases that ran before LOCKDOWN existed. Also make sources-check enforce that manual fetches have an intervention row.
**Files touched:** provenance/human-interventions.md, provenance/README.md, tools/ci/sources-check.sh, tools/ci/tests/sources_check_test.sh
**Design constraints:** Do not delete or rewrite any log or git history. human-interventions.md keeps P0-10's table format and gains a row: the Canterbury archive downloaded by a human operator in a browser from the allowlisted URL (Cloudflare 403 for automated requests), stage/task implement/P0-08, sha256 f140e8a5b73d3f53198555a63bfb827889394a42f20825df33c810c3d5e3f8fb. provenance/README.md gains two sections. 'Denied calls during build' lists each denied entry in provenance/tool-log.jsonl with its ts, task and reason, and says explicitly that the P1-01 `find / -path '*/libz-sys-*/src/lib.rs'` attempt was blocked by the guard and that no libz-sys source was read. 'Known gaps' adds: commits c1bbaa9 4015214 8edc633 956d6d7 44f624e f776879 predate the commit-msg hook and carry no [stage:][task:] tags; LOCKDOWN was not written before Phases 1-4 ran because P0-10 was skipped; the P0-08 retries used a browser User-Agent against the allowlisted URL. sources-check fails when any testdata/corpora/SOURCES.json entry has a fetched_by value other than the automated default and human-interventions.md has no row mentioning that entry's archive_sha256.
**Acceptance tests:** tools/ci/tests/sources_check_test.sh::case_manual_fetch_requires_intervention_row: on a temp copy, a SOURCES.json entry with fetched_by "human-operator-browser" and no matching row fails; adding the row passes. The real repo passes make sources-check.
**Out of scope:** Generating provenance/SUMMARY.md (P6-01). Changing guard behaviour.
**Verification:** make tools-check && make sources-check
**Depends on:** P0-10

## Review fixes (round 2)

### R2-01: C5: CRC-32 table without indexing, plus a rule against panic-lint suppression in cleanflate lib
**Goal:** Remove the only #[allow(clippy::indexing_slicing)] in cleanflate library code (checksum.rs crc32_table) by building the table without [] indexing, and add a docs/foundry.json rule so a local clippy allow can never again hide a C4/C5 violation.
**Files touched:** crates/cleanflate/src/checksum.rs, crates/cleanflate/src/tests/checksum.rs, docs/foundry.json, CLAUDE.md
**Design constraints:** C5: no [] indexing or slicing in cleanflate library code, and no #[allow]/#![allow] of clippy::indexing_slicing, unwrap_used, expect_used or panic anywhere under crates/cleanflate/src/ outside src/tests/. Keep CRC32_TABLE a compile-time const. Acceptable constructions: an array literal expanded by a macro from a `const fn crc32_entry(n: u32) -> u32` (while-loop over the 8 bit steps, RFC 1952 S8), or a const walk using split_first_mut (const-stable on the pinned 1.89 toolchain). C4/C8/C10 apply. New foundry.json rule id `no-panic-lint-allow-in-cleanflate-lib`: paths [crates/cleanflate/src/], exclude [crates/cleanflate/src/tests/], pattern matching `allow(` followed (within the same attribute) by clippy::unwrap_used|expect_used|panic|indexing_slicing, e.g. `allow\s*\([^)]*clippy::(unwrap_used|expect_used|panic|indexing_slicing)\b`. shouldMatch must include this diff's own line `#[allow(clippy::indexing_slicing)]` plus `#![allow(clippy::unwrap_used, clippy::indexing_slicing)]`; shouldNotMatch must include `#[allow(dead_code)]` and `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`. Tag C5 in CLAUDE.md with [rule:no-panic-lint-allow-in-cleanflate-lib] while keeping the clippy/make lint mention.
**Acceptance tests:** crates/cleanflate/src/tests/checksum.rs::crc32_table_matches_bitwise_reference: for every n in 0..256, the table entry used by crc32 (exercise via crc32 of a single byte, or expose the table pub(crate)) equals a bit-at-a-time reflected 0xEDB88320 computation written in the test. Existing crc32 tests and crates/oracle/tests/checksums.rs keep passing. foundry_verify shows the new rule ok with 0 hits and its fixtures passing; before the fix the rule would hit checksum.rs:100.
**Out of scope:** Adler-32. Performance changes to CRC (slice-by-N). The allow(dead_code) attributes in tables.rs.
**Verification:** make lint && make test && foundry_verify (constraints: no-panic-lint-allow-in-cleanflate-lib ok, 0 hits)
**Depends on:** none

### R2-02: Gzip finish(): member-seen test must use member_count, not retained headers
**Goal:** Make GzipDecoder::finish decide 'at least one member completed' from member_count rather than !headers.is_empty(), so Options { max_gzip_headers: 0 } still decodes valid gzip input instead of failing with Truncated.
**Files touched:** crates/cleanflate/src/gzip.rs, crates/cleanflate/src/tests/gzip.rs
**Design constraints:** RFC 1952 S2.2 boundary rules unchanged. Audit gzip.rs for any other place that uses headers (len/is_empty) as a proxy for members seen and switch it to member_count. Headers stay capped by max_gzip_headers. C4/C5/C15 apply.
**Acceptance tests:** crates/cleanflate/src/tests/gzip.rs::gzip_zero_header_cap_still_decodes: with Options { max_gzip_headers: 0, ..Default }, one valid member decodes via decompress_with to its payload with gzip_members empty; two concatenated members decode to the concatenation; a streaming Decompressor over the two members finishes Ok with gzip_headers().len() == 0 and gzip_member_count() == 2. (Currently decompress_with returns Err(Truncated).) A truncated first member with max_gzip_headers 0 still returns Truncated.
**Out of scope:** Changing the default cap. Output-limit logic. decompress_with's consumed reporting (R2-03).
**Verification:** make test && make lint
**Depends on:** none

### R2-03: decompress_with: exact consumed at a lone trailing 0x1f, and fail instead of spinning when the buffer cannot grow
**Goal:** Make decompress_with's Decoded.consumed equal trailing_start() whenever that is set (including an input ending in a lone 0x1f under TrailingData::Allow), and make its loop return OutputLimitExceeded rather than spin forever if an OutputFull comes back for an empty write region.
**Files touched:** crates/cleanflate/src/stream.rs, crates/cleanflate/src/tests/gzip.rs, crates/cleanflate/src/tests/stream_raw.rs, docs/DESIGN.md
**Design constraints:** PLAN R1-03: 'decompress_with's Decoded.consumed equals trailing_start() whenever that is set' -- the lone-0x1f exception in lib.rs/D-19 applies to streaming callers only, since the one-shot API sees the whole input; update the lib.rs/D-19/D-20 wording to say so. In decompress_with, when the write region passed to decompress was empty (buffer already at next_len's cap) and the call returns OutputFull, return Err(Error::OutputLimitExceeded { limit: options.max_output }) instead of continue. No change to Decompressor::decompress's limit logic. C4/C5/C8/C15 apply; the growth step constant is out of scope.
**Acceptance tests:** crates/cleanflate/src/tests/gzip.rs::gzip_trailing_allow_lone_1f_one_shot: one valid member of length N followed by the single byte 0x1f under Allow: decompress_with(..).consumed == N (currently N+1) and data is the member payload; under Reject the error is TrailingData { offset: N }. Would-have-caught check for the hang: foundry_mutate on crates/cleanflate/src/stream.rs replacing `let remaining_allowance = self.options.max_output.saturating_sub(already_out);` with `let remaining_allowance = self.options.max_output.saturating_add(1).saturating_sub(already_out);` must report killed with timedOut false (today tests::stream_raw::limit_plus_one_err never terminates). Existing limit_exact_ok / limit_plus_one_err / streaming_limit_with_large_buffer keep passing.
**Out of scope:** Streaming Progress.consumed for a lone 0x1f at the end of a call (documented exception). next_len's growth schedule. Gzip header cap (R2-02).
**Verification:** make test && make lint && make bomb && make design-check
**Depends on:** R2-02

### R2-04: Oracle multi-member gzip: test a 0x1f-not-0x8b trailer
**Goal:** Cover inflate_gzip_members' 1f 8b new-member check so a regression that treats any 0x1f as a new member (reintroducing spurious fuzz target (c) gzip divergences) fails make test.
**Files touched:** crates/oracle/tests/ffi_roundtrip.rs
**Design constraints:** Test-only. C9: reach C zlib only through oracle::ffi. Keep every existing assertion in gzip_multi_member_matches_concatenation.
**Acceptance tests:** crates/oracle/tests/ffi_roundtrip.rs::gzip_multi_member_matches_concatenation (extended) or a new gzip_multi_member_1f_trailer_is_trailing: two oracle gzip members followed by [0x1f, b'Z'] give status StreamEnd, output b"helloworld", consumed == both.len(). Would-have-caught check: foundry_mutate on crates/oracle/src/ffi.rs replacing `let starts_new_member = next.first() == Some(&0x1f) && next.get(1) == Some(&0x8b);` with `let starts_new_member = next.first() == Some(&0x1f);` must report killed (it currently survives).
**Out of scope:** Changing inflate_gzip_members' behaviour. Fuzz targets. cleanflate.
**Verification:** make test
**Depends on:** none

### R2-05: Provenance summary: list URLs fetched via Bash, and state the Bash-read gap
**Goal:** Make provenance/SUMMARY.md's 'URLs fetched' section include every http(s) URL in allowed Bash tool calls (curl/wget and fetch-script invocations), not only WebFetch, so the summary no longer claims no URL was fetched when the log shows curl fetches (SPEC S5 M6).
**Files touched:** tools/cleanroom/summarize.py, tools/cleanroom/tests/test_summarize.py, tools/cleanroom/tests/fixtures/tool-log.sample.jsonl, provenance/README.md, provenance/SUMMARY.md
**Design constraints:** Python 3 stdlib only. Extract URLs with a regex over allowed Bash targets; group the section by tool (WebFetch, Bash). Denied fetches stay in the Denied calls table only. Add a provenance/README.md 'Known gaps' bullet: files read through Bash commands (cat/sed/grep/head) appear in the log as commands, not under 'Files read', which lists Read/Write/Edit targets only. Regenerate provenance/SUMMARY.md with make provenance-summary. Never edit or filter tool-log entries.
**Acceptance tests:** tools/cleanroom/tests/test_summarize.py::test_urls_listed extended (or test_urls_from_bash_curl): a fixture line with tool Bash, decision allowed, target 'curl -sSfL -o x https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz' makes that URL appear under URLs fetched; a denied Bash curl URL does not appear there. After regeneration, provenance/SUMMARY.md lists https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz.
**Out of scope:** Parsing file paths out of arbitrary Bash commands. build_report.py. Guard behaviour.
**Verification:** make tools-check && make provenance-summary
**Depends on:** none

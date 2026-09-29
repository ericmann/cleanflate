# Build summary

**Merge line:** `build/2026-09-29`, base `c4afeb796ddf` → head `7689af5`, 151 commits, 4 review rounds (3 CHANGES REQUESTED, then APPROVED), final verdict **APPROVED**. 62/62 tasks done, 0 blocked, 0 skipped.

## What was built

**Phase 0: clean-room harness and scaffolding (P0-01..P0-11).** This phase set up a Cargo workspace pinned to Rust 1.89 and a Makefile whose `make ci` target is authoritative. It also added the provenance tooling:
- a stdlib-Python PreToolUse guard that fails closed and logs its own denials;
- a PostToolUse logger that writes to a gitignored live `provenance/tool-log.jsonl`, with committed snapshots;
- a cleanroom self-test and a `commit-msg` hook that adds `[stage:] [task:]` tags.

The phase also vendored the RFC 1950/1951/1952 texts, fetched the Canterbury and Silesia corpora (hashes committed, data gitignored), and added the CI workflow and the post-M0 LOCKDOWN flag.

**Phase 1: bit reader, stored and fixed-Huffman blocks, checksums (P1-01..P1-12).** This phase built the `crates/oracle` crate, which wraps C zlib via `libz-sys` (static, bundled). It includes a deterministic synthetic-input generator and a bounded generation matrix that writes `testdata/MANIFEST.json`. In `cleanflate` it added Adler-32 and CRC-32, the error, options and config types, a resumable bit reader with a 32 KiB window, canonical Huffman decoding, and the inflater state machine for stored and fixed blocks. It also added the public raw API and `Decompressor`, plus the conformance harness.

**Phase 2: dynamic Huffman and complete raw inflate (P2-01..P2-06).** This phase added Huffman table validation, including the RFC 3.2.7 single-code and all-zero distance exceptions, and dynamic block headers. It also added a table-driven fast decode path and a malformed-input corpus built from scratch. Full raw conformance was reached against the generated corpus.

**Phase 3: zlib and gzip wrappers (P3-01..P3-06).** This phase added preset dictionaries and the zlib wrapper (FCHECK/CM/CINFO, Adler-32 trailer). It added the gzip header parser, which keeps a bounded amount of each field, and gzip members with CRC-32/ISIZE trailers and multi-member streams. Zlib and gzip conformance passed, along with malformed zlib and gzip cases.

**Phase 4: streaming, limits, fuzzing, performance (P4-01..P4-10).** This phase:
- finalized the streaming contract across all three formats;
- added a proptest chunking-invariance property (512 cases by default, 10k via `make proptest-10k`) and a zip-bomb output-limit fixture;
- added a cargo-fuzz workspace with 3 never-panic targets and 2 differential targets, plus a nightly fuzz workflow;
- added a benchmark harness and ran two performance passes.

Result: throughput is **25.6% (raw) and 27.3% (zlib) of C zlib** on Silesia, against a SPEC target of 70%, which is **NOT MET** (see `docs/PERF.md`).

**Phase 6: freeze and hand-off (P6-01..P6-03).** This phase added a provenance summary generator (`provenance/SUMMARY.md`), a build report generator (`docs/BUILD_REPORT.md`), a DESIGN.md audit, and the hand-off push. There is no Phase 5: optional M5 was deferred.

**Review fixes (R1-01..R1-08, R2-01..R2-05, R3-01).** These made checksums in the test fixtures independent of the crate under test, and fixed real output-limit, gzip `consumed`, gzip `finish` and one-shot hang bugs. They bounded how many gzip headers are retained, stopped hanging test drivers, and committed `fuzz/Cargo.lock`. The differential oracle became multi-member aware. The provenance record was completed: interventions, denials, and URLs fetched via Bash and via the fetch scripts. C5 was enforced mechanically.

## Decisions that shaped it

From PLAN.md Decisions:
- **No Phase 5** (M5 no_std/compressor deferred); Phase N = milestone MN (plan-wide).
- **One review loop per flight**, not per milestone; each phase ends with an acceptance-and-push task (P0-11, P1-12, P2-06, P3-06, P4-10, P6-03).
- **`make ci` is authoritative**; `.github/workflows/ci.yml` runs the same target (P0-09).
- **Hook liveness** is recorded but not blocked on; a restart is a manual check (P0-10/P0-11).
- **Live tool log is gitignored**; `make provenance-snapshot` commits a snapshot at each phase end (P0-03, P0-10).
- **Guard logs its own denials**, because PostToolUse never fires for blocked calls (P0-04).
- **Guard is stdlib Python 3 with `shlex` and fails closed** (P0-04).
- **Forbidden-word path rule applied literally**: `rfc1950/1951/1952` names and `never_panic_*`/`diff_oracle_*` fuzz targets, enforced by `make crate-checks` (P0-02).
- **Guard extras**: it denies the whole cargo registry directory, rustdoc source pages, the underscore spelling of the libz-sys crate name, and inflate/deflate/gzip/puff paths outside the repo (P0-04).
- **Commit tags go in the body** and a `commit-msg` hook adds them (P0-06).
- **Corpora are not committed**: SOURCES.json hashes only, and CI fetches Canterbury (P0-08).
- **Generated corpus is gitignored**; the MANIFEST is committed and SHA-verified. The quick tier runs in `make test` and the full tier in `make conformance-full` (P1-03).
- **Bounded generation matrix**: four sub-matrices instead of the ~9,450-stream cross product (P1-02).
- **Oracle uses `libz-sys` static/bundled** for deterministic hashes (P1-01).
- **`cleanflate` has no dev-dependencies**; proptest, serde and oracle tests live in `crates/oracle/tests/` (plan-wide).
- **Resumable state machine from M1**; the one-shot API drives it (P1-08, P1-10).
- **Public API names and Error enum** are fixed up front (P1-05, P1-10).
- **`DEFAULT_MAX_OUTPUT = 1 GiB`**, inclusive: exactly the limit succeeds, limit+1 fails (P1-05, P1-10).
- **Gzip fields are stored up to 64 KiB each, then truncated**; valid streams are never rejected (P3-02).
- **Trailing data is rejected by default**; after a gzip member, `1f 8b` always starts a new member (P1-10, P3-03).
- **Incomplete Huffman codes are rejected** except the two distance-table cases. HDIST up to 32 is accepted, and length 284+31 decodes to 258 (P2-01, DESIGN D-11/D-13/D-14).
- **Zlib CINFO ≤ 7 is validated** but back-references are not limited to the declared window; header checks run in the order FCHECK, CM, CINFO (P3-01).
- **Bomb fixture is ~1 MiB of zeros expanding past the limit**, because 1 KiB → 1 GiB is impossible (P4-03).
- **Fuzzing**: 60 s smoke runs in-flight; 1-hour runs are nightly or done by a human (P4-04, P4-06).
- **Chunking cases** come from `CLEANFLATE_CHUNKING_CASES`: 512 by default, 10k at phase ends (P4-02).
- **The implementer never tags `v0.1.0-cleanroom`**; a human tags after merge (P6-03).
- **No transcript backfill** of pre-hook activity; the gap is documented instead (P0-10).
- **Toolchain 1.89.0, edition 2021, resolver 3**, dev opt-level 1 with deps at 3 (P0-01).
- **Performance is measured** as best-of-N one-shot MB/s on Silesia at level 6, raw and zlib (P4-07).

From HANDOFF.md Interpretation choices:
- **DESIGN.md IDs**: the next free `D-NN` is used instead of PLAN's literal IDs. Streaming is D-20 (not D-12) and bomb ratio is D-21 (not D-19) (P4-01, P4-03).
- **`fuzz-smoke` writes to the gitignored `fuzz/corpus/<t>/`** and reads seeds second, so checked-in seeds are not mutated (P4-04).
- **`fuzz-long` uses per-target pattern rules** with a target-specific variable, not recursive `$(MAKE)`, so `make -n fuzz-long` does not launch real runs. `.PHONY` was dropped on those targets because of a GNU Make 4.3 quirk (P4-06).
- **Seed files were renamed to `rfc1950_*`** after `crate-checks` caught "zlib" in the paths (P4-05/P4-06).
- **Performance pass 1**: the fast decode loop was implemented, a correctness bug was found (one byte short on an ~11 KiB Fixed stream), and the loop was reverted per the task's escape hatch. Only the bulk non-overlapping `copy_match` was kept (P4-08, D-22).
- **Table-bits sweep kept `{10, 8}`**, which was tied or ahead within noise (P4-09).
- **Re-verification of reopened tasks**: an empty or near-empty `[stage:fix]` commit was used where the original work predated the round (P0-08..P6-03).
- **`fuzz/Cargo.lock` was gitignored** (P4-04). This was superseded by R1-06, which commits it and excludes that path from `libz-only-in-oracle`.
- **Checksums in the test fixtures** come from literal lookups computed once via the oracle. FHCRC became a mandatory override when set (R1-01).
- **Lone trailing `0x1f`** at end-of-call is consumed in streaming only. One-shot `decompress_with` corrects this via `trailing_start()` (R1-03, R2-03, D-19/D-25).
- **`split_first_mut` const walk** was chosen over a generated 256-entry literal for the CRC table (R2-01).
- **Narrow "invoked"**: only a pipeline piece whose command word is `bash`/`sh <script>` or the script path counts. Wrapped forms like `env X=1 bash …` or `bash -x …` are missed (R3-01).

## Assumptions still in play

No `⚠️ ASSUMPTION` markers exist in PLAN.md or the source. The tunables in `crates/cleanflate/src/config.rs` (C8) are:

| Key | Final default | Status |
|---|---|---|
| `DEFAULT_MAX_OUTPUT` | `1 << 30` (1 GiB) | Guess: reasoned in PLAN, never tuned |
| `DEFAULT_MAX_GZIP_FIELD` | `65_536` | Guess: never tuned |
| `DEFAULT_MAX_GZIP_HEADERS` | `1_024` | Guess: added by R1-04, never tuned |
| `LITLEN_TABLE_BITS` | `10` | Tuned: P4-09 sweep of {9,10,11}, kept |
| `DIST_TABLE_BITS` | `8` | Tuned: P4-09 sweep of {7,8,9}, kept |
| one-shot growth floor `32768` (`stream.rs`, not in config.rs) | `32768` | Guess: a literal outside config.rs, flagged in review notes |

## Spec issues (edits to make in SPEC.md)

From PLAN.md:
- **§2.3 hooks mid-session**: hooks may be snapshotted at session start. SPEC should say to run Phase 0, restart Claude Code, then resume.
- **§2.3 denials**: the logger cannot see denied calls, so the guard has to log them. SPEC should say so.
- **§2.3/§2.4**: a tracked log that grows on every tool call conflicts with a clean tree. SPEC should allow a gitignored live log plus snapshots.
- **§2.3 forbidden-word paths**: SPEC should bless the `rfc1950/1951/1952` naming convention.
- **§2.4 commit tags**: SPEC should allow the tags to live in the commit body. Commits made with `--no-verify` stay untagged.
- **§2.1 provenance start**: the plan stage predates the harness. SPEC should acknowledge that gap.
- **§2.1**: SPEC refers to "zlib manual signatures written below", but none exist. Either add them or drop the reference.
- **§3.1 dev-deps**: SPEC should state that property tests live in the oracle crate.
- **§3.1 MSRV pin**: pinning requires a rustup download. This is accepted.
- **§4 generation matrix**: the full cross product (~9,450 streams per input) is infeasible. SPEC should adopt the bounded matrix.
- **§4 corpus hosting**: SPEC should say that neither corpus is committed, only hashes.
- **§5 M4 bomb ratio**: "1 KiB → >1 GiB" is impossible (the maximum is ~1032:1). The target should be restated.
- **§5 per-milestone review**: this does not match Foundry's single review per flight.
- **§5 M4 fuzz duration**: 1-hour runs cannot happen in-flight. SPEC should make them nightly or manual.
- **§5 M6 ordering**: the tag and the per-milestone review counts can only come after merge.
- **§3.2**: RFC 1951 is silent on incomplete codes, HDIST 31/32 and 284+31. SPEC should reference the DESIGN.md choices.
- **§3.3**: unbounded gzip FNAME/FCOMMENT conflicts with bounded memory. SPEC should adopt store-and-truncate.
- **§3.3**: the default output limit has no value. SPEC should state 1 GiB.

From REVIEW.md (rounds 1-4):
- **C16 vs C3 plus test placement**: say whether fixture checksums are pinned literals or whether those tests move to `crates/oracle/tests/` (round 1).
- **C12 bootstrap gap**: commits made before the tag hook exists are necessarily untagged. SPEC should acknowledge this (round 1).
- **§5 M4/§6 "zero divergences"**: this is only meaningful with a multi-member-aware oracle. SPEC should say so (round 1).
- **§5 M4's 70% throughput target is NOT MET** (25.6% raw, 27.3% zlib). SPEC says to report the number either way, but whether M4 counts as complete is a human call (rounds 2-4).

## Manual checks owed

**Phase 0**
- Spot-check `docs/rfc/*.txt` against rfc-editor.org and look for byte differences.
- Confirm `provenance/tool-log.jsonl` keeps accumulating after a Claude Code restart.
- Spot-check the Canterbury corpus files beyond the SHA-256 match.

**Phases 0-3 (CI)**
- Confirm the pushed branch's GitHub Actions `ci` run is green. The Canterbury fetch step is the likely failure, because the build host got Cloudflare 403s.

**Phase 4**
- Run `make fuzz-long`, or trigger `.github/workflows/fuzz-nightly.yml`, to get the 1-hour-per-target signal. Look for zero crashes and zero divergences in `fuzz/artifacts/divergences.jsonl`. Only 60 s smoke runs have happened.
- Re-run `make bench` on a quiet machine, check `docs/PERF.md`'s numbers and result line, and decide whether M4 is acceptable at ~26%.

**Phase 6 (after merge)**
- Run `make provenance-snapshot provenance-summary build-report` and commit. BUILD_REPORT's "Defects caught" and "Open divergence classes" sections predate the R1-R3 fixes.
- Tag `v0.1.0-cleanroom` and push the tag.
- Complete `provenance/human-interventions.md`, for example with the round-1 reopening of P0-08 and its 17-task skipped cascade, which the operator did by hand.
- Glance at the "### Fetch scripts" subsection of the regenerated `provenance/SUMMARY.md` and confirm it lists the three rfc-editor.org URLs plus the Canterbury and Silesia URLs.

## Review history

- **Round 1: CHANGES REQUESTED.** 9 findings, 8 fix tasks (R1-01..R1-08), 1 unblock (P0-08). No recurrence: this was the first round.
- **Round 2: CHANGES REQUESTED.** 6 findings, 5 fix tasks (R2-01..R2-05). **Recurrence:** F2-F5 trace to round-1 fix tasks R1-04, R1-02, R1-07 and R1-03, as regressions or unmet constraints.
- **Round 3: CHANGES REQUESTED.** 1 finding, 1 fix task (R3-01). **Recurrence:** F1 was R2-05's goal not met (fetch-script URLs), the same provenance-summary area as round 2's F6.
- **Round 4: APPROVED.** 0 findings, 0 fix tasks. None of the rounds was flagged non-converging.

**Notes-only approval:** round 4's REVIEW.md carries `## Notes` that were flagged but not queued as work:
- In `summarize.py`, a dead `MENTION_ONLY_VERBS` `continue`, and per-script headings rendered at the same `###` level as their parent.
- The `32768` growth floor is a literal outside `config.rs`.
- A stale `#[allow(dead_code)]` in `tables.rs`.
- Garbled "this crate's own this crate's own" comments from R1-01.
- The top half of HANDOFF.md is superseded round-0 text: it says 1 blocked, fuzz-long passed, and `Cargo.lock` gitignored, all no longer true.
- The guard's substring match on fetch-script names produces false positives.

## Pipeline friction

1. plan / spec-interaction: SPEC required a tracked, per-tool-call provenance log and per-commit tags. Foundry's clean-tree check, its fixed `<ID>:` subjects, and its own MCP-authored commits forced two workarounds: a gitignored log with snapshots, and a commit-msg hook. The skill has no guidance for projects that must log or tag Foundry's own activity.
2. implement / other: the allowlisted Canterbury URL returned Cloudflare 403 from the sandbox, which blocked P0-08 and cascaded skips to most corpus-dependent tasks. This was an environment constraint.
3. implement / other: after a human supplied the corpus and the fix was committed, no MCP tool could reopen a `[!]` or `[-]` task. P0-08 and its dependents stayed stuck, and a `foundry_task_reopen` tool would have been needed.
4. review / stall: `foundry_mutate` has no per-run timeout short of 1800 s. A mutation causing an infinite decode loop stalled the reviewer for 30 minutes and left orphaned test binaries. Parallel calls queued behind it and also timed out.
5. review / tool-gap: `foundry_review_submit`'s unblock reset P0-08 but left its 17 cascade-skipped dependents as `[-]`. The reviewer had no way to reopen them.
6. review / tool-gap: `foundry_mutate` has no per-call timeout override. A suspected-hanging mutation had to be reproduced by hand in a scratchpad copy with `timeout 60`.
7. implement / other: a `git commit -m` heredoc that only mentioned the download tool's name in prose was denied by the cleanroom guard's post-lockdown pattern, and the message had to be reworded.

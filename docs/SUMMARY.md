# SUMMARY: phase 2 (performance) build

**Merge line:** `build/2026-09-29-2` into `feature/perf`, base `13112c175a17` to head `eda63cf`, 88 commits (before this summary), 3 review rounds, final verdict **APPROVED**. 36 tasks were done, with 0 blocked and 0 skipped.

**Headline result (docs/PERF.md, `### Final (P5-02)`):** On Silesia level 6, cleanflate reaches 66.9% of C zlib raw, 68.8% zlib and 68.8% gzip for one-shot decoding. The target was 70%, so it is **NOT MET**. Streaming with 64 KiB chunks reaches 74.9% raw against a 60% target, so it is **MET**. One-shot raw went from 62.3 to 190.1 MiB/s and streaming raw from 65.0 to 212.8 MiB/s. There were 0 per-file REGRESSION lines, 0 LOSS lines and 36 WIN aggregates.

## What was built

- **Phase 0 (M0), baseline, instrumentation and safety net (P0-01..P0-13).**
  - Added the opt-in `stats` and `test-hooks` features, with Makefile targets and ignore rules.
  - The clean-room guard now denies profiling of anything except the cleanflate-only `profile_decode` example.
  - design-check enforces the `### D-NN: [perf]` format.
  - Added a `DecodePath::{Fast, Reference}` switch, plus one table-width-independent rule for when an invalid code is reported.
  - Added a fast-vs-reference differential (`crates/oracle/tests/fastslow.rs`) that compares every call, and a regression fixture store (`testdata/regressions/`, `oracle regress-add`/`regress-seed`).
  - Added count-only stats counters, a bench harness (medians, framings, levels, modes, JSON), the `bench-compare`/`bench-noise`/`bench-md` commands, and a `make profile` entry point.
  - Recorded the baseline, the noise measurement and the H1-H6 hypothesis profile.
- **Phase 1 (M1), bit reader and symbol decoding (P1-01..P1-05).**
  - Bulk bit refill, with an exact give-back invariant (D-27) so the Fast reader always holds exactly the bits the Reference reader holds.
  - Two-level lookup tables for every symbol (D-28).
  - Per-block table reuse (P1-03) and length/distance payloads in table entries (P1-04) were gated out as too cheap to matter, with no code change.
- **Phase 2 (M2), fast decode loop (P2-01..P2-04).**
  - A fast literal/length/distance loop (D-29) that consumes only whole symbols and leaves through the shared exit.
  - A new fuzz target, `diff_fast_reference`.
  - Copyable slow-path state (P2-03) was gated out.
- **Phase 3 (M3), output and match history (P3-01..P3-04).**
  - The output buffer now serves as match history. The window is carried only across calls and refreshed at every `step` return (D-30).
  - One-shot output sizing uses the gzip ISIZE and an input-ratio hint, capped by `max_output` and `ONESHOT_MAX_PREALLOC` (D-31).
  - Overlapping-match pattern doubling (P3-02) was implemented and measured twice, then reverted as neutral. Its tests and measurements stay.
- **Phase 4 (M4), checksums, wrappers and tuning (P4-01..P4-04).**
  - CRC-32 now processes several bytes at a time, 8 slices (D-32).
  - An Adler-32 revisit (P4-02) was gated out.
  - All six new tunables plus the two table widths were re-swept on the final code, and no default changed.
- **Phase 5 (M5), report and hand-off (P5-01..P5-03).**
  - The provenance summary and build report are scoped to this phase with `--since`/`--since-commit` (`PHASE_BASE ?= 599ca51`).
  - The final PERF.md phase-2 report was written, including the negative results, and DESIGN was audited.
  - Provenance was completed and the branch was pushed.
- **Review fixes (R1-01, R1-02, R2-01).**
  - These are test-only hardening changes, plus one `debug_assert!`, two doc fixes and two `pub(crate)` read-only accessors.
  - They pin the D-27 pending-literal give-back, the fast-loop distance `Invalid` error, and the flow of the decode path into the zlib and raw-with-dictionary inflaters.

## Decisions that shaped it

**From PLAN.md:**

- **Phases vs milestones (plan):** Phase N implements SPEC 6 milestone MN, in order, not merged.
- **Review cadence (plan):** One Foundry review loop runs per flight, with up to 6 rounds. Each phase ends with an acceptance-and-push task instead of its own review.
- **Branches (plan):** The base is `feature/perf`. Build branches use the `build/` prefix and PR into `feature/perf`.
- **Dependencies (plan):** Dependencies are narrow. Optimisation tasks depend on the M0 infrastructure, never on acceptance tasks.
- **Task order (plan, P0-12):** Milestone order is fixed instead of ordered by measured cost. The cheap hypotheses (P1-03, P1-04, P2-03, P4-02) are **gated**: each is dropped when its cost is below max(2%, 2x noise) of samples.
- **Slow-path switch (P0-04):** This is the cargo feature `test-hooks`, off by default, which the oracle enables only as a dev-dependency. It is not `cfg(test)`.
- **What Reference switches off (plan):** Bulk refill, table lookup, the fast loop and slice/doubling copies. Shared storage changes and checksums are not switched; conformance and oracle tests cover them instead.
- **"Identical consumed" (P1-01):** Consumed is compared on every call, together with produced, status, the full `Error` value, `finish()`, totals, `trailing_start` and the member count.
- **Invalid-code rule (P0-04):** A code is `Invalid` as soon as at least 1 buffered bit proves that no code matches it. This rule does not depend on table width.
- **Stats (P0-07):** Stats are counts only, never time (C7). Counters are per-instance fields, with no global state. The API is `Stats`, `Decompressor::stats()` and `decompress_with_stats`.
- **Profiling entry point (P0-10):** The profiling entry point is `examples/profile_decode.rs`, so the profiled process contains no C code. The guard mechanically denies other profiling targets (P0-02).
- **Streaming target (plan):** The streaming target is measured against C zlib **one-shot**, which is the conservative denominator.
- **Bench method (P0-08):** Each run does a verifying warm-up and then N samples: 7 for full, 5 for quick. Each sample lasts at least 20 ms, with the implementations interleaved. Results report the median and the min. Quick = Canterbury + Silesia {dickens, mr, ooffice, sao, xml}.
- **Noise and verdicts (P0-09):** Noise is the relative difference between two baseline runs. `WIN` means the delta is more than 2x the noise. A per-file `REGRESSION` means the delta is below -max(file noise, aggregate noise).
- **Timing files (plan):** Timings live only in `docs/perf/*.json` from `oracle bench`. PERF.md tables are generated from them.
- **Benchmark gates (plan):** Benchmarks are not CI gates. An unmet performance target is recorded as NOT MET and does not block. A correctness failure does block.
- **3000-case rule (plan):** `make proptest-3k` runs through `extraVerify` for any change under `crates/cleanflate/src/`. Acceptance tasks run `make heavy`.
- **Tunables (plan, P4-03):** The six new tunables live in config.rs and were re-swept in P4-03.
- **Size hints (P3-03):** ISIZE and the input-ratio hint are used for allocation only, capped by `max_output` and `ONESHOT_MAX_PREALLOC`.
- **Toolchain (plan):** 1.89.0, with no SIMD, arch intrinsics, threads or nightly (constraint-enforced).
- **Task-ID collisions (P5-01):** Phase-2 IDs collide with phase-1 IDs, so reports are scoped with `--since-commit`.
- **DESIGN numbering (P0-03):** New DESIGN entries take the next free `D-NN`, and performance entries use the `[perf]` prefix.
- **Tagging (P5-03):** The implementer never tags. A human tags `v0.2.0-cleanroom`.

**Interpretation choices (HANDOFF.md):**

- **P0-02:** `perf list`/`--version` is allowed only as the first argument, and `--help` only as the sole argument.
- **P0-04:**
  - With fewer than `table_bits` bits buffered, the table path hands off to the bit-serial walk. D-28 later superseded this.
  - Test-only helpers use `cfg_attr(not(test), allow(dead_code))`.
- **P0-05:** `fastslow_malformed` counts a shared error as agreement. Streaming conformance also checks the manifest hash.
- **P0-06:** A row with a single chunk list uses 1021/997 for the missing side. `err` rows are checked by `ErrorKind` one-shot and by Debug prefix in traces.
- **P0-07:**
  - `table_builds` excludes the two fixed tables built at construction.
  - `fixed_table_clones` counts 2 per fixed block.
  - `window_bytes_written` excludes the dictionary prefill.
  - Gzip stats are kept in a boxed field.
- **P0-08:** `reps` is chosen per implementation. Streaming uses a pre-sized scratch buffer. A missing corpus is an error.
- **P0-09:**
  - With no noise file, the per-file regression check is skipped and says so.
  - A rounding tolerance was added in a separate `chore:` commit.
- **P0-13 / P1-05 / P2-04 / P3-04 / P4-04:** The per-file REGRESSION and LOSS lines against earlier milestone runs are recorded as run-to-run drift, not as failures.
- **P1-01:** Give-back runs on both paths and returns 0 on Reference. `step` wraps `step_inner` so that a single exit helper handles every exit.
- **P1-02:** This task was kept as NEUTRAL, because it is the prerequisite of P2-01.
- **P2-01:** After a margin exit, one state-machine dispatch runs before the fast loop is retried. There is also a defensive `Copy` exit.
- **P3-01:**
  - `step_inner` returns `(pos, out_pos, result)`, so the window is refreshed exactly once on every return.
  - `window_bytes_written` counts the bytes actually copied.
- **P3-03:** Growth happens only on `OutputFull`. This removes the extra buffer gzip allocated at a member boundary.
- **P4-02:** The gate was read against 2x the quick-bench noise.
- **P4-03:** Quick WINs pointing in both directions were treated as drift, so no default changed. Review noted that the full-bench confirmation was skipped.
- **P5-01:** Log entries without a parseable timestamp are dropped. An unresolvable revision is an error.
- **P5-03:** `provenance/human-interventions.md` gets one dated sentence.
- **R1-02:** The block is hand-built, because the dynamic builder always appends an end-of-block code.

## Assumptions still in play

No config key is marked `⚠️ ASSUMPTION`. The keys this phase added, all in `crates/cleanflate/src/config.rs`:

| key | default | status |
|---|---:|---|
| `FAST_LOOP_MIN_INPUT` | 16 | tuned: swept {8, 16, 32}, kept |
| `FAST_LOOP_MIN_OUTPUT` | 258 | tuned: swept {258, 320, 512}, kept |
| `CRC32_SLICES` | 8 | tuned: swept {4, 8, 16}, kept |
| `ONESHOT_INITIAL_RATIO` | 4 | tuned: swept {2, 4, 8}, kept |
| `ONESHOT_GROWTH_FLOOR` | 32768 | safety bound, not swept |
| `ONESHOT_MAX_PREALLOC` | 64 MiB | safety bound, not swept (see spec issues) |

`LITLEN_TABLE_BITS` (10) and `DIST_TABLE_BITS` (8) were re-swept and kept. The sweeps ran on a noisy machine (about 10% drift), so their conclusions are weak.

## Spec issues (edits a human should consider making to SPEC.md)

From PLAN.md:

- **4.1 streaming target denominator:** The denominator is not stated. The plan resolved it as C zlib one-shot.
- **4.3 "time or calls spent building tables" vs C7:** The plan resolved this as counts only.
- **4.4 / M0 "order by measured cost":** An unattended plan cannot be reordered. The plan used a fixed order plus gated tasks.
- **5 "cfg(test)-only or feature-gated":** `cfg(test)` cannot reach the oracle's tests. The plan used feature `test-hooks`.
- **5 "identical consumed":** This implies the exact give-back invariant (D-27). SPEC should state it.
- **Phase-1 D-16 vs the serial walk:** They disagreed on invalid-code timing. Consider noting in SPEC 3 that ErrorKind may move from `Truncated` to `InvalidSymbol`, but only for streams that are both truncated and invalid.
- **4.2 full bench runtime:** A full run takes 10-15 minutes, which exceeds the agent's foreground limit.
- **4.1 per-file regression vs noise:** The threshold is max(file noise, aggregate noise).
- **M3 "gzip ISIZE when trustworthy":** ISIZE is never trustworthy for allocation. It is used only as a capped hint.
- **3 "additive changes only if measured":** The `stats` and `test-hooks` APIs are treated as allowed because SPEC itself requires them.
- **4.2 Silesia subset:** SPEC does not name it. The plan chose dickens, mr, ooffice, sao and xml.
- **M5 BUILD_REPORT/SUMMARY "for phase 2":** Task IDs collide with phase 1. The plan resolved it with `--since` scoping.
- **8 dates:** The profiler install is dated 2026-09-30 but SPEC was committed 2026-09-29. This is cosmetic.

From REVIEW.md, rounds 1-3:

- **M3 one-shot sizing cap:**
  - An 18-byte hostile gzip input with ISIZE 0xFFFFFFFF makes `decompress_with` allocate and zero-fill 64 MiB before it decodes a byte. This is bounded and within SPEC.
  - Consider a smaller `ONESHOT_MAX_PREALLOC`, or trusting the hint only up to a multiple of `input.len()`.
- **4.1 targets:** The 70% one-shot target is NOT MET: 66.9 / 68.8 / 68.8%. Streaming at 74.9% is MET. SPEC may want to reset the target or accept the shortfall.

## Manual checks owed (all NOT VERIFIED)

- **Phase 0 (P0-11 / P0-13):**
  - Aggregate noise is 6.1% for full and 6.5% for quick, above the 5% and 3% goals, and drift is about 10%. Re-run the baseline and the final `make bench` on a quiet machine with a pinned governor (SPEC 8). Look for whether the one-shot ratios cross 70%.
  - Sanity-check the P0-12 profile tables against `make flamegraph`.
  - The denied provenance entries are benign: a recursive grep at the repository root and a `sed` regex containing a forbidden word.
- **Every phase end:** Confirm GitHub Actions `ci` is green on the pushed branch, including the `stats` and `test-hooks` feature jobs.
- **Phase 1 (P1-05):** Review D-27 and D-28 for clean-room wording (SPEC 2.1).
- **Phase 2 (P2-02 / P2-04):**
  - Review the fast-loop code by hand. Review rounds 1-3 mutation-sampled it.
  - Run `make fuzz-long` or the nightly workflow for all six targets, including `diff_fast_reference`, for an hour each, then triage `make fuzz-divergences`. Only `make fuzz-smoke` has been run, and it found no crashes.
- **Phase 3 (P3-04):**
  - Review D-30 and D-31.
  - Compare peak RSS of `make bomb` against a phase-1 checkout with `/usr/bin/time -v`. `size_of::<Decompressor>()` is unchanged at 33,688 bytes.
- **Phase 4 (P4-04):** Review the CRC derivation in D-32.
- **Phase 5 (P5-03):**
  - Review the PERF.md result line and the negative results.
  - Complete `provenance/human-interventions.md`.
  - After merging into `feature/perf` and then `main`, tag `v0.2.0-cleanroom`.

## Review history

- **Round 1: CHANGES REQUESTED.** 2 findings, 2 fix tasks (R1-01, R1-02), 0 unblocked.
  - F1 on P1-01: the pending-literal give-back was untested.
  - F2 on P2-01: a fast-loop distance `Invalid` was caught only by chance.
  - The review also noted two stale doc comments, the P3-01 commit size and the skipped P4-03 full-bench confirmation.
- **Round 2: CHANGES REQUESTED.** 1 finding, 1 fix task (R2-01), 0 unblocked.
  - F1 on P0-04: the decode path was not tested to reach the zlib and raw-dictionary inflaters.
  - No finding recurred from round 1.
- **Round 3: APPROVED.** 0 findings, 0 fix tasks. No finding recurred.
  - All three round-2 mutants were killed by tests.
  - This was a **notes-only approval**: REVIEW.md carries a `## Notes` section. It observes that the "Raw with dictionary" case decodes a stored block and never reads the dictionary, which `tests/zlib.rs::raw_with_dictionary_prefill` already covers. It also repeats that the P3-01 commit-size and P4-03 full-bench notes stand. None of this was queued as work.
- None of the rounds was non-converging.

## Pipeline friction

1. **plan / ambiguous-prompt:** plan-build asks for a PLAN.md `## Conventions` section. It also says the implementer reads only CLAUDE.md, the task and the cited SPEC sections. The shared cross-task protocols (bench-before/after, the gated-task rule, the repro rule) therefore had to move into CLAUDE.md and be cross-referenced from each task. The skill could say explicitly where shared per-task protocols belong.
2. **review / tool-failure:**
   - `foundry_mutate` returned "Connection closed" on its second call, the fast_loop.rs dsym>=30 mutation, with no result. The verdict was lost and had to be retried.
   - The first mutate result, 136 KB of `make test` output, overflowed the tool-result limit and had to be parsed from a saved file.
3. **review / tool-side-effect:** `foundry_mutate` restored the mutated file, but a killed mutant made proptest append a failing seed to the tracked `crates/oracle/tests/chunking.proptest-regressions`. That dirtied the tree and had to be checked out by hand. Mutate should restore or snapshot every tracked file its commands touch.
4. **controller / mcp-disconnect:** The foundry MCP server disconnected after review round 1, so `foundry_next` was unavailable. The flight paused until the operator reconnected with `/mcp`. Because of this, R2-01's mutants were confirmed by hand with sed and cargo test.

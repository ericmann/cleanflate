# Build summary: cleanflate phase 3 (performance)

**Merge line:** `build/2026-09-30`, `main` `fb7bca0` → `1035017` (plus this summary commit). 85 commits, 37 tasks (36 planned + 1 review fix), 2 review rounds. Final verdict: **APPROVED**.

**Headline:** one optimisation moved throughput: P4-03, the inlined fast-loop lookup. It gave +18% to +31% on every Silesia and Canterbury aggregate at levels 1 and 6. On Silesia level 6, cleanflate compared like for like with C zlib is at 84.1% raw, 85.7% zlib and 85.4% gzip in one-shot mode. Phase 2 was at 66.9%, 68.8% and 68.8%. Streaming is at 81.8%, 83.9% and 78.0%. **T5 is met. T1, T2, T3, T4 and T6 are not met.** `docs/perf/PHASE3.md` `## Targets` records each shortfall and its next step. None of the targets block the merge.

## What was built

**Phase 0, M0 bench hygiene (P0-01..P0-10).**
- Scaffolding: fuzz gates, quick-bench levels 1 and 6, a copy of the phase-2 baseline, and a report skeleton.
- `make design-check` now requires a `Profile:` line in phase-3 `[perf]` DESIGN entries (D-34 onward).
- The oracle FFI gained C zlib build flags and a bounded streaming inflate. With it, streaming rows are compared with C zlib streaming, like for like.
- Bench JSON schema 2: `best` replaces `min`. Schema-1 files still load.
- Every run captures its environment.
- `bench-compare` gives a drift-corrected verdict based on the like-for-like ratio.
- `oracle bench-targets` computes T1-T6 mechanically.
- The phase-3 starting baseline and noise files were recorded.

**Phase 1, M1 profile first (P1-01..P1-03).**
- `make profile`, `profile-stat` and `flamegraph-save` work on any bench key.
- Before-profiles and flamegraphs were committed.
- `docs/perf/phase3-profile.md` ranks the hotspots and predicts which later tasks would be gated out.

**Phase 2, M2 checksums and the one-shot output path (P2-01..P2-09).**
- A cleanflate-only checksum microbenchmark, behind `test-hooks`.
- Three tasks were gated out because the profile showed too little in them: the Adler-32 batch (P2-02), wrapper overhead (P2-04) and the size-hint revisit (P2-08).
- CRC-32 slicing-by-16 (P2-03) was measured and reverted. `CRC32_SLICES` stays 8.
- The decoder core gained a crate-private `OutputSink` abstraction: `SliceSink`, plus a capacity-bounded `VecSink` with a sink differential (P2-05, P2-06).
- The public one-shot switch to the Vec sink (P2-07) was measured and reverted under PT4. The Vec path is still in the library but reachable only through `test-hooks`.

**Phase 3, M3 per-stream setup (P3-01..P3-05).**
- Per-block Huffman table reuse (P3-01) and per-stream setup changes (P3-02) were gated out.
- A new public `Decompressor::reset()` (DESIGN D-35) returns the decoder to the state of a freshly built one and keeps its allocations. A reset differential (unit, conformance, options and proptest) checks it.
- A new "reuse" bench mode (P3-04) measures cleanflate only, so its `ratio` is null.

**Phase 4, M4 decode hot loop (P4-01..P4-06).**
- An assembly-evidence tool: `make asm` / `asm-count` and `tools/perf/asm_branches.py`.
- Branch-light refill (P4-02) was measured and reverted.
- **P4-03 (D-36):** the fast loop does its own code lookups on fixed-size first-level table views and keeps the bit buffer in locals. Instructions dropped by 29%. This is the only kept performance win of the phase.
- Multi-literal table entries (P4-04) were reverted.
- The table widths were re-swept on the final loop (P4-05), and 10/8 were kept.

**Phase 5, M5 report and hand-off (P5-01..P5-03).**
- After-profiles and flamegraphs.
- The final bench (`docs/perf/phase3/P5-02.json`).
- The `docs/perf/PHASE3.md` report and the PERF.md phase-3 result line.
- Provenance, build report and push.

**Review fix (R1-01).** The reset differential now covers re-applying a raw dictionary. An empty selection now fails instead of skipping (C13), and `reset_proptest` resets at random points mid-stream. Only tests changed.

## Decisions that shaped it

From PLAN.md `## Decisions`:
- **Phases and milestones (plan):** phase N is milestone MN. There is one Foundry review per flight, and each phase ends with an acceptance-and-push task.
- **Dependencies (plan):** they are narrow. Optimisation tasks depend only on the infrastructure they use, never on acceptance tasks.
- **Gating instead of re-ordering (plan, P1-02):** "re-order M2-M4 by profile" was implemented as profile gates. A task whose hotspot is below max(2%, 2x noise) records a negative result and makes no code change.
- **Bench README (plan):** it lives in `docs/perf/README.md`.
- **Schema 2 (P0-04):** `min` was renamed `best`. Schema-1 files load through serde aliases.
- **Like-for-like denominators (P0-03, P0-05):**
  - Streaming is compared with C zlib streaming, using 64 KiB in/out windows.
  - Reuse rows have no C denominator, because `inflateReset` is not a permitted FFI signature.
- **Drift (P0-07):** a change is a WIN when the throughput verdict or the ratio verdict is WIN and neither is LOSS.
- **Quick bench (P0-01):** levels 1 and 6. Reuse mode joins in P3-04.
- **Phase-3 file layout (plan):**
  - JSONs live under `docs/perf/phase3/`, because the task IDs collide with phase 2's.
  - Write-ups go in `docs/perf/PHASE3.md`, and PERF.md holds only a pointer and the result line.
- **Profile evidence (P0-02):** `Profile:` is required in `[perf]` entries D-34 and later.
- **Fuzz gates (plan):**
  - Per task: `make fuzz-smoke-decoder`, 60 s per target, plus `proptest-3k`.
  - Per milestone: `make fuzz-milestone`, 30 minutes, including M0 and M1.
- **Checksum microbench (P2-01):** a `test-hooks` example. No C code runs in the process.
- **Adler batch and CRC slices (P2-02, P2-03):** the Adler batch is a derived constant, not a tunable. CRC slicing goes through the existing `CRC32_SLICES` tunable.
- **Output sink (P2-05..P2-07):** split into three tasks, so that the public switch could be reverted on its own (and it was).
- **Vec sink growth (P2-06):** capacity grows only by `reserve_exact` on the existing `next_len` schedule. Memory bounds are unchanged.
- **Reset API (P3-03):**
  - No `reset_with`. There is only `reset(&mut self)`, which keeps format, options and decode path.
  - After `reset()`, every observable equals that of a freshly built decoder.
- **Wrapper overhead (P2-04):** handled as a gated task.
- **Hot-loop scope (P4):** limited to SPEC's list. Length/distance payloads in table entries and a short-match copy path were not planned.
- **Bounds-check evidence (P4-01):** `--emit asm` plus a counting script. No cargo-asm.
- **Targets (P0-08):**
  - Computed mechanically, recorded and never blocking.
  - T2's "row" is an aggregate key.
  - T4 is measured as (raw - framed) / raw ≤ 3%.
  - T6 is `bench-compare` against the phase-2 baseline.
- **M4 WIN (P4-06):** level-1 keys, P3-05 JSON against P4-06 JSON.
- **File names (plan):** they use `rfc195x` slugs, never framing names (C11).
- **Report scoping (P5-03):** `PHASE_BASE ?= cd4a472`.
- **DESIGN numbering (plan):** each new entry takes the next free `D-NN`.
- **Tagging (plan):** the implementer never tags. A human tags `v0.3.0-cleanroom` after merge.

Interpretation choices from HANDOFF.md:
- **P0-02:** the `[perf]` test fixtures were renumbered from D-90..92 to D-30..32, so they stay below the `Profile:` threshold of 34.
- **P0-03:** when a call makes no progress, a full output buffer maps to `OutputLimit` and otherwise exhausted input maps to `Truncated`.
- **P0-04:** the noise file keeps `min_sample_ms` and stays at schema 1.
- **P0-05:** `bench-md` per-file rows use the same-mode C zlib row, and fall back to one-shot for schema-1 files.
- **P0-06:** `cargo metadata` runs with `--offline --filter-platform <host>`. `loadavg_end` is recorded in the JSON only.
- **P0-07:** `compare()` and `render_md()` keep their signatures. The ratio verdict is `UNKNOWN` when a noise file has no ratio noise.
- **P0-08:** "reuse rows" are the non-one-shot, non-streaming aggregates. A target with no rows it can judge prints `n/a`.
- **P1-01:** flamegraphs use `--no-inline`, because the inline view strips the crate paths that the `cleanflate::` frame check needs.
- **P1-02:** unresolved samples are reported as unattributed. The guard denies `perf report --sort dso`, and this was not worked around.
- **P2-01:**
  - `crc32_update_sliced::<N>` falls back to the bytewise path when N is outside 4..=16, instead of asserting.
  - There is one shared 16-row CRC table, `CRC32_MAX_SLICES`. It is a derived capacity, not a tunable.
- **P2-05:** the generic entry points are `step_into` and `copy_match_sink`. The slice-shaped wrappers live in `src/tests/slice_api.rs`.
- **P2-06:**
  - `VecSink` never grows the Vec itself, and `OutputFull` means the capacity is full.
  - The driver grows the buffer by `next_len`, and the limit logic is shared with `decompress`.
- **P2-07:** the switch was reverted. `decompress_core` became `pub(crate)` so the tests can compare both drivers.
- **P2-08:** the gate was judged on the named growth, realloc and copy frames (there were none) plus an over-allocation bound.
- **P3-03:**
  - `Inflater::reset` was added here, because P3-02 was gated out.
  - In the conformance reset test, each entry gets one prefix scenario. One-byte chunks run only for every fourth entry of up to 64 KiB.
- **P3-04:** `Aggregate.ratio` and `c_zlib_mode` became `Option`.
- **P4-01:** the fast loop is inlined into `step_into`, so assembly is attributed by `.loc` source file (`--src`, `ASM_SRCS`).
- **P4-03:** `fast_loop_all_table_widths_same_trace` was replaced by the width check in `fast_table_lookup_matches_decode_table`, because there is no runtime width to vary.
- **P4-04:** the gate was judged by the literal share of symbols (46.6%). No `perf annotate`.
- **P4-05:** a candidate counts as a WIN only if it is a quick-bench WIN on the level-1 raw keys against both default runs. None qualified.
- **R1-01:** the raw-dictionary case uses a fixed dictionary and cyclic data compressed by the oracle. It needs no checksum, so C16 holds.

## Assumptions still in play

No `⚠️ ASSUMPTION` markers remain in `crates/cleanflate/src/config.rs`. No tunable changed value in this phase. Their current status:

| Key | Default | Status |
|---|---|---|
| `LITLEN_TABLE_BITS` | 10 | Tuned. Re-swept on the final loop (P4-05) and kept. |
| `DIST_TABLE_BITS` | 8 | Tuned. Re-swept (P4-05) and kept. |
| `CRC32_SLICES` | 8 | Tuned. Measured against 16 (P2-03), and 16 lost end to end. It is the first thing to revisit, because CRC-32 is now 12.4% of gzip samples. |
| `FAST_LOOP_MIN_INPUT` | 16 | Swept in phase 2 and not changed this phase. 8 is a safety floor. |
| `FAST_LOOP_MIN_OUTPUT` | 258 | Swept in phase 2 and not changed. 258 is a safety floor. |
| `ONESHOT_INITIAL_RATIO` | 4 | Swept in phase 2 and within drift. Revisit was gated out (P2-08). |
| `ONESHOT_GROWTH_FLOOR` | 32768 | A safety bound that was never swept. Still a guess. |
| `ONESHOT_MAX_PREALLOC` | 64 MiB | A safety bound that was never swept. Still a guess. |
| `DEFAULT_MAX_OUTPUT`, `DEFAULT_MAX_GZIP_FIELD`, `DEFAULT_MAX_GZIP_HEADERS` | 1 GiB, 65536, 1024 | Documented policy defaults. They are not performance tunables. |

## Spec issues

None were raised in review: rounds 1 and 2 both report "None". PLAN.md resolved the following in the plan. Consider editing SPEC.md for each:
- **§4.1/§4.2 reuse mode:** reuse has no like-for-like C denominator, because `inflateReset` is not a permitted FFI signature. State that T3's "every mode" excludes reuse.
- **§4.1 T2:** one-shot includes kernel page-fault time that streaming's reused buffer avoids, so T2 may be unreachable without changing what is measured. Either reword T2 or accept that it measures this difference.
- **§6 M1:** "re-order M2-M4 by profile" cannot be done in an unattended plan. Reword it as profile gates.
- **§5/§9:** milestone fuzz runs at M0 and M1, where no decoder code changed. Make this optional or say so explicitly.
- **§6 M4:** phase 2's other next steps (length/distance payloads in entries, a short-match copy path) are excluded. Add them if wanted.
- **§6 M2:** "per implementation variant" needs `test-hooks` entry points. Name the feature in SPEC.
- **§4.2:** define "cargo profile flags" and "libz-sys version and enabled features".
- **§2/§3/§4.3:** SPEC cites PT1-PT8 and CR7, which the root CLAUDE.md on `main` lacked. They are restored now. Make sure SPEC and CLAUDE.md stay in sync.
- **§3 additive API:** list the `test_hooks` additions as allowed, opt-in test surface.
- **Noise:** about 6% noise and 10% drift is larger than most remaining gains. Require a quiet, pinned-governor machine for numbers of record.
- **Acceptance cost:** about an hour per milestone, six times. Spell out the §8 fuzz-shortening allowance.
- **§6 M2:** the conditional "revisit the hint" task. Say that it is a gated task.

## Manual checks owed

- **M0 (P0-10):**
  - GitHub Actions `ci` is green on the branch.
  - Re-measure the phase-3 start noise on a quiet machine with a pinned governor. It was 8.2% full and 8.8% quick, above the 3% target.
  - Spot-check the Environment block: `c_zlib.compile_flags` 0xa9 and `libz_sys_features`.
  - Confirm there are no unexplained provenance denials. All nine are explained in `provenance/human-interventions.md`.
- **M1 (P1-03):** open the three before-flamegraphs and check the `phase3-profile.md` tables against them. They should show only cleanflate, std and libc frames.
- **M2 (P2-09):**
  - Measure the peak RSS of `make bomb` and of a one-shot Silesia decode with `/usr/bin/time -v`, and compare it with the phase-3 start.
  - Review the sink code by hand and mutation-sample it.
- **M3 (P3-05):**
  - Review the `Decompressor::reset` rustdoc and DESIGN D-35 as a public API addition.
  - Measure the peak RSS of `make bomb`.
- **M4 (P4-06):**
  - Review the fast loop by hand and mutation-sample it. Prefer mutants that cannot run away: the `dsym >= 30 -> >= 32` mutant got no verdict.
  - Review D-36 for clean-room wording.
- **M5 (P5-03):**
  - Run the final `make bench` on a quiet, pinned-governor machine and compare it with `P5-02.json`.
  - Run `make fuzz-long` (or the nightly workflow) on all six targets and triage `make fuzz-divergences`.
  - Review PHASE3.md: the T1-T6 verdicts, the attribution and the negative results. Also review the PERF.md result line, the flamegraphs and the assembly evidence.
  - Tag `v0.3.0-cleanroom` after merge.
  - Complete `provenance/human-interventions.md`.
- **Known staleness:** `docs/BUILD_REPORT.md` and `provenance/SUMMARY.md` were generated before the last bookkeeping commits, so their commit count is slightly behind.

## Review history

- **Round 1: CHANGES REQUESTED.** 1 finding (F1, P3-03), which became 1 fix task (R1-01). No recurrence, because this was the first round.
  - F1: the reset differential silently skipped the raw-dictionary case, and the re-prefill in `Backend::reset` had no test. A mutation of it survived.
- **Round 2: APPROVED.** 0 findings and 0 fix tasks. No finding recurred: P3-03 does not reappear, and F1 is closed (the mutant is now killed). Neither round was non-converging.
- **Approved with notes.** Round 2 approved but carried a `## Notes` section with items that were not queued as work:
  - The prefix loop in `reset_proptest` drives only valid input. Corrupted prefixes are covered by scenario 4.
  - The `member_crc` reset in `GzipDecoder::reset` is redundant but harmless.
  - The Vec-sink driver is reachable only through `test-hooks`.
  - `CRC32_TABLES` has 16 rows, but production uses 8.
  - Round 1 notes, also unqueued: `reset_equals_fresh_conformance` alone does not catch a `Window::reset` mutant.

## Pipeline friction

Four entries, in the order they were logged:
- **plan / id-collision:** task IDs `P<phase>-<nn>` repeat across flights. Phase-3 artifacts had to be namespaced under `docs/perf/phase3/`, and history tags collide. An optional flight prefix for task IDs would fix this.
- **implement / tool-refusal:**
  - The cleanroom guard denies a whole Bash command, including background jobs started in it, whenever its text contains forbidden words. Large scripts had to go through the Write tool, and one denied command silently skipped a bench.
  - Waiting on jobs that run 30 minutes or more needs repeated 10-minute polls.
- **review / mutate-residue:** `foundry_mutate` left an untracked `crates/oracle/tests/reset.proptest-regressions` file, which had to be deleted by hand.
- **review / stall:** `foundry_mutate` on the `fast_loop.rs` mutant `dsym >= 30 -> >= 32` returned "Connection closed" with no verdict. It was probably a runaway mutant. The tree was clean afterwards.

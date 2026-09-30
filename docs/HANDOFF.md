# HANDOFF: phase 2 (performance) implementation

- Branch: `build/2026-09-29-2` (base `feature/perf`, plan commit `13112c1`)
- Head at the last task: `2db0ec2` (before this file's commit)
- Tasks: 33 done, 0 blocked, 0 skipped. Round 0 (initial build).
- Final result line (docs/PERF.md, `### Final (P5-02)`): phase 2: cleanflate / C zlib (Silesia, level 6, one-shot) = 66.9% raw, 68.8% zlib, 68.8% gzip (target 70%: NOT MET); streaming 64 KiB = 74.9% raw (target 60%: MET).

## Outcome in numbers

Silesia level 6, median MiB/s (baseline `docs/perf/baseline.json` to final `docs/perf/P5-02.json`): one-shot raw 62.3 to 190.1, zlib 61.3 to 177.3, gzip 51.8 to 177.7; streaming raw 65.0 to 212.8, zlib 63.7 to 198.8, gzip 55.7 to 191.6. Against baseline: 0 per-file REGRESSION, 0 LOSS, 36 WIN aggregates. Primary one-shot target (70%) is missed by 3.1 / 1.2 / 1.2 points; streaming (60%) is met by 14 to 17 points; stretch (90%) not met.

## Blocked and skipped tasks

None.

## Gated tasks and reverted attempts (negative results, PT2/PT4)

- P1-03 table reuse: gated out (tables 1.7% of samples). No code.
- P1-04 length/distance payload in entries: gated out (1.4%). No code.
- P2-03 copyable state: gated out after the fast loop (State clone 0.15%, `Inflater::step` self 0.5%). No code.
- P3-02 pattern doubling: implemented and measured twice (run 1: 6 WIN / 6 NEUTRAL; run 2: 1 WIN / 1 LOSS / 10 NEUTRAL), code reverted per PT4; its two tests and `docs/perf/P3-02*.json` stay.
- P4-02 Adler-32: gated out (share 6.15% but at most +6.6% attainable, below twice the quick-bench noise). No code.
- P4-03 tunables: swept all six keys (20 runs plus a repeat of the defaults); no default changed.

## Interpretation choices (task id: choice)

- P0-01: none.
- P0-02: `perf list`/`--version` allowed only as the first argument, `--help` only as the sole argument.
- P0-04: the table path delegates to the bit-serial walk when fewer than `table_bits` bits are buffered (equivalent to the specified "Invalid entry with available >= 1"); dead-code allows on test-only helpers are `cfg_attr(not(test), ...)` because the oracle dev-dependency enables `test-hooks` in workspace builds.
- P0-05: `fastslow_malformed` compares one-shot agreement (a shared error counts as agreement) and traces; conformance streaming also checks the manifest hash.
- P0-06: a row with only one chunk list uses 1021/997 for the missing side; `err` rows are checked by `ErrorKind` one-shot and by Debug prefix in traces.
- P0-07: `table_builds` excludes the two fixed tables built at decoder construction; `fixed_table_clones` counts 2 per fixed block; `window_bytes_written` excludes dictionary prefill; gzip stats are accumulated in a boxed field to keep `Backend` variants balanced.
- P0-08: `reps` chosen per implementation; streaming mode uses a pre-sized scratch buffer; a missing corpus is an error (no Canterbury fallback).
- P0-09: with no noise file, the per-file regression check is skipped (printed) rather than using a zero threshold. A rounding tolerance was added in a separate `chore:` commit after the baseline pair exposed false REGRESSION lines (a P0-09 defect, `a_pair_is_neutral_against_its_own_noise`).
- P0-10: `perf report -g none`; the flamegraph recipe moves the stray `perf.data` into `target/profile/`.
- P0-13 / P1-05 / P2-04 / P3-04 / P4-04: verdicts of dozens of per-file REGRESSION and some LOSS lines against earlier milestone runs are recorded as run-to-run drift (no code changed on those keys), not as failures.
- P1-01: give-back is applied on both paths (it returns 0 on Reference); `step` wraps `step_inner` so one exit helper covers every non-`NeedsInput` return.
- P1-02: kept under PT4 as NEUTRAL on Silesia because it is the named prerequisite of P2-01.
- P2-01: after a margin exit, one state-machine dispatch runs before the fast loop is retried (guarantees progress); a defensive `Copy` exit exists.
- P3-01: `step_inner` returns `(pos, out_pos, result)` so the window is refreshed exactly once on every return, including errors; `window_bytes_written` counts bytes actually copied into the ring.
- P3-03: growth now happens only on `OutputFull` (D-25's empty-region backstop restated as "cannot grow past the limit"), which also removes the extra zero-filled buffer gzip allocated at a member boundary.
- P4-02: the gate was read against the noise of the keys a change would be judged on (twice the quick-bench noise), since eliminating Adler-32 entirely could not clear it.
- P4-03: quick WINs that pointed in both directions were treated as drift against a slow default run; no default changed.
- P5-01: entries without a parseable timestamp are dropped when filtering by `--since-commit`; an unresolvable revision is an error.
- P5-03: `provenance/human-interventions.md` gets one dated sentence (no interventions recorded in the flight).

## Config keys (tunables) and their status

Introduced in this phase, all in `crates/cleanflate/src/config.rs`, each with the sweep that kept it cited in its doc comment:

| key | default | tuned? |
|---|---:|---|
| `FAST_LOOP_MIN_INPUT` | 16 | swept {8, 16, 32}: kept |
| `FAST_LOOP_MIN_OUTPUT` | 258 | swept {258, 320, 512}: kept |
| `CRC32_SLICES` | 8 | swept {4, 8, 16}: kept (4 was 11% slower) |
| `ONESHOT_INITIAL_RATIO` | 4 | swept {2, 4, 8}: kept |
| `ONESHOT_GROWTH_FLOOR` | 32768 | safety bound, not swept (literal moved from `stream.rs`) |
| `ONESHOT_MAX_PREALLOC` | 64 MiB | safety bound, not swept |

`LITLEN_TABLE_BITS` (10) and `DIST_TABLE_BITS` (8) were re-swept and kept. There are no `ASSUMPTION` config keys in this phase.

## What a human must check by hand (all NOT VERIFIED)

- P0-11 / P0-13: aggregate noise is 6.1% (full) and 6.5% (quick), above 5% and 3%; measured run-to-run drift is about 10%. Re-run the baseline on a quiet machine with a pinned governor (SPEC 8) and repeat the final `make bench`; the ratios are conditioned on the recorded machine state (schedutil governor, load about 3).
- Every phase-end task: confirm GitHub Actions `ci` is green on the pushed branch (it now lints and tests the `stats` and `test-hooks` features).
- P0-13: sanity-check the P0-12 profile tables against `make flamegraph`; the provenance log's denied entries from this phase are benign (recursive grep at the repository root, a `sed` regex containing a forbidden word).
- P1-05: review D-27 and D-28 for clean-room wording (SPEC 2.1); P3-04: review D-30 and D-31; P4-04: review the CRC derivation (D-32); P2-04: review the fast-loop code by hand and mutation-sample it (SPEC 5).
- P2-02 / P2-04 / P5-03: run `make fuzz-long` or the nightly workflow for all six targets (including `diff_fast_reference`) for the one-hour signal, and triage `make fuzz-divergences`. The implementer ran only `make fuzz-smoke` (six targets, 60 s each, plus 120 s for `diff_fast_reference`): no crashes.
- P3-04: compare peak RSS of `make bomb` against a phase-1 checkout (`/usr/bin/time -v`); `size_of::<Decompressor>()` is unchanged at 33,688 bytes.
- P5-03: review the PERF.md result line and negative results, complete `provenance/human-interventions.md`, and, after review and merge into `feature/perf` (and on to `main`), tag `v0.2.0-cleanroom`. The implementer does not tag.

## For a reviewer who has not seen this code

- New public surface is additive and opt-in: features `stats` and `test-hooks` (off by default, no dependencies), `cleanflate::Stats`, `Decompressor::stats()`, `decompress_with_stats`, and `cleanflate::test_hooks::{DecodePath, decompress_with_path, decompressor_with_path}`. The oracle enables `test-hooks` only as a dev-dependency.
- `DecodePath::Reference` keeps the simple behaviour (exact byte fill, bit-serial decode, byte-by-byte copy, no fast loop) and is the differential oracle: `crates/oracle/tests/fastslow.rs` compares per-call consumed/produced/status, errors and `finish()` on the conformance corpus, the malformed corpus and proptest streams; `testdata/regressions/` holds committed repro fixtures (`oracle regress-add`, `regress-seed`). The fuzz target `diff_fast_reference` does the same on arbitrary bytes.
- The bit-reader invariant (D-27) is the correctness core: every non-`NeedsInput` exit of `Inflater::step` gives whole unneeded bytes back so the Fast reader holds exactly what the Reference reader holds. The fast loop (D-29) only consumes whole symbols and relies on the shared exit. The window is refreshed from the output at every `step` return (D-30).
- Each kept technique is a `### D-NN: [perf]` entry (D-27 to D-32) with the derivation from first principles, the phrase required by SPEC 2.1 and a `Measurement:` line citing `docs/perf/`. D-26 (decode path and the uniform invalid-code rule) is not a performance entry.
- All timing files are produced by `oracle bench` and live in `docs/perf/`; PERF.md tables come from them. The guard now denies profiling of anything but the cleanflate-only `profile_decode` example (`make profile`); the profiled process never runs C zlib.
- Known weak spots to look at: timing conclusions rest on a noisy machine (use the one-shot to streaming ratio or repeat runs, as P3-03 and P4-03 did); the doubling copy (P3-02) was reverted, so overlapping matches copy `distance` bytes at a time.

## Round 1

Branch build/2026-09-29-2. Tasks: 2 of 2 done (R1-01, R1-02), none blocked or skipped. Test-only changes plus one debug_assert and two doc fixes; no behaviour change.

- R1-01: tests/bitreader_bulk.rs::pending_literal_bits_kept_at_output_full and tests/stats.rs::reference_path_never_gives_back. Added `debug_assert!` in `Inflater::step` that the Reference path never gives bytes back. Fixed stale docs in bitreader.rs (D-27) and huffman.rs (D-26). The `keep_bits = len.min(7)` mutant was applied by hand and is killed.
- R1-02: new case in tests/fast_loop.rs::fast_loop_errors_identical_to_reference. Interpretation: the block is hand-built (raw header plus manual code-length program) because the dynamic builder always appends an end-of-block code. The `Error::BadBlockType` mutant at fast_loop.rs:137 was applied by hand and is killed.
- No new config keys. Manual check: none.

## Round 2
Branch build/2026-09-29-2; fix commit c43ea88. Tasks: 36 done, 0 blocked, 0 skipped.
- R2-01: added pub(crate) read-only accessors `ZlibDecoder::inflater_path` and `Decompressor::inflater_path` (dead_code allow only) and tests `zlib_decoder_uses_same_path` and `decompressor_passes_path_every_format` in tests/decode_path.rs. The three listed mutants were each confirmed killed by hand (sed plus cargo test), since foundry_mutate was not available.
- Interpretation: none. No behaviour or public API change.
- Manual check: n/a.

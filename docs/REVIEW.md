# REVIEW: phase 2 (performance) build branch `build/2026-09-29-2`
Round: 1

Base `13112c175a17` (feature/perf) .. head `467c29d` (plus two review-stage `chore: pipeline friction` commits). 33 tasks done, 0 blocked, 0 skipped.

## Verdict

**CHANGES REQUESTED** (two fix tasks: R1-01 for F1, R1-02 for F2)

The branch is in good shape. Every mechanical constraint passes, every acceptance test the plan names exists and passes, and I found no correctness bug in the decoder. The fast-vs-reference differential, the chunking proptests at 3000 cases, conformance, malformed and regressions are all green. Mutation sampling found one mechanic with no test at all: the pending-literal give-back at the shared `step` exit (D-27 (b)). It is category 3, which blocks approval.

## What I checked

- `foundry_verify` (all files, then again with `tools/`, `docs/DESIGN.md`, `fuzz/` and `crates/cleanflate/src/` files so every extraVerify ran). All 15 constraints were ok, with no fixture failures and no hits. Every command passed: `make fmt-check`, `lint` (including the stats, test-hooks and default-lib clippy lines), `corpus`, `test` (207 cleanflate unit tests plus the oracle suites), `crate-checks`, `cleanroom-selftest`, `tools-check`, `design-check`, `fuzz-check` and `proptest-3k`.
- C12: every commit body on the branch carries `[stage:implement] [task:<id>]` (`git log --format=%B`).
- C13: `fastslow.rs`, `regressions.rs` and `common/mod.rs` panic on a missing manifest, a missing file or zero rows. They never skip.
- C15: I read the new arithmetic in `bitreader.rs`, `fast_loop.rs`, `window.rs`, `stream.rs` and `checksum.rs`. It is checked or saturating wherever input controls the value. Everywhere else the bounds are proven locally: count ≤ 63, margins ≥ 16/258, `remaining > 0` guards.
- C18: the oracle enables `test-hooks` only in `[dev-dependencies]`, and neither feature is a default. C9, C11 and C17 are clean.
- Boundaries: the `.gitignore` edit (`/testdata/perf-inputs/`, P0-01) and the `fuzz-nightly.yml` matrix entry (P2-02) were both called for. No other ignore, CI or editor config changed. The new tunables live only in `config.rs`, and each one cites its sweep JSON.
- Every plan-named acceptance test exists: I grepped all 79 names across P0-02..P5-01.
- DESIGN D-26..D-32 all pass `design-check`, carry the SPEC 2.1 phrase and use the project's own wording. I found no reference-implementation names.

### Mutation sampling (`foundry_mutate`)

| file | mutation | commands | result |
|---|---|---|---|
| `fast_loop.rs` | per-iteration refill threshold `< ITERATION_BITS` -> `< 8` | make test | killed (6 tests) |
| `fast_loop.rs` | distance-lookup `Invalid` -> `Error::BadBlockType` | make test | **survived** |
| `fast_loop.rs` | same | make proptest-3k | killed, but only by `chunking_invariance_on_corrupted_streams` after 524 random successes |
| `window.rs` | slice copy chunk `left.min(dist)` -> `left` (overlap) | make test | killed (25 tests) |
| `huffman.rs` | table symbol trusted when `len <= avail + 1` | make test | killed (11 tests) |
| `inflate.rs` | exit `give_back(keep_bits)` -> `give_back(keep_bits + 8)` | make test | killed (20 tests) |
| `inflate.rs` | pending literal `keep_bits = len` -> `len.min(7)` | make test | **survived** |
| `inflate.rs` | same | make proptest-3k | **survived** |
| `stream.rs` | drop the `ONESHOT_MAX_PREALLOC` cap in `initial_len` | make test | killed |
| `checksum.rs` | slice tables not reversed in `crc32_fold_chunk` | make test | killed (6 tests) |
| `bitreader.rs` | `give_back` no longer masks the returned bytes | make test | killed (oracle chunking proptest) |

I could not complete a `dsym >= 30` -> `dsym >= 31` mutation in `fast_loop.rs`. The tool connection closed twice, most likely because the mutant turns distance code 30 into an endless `OutputFull`/grow loop to the 1 GiB limit in one-shot tests. `fast_loop_errors_identical_to_reference` does cover that code by construction.

## Findings

### F1 (category 3, tests): the pending-literal give-back at the `step` exit is untested, and the differential cannot see it
- **Where:** `crates/cleanflate/src/inflate.rs:675` (`self.keep_bits = len;`) and the shared exit at `crates/cleanflate/src/inflate.rs:277-281` (`let k = self.br.give_back(self.keep_bits);`).
- **What is wrong:** D-27 (b) is the invariant HANDOFF calls "the correctness core". It says an `OutputFull` exit keeps exactly the bits of the decoded-but-unconsumed literal. Mutating `keep_bits = len` to `len.min(7)` survives both `make test` and `make proptest-3k`. The mutant makes both paths hand back a byte that holds the tail of a pending 8- or 9-bit literal.
  - It goes unseen because the shared exit helper runs on the Reference path too. HANDOFF says it "returns 0 on Reference", but nothing asserts that. A bug in the helper therefore moves Fast and Reference together, and the fast-vs-reference traces stay equal.
  - `give_back_keeps_pending_bits` only tests `BitReader::give_back` in isolation, not the `keep_bits` the inflater passes to it.
  - PT3 and C20 require Reference to keep the simple exact-fill behaviour, so that it stays an independent oracle for per-call `consumed`.
- **What would break:** per-call `consumed` at `OutputFull` silently drifts away from exact-fill accounting (the SPEC 3 streaming contract, "exact consumed accounting", and PLAN's "identical consumed per call"), and no gate notices. A later change to the exit helper, the fast loop's exit or the literal path could also break the D-27 invariant on both paths at once and still pass every differential.
- **Minimal fix:**
  1. Add a test that the Reference path never gives bytes back. Under `#[cfg(feature = "stats")]`, check that `Stats::given_back_bytes == 0` on `DecodePath::Reference`. Run it over the hand-built fixed and dynamic streams at output chunks 1 and 3, including fixed-block literals 144..=255 (9-bit codes).
  2. Add a test that pins per-call `consumed` at `OutputFull` to an independent bit-position model. Use a fixed block of 9-bit literals with 1-byte output on both paths. After every call, the cumulative `consumed` must equal ceil((bits consumed so far + the pending literal's 9 bits) / 8), computed from the test's own bit accounting, not from cleanflate.
  3. Optionally, add `debug_assert!(self.path == DecodePath::Fast || k == 0)` at the exit, so every Reference-path test enforces it.
- **Task:** P1-01 (introduced the exit helper and `keep_bits`).

### F2 (category 3, tests): a fast-loop distance `Invalid` is only caught by chance
- **Where:** `crates/cleanflate/src/fast_loop.rs:137` (`Decoded::Invalid => return FastExit::Error(Error::InvalidSymbol)` for the distance lookup).
- **What is wrong:** the only way a distance lookup reaches `Invalid` in the fast loop is the unused code of an RFC 1951 S3.2.7 one-code distance table. Changing that error value survives `make test`, which is where SPEC 5 says the differential runs. It is killed under `make proptest-3k` only by a random corrupted stream, after 524 successes. `fast_loop_errors_identical_to_reference` covers too-far distances, distance code 30 and literal/length 286/287, but not this case. The existing malformed fixture `dynamic_single_dist_code` is too short (46 bytes) to enter the loop, which needs a 16-byte input margin.
- **What would break:** a fast-loop error that differs from the Reference path (SPEC 5 "identical ... errors") could pass the quick tier.
- **Minimal fix:** add a hand-built case to `fast_loop_errors_identical_to_reference`: a dynamic block with a single distance code of length 1, whose unused code (bit 1) appears after at least `FAST_LOOP_MIN_INPUT` bytes of other symbols. Assert the same `Error` at the same call index on both paths.
- **Task:** P2-01.

## Spec issues

- **SPEC 6 M3, one-shot sizing cap.** "always capped by the output limit" plus PLAN's `ONESHOT_MAX_PREALLOC = 64 MiB` means an 18-byte hostile gzip input whose ISIZE is 0xFFFFFFFF makes `decompress_with` allocate and zero-fill 64 MiB before decoding a single byte. The cap was checked by mutation. The allocation is bounded and within SPEC and PLAN, so this is not a finding. It is, however, a large amplification. A human may want a smaller cap, or a hint that is trusted only up to some multiple of `input.len()`.
- **SPEC 4.1 targets.** The primary 70% one-shot target is NOT MET (66.9% raw, 68.8% zlib, 68.8% gzip). The streaming 60% target is MET. The result is reported honestly with a shortfall analysis, as SPEC 4.1 and 9 allow, so it is not a finding.

## Manual checks still owed (from HANDOFF.md)

- P0-11 / P0-13: aggregate noise is 6.1% (full) and 6.5% (quick), above 5% and 3%, and measured run-to-run drift is about 10%. Re-run the baseline on a quiet machine with a pinned governor (SPEC 8) and repeat the final `make bench`. The ratios depend on the recorded machine state (schedutil governor, load about 3).
- Every phase-end task: confirm GitHub Actions `ci` is green on the pushed branch. It now lints and tests the `stats` and `test-hooks` features.
- P0-13: sanity-check the P0-12 profile tables against `make flamegraph`. The provenance log's denied entries from this phase are benign (a recursive grep at the repository root, and a `sed` regex containing a forbidden word).
- P1-05: review D-27 and D-28 for clean-room wording (SPEC 2.1). P3-04: review D-30 and D-31. P4-04: review the CRC derivation (D-32). P2-04: review the fast-loop code by hand and mutation-sample it (SPEC 5). This review sampled it; see the table above.
- P2-02 / P2-04 / P5-03: run `make fuzz-long` or the nightly workflow for all six targets, including `diff_fast_reference`, for the one-hour signal, and triage `make fuzz-divergences`. Only `make fuzz-smoke` was run.
- P3-04: compare peak RSS of `make bomb` against a phase-1 checkout (`/usr/bin/time -v`).
- P5-03: review the PERF.md result line and the negative results, and complete `provenance/human-interventions.md`. After review and merge into `feature/perf` (and on to `main`), tag `v0.2.0-cleanroom`. The implementer does not tag.

## Notes

- Stale doc comments:
  - `crates/cleanflate/src/bitreader.rs:11` still says the reader "Holds at most 7 bits between completed decode steps (see DESIGN.md D-07)". D-27 superseded that.
  - `crates/cleanflate/src/huffman.rs:121` still describes `Decoded::Invalid` as "15 bits were available and none of them formed a valid code". D-26 superseded that.
  - R1-01 may fix these doc comments opportunistically. They are not a finding.
- P4-03 did not run the full-bench confirmation PLAN asks for after a quick WIN. It re-ran the default quick bench instead and kept every default. The outcome is conservative, and nothing changed.
- The P3-01 commit is well over the ~400 non-test-line guideline, mostly because `step` was wrapped in a closure and reindented. It reads fine in `git show -w`.
- The HANDOFF interpretation choices are each consistent with SPEC. P0-04's table delegation is superseded by D-28. P3-03's "grow only on `OutputFull`" keeps D-25's limit semantics: `limit_exact_ok` and `limit_plus_one_err` pass, and no wrapper emits `OutputFull` without a pending byte. P4-02's gate against 2x aggregate noise matches PT2's "aggregate noise".

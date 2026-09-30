# Review: phase-3 build branch `build/2026-09-30`
Round: 2

**Verdict: APPROVED**

Scope: the whole branch `fb7bca0..HEAD`. Round 1 reviewed every task commit in
depth (see the round-1 review in git history, commit 97662d4) and raised one
finding (F1, reset differential). Since then the branch has changed only by
R1-01 (`8a84b96`, tests only) plus bookkeeping (`docs/PLAN.md`,
`docs/PROGRESS.md`, `docs/HANDOFF.md`, `.foundry/*`). `git diff f026d71..HEAD`
touches no library file under `crates/cleanflate/src/` outside `src/tests/`.

## Evidence gathered this round

- **`foundry_verify`:** every `docs/foundry.json` constraint ok, with no fixture failures and no hits. The following are all green:
  - fmt-check, lint, corpus, test, crate-checks, cleanroom-selftest
  - proptest-3k, fuzz-smoke-decoder (run because I passed the touched test files)
- **C12:** every commit in `fb7bca0..HEAD` carries `[stage:..] [task:..]`. I checked each body with `git log --format=%B`.
- **Boundaries:** the branch does not touch `.gitignore`, `.gitattributes`, `.github/` or editor config.
- **R1-01 against its PLAN task:**
  - `reset_reapplies_raw_dictionary` exists in `crates/cleanflate/src/tests/reset.rs`. It builds a raw fixed block by hand whose match copies `fgh` from the dictionary. It then checks the output and totals against a fresh decompressor after `reset()`, on both `DecodePath::Fast` and `DecodePath::Reference`.
  - `reset_equals_fresh_options` now builds the raw-dictionary case with `oracle::ffi::deflate_bytes(.., Some(dict))`. The zlib-dictionary selection panics with a `make corpus` hint instead of `continue`, which satisfies C13. No checksum is computed for the fixture, so C16 is satisfied.
  - `reset_proptest` draws `prefix_calls` in `0..=6` and drives the case's own in/out schedules before `reset()`. So mid-stream resets now land at arbitrary points.
- **Mutation samples (`foundry_mutate`, `make test`):**
  - `stream.rs`, `Backend::reset` without `prefill_dictionary`: **killed** by `reset_reapplies_raw_dictionary` (`InvalidDistance { distance: 3, available: 0 }`). Round 1 had this as a survivor, so F1 is closed.
  - `inflate.rs`, `Inflater::reset` without `self.br = BitReader::new()`: **killed** by `reset_clears_sticky_error` and `reset_reapplies_raw_dictionary`.
  - `gzip.rs`, `GzipDecoder::reset` without `member_count = 0`: **killed** by `reset_clears_trailing_and_headers`.
  - `gzip.rs`, `GzipDecoder::reset` without `member_crc = Crc32::new()`: survived. This mutant is equivalent: `gzip.rs:563` re-initialises `member_crc` at the start of every member body, so the reset line is redundant. Not a finding.
  - `fast_loop.rs`, distance check `>` -> `>=`: **killed** by four `tests::fast_loop` trace tests.
  - `fast_loop.rs`, `dsym >= 30` -> `>= 32`: no verdict. The tool connection closed, probably because the mutant ran away. The working tree was clean afterwards, and the failure is logged as pipeline feedback. The `>` -> `>=` mutant above already covers this module.

## Findings

None in categories 1-7.

## Interpretation choices (HANDOFF)

- **Round 1 readings:** I accepted all of them in round 1 and the reasoning has not changed.
- **R1-01 (new):** the raw dictionary case uses a fixed dictionary and cyclic data that the oracle compresses. This is the reading PLAN R1-01 asked for, since it prefers a raw stream so that no checksum is needed. Accepted.

## Blocked and skipped tasks

None. The gated-out and reverted tasks (P2-02, P2-03, P2-04, P2-07, P2-08, P3-01, P3-02, P4-02, P4-04) record their negative results in PHASE3.md, which is what PLAN requires of them.

## Spec issues

None.

## Manual checks still owed (from HANDOFF.md)

- **M0 (P0-10):**
  - GitHub Actions `ci` is green on the branch.
  - Re-measure the phase-3 start noise (8.2% full / 8.8% quick, above the 3% the plan asked for) on a quiet machine with a pinned governor.
  - Spot-check the Environment block (`c_zlib.compile_flags` 0xa9, `libz_sys_features`).
  - Confirm the provenance log has no unexplained denials. The nine denials are explained in `provenance/human-interventions.md`.
- **M1 (P1-03):**
  - Open the three before-flamegraphs and sanity-check the `phase3-profile.md` tables against them.
  - Confirm the SVGs show only cleanflate, std and libc frames.
- **M2 (P2-09):**
  - Measure peak RSS of `make bomb` and of a one-shot Silesia decode against the phase-3 start (`/usr/bin/time -v`).
  - Review the sink code by hand and mutation-sample it.
- **M3 (P3-05):**
  - Review the `Decompressor::reset` rustdoc and DESIGN D-35 as a public API addition.
  - Measure peak RSS of `make bomb`.
- **M4 (P4-06):**
  - Review the fast-loop code by hand and mutation-sample it. Prefer mutants that cannot run away (see the no-verdict mutant above).
  - Review DESIGN D-36 for clean-room wording.
- **M5 (P5-03):**
  - Run the final `make bench` on a quiet machine with a pinned governor and compare with `P5-02.json`.
  - Run `make fuzz-long` (or the nightly workflow) for all six targets and triage `make fuzz-divergences`.
  - Review PHASE3.md's T1-T6 verdicts, attribution and negative results, and the PERF.md result line.
  - Review the committed flamegraphs and assembly evidence.
  - After merge, tag `v0.3.0-cleanroom`.
  - Complete `provenance/human-interventions.md`.

## Notes

- **Prefix drive uses only valid input:** the new prefix loop in `reset_proptest` drives only the uncorrupted stream `a`. Corrupted-prefix resets remain covered by scenario 4 (`drive_prefix`) in the same test, so this is not a gap.
- **Redundant `member_crc` reset:** `GzipDecoder::reset` re-initialises `member_crc`, which the member-header path already does. It is harmless and keeps `reset` obviously complete.
- **Carried over from round 1, still true:**
  - The Vec-sink driver is reachable only through `test-hooks`.
  - `CRC32_TABLES` has 16 rows while production folds 8.

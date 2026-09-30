# Review: phase-3 build branch `build/2026-09-30`
Round: 1

**Verdict: CHANGES REQUESTED**

Scope: the whole branch `fb7bca0..f026d71` (36 tasks, 76 commits). I read every
library diff under `crates/cleanflate/src/` (sink, window, inflate, fast loop,
huffman, bitreader, checksum, stream, zlib, gzip, test hooks), the oracle FFI
additions, the new oracle tests (reset, sink differential), the fuzz-target
change, the Makefile, and DESIGN D-34..D-36, and checked them against SPEC and
the PLAN task text.

Evidence I gathered myself:
- `foundry_verify`: every `docs/foundry.json` constraint ok, with no fixture
  failures and no hits. fmt, lint, corpus, test, crate-checks,
  cleanroom-selftest, tools-check, design-check, fuzz-check, proptest-3k and
  fuzz-smoke-decoder are all green.
- Every commit body carries `[stage:..] [task:..]` (C12, checked with
  `git log --format=%B`).
- No edits to `.gitignore`, `.gitattributes` or CI config. No manifest gained a
  dependency or comparator crate.
- Mutation samples (`foundry_mutate`, `make test`):
  - `huffman.rs`, fast-table subtable mask `- 1` -> `- 2`: **killed**
    (`fast_loop_long_codes_via_subtable`, `fast_loop_vec_and_slice_sinks_same_trace`).
  - `sink.rs`, `VecSink::copy_within_back` source `len - dist` -> `len - n`:
    **killed**.
  - `window.rs`, `Window::reset` without `total = 0`: **killed**, but only by
    `reset_proptest`. `reset_equals_fresh_conformance` did not see it.
  - `zlib.rs`, `ZlibDecoder::reset` without the Adler-32 reset: **killed**.
  - `oracle/src/bench_targets.rs`, T4 threshold 0.03 -> 0.30: **killed**.
  - `checksum.rs`, `crc32_update_sliced` bytewise cut-off `N < 4` -> `N < 8`:
    survived. This mutant is equivalent: the output is identical and only the
    speed of the test-hooks-only `sliced4` variant changes. Not a finding.
  - `stream.rs`, `Backend::reset` without re-applying the raw dictionary:
    **survived**. See F1.

## Findings

### F1 (category 1, C13; also category 3): the reset differential silently skips the raw-dictionary case, and the raw-dictionary re-prefill in `reset` is untested

- **Where:** `crates/oracle/tests/reset.rs:241` (`let Some(entry) = with else { continue };` in `reset_equals_fresh_options`), with the code at `crates/cleanflate/src/stream.rs:42-49` (`Backend::reset` -> `prefill_dictionary`).
- **What is wrong:**
  - PLAN P3-03 requires `reset_equals_fresh_options` to cover "a dictionary (raw and zlib)".
  - The quick-tier manifest has no raw entry with a dictionary. The only dictionary entries are 9 zlib/canterbury ones. So the raw iteration hits `continue` and passes without testing anything.
  - This is exactly the "empty selection skips silently" shape that C13 forbids for reset tests.
  - Nothing else covers the path. `src/tests/reset.rs` has no dictionary test, and `reset_equals_fresh_conformance` has no raw dictionary entries either.
  - I confirmed this by mutation. With the `prefill_dictionary` call removed from `Backend::reset`, `make test` passes.
- **What would break:**
  - The public rustdoc of `Decompressor::reset` and DESIGN D-35 both promise that `Options::dictionary` is applied again to a raw stream.
  - A regression would make every raw stream decoded after `reset()` with a dictionary fail with `InvalidDistance` (or decode wrong bytes), and no test would notice.
  - D-35 also claims the differential covers "dictionaries", which is not true for raw.
- **Minimal fix:**
  - In `reset_equals_fresh_options`, build the raw-dictionary case directly, e.g. `oracle::ffi::deflate_bytes(data, &DeflateParams{framing: Raw, ..}, Some(dict))` with the dictionary in `Options::dictionary`.
  - Make an empty selection a failure (`panic!`/push to `failures`), never `continue`.
  - Add a unit test `reset_reapplies_raw_dictionary` to `crates/cleanflate/src/tests/reset.rs`: a hand-built raw fixed block whose match reaches into the dictionary, decoded, `reset()`, decoded again, same output as a fresh decompressor, on both paths. No checksums are needed.
  - While in this file: PLAN P3-03's `reset_proptest` asks for "a random reset call index". The implementation only uses six fixed prefix scenarios, whose only mid-stream point is "one call of 100 bytes in / 50 bytes out". Add a strategy-drawn number of prefix calls (driving the prefix stream with the case's own schedules) before `reset()`, so mid-stream resets land in arbitrary decoder states (SPEC §5 "mid-stream resets").
- **Task:** P3-03.

No other findings in categories 1-7.

## Interpretation choices (HANDOFF)

I checked each one against SPEC and PLAN. All are acceptable readings:

- **P2-05:** the slice-shaped test wrappers live in `tests/slice_api.rs`.
- **P2-06:** `VecSink` is capacity-bounded, and the driver grows the buffer by the unchanged `next_len` schedule. This keeps peak memory identical to the old path, and SPEC §3 asks for exactly that.
- **P2-07:** the switch was reverted under PT4. SPEC M2 explicitly allows the negative result.
- **P2-01:** `CRC32_MAX_SLICES` is a derived table capacity, not a tunable.
- **P3-03:** the conformance reset runs one-byte chunks for a subset of entries. The proptest and the options test also cover small chunks.
- **P4-03:** there is no runtime width to vary, and `fast_table` returns `None` for any other width, so the loop hands over to the state machine.
- **P0-03, P0-04..P0-08, P1-01, P1-02, P4-01, P4-04, P4-05:** bench and tooling readings that do not affect the decoder.

## Blocked and skipped tasks

None. The gated-out tasks (P2-02, P2-04, P2-08, P3-01, P3-02) name "If gated out, the PHASE3.md entry is the deliverable". Their acceptance tests are therefore legitimately absent, and each has its PT2 numbers in PHASE3.md.

## Spec issues

None.

## Manual checks still owed (from HANDOFF.md)

- **M0 (P0-10):**
  - GitHub Actions `ci` is green on the branch.
  - Re-measure the phase-3 start noise (8.2% full / 8.8% quick, above the 3% the plan asked for) on a quiet machine with a pinned governor.
  - Spot-check the Environment block (`c_zlib.compile_flags` 0xa9, `libz_sys_features`).
  - Confirm the provenance log has no unexplained denials. The nine denials are explained in `provenance/human-interventions.md`.
- **M1 (P1-03):** open the three before-flamegraphs and sanity-check the `phase3-profile.md` tables against them. The SVGs should show only cleanflate, std and libc frames.
- **M2 (P2-09):**
  - Measure peak RSS of `make bomb` and of a one-shot Silesia decode against the phase-3 start (`/usr/bin/time -v`).
  - Review the sink code by hand and mutation-sample it.
- **M3 (P3-05):**
  - Review the `Decompressor::reset` rustdoc and DESIGN D-35 as a public API addition.
  - Measure peak RSS of `make bomb`.
- **M4 (P4-06):**
  - Review the fast-loop code by hand and mutation-sample it.
  - Review DESIGN D-36 for clean-room wording.
- **M5 (P5-03):**
  - Run the final `make bench` on a quiet machine with a pinned governor and compare with `P5-02.json`.
  - Run `make fuzz-long` (or the nightly workflow) for all six targets and triage `make fuzz-divergences`.
  - Review PHASE3.md's T1-T6 verdicts, attribution and negative results, and the PERF.md result line.
  - Review the committed flamegraphs and assembly evidence.
  - After merge, tag `v0.3.0-cleanroom`.
  - Complete `provenance/human-interventions.md`.

## Notes

- **Unused Vec-sink driver:** `VecSink`, `decompress_vec`, `decompress_core_vec` and `grow_output` remain in the library behind `cfg_attr(..., allow(dead_code))`. They are reachable only through `test-hooks` since P2-07 was reverted. PLAN kept them on purpose (the sink differential and the fuzz target use them). If a later phase does not revive the Vec path, consider whether to keep maintaining it.
- **Weak reset conformance test:** `reset_equals_fresh_conformance` did not notice the `Window::reset` mutant, because valid streams never reach past their own history. Only the corrupted-stream proptest noticed. The F1 fix (random reset points) strengthens this indirectly.
- **CRC table size:** `CRC32_TABLES` is now 16 rows (16 KiB static), while production folds only 8. Harmless (the unused rows are never touched), but worth knowing if binary size ever matters.
- **Tooling residue:** `foundry_mutate` left an untracked `crates/oracle/tests/reset.proptest-regressions` after the killed mutants. I removed it; it is logged as pipeline feedback.

# REVIEW: phase 2 (performance) build branch `build/2026-09-29-2`
Round: 3

Base `13112c175a17` (feature/perf) .. head `129c7fe`. 36 tasks done (P0-01..P5-03, R1-01, R1-02, R2-01), 0 blocked, 0 skipped.

## Verdict

**APPROVED**

R2-01 closes the round-2 finding (F1). All three mutants that survived in round 2 are now killed by tests, not by lints. The library change is limited to two read-only `pub(crate)` accessors. Every mechanical constraint passes, and every verify command is green. Nothing else on the branch changed since round 2, which reviewed the whole branch, and round 1 before it.

## What I checked

- `foundry_verify`, run with the R2-01 touched files (`stream.rs`, `zlib.rs`, `tests/decode_path.rs`) so every extraVerify ran.
  - All 15 constraints were ok, with no fixture failures and no hits.
  - `make fmt-check`, `lint` (all five clippy lines), `corpus`, `test` (211 cleanflate unit tests plus the oracle suites), `crate-checks`, `cleanroom-selftest` and `proptest-3k` all pass. The `proptest-3k` run includes the `fastslow` differential at 3000 cases.
- C12: every commit in `13112c175a17..HEAD` carries `[stage:<..>] [task:<..>]`. `c43ea88` carries `[stage:fix] [task:R2-01]` and follows the commit template.
- Boundaries: since round 2 (`6686b1a..HEAD`), the only files changed are the three R2-01 files plus pipeline bookkeeping (`.foundry/state.json`, HANDOFF, PLAN, PROGRESS, REVIEW). No ignore, CI or editor config changed.
- R2-01 library diff:
  - It adds `ZlibDecoder::inflater_path` and `Decompressor::inflater_path`.
  - Each carries only `#[cfg_attr(not(test), allow(dead_code))]`, mirroring `GzipDecoder::inflater_path`. There is no clippy panic or indexing allow (C5), and nothing panics in library code (C4).
  - There is no behaviour change and no public API change.
- R2-01 tests follow C16:
  - The DICTID and trailer Adler-32 come from the test's own `adler_by_hand` arithmetic.
  - The gzip CRC-32 comes from an inline bitwise loop.
  - Neither uses `crate::checksum`, and the `no-cleanflate-checksums-in-fixtures` rule reports no hits.
- R2-01 acceptance coverage:
  - `zlib_decoder_uses_same_path` checks the plain zlib decoder before and after stepping. It also checks an FDICT stream (`78 20`, valid FCHECK) after the DICTID is accepted and `abc` is produced.
  - `decompressor_passes_path_every_format` covers Raw, Raw with a dictionary, Zlib and Gzip. It checks the path at construction and after one `decompress` call that produces 3 bytes.

### Mutation sampling (`foundry_mutate`, this round)

| file | mutation | commands | result |
|---|---|---|---|
| `zlib.rs:47` | `ZlibDecoder::new` inflater built with `DecodePath::Fast` | make test | killed (`zlib_decoder_uses_same_path`, `decompressor_passes_path_every_format`) |
| `zlib.rs:156` | FDICT dictionary inflater built with `DecodePath::Fast` | make test | killed (`zlib_decoder_uses_same_path`, a test failure rather than the dead_code lint) |
| `stream.rs:172` | raw-with-dictionary inflater built with `DecodePath::Fast` | make test | killed (`decompressor_passes_path_every_format`, "Raw initial") |
| `stream.rs:175` | Zlib backend `ZlibDecoder::new(.., DecodePath::Fast)` | make test | killed (`decompressor_passes_path_every_format`, "Zlib initial") |

Earlier rounds sampled `fast_loop.rs`, `window.rs`, `huffman.rs`, `inflate.rs`, `stream.rs`, `checksum.rs`, `bitreader.rs` and `gzip.rs`. Their tables are in git history: round 1 in `88bbb49`, round 2 in `a9f1c33`.

## Findings

None.

## Spec issues

- **SPEC 6 M3, one-shot sizing cap** (carried over from rounds 1 and 2):
  - SPEC says the allocation is "always capped by the output limit", and PLAN sets `ONESHOT_MAX_PREALLOC = 64 MiB`.
  - Together they let an 18-byte hostile gzip input whose ISIZE is 0xFFFFFFFF make `decompress_with` allocate and zero-fill 64 MiB before it decodes a byte.
  - This is bounded and within SPEC. A human may want a smaller cap, or to trust the hint only up to a multiple of `input.len()`.
- **SPEC 4.1 targets:**
  - The primary 70% one-shot target is NOT MET: 66.9% raw, 68.8% zlib, 68.8% gzip.
  - The streaming 60% target is MET: 74.9% raw.
  - The shortfall is reported with measurements and next steps, as SPEC 4.1 and 9 allow.

## Manual checks still owed (from HANDOFF.md)

- **P0-11 / P0-13, benchmark noise:** aggregate noise is 6.1% (full) and 6.5% (quick), above the 5% and 3% goals, and measured run-to-run drift is about 10%. Re-run the baseline on a quiet machine with a pinned governor (SPEC 8), then repeat the final `make bench`. The recorded ratios depend on the machine state at the time: schedutil governor, load about 3.
- **Every phase-end task:** confirm GitHub Actions `ci` is green on the pushed branch. It now lints and tests the `stats` and `test-hooks` features.
- **P0-13, profiles:** sanity-check the P0-12 profile tables against `make flamegraph`. The denied entries in the provenance log from this phase are benign: a recursive grep at the repository root, and a `sed` regex containing a forbidden word.
- **DESIGN entries and fast-loop code:**
  - P1-05: review D-27 and D-28 for clean-room wording (SPEC 2.1).
  - P3-04: review D-30 and D-31.
  - P4-04: review the CRC derivation in D-32.
  - P2-04: review the fast-loop code by hand and mutation-sample it (SPEC 5). Rounds 1 to 3 sampled it; see their tables.
- **P2-02 / P2-04 / P5-03, fuzzing:** run `make fuzz-long` or the nightly workflow for all six targets, including `diff_fast_reference`, for the one-hour signal. Then triage `make fuzz-divergences`. Only `make fuzz-smoke` has been run.
- **P3-04, memory:** compare peak RSS of `make bomb` against a phase-1 checkout (`/usr/bin/time -v`).
- **P5-03, hand-off:**
  - Review the PERF.md result line and the negative results.
  - Complete `provenance/human-interventions.md`.
  - After review and merge into `feature/perf` (and on to `main`), tag `v0.2.0-cleanroom`. The implementer does not tag.

## Notes

- `Raw with dictionary` in `decompressor_passes_path_every_format` decodes a stored block, so it never reads from the dictionary. That is fine, because the test pins the path, not the dictionary copy. `tests/zlib.rs::raw_with_dictionary_prefill` already covers the copy.
- The round-1 notes on the P3-01 commit size and the P4-03 full-bench confirmation still stand. Neither needs action.

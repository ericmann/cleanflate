# REVIEW: phase 2 (performance) build branch `build/2026-09-29-2`
Round: 2

Base `13112c175a17` (feature/perf) .. head `6686b1a`. 35 tasks done (P0-01..P5-03, R1-01, R1-02), 0 blocked, 0 skipped.

## Verdict

**CHANGES REQUESTED** (one fix task, for F1)

Both round-1 fixes land as specified and kill the mutants they target. Every mechanical constraint passes, and every verify command is green. I widened mutation sampling to the decode-path plumbing that round 1 did not sample, and found a category-3 gap there. The `DecodePath` a caller asks for is not tested to reach the zlib inflaters or the raw-with-dictionary inflater. If that plumbing regresses, the SPEC 5 differential quietly compares Fast against Fast for every zlib-framed and dictionary stream, and nothing fails.

## What I checked

- `foundry_verify`, with the round-2 touched files so every extraVerify ran. All 15 constraints were ok, with no fixture failures and no hits. `make fmt-check`, `lint` (all five clippy lines), `corpus`, `test` (209 cleanflate unit tests plus the oracle suites), `crate-checks`, `cleanroom-selftest` and `proptest-3k` all pass.
- C12: every commit in `13112c175a17..HEAD` carries `[stage:<..>] [task:<..>]`. The round-1 fix commits carry `[stage:fix] [task:R1-0N]`.
- Boundaries: round 1 touched only the files its two tasks named, plus pipeline bookkeeping. No ignore, CI or editor config changed.
- R1-01 (`0b0ab82`):
  - Library changes are limited to the two permitted `debug_assert!`s, which are allowed by C4, and the two doc fixes.
  - `pending_literal_bits_kept_at_output_full` checks cumulative `consumed` against the test's own bit accounting, `ceil((3 + 9w + 9) / 8)`, on both paths at output chunks 1 and 3.
  - `reference_path_never_gives_back` covers the literal, fixed and dynamic streams at chunks 1 and 3.
- R1-02 (`773445d`):
  - The hand-built block has a complete literal/length table (`a` = 1 bit, 256 and 257 = 2 bits) and one distance code of length 1.
  - The 48 pad bytes satisfy the fast-loop input margin. The mutant's failure message ("in 4611686018427387903 out 4096") shows the fast loop reached the bad code.
- C13, C15 and the rest of the branch: re-confirmed from round 1. Nothing outside the files above changed since then.

### Mutation sampling (`foundry_mutate`, this round)

| file | mutation | commands | result |
|---|---|---|---|
| `inflate.rs:683` | `self.keep_bits = len` -> `len.min(7)` (R1-01 target) | make test | killed (3 tests, including the new one and the new `debug_assert!`) |
| `fast_loop.rs:137` | distance `Invalid` -> `Error::BadBlockType` (R1-02 target) | make test | killed (`fast_loop_errors_identical_to_reference`) |
| `gzip.rs:457` | first-member inflater built with `DecodePath::Fast` | make test | killed (`gzip_second_member_uses_same_path`) |
| `gzip.rs:541` | next-member inflater built with `DecodePath::Fast` | make test | killed |
| `stream.rs:381` | gzip ISIZE hint disabled | make test | killed (4 `oneshot_sizing` tests) |
| `stream.rs:173` | raw inflater (no dictionary) built with `DecodePath::Fast` | make test | killed (`stats::reference_path_never_gives_back`) |
| `stream.rs:172` | raw-with-dictionary inflater built with `DecodePath::Fast` | make test | **survived** |
| `zlib.rs:47` | `ZlibDecoder::new` inflater built with `DecodePath::Fast` | make test, make lint | **survived** |
| `zlib.rs:150` | FDICT dictionary inflater built with `DecodePath::Fast` | make test | **survived** |
| `zlib.rs:150` | same | make lint | killed, but only by the `dead_code` lint on the then-unread `path` field, not by any test |

Round 1 sampled `fast_loop.rs`, `window.rs`, `huffman.rs`, `inflate.rs`, `stream.rs`, `checksum.rs` and `bitreader.rs` (see its table in git history, `88bbb49`).

## Findings

### F1 (category 3, tests): the decode path is not tested to reach the zlib or raw-with-dictionary inflaters
- **Where:**
  - `crates/cleanflate/src/zlib.rs:47`: `inflater: Inflater::new_with_path(path)`.
  - `crates/cleanflate/src/zlib.rs:150`: `Inflater::with_dictionary(dict, self.path)`.
  - `crates/cleanflate/src/stream.rs:172`: `Backend::Raw(Inflater::with_dictionary(dict, path))`.
- **What is wrong:** P0-04 requires that "`ZlibDecoder` and `GzipDecoder` take a path and pass it to every `Inflater` they create", and that the dictionary constructor take a path. The code does this today. Only gzip has a test for it, though (`gzip_second_member_uses_same_path`). Hard-coding `DecodePath::Fast` at any of the three sites above survives `make test`, including the oracle `fastslow` differential. It survives because both sides of that differential then run Fast and agree trivially. The `zlib.rs:150` mutant is caught only because it leaves the `path` field unread, which trips `-D dead_code` in `make lint`. That is incidental, and a refactor that reads `path` anywhere else would remove it.
- **What would break:** SPEC 5 requires the differential to decode "the entire conformance corpus ... both ways". A regression at `zlib.rs:47` would make every rfc1950 conformance, malformed, proptest and `diff_fast_reference` fuzz case compare Fast against Fast. That is roughly a third of the differential, and it would pass silently. A regression at the two dictionary sites would do the same for every preset-dictionary stream. A later fast-path bug that shows up only under zlib framing or with a dictionary would then pass every gate.
- **Minimal fix:**
  1. Add a `pub(crate) fn inflater_path(&self) -> DecodePath` accessor to `ZlibDecoder`, mirroring `GzipDecoder::inflater_path`, with the same `#[cfg_attr(not(test), allow(dead_code))]`.
  2. Add a `pub(crate)` accessor on `Decompressor` that returns the current backend inflater's path, using the same `cfg_attr`.
  3. Add tests in `crates/cleanflate/src/tests/decode_path.rs` asserting `DecodePath::Reference` for:
     - a zlib decoder built with Reference, both before and after it parses a plain header;
     - a zlib FDICT stream decoded with the matching dictionary, after the DICTID is accepted;
     - `Decompressor::new_with_path(Format::Raw, options_with_dictionary, Reference)`;
     - `Decompressor::new_with_path` for Raw, Zlib and Gzip without a dictionary.
- **Task:** P0-04 (introduced the plumbing and its acceptance tests).

## Spec issues

- **SPEC 6 M3, one-shot sizing cap:** carried over from round 1. SPEC says the allocation is "always capped by the output limit", and PLAN sets `ONESHOT_MAX_PREALLOC = 64 MiB`. Together they let an 18-byte hostile gzip input whose ISIZE is 0xFFFFFFFF make `decompress_with` allocate and zero-fill 64 MiB before decoding a single byte. This is bounded and within SPEC, so it is not a finding. A human may want a smaller cap, or to trust the hint only up to some multiple of `input.len()`.
- **SPEC 4.1 targets:** the primary 70% one-shot target is NOT MET (66.9% raw, 68.8% zlib, 68.8% gzip). The streaming 60% target is MET. The shortfall is reported honestly with an analysis, as SPEC 4.1 and 9 allow.

## Manual checks still owed (from HANDOFF.md)

- P0-11 / P0-13: aggregate noise is 6.1% (full) and 6.5% (quick), above 5% and 3%, and measured run-to-run drift is about 10%. Re-run the baseline on a quiet machine with a pinned governor (SPEC 8) and repeat the final `make bench`. The ratios depend on the recorded machine state (schedutil governor, load about 3).
- Every phase-end task: confirm GitHub Actions `ci` is green on the pushed branch. It now lints and tests the `stats` and `test-hooks` features.
- P0-13: sanity-check the P0-12 profile tables against `make flamegraph`. The provenance log's denied entries from this phase are benign (a recursive grep at the repository root, and a `sed` regex containing a forbidden word).
- P1-05: review D-27 and D-28 for clean-room wording (SPEC 2.1). P3-04: review D-30 and D-31. P4-04: review the CRC derivation (D-32). P2-04: review the fast-loop code by hand and mutation-sample it (SPEC 5). Rounds 1 and 2 sampled it; see the tables.
- P2-02 / P2-04 / P5-03: run `make fuzz-long` or the nightly workflow for all six targets, including `diff_fast_reference`, for the one-hour signal, and triage `make fuzz-divergences`. Only `make fuzz-smoke` was run.
- P3-04: compare peak RSS of `make bomb` against a phase-1 checkout (`/usr/bin/time -v`).
- P5-03: review the PERF.md result line and the negative results, and complete `provenance/human-interventions.md`. After review and merge into `feature/perf` (and on to `main`), tag `v0.2.0-cleanroom`. The implementer does not tag.

## Notes

- The R1-02 case writes `FAST_LOOP_MIN_INPUT + 8` one-bit literals, which is 3 bytes. The plan asked for that many bytes of symbols. It also runs the file's existing (whole, 4096) / (64, 300) / (whole, 1) schedules rather than the 7/13 schedule the plan named. The 48 pad bytes still put the bad code inside the fast loop, and the mutant is killed on the whole-input call, so the task's goal is met.
- The round-1 notes on the P3-01 commit size and the P4-03 full-bench confirmation still stand. Neither needs action.

# cleanflate

Clean-room DEFLATE/zlib/gzip decompressor in safe Rust, built from RFC 1950/1951/1952 alone.
Phase 3 is the "round 3" performance pass (docs/SPEC.md). The provenance record matters as much as the code.

## Principles
- [ ] Priorities in order: provenance, correctness (byte-identical to C zlib, same accept/reject, same `Error` values, same streaming semantics), safety (no unsafe, no panics, bounded memory), then performance. When performance conflicts with the others it loses, and the negative result is recorded.
- [ ] Measure first. Every optimisation names its hotspot (key, frames, share, from `docs/perf/phase3-profile.md` or a fresh cleanflate-only profile) and shows that share and the bench numbers before and after.
- [ ] Compare like with like: one-shot against C zlib one-shot, streaming against C zlib streaming with the same 64 KiB input and output chunks.
- [ ] No benchmark-specific code paths. Nothing in `crates/cleanflate` detects corpora, sizes or call patterns; every fast path is justified for arbitrary input and exercised by the correctness suites.
- [ ] Every design decision is recorded in `docs/DESIGN.md` as `### D-NN:` (next free number) with an `RFC:` line. Performance decisions follow PT7.
- [ ] Each task is one small commit (≤ ~400 non-test lines). Tests land in the same commit as the code they cover.
- [ ] Tests never skip silently. A missing corpus, manifest, baseline or empty selection is a failure that says how to fix it.
- [ ] One resumable decoder core. The streaming `Decompressor` and the one-shot APIs drive the same `Inflater` through an output sink. Fast paths are layered on the resumable state machine, never beside it.
- [ ] Every error is a typed `Error` variant. No strings, no panics.
- [ ] Tunable numbers live only in `crates/cleanflate/src/config.rs`, each with the measurement that chose it.
- [ ] When SPEC and PLAN allow two readings, choose one and record it under `Interpretation:` in the commit.

## Clean-room rules (SPEC §2)
- CR1: Never read, fetch, search for or paste source code, implementation docs or write-ups of any DEFLATE/zlib implementation: zlib, zlib-ng, zlib-rs, libz-rs-sys, miniz, miniz_oxide, flate2 internals, libdeflate, Go compress/flate, Python zlib C source, Java/.NET inflaters, Zopfli, puff.c, or articles/talks/benchmark write-ups describing how any of them structures its fast path, tables (including multi-literal tables), bit buffer or checksums.
- CR2: Never open `~/.cargo/registry/**`, `~/.cargo/git/**`, `target/**/build/**` or `target/doc/src/**`.
- CR3: No WebSearch, WebFetch, curl or wget. `tools/cleanroom/LOCKDOWN` is in force. Never install or upgrade tooling (cargo-asm is not installed and must not be).
- CR4: Permitted inputs: SPEC and everything under `docs/` (including `docs/phase-1/` and `docs/phase-2/`), `docs/rfc/*.txt`, this repo's own code and history, Rust std docs and general Rust and computer-science knowledge, corpora as opaque bytes, and measurements of cleanflate itself (profiles, counters, its own generated assembly).
- CR5: libz-sys only inside `crates/oracle`, only through the signatures in `docs/phase-1/PLAN.md` task P1-01 plus `zlibCompileFlags() -> uLong`. Streaming C zlib uses `inflateInit2_`/`inflate`/`inflateEnd` with bounded `avail_in`/`avail_out`. C zlib is a black-box stopwatch and oracle only.
- CR6: If the guard denies a call, do not work around it. Note the denial in the progress log and continue another way. The guard matches command text: keep forbidden words (library names, `zlib`, `libz`, network tools) out of read-family command arguments and out of commit command lines. Use rfc1951/rfc1950/rfc1952 in keys, paths and commands, and put word lists inside scripts.
- CR7: Profile, disassemble or debug only cleanflate-only processes (`make profile`, `make profile-stat`, `make flamegraph`, `target/release/examples/profile_decode`) and cleanflate's own `.s` output (`make asm`). Never run `perf`, `cargo flamegraph`, `objdump`, `gdb`, `lldb` or `valgrind` on `oracle`, its test binaries, or anything that executes C zlib. An accidental view of C internals is logged in `provenance/human-interventions.md`.
- CR8: Never add comparator crates (flate2, miniz_oxide, zlib-rs, libdeflater or similar) to any manifest, even as dev-dependencies.
- Use the Grep tool, not `grep -r`, to search this repo. The guard blocks recursive grep of the repo root without `--exclude-dir=target`.

## Performance task protocol
- PT1 Measure: before editing code, run `make bench-quick BENCH_ID=<ID>-before BENCH_OUT=docs/perf/phase3/<ID>-before.json`. After the change, run `make bench-quick BENCH_ID=<ID> BENCH_OUT=docs/perf/phase3/<ID>.json`. Run both in the background (PT8), then `make bench-compare A=docs/perf/phase3/<ID>-before.json B=docs/perf/phase3/<ID>.json`. Also run `make profile PROFILE_KEY=<hotspot key>` before and after, and record the hotspot's share. Commit both JSONs. Write `### <ID>` under `## Task log` in `docs/perf/PHASE3.md`, and put the headline in the commit's `Measurement:` line. A change is a WIN when the targeted key's throughput verdict or its ratio verdict is WIN and neither is LOSS.
- PT2 Gated tasks: first run `make profile PROFILE_KEY=<key>` for the task's keys (plus `make bench-stats` or `make profile-stat` if it names counters). If the named frames total less than max(2%, 2x the key's quick noise in `docs/perf/baselines/phase3-noise-quick.json`) of samples, record a negative result in PHASE3.md, change no code, and commit.
- PT3 Reference path: every fast technique runs only under `DecodePath::Fast`, for both output sinks. `DecodePath::Reference` keeps the simple, obviously-correct behaviour. Every exit from a fast path restores exactly the state the reference path would hold (the D-27 bit-reader invariant).
- PT4 Keep or revert: keep a change that is a WIN, or that is NEUTRAL and a named prerequisite of a later task. Otherwise revert the code, keep its tests and fixtures, and record the negative result.
- PT5 Never lose a repro: on any wrong output, error or `consumed`, minimise the case, add it with `oracle regress-add` to `testdata/regressions/`, commit it, then root-cause. If you must stop, block the task with the fixture path, the hypothesis and what was ruled out. A revert keeps the fixture.
- PT6 Decoder tasks: any change under `crates/cleanflate/src/` must pass `make test`, `make proptest-3k` and `make fuzz-smoke-decoder` (verify runs them) before it is done.
- PT7 DESIGN entries for techniques are `### D-NN: [perf] <title>`, followed by these lines:
  - `RFC:`;
  - `Decision:` and `Rationale:`, in the project's own words from first principles;
  - a line containing `general technique; not derived from any specific implementation`;
  - `Profile:` (hotspot key, frames, share before -> after);
  - `Measurement:`, citing `docs/perf/phase3/<ID>.json`.
  `make design-check` enforces this (the `Profile:` line from D-34 onward).
- PT8 `make bench` (~15 min), `make bench-quick` (~6 min), `make heavy` and `make fuzz-milestone` (30 min) run in the background; wait for completion. Never run two of them at once, and never bench while fuzzing.
- PT9 Milestone acceptance runs these steps in order:
  1. `make ci`;
  2. `make heavy`;
  3. `make fuzz-milestone` (diff_oracle_roundtrip and diff_fast_reference, 1800 s each, zero crashes);
  4. `make bench BENCH_ID=<ID> BENCH_OUT=docs/perf/phase3/<ID>.json`;
  5. `make bench-compare` against `docs/perf/baselines/phase3-start.json` and against `docs/perf/baselines/phase2.json`;
  6. `make bench-targets J=docs/perf/phase3/<ID>.json`;
  7. write the durations, exec counts and verdicts into PHASE3.md and the commit;
  8. `make provenance-snapshot`, commit, push.
  A shortened fuzz run records its real duration and becomes a manual check (SPEC §8).

## Commands
```
make fmt-check lint test corpus crate-checks cleanroom-selftest   # verify set (lint/test also cover features stats, test-hooks)
make tools-check design-check fuzz-check                          # extra verify when tools/, docs/DESIGN.md, fuzz/ change
make proptest-3k fuzz-smoke-decoder   # extra verify for crates/cleanflate/src/ (3000-case chunking+fastslow; 4 fuzz targets x 60 s)
make ci                               # fmt-check lint corpus test crate-checks cleanroom-selftest tools-check design-check
make heavy                            # proptest-10k, bomb, conformance-full, fastslow-full (milestone acceptance)
make fuzz-milestone [FUZZ_MILESTONE_SECONDS=1800]            # diff_oracle_roundtrip + diff_fast_reference, concurrently
make bench BENCH_ID=<ID> BENCH_OUT=docs/perf/phase3/<ID>.json # full: both corpora, raw/zlib/gzip, L1/6/9, all modes, N=7
make bench-quick BENCH_ID=<ID> BENCH_OUT=...                  # Canterbury + Silesia subset, L1 and L6, N=5
make bench-compare A=<json> B=<json> [NOISE=] [BASELINE=]     # deltas, WIN/NEUTRAL/LOSS, ratio verdict, REGRESSION, vs phase 2
make bench-md J=<json> [FILES=1 FILTER=canterbury/]          # markdown tables (per-file rows with sizes)
make bench-targets J=<json>                                   # T1-T6 verdicts
make bench-noise A= B= OUT=    make bench-stats [STATS_CORPUS= STATS_LEVELS=]    make bench-checksum BENCH_ID= BENCH_OUT=
make profile PROFILE_KEY=silesia/rfc1951/l1/oneshot [PROFILE_ITERS=3]   # -> target/profile/<slug>/top.txt, top-children.txt
make profile-stat PROFILE_KEY=...    make flamegraph-save PROFILE_KEY=... TAG=before|after    make profile-required TAG=
make asm && make asm-count ASM_FUNCS=fast_loop ASM_OUT=<json>  # cleanflate's own assembly: panic sites, branches
make malformed regress-seed fuzz-smoke fuzz-long fuzz-divergences
make provenance-snapshot provenance-summary build-report hooks-install
```

## Module map
- `crates/cleanflate/` is the product. It has zero dependencies, `#![forbid(unsafe_code)]`, and the features `stats` and `test-hooks` (both off by default).
  - `src/lib.rs`: crate attributes and re-exports. `src/stream.rs`: `Decompressor` (including `reset`), `Progress`, `Status`, `Decoded`, the one-shot drivers and sizing.
  - `src/error.rs`, `src/options.rs`, `src/config.rs` (all tunables), `src/tables.rs` (RFC 1951 constants), `src/checksum.rs` (Adler-32, CRC-32; public).
  - `src/bitreader.rs`: LSB-first bit buffer with exact fill and bulk refill/give-back. `src/huffman.rs`: canonical construction, validation, two-level lookup tables, and the bit-serial reference.
  - `src/inflate.rs`: `Inflater`, the resumable raw DEFLATE state machine. `src/fast_loop.rs`: the Fast-path loop. `src/window.rs`: 32 KiB history carried across calls. `src/sink.rs`: `OutputSink`, `SliceSink`, `VecSink`.
  - `src/decode_path.rs`: `DecodePath { Fast, Reference }`. `src/test_hooks.rs` (feature `test-hooks`): path and sink selection, checksum variants. `src/stats.rs` (feature `stats`): per-instance counters.
  - `src/zlib.rs`, `src/gzip.rs`: RFC 1950/1952 wrappers. `src/tests/`: every unit test, declared once in lib.rs.
  - `examples/profile_decode.rs`: the only profiling target (cleanflate and std only). `examples/checksum_bench.rs`: checksum microbench (needs `test-hooks`).
- `crates/oracle/` is dev-only:
  - the libz-sys FFI (`ffi.rs`), the corpus generator and the malformed builder;
  - `bench`, `bench-compare`, `bench-noise`, `bench-md`, `bench-targets`, `perf-prep` and `regress-add`/`regress-seed`;
  - the conformance, chunking, fastslow (paths and sinks), reset, regression, bomb and malformed tests (`tests/common/` holds the trace driver).
- `fuzz/` (outside the workspace): `never_panic_{raw,rfc1950,rfc1952}`, `diff_oracle_{roundtrip,arbitrary}`, `diff_fast_reference`.
- `tools/cleanroom/`: guard, logger, self-test, LOCKDOWN, summary and report generators. `tools/ci/`: crate, sources and design checks. `tools/perf/`: fuzz-milestone runner, flamegraph check and assembly counter, with their tests. `tools/git-hooks/commit-msg` adds stage and task tags.
- `testdata/`:
  - `MANIFEST.json`, `malformed/`, `regressions/` (committed repro fixtures), `corpora/`;
  - `generated/` and `perf-inputs/` are gitignored.
  - Paths use rfc1951, rfc1950 and rfc1952.
- `docs/`: `SPEC.md`, `PLAN.md`, `PROGRESS.md`, `DESIGN.md`, `PERF.md`, `rfc/`, `phase-1/` and `phase-2/` (archived, read-only).
  - `docs/perf/`:
    - `README.md` (schemas and columns);
    - `baselines/` (phase2.json, phase3-start*, phase3-noise*);
    - `phase3/` (every phase-3 JSON; phase-2 JSONs stay in `docs/perf/`);
    - `flamegraphs/`, `PHASE3.md` (the report and task log), `phase3-profile.md`.

## Constraints
Rules marked [rule:<id>] are enforced by `docs/foundry.json` constraints. The others name how they are checked.
- C1: The word `unsafe` never appears under `crates/cleanflate/`, including comments. [rule:no-unsafe-token-in-cleanflate] (also `make crate-checks`)
- C2: `src/lib.rs` keeps `#![forbid(unsafe_code)]` and the `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, ...)]` block. Checked by `make crate-checks` (a presence check).
- C3: `crates/cleanflate` has no dependencies of any kind and no build.rs. Checked by `make crate-checks` via cargo metadata (manifest-level, not a line rule).
- C4: No `.unwrap()`, `.expect(`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `assert!`, `assert_eq!` or `assert_ne!` in `crates/cleanflate/src/` outside `src/tests/`, doc examples included. `debug_assert!` is allowed. [rule:no-panicking-calls-in-cleanflate-lib]
- C5: No `[]` indexing or slicing in cleanflate library code. Checked by `make lint` (clippy `indexing_slicing`), because a line pattern cannot tell indexing from array types. No local allow of the panic lints. [rule:no-panic-lint-allow-in-cleanflate-lib]
- C6: cleanflate unit tests live only under `crates/cleanflate/src/tests/`. [rule:no-inline-tests-in-cleanflate-src]
- C7: No clocks, hash maps, randomness, filesystem, network, process, env or thread use in `crates/cleanflate/src/`, and no global mutable state. [rule:no-io-or-nondeterminism-in-cleanflate] [rule:no-global-state-in-cleanflate]
- C8: Tunables get values only in `config.rs`; the 1 GiB limit is never a literal. [rule:tunables-defined-only-in-config] [rule:option-defaults-not-literal] [rule:no-output-limit-literal]
- C9: `libz-sys` is named only inside `crates/oracle/`. [rule:libz-only-in-oracle]
- C10: cleanflate code, DESIGN.md, PERF.md and `docs/perf/` never name reference implementations or their files. [rule:no-reference-implementation-names]
- C11: No tracked path outside `crates/cleanflate/` and `crates/oracle/` contains zlib, miniz, flate2, libdeflate, libz-sys, libz_sys or zopfli. Checked by `make crate-checks` (a path-name rule).
- C12: Every commit body carries `[stage:<plan|implement|review|fix|summarize>] [task:<id>]` (the hook adds it). The reviewer checks `git log --format=%B` (history, not file content).
- C13: Conformance, malformed, fastslow, sink, reset and regression tests fail, never skip, on missing files or empty selections. The reviewer checks by reading (behavioural).
- C14: Every `### D-` block has an `RFC:` line. Every `[perf]` block also has the PT7 phrase and a `Measurement:` line, plus `Profile:` from D-34 onward. Checked by `make design-check` (a multi-line rule).
- C15: Arithmetic that can overflow on hostile input uses checked, saturating or wrapping ops. The reviewer reads; fuzz target (a) checks too.
- C16: Checksums and expected hashes in fixtures come from `oracle::ffi`, never from cleanflate. [rule:no-cleanflate-checksums-in-fixtures] covers `src/tests/`; the reviewer reads elsewhere.
- C17: No `std::arch`/`core::arch`, `std::simd`, `#[target_feature]`, feature detection or nightly `#![feature]` under `crates/cleanflate/`. [rule:no-simd-arch-or-nightly-in-cleanflate]
- C18: Features `stats` and `test-hooks` are never default. [rule:no-default-diagnostic-features] With both off, behaviour is unchanged. The oracle enables `test-hooks` only in `[dev-dependencies]`, and the fuzz crate and the `checksum_bench` example (`required-features`) are the only other users. The reviewer reads the Cargo.toml sections.
- C19: Makefile and CI never profile, disassemble or debug oracle or C code. [rule:no-profiling-of-oracle] The guard enforces CR7 for agent commands.
- C20: Every fast path is gated on `DecodePath::Fast` and has a reference twin compared by `crates/oracle/tests/fastslow.rs`, for both output sinks. The reviewer checks by reading and by mutation sampling of fast-path and sink code (a structural rule).
- C21: No comparator crates in any Cargo.toml. [rule:no-comparator-crates]
- C22: The oracle calls only the permitted C zlib symbols (CR5). [rule:oracle-ffi-allowlist] covers `sys::`/`libz_sys::` paths; the reviewer reads any `extern "C"` block.
- C23: No benchmark-specific paths: no string literal in cleanflate library code names a corpus, corpus file or benchmark. [rule:no-benchmark-specific-paths-in-cleanflate] The reviewer reads for size or pattern detection.
- C24: No `MaybeUninit`, `set_len`, `spare_capacity_mut`, `assume_init` or `transmute` anywhere under `crates/cleanflate/`, comments included. [rule:no-uninit-tricks-in-cleanflate]
- C25: Committed flamegraphs contain only cleanflate, std and libc frames, and are named `<corpus>-<rfc195x>-l<level>-<mode>-<tag>.svg`. [rule:flamegraphs-cleanflate-only] (also `tools/perf/check-flamegraph.sh`)

## Commit template
```
<ID>: <title>

[stage:implement] [task:<ID>]

Goal: <one sentence>
Tests: <test files::names added or changed>
Interpretation: <readings chosen where SPEC/PLAN allowed more than one, or "none">
Measurement: <hotspot share and throughput before -> after, verdicts, JSON paths; perf tasks only, else omit>
Defects-caught: <defects found by tests/conformance/fuzz/differential during this task, or "none">
Manual check: <NOT VERIFIED (human): list | n/a>
```
Review-fix tasks use `[stage:fix]`.

SPEC.md wins over PLAN.md, and PLAN.md wins over code comments.

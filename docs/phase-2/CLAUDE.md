# cleanflate

Clean-room DEFLATE/zlib/gzip decompressor in safe Rust, built from RFC 1950/1951/1952 alone.
Phase 2 is a performance pass (docs/SPEC.md). The provenance record matters as much as the code.

## Principles
- [ ] Priorities in order: provenance, correctness (byte-identical to C zlib, same accept/reject, same streaming semantics), safety (no unsafe, no panics, bounded memory), then performance. When performance conflicts with the others it loses, and the negative result is recorded.
- [ ] Every design decision is recorded in `docs/DESIGN.md` as `### D-NN:` (next free number) with an `RFC:` line. Performance decisions use `### D-NN: [perf] <title>` (see PT7).
- [ ] Each task is one small commit (≤ ~400 non-test lines). Tests land in the same commit as the code they cover.
- [ ] Tests never skip silently. A missing corpus, manifest or empty selection is a failure that says how to fix it.
- [ ] One resumable decoder core. The one-shot APIs drive the streaming `Decompressor`. Fast paths are layered on the resumable state machine, never beside it.
- [ ] Every error is a typed `Error` variant. No strings, no panics.
- [ ] Tunable numbers live only in `crates/cleanflate/src/config.rs`, each with the measurement that chose it.
- [ ] Measure first. Every optimisation names the counter or profile that justified it and the bench numbers before and after.
- [ ] When SPEC and PLAN allow two readings, choose one and record it under `Interpretation:` in the commit.

## Clean-room rules (SPEC §2)
- CR1: Never read, fetch, search for or paste source code, implementation docs or write-ups of any DEFLATE/zlib implementation: zlib, zlib-ng, zlib-rs, libz-rs-sys, miniz, miniz_oxide, flate2 internals, libdeflate, Go compress/flate, Python zlib C source, Java/.NET inflaters, Zopfli, puff.c, or articles/talks describing how any of them structures its fast path, tables or bit buffer.
- CR2: Never open `~/.cargo/registry/**`, `~/.cargo/git/**`, `target/**/build/**` or `target/doc/src/**`.
- CR3: No WebSearch, WebFetch, curl or wget. `tools/cleanroom/LOCKDOWN` is in force. Never install or upgrade tooling.
- CR4: Permitted inputs: SPEC and everything under `docs/` (including `docs/phase-1/`), `docs/rfc/*.txt`, this repo's own code and history, Rust std docs and general Rust knowledge, corpora as opaque bytes, and measurements of cleanflate itself.
- CR5: Use libz-sys only inside `crates/oracle`, only through the signatures listed in `docs/phase-1/PLAN.md` task P1-01. C zlib is a black-box stopwatch and oracle only.
- CR6: If the guard denies a call, do not work around it. Note the denial in the progress log and continue another way.
- CR7: Profile, disassemble or debug only cleanflate-only processes (`make profile`, `make flamegraph`, `target/release/examples/profile_decode`). Never run `perf`, `cargo flamegraph`, `objdump`, `gdb`, `lldb` or `valgrind` on `oracle`, its test binaries, or anything that executes C zlib. An accidental view of C internals is logged in `provenance/human-interventions.md`.
- Use the Grep tool, not `grep -r`, to search this repo. The guard blocks recursive grep of the repo root without `--exclude-dir=target`.

## Performance task protocol
- PT1 Measure: before editing code, `make bench-quick BENCH_ID=<ID> BENCH_OUT=docs/perf/<ID>-before.json`; after, `make bench-quick BENCH_ID=<ID> BENCH_OUT=docs/perf/<ID>.json`, then `make bench-compare A=docs/perf/<ID>-before.json B=docs/perf/<ID>.json`. Commit both JSONs. Put the headline lines in `docs/PERF.md` (under `### <ID>`) and in the commit's `Measurement:` line. Only a `WIN` verdict (beats 2x measured noise) counts as a win.
- PT2 Gated tasks: first run `make profile` and `make bench-stats`. If the targeted functions/counters account for less than max(2%, 2x aggregate noise) of samples, record a negative result in PERF.md, change no code, and commit.
- PT3 Reference path: every fast technique runs only under `DecodePath::Fast`. `DecodePath::Reference` keeps the simple, obviously-correct behaviour. Every exit from a fast path restores exactly the state the reference path would hold (the bit-reader invariant in DESIGN.md).
- PT4 Keep or revert: keep a change that is a `WIN`, or `NEUTRAL` and a named prerequisite of a later task. Otherwise revert the code, keep its tests and fixtures, and record the negative result.
- PT5 Never lose a repro: on any wrong output, error or `consumed`, minimise the case, add it with `oracle regress-add` to `testdata/regressions/`, commit it, then root-cause. If you must stop, block the task with the fixture path, the hypothesis and what was ruled out. A revert keeps the fixture.
- PT6 Decoder tasks: any change under `crates/cleanflate/src/` must pass `make proptest-3k` (run by verify) and `make test` (which includes the fast-vs-reference differential) before it is done.
- PT7 DESIGN entries for techniques: `### D-NN: [perf] <title>`, then `RFC:`, `Decision:`, `Rationale:` in the project's own words from first principles, a line containing `general technique; not derived from any specific implementation`, and a `Measurement:` line citing `docs/perf/<ID>.json`. `make design-check` enforces this.
- PT8 `make bench` takes 10-20 minutes and `make heavy` longer: run them in the background and wait for completion before continuing.

## Commands
```
make fmt-check lint test corpus crate-checks cleanroom-selftest   # verify set (lint/test also cover features stats, test-hooks)
make tools-check design-check fuzz-check                          # extra verify when tools/, docs/DESIGN.md, fuzz/ change
make proptest-3k                  # chunking + fast-vs-reference proptests at 3000 cases (decoder tasks)
make ci                           # everything above, in order (GitHub Actions runs this)
make heavy                        # proptest-10k, bomb, conformance-full, fastslow-full (milestone acceptance)
make bench BENCH_ID=<ID> BENCH_OUT=docs/perf/<ID>.json   # full: Silesia+Canterbury, raw/zlib/gzip, L1/6/9, one-shot+streaming, N=7
make bench-quick BENCH_ID=<ID> BENCH_OUT=...             # Canterbury + Silesia subset, L6, N=5
make bench-compare A=<json> B=<json>                     # per-file and aggregate deltas, WIN/NEUTRAL/LOSS, REGRESSION
make bench-stats BENCH_ID=<ID> BENCH_OUT=...             # decoder counters (feature stats) on Silesia
make profile [PROFILE_FRAMING=raw PROFILE_LEVEL=6 PROFILE_ITERS=3]   # perf on the cleanflate-only example -> target/profile/top.txt
make flamegraph                   # same, SVG in target/profile/
make malformed fuzz-smoke fuzz-long fuzz-divergences
make provenance-snapshot provenance-summary build-report hooks-install
```

## Module map
- `crates/cleanflate/` is the product: zero dependencies, `#![forbid(unsafe_code)]`, features `stats` and `test-hooks` (both off by default).
  - `src/lib.rs`: crate attributes, re-exports; `src/stream.rs`: `Decompressor`, `Progress`, `Status`, `Decoded`, `decompress_with`, one-shot sizing.
  - `src/error.rs`, `src/options.rs`, `src/config.rs` (all tunables), `src/tables.rs` (RFC 1951 constants), `src/checksum.rs` (Adler-32, CRC-32; public).
  - `src/bitreader.rs`: LSB-first bit buffer, exact fill and bulk refill/give-back. `src/huffman.rs`: canonical construction, validation, lookup tables and the bit-serial reference decode.
  - `src/inflate.rs`: `Inflater`, the resumable raw DEFLATE state machine, plus the fast loop. `src/window.rs`: 32 KiB history carried across calls and match copies.
  - `src/decode_path.rs`: `DecodePath { Fast, Reference }`. `src/test_hooks.rs` (feature `test-hooks`) exposes path selection. `src/stats.rs` (feature `stats`) holds the counters.
  - `src/zlib.rs`, `src/gzip.rs`: RFC 1950/1952 wrappers. `src/tests/`: every unit test, declared once in lib.rs.
  - `examples/profile_decode.rs`: cleanflate-only decode loop over prepared inputs, the only profiling target.
- `crates/oracle/` is dev-only: libz-sys FFI (`ffi.rs`), corpus generator, malformed builder, `bench`/`bench-compare`/`bench-noise`/`bench-md`, `perf-prep`, `regress-add`/`regress-seed`, and the conformance, chunking, fastslow, regression, bomb and malformed tests (`tests/common/` holds the shared trace driver).
- `fuzz/` (outside the workspace): `never_panic_{raw,rfc1950,rfc1952}`, `diff_oracle_{roundtrip,arbitrary}`, `diff_fast_reference`.
- `tools/cleanroom/`: guard, logger, self-test, LOCKDOWN, summary and report generators. `tools/ci/`: crate, sources and design checks. `tools/git-hooks/commit-msg` adds stage and task tags.
- `testdata/`: `MANIFEST.json` (generated corpus hashes), `malformed/`, `regressions/` (committed repro fixtures and `MANIFEST.tsv`), `perf-inputs/` (gitignored), `corpora/`, `generated/` (gitignored). Paths use rfc1951, rfc1950, rfc1952.
- `docs/`: `SPEC.md`, `PLAN.md`, `PROGRESS.md`, `DESIGN.md`, `PERF.md`, `perf/*.json`, `rfc/`, `phase-1/` (archived, read-only).

## Constraints
Rules marked [rule:<id>] are enforced by `docs/foundry.json` constraints. The others name how they are checked.
- C1: The word `unsafe` never appears under `crates/cleanflate/`, including comments. [rule:no-unsafe-token-in-cleanflate] (also `make crate-checks`)
- C2: `src/lib.rs` keeps `#![forbid(unsafe_code)]` and the `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, ...)]` block. `make crate-checks` (presence check).
- C3: `crates/cleanflate` has no dependencies of any kind and no build.rs. `make crate-checks` via cargo metadata (manifest-level, not a line rule).
- C4: No `.unwrap()`, `.expect(`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `assert!`, `assert_eq!`, `assert_ne!` in `crates/cleanflate/src/` outside `src/tests/`, doc examples included. `debug_assert!` is allowed. [rule:no-panicking-calls-in-cleanflate-lib]
- C5: No `[]` indexing or slicing in cleanflate library code (`make lint`, clippy `indexing_slicing`; a line pattern cannot tell indexing from array types). No local allow of the panic lints. [rule:no-panic-lint-allow-in-cleanflate-lib]
- C6: cleanflate unit tests live only under `crates/cleanflate/src/tests/`. [rule:no-inline-tests-in-cleanflate-src]
- C7: No clocks, hash maps, randomness, filesystem, network, process, env or thread use in `crates/cleanflate/src/`, and no global mutable state (stats are per-instance counters). [rule:no-io-or-nondeterminism-in-cleanflate] [rule:no-global-state-in-cleanflate]
- C8: Tunables get values only in `config.rs`; the 1 GiB limit is never a literal. [rule:tunables-defined-only-in-config] [rule:option-defaults-not-literal] [rule:no-output-limit-literal]
- C9: `libz-sys` is named only inside `crates/oracle/`. [rule:libz-only-in-oracle]
- C10: cleanflate code, DESIGN.md, PERF.md and `docs/perf/` never name reference implementations or their files. [rule:no-reference-implementation-names]
- C11: No tracked path outside `crates/cleanflate/` and `crates/oracle/` contains zlib, miniz, flate2, libdeflate, libz-sys, libz_sys or zopfli. `make crate-checks` (path-name rule).
- C12: Every commit body carries `[stage:<plan|implement|review|fix|summarize>] [task:<id>]` (hook adds it). Reviewer checks `git log --format=%B` (history, not file content).
- C13: Conformance, malformed, fastslow and regression tests fail, never skip, on missing files or empty selections. Reviewer checks by reading (behavioural).
- C14: Every `### D-` block has an `RFC:` line; every `[perf]` block also has the PT7 phrase and a `Measurement:` line. `make design-check` (multi-line rule).
- C15: Arithmetic that can overflow on hostile input uses checked, saturating or wrapping ops. Reviewer reads; fuzz target (a) checks too.
- C16: Checksums and expected hashes in fixtures come from `oracle::ffi`, never from cleanflate. [rule:no-cleanflate-checksums-in-fixtures] for `src/tests/`; reviewer reads elsewhere.
- C17: No `std::arch`/`core::arch`, `std::simd`, `#[target_feature]`, feature detection or nightly `#![feature]` under `crates/cleanflate/`. [rule:no-simd-arch-or-nightly-in-cleanflate]
- C18: Features `stats` and `test-hooks` are never default features. [rule:no-default-diagnostic-features] With both off, behaviour is unchanged; the oracle enables `test-hooks` only in `[dev-dependencies]` (reviewer reads Cargo.toml sections).
- C19: Makefile and CI never profile, disassemble or debug oracle or C code. [rule:no-profiling-of-oracle] The guard enforces CR7 for agent commands.
- C20: Every fast path is gated on `DecodePath::Fast` and has a reference-path twin compared by `crates/oracle/tests/fastslow.rs`. Reviewer checks by reading and by mutation sampling of fast-path code (a structural rule).

## Commit template
```
<ID>: <title>

[stage:implement] [task:<ID>]

Goal: <one sentence>
Tests: <test files::names added or changed>
Interpretation: <readings chosen where SPEC/PLAN allowed more than one, or "none">
Measurement: <before -> after, verdict, JSON paths; perf tasks only, else omit>
Defects-caught: <defects found by tests/conformance/fuzz/differential during this task, or "none">
Manual check: <NOT VERIFIED (human): list | n/a>
```
Review-fix tasks use `[stage:fix]`.

SPEC.md wins over PLAN.md, and PLAN.md wins over code comments.

# cleanflate

Clean-room DEFLATE/zlib/gzip decompressor in safe Rust, built from RFC 1950/1951/1952 alone.
The provenance record matters as much as the code.

## Principles
- [ ] Priorities in order: provenance, correctness (byte-identical to C zlib), safety (no unsafe, no panics, bounded memory). Performance comes after all three.
- [ ] Every design decision comes from `docs/rfc/*.txt` and is recorded in `docs/DESIGN.md` as `### D-NN:` with an `RFC:` line.
- [ ] Each task is one small commit (≤ ~400 non-test lines). Tests land in the same commit as the code they cover.
- [ ] Tests never skip silently. A missing corpus or empty selection is a failure that says how to fix it.
- [ ] One resumable decoder core. The one-shot APIs drive the streaming `Decompressor`, so chunking invariance is structural.
- [ ] Every error is a typed `Error` variant. No strings, no panics.
- [ ] Tunable numbers live only in `crates/cleanflate/src/config.rs`.
- [ ] When SPEC and PLAN allow two readings, choose one and record it under `Interpretation:` in the commit.

## Clean-room rules (SPEC §2)
- CR1: Never read, fetch, search for or paste source code or implementation docs of any DEFLATE/zlib implementation: zlib, zlib-ng, zlib-rs, libz-rs-sys, miniz, miniz_oxide, flate2 internals, libdeflate, Go compress/flate, Python zlib C source, Java Inflater, Zopfli, puff.c, or blog walkthroughs of any of them.
- CR2: Never open `~/.cargo/registry/**`, `~/.cargo/git/**`, `target/**/build/**` or `target/doc/src/**`.
- CR3: No WebSearch, ever. WebFetch, curl and wget are allowed only for the URLs in `tools/cleanroom/allowlist.txt`, and only before `tools/cleanroom/LOCKDOWN` exists.
- CR4: Permitted inputs: SPEC and `docs/`, `docs/rfc/*.txt`, Rust std docs and general Rust knowledge, corpora as opaque bytes.
- CR5: Use libz-sys only inside `crates/oracle`, only through the signatures listed in task P1-01. Fix compile errors from compiler messages, never by reading libz-sys sources.
- CR6: If the guard denies a call, do not work around it. Note the denial in the progress log and continue another way.
- Use the Grep tool, not `grep -r`, to search this repo. The guard blocks recursive grep of the repo root without `--exclude-dir=target`.

## Commands
```
make fmt-check           # cargo fmt --all -- --check
make lint                # cargo clippy --workspace --all-targets -- -D warnings
make corpus              # generate/verify quick-tier conformance corpus (testdata/generated, MANIFEST.json)
make test                # cargo test --workspace (quick tier)
make crate-checks        # zero deps, no unsafe token, crate attrs, forbidden path names
make cleanroom-selftest  # guard/logger end-to-end self-test
make tools-check         # shellcheck, python unittest, script tests (runs when tools/ changes)
make design-check        # every DESIGN.md decision has an RFC: line
make ci                  # all of the above, in order
make corpus-full conformance-full   # adds Silesia tier (release)
make proptest-10k bomb heavy        # 10k chunking cases, zip-bomb, all heavy suites
make malformed           # regenerate testdata/malformed from the builder
make fuzz-check fuzz-smoke fuzz-long fuzz-divergences   # cargo-fuzz (smoke/long need nightly)
make bench               # cleanflate vs C zlib throughput -> docs/PERF.md
make provenance-snapshot provenance-summary build-report hooks-install
```

## Module map
- `crates/cleanflate/` is the product. It has zero dependencies and `#![forbid(unsafe_code)]`.
  - `src/lib.rs` holds the crate attributes, re-exports, and `inflate`, `zlib_decompress`, `gzip_decompress`, `decompress_with`.
  - `src/error.rs` defines `Error`, `HeaderError`, `HuffmanError`, `ChecksumKind` and `ErrorKind` (`Error::kind()`, `ErrorKind::as_str()`).
  - `src/config.rs` holds `DEFAULT_MAX_OUTPUT`, `DEFAULT_MAX_GZIP_FIELD`, `DEFAULT_MAX_GZIP_HEADERS`, `LITLEN_TABLE_BITS` and `DIST_TABLE_BITS`, and is the only place they get values.
  - `src/options.rs` defines `Format { Raw, Zlib, Gzip }`, `Options`, and `TrailingData { Reject, Allow }`.
  - `src/checksum.rs` provides `Adler32`/`adler32` (RFC 1950) and `Crc32`/`crc32` (RFC 1952). It is public.
  - `src/tables.rs` holds the RFC 1951 §3.2.5–3.2.7 constant tables.
  - `src/bitreader.rs` is the LSB-first resumable bit buffer. It keeps at most 7 bits between steps.
  - `src/window.rs` is the 32 KiB history ring, including overlapping copies.
  - `src/huffman.rs` does canonical construction, validation (`TableKind`), and table and bit-serial decoding.
  - `src/inflate.rs` is `Inflater`, the resumable raw DEFLATE state machine (`step`).
  - `src/zlib.rs` and `src/gzip.rs` handle the RFC 1950 and RFC 1952 headers and trailers. `gzip.rs` also defines `GzipHeader`.
  - `src/stream.rs` has the public `Decompressor`, `Progress`, `Status`, `Decoded`, the limits and the trailing policy.
  - `src/tests/` holds every unit test. It is declared once in lib.rs as `#[cfg(test)] mod tests;`.
- `crates/oracle/` is dev-only. It is the libz-sys FFI (`ffi.rs`), the corpus generator (`oracle generate`), the malformed builder (`oracle malformed`), the benchmark (`oracle bench`), and all conformance, property and bomb tests.
- `fuzz/` is cargo-fuzz and sits outside the workspace. Targets: `never_panic_{raw,rfc1950,rfc1952}`, `diff_oracle_{roundtrip,arbitrary}`.
- `tools/cleanroom/` holds the guard (PreToolUse), the logger (PostToolUse), the self-test, fetch scripts, `LOCKDOWN`, and the summary and report generators.
- `tools/ci/` holds `crate-checks.sh`, `sources-check.sh`, `design-check.sh` and their tests. `tools/git-hooks/commit-msg` adds stage and task tags.
- `provenance/` holds the live `tool-log.jsonl` (gitignored), `tool-log.snapshot.jsonl`, `README.md`, `SUMMARY.md` and `human-interventions.md`.
- `testdata/`:
  - `corpora/` holds downloads (gitignored) and `SOURCES.json`.
  - `generated/` is gitignored.
  - `MANIFEST.json` is the committed hash manifest for `generated/`.
  - `malformed/` holds the committed malformed files and `EXPECTED.tsv`.
  - Paths use rfc1951, rfc1950 and rfc1952 for the three framings.
- `docs/`: `SPEC.md`, `PLAN.md`, `PROGRESS.md`, `DESIGN.md`, `PERF.md`, `BUILD_REPORT.md`, `rfc/`.

## Constraints
Rules marked [rule:<id>] are enforced by `docs/foundry.json` constraints. The others name how they are checked.
- C1: The word `unsafe` never appears anywhere under `crates/cleanflate/`, including code, comments and docs. [rule:no-unsafe-token-in-cleanflate] (also `make crate-checks`)
- C2: `crates/cleanflate/src/lib.rs` keeps `#![forbid(unsafe_code)]` and `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, ...)]`. Checked by `make crate-checks`. This is a presence check, not a line pattern.
- C3: `crates/cleanflate` has no dependencies of any kind (normal, dev or build) and no build.rs. Checked by `make crate-checks` via cargo metadata, because it spans the manifest and cannot be a line pattern.
- C4: No `.unwrap()`, `.expect(`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `assert!`, `assert_eq!` or `assert_ne!` in `crates/cleanflate/src/` outside `src/tests/`, and that includes doc examples, which use `?`. `debug_assert!` is allowed. [rule:no-panicking-calls-in-cleanflate-lib]
- C5: No `[]` indexing or slicing in cleanflate library code. Use `get`, `get_mut`, iterators, `split_at_checked` or `chunks`. Checked by clippy `indexing_slicing` (`make lint`). A line pattern cannot tell indexing apart from array types.
- C6: cleanflate unit tests live only under `crates/cleanflate/src/tests/`. [rule:no-inline-tests-in-cleanflate-src]
- C7: No clocks, hash maps, randomness, filesystem, network, process, env or thread use in `crates/cleanflate/src/`. [rule:no-io-or-nondeterminism-in-cleanflate]
- C8: Tunables get values only in `crates/cleanflate/src/config.rs`. Everything else refers to them by name, and the 1 GiB limit is never written as a literal. [rule:tunables-defined-only-in-config] [rule:option-defaults-not-literal] [rule:no-output-limit-literal]
- C9: `libz-sys` is named only inside `crates/oracle/`. [rule:libz-only-in-oracle]
- C10: cleanflate code, DESIGN.md and PERF.md never name reference implementations or their files. [rule:no-reference-implementation-names]
- C11: No tracked path outside `crates/cleanflate/` and `crates/oracle/` contains zlib, miniz, flate2, libdeflate, libz-sys, libz_sys or zopfli. Checked by `make crate-checks`. This is a path-name rule, not a line rule.
- C12: Every commit body carries `[stage:<plan|implement|review|fix|summarize>] [task:<id>]`. The `tools/git-hooks/commit-msg` hook adds it. The reviewer checks with `git log --format=%B`, because this is history, not file content.
- C13: Conformance and malformed tests fail, never skip, when files are missing or a selection is empty. The reviewer checks this by reading, since it is a behavioural rule.
- C14: Every `### D-` block in DESIGN.md has an `RFC:` line. Checked by `make design-check`, a multi-line structural rule.
- C15: Arithmetic that can overflow on hostile input uses checked, saturating or wrapping ops, because debug builds and fuzzing panic on overflow. The reviewer checks this by reading, and fuzz target (a) checks it too.
- C16: Checksums used to build test fixtures come from `oracle::ffi`, never from cleanflate, so fixtures stay independent. Enforced for `crates/cleanflate/src/tests/` (except `tests/checksum.rs` itself, which legitimately tests `crate::checksum`) by [rule:no-cleanflate-checksums-in-fixtures]; a value used elsewhere in `crates/oracle/` is checked by the reviewer reading.

## Commit template
```
<ID>: <title>

[stage:implement] [task:<ID>]

Goal: <one sentence>
Tests: <test files::names added or changed>
Interpretation: <readings chosen where SPEC/PLAN allowed more than one, or "none">
Measurement: <before -> after numbers; tuning/perf tasks only, else omit>
Defects-caught: <defects found by tests/conformance/fuzz during this task, or "none">
Manual check: <NOT VERIFIED (human): list | n/a>
```
Review-fix tasks use `[stage:fix]`.

SPEC.md wins over PLAN.md, and PLAN.md wins over code comments.

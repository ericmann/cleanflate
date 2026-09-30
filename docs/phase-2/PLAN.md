# cleanflate phase 2 build plan
Derived from docs/SPEC.md v(unversioned, commit 877892c) on 2026-09-29. SPEC.md wins over this file.

## Decisions
- Phases vs milestones → Phase N implements SPEC §6 milestone MN (M0–M5), in order, not merged. Rationale: SPEC defines the milestones and their acceptance.
- Review cadence (SPEC §6 "each milestone ends with an acceptance task") → one Foundry review loop per flight, up to 6 rounds (SPEC §7). Each phase ends with an acceptance-and-push task. Rationale: Foundry reviews once per flight. The acceptance tasks carry the per-milestone gates.
- Base branch → `feature/perf`; build branches use the `build/` prefix and PR into `feature/perf` (SPEC §7). Rationale: stated by SPEC.
- Narrow dependencies (SPEC §6) → optimisation tasks depend on the M0 infrastructure tasks they use (switch, differential, fixtures, stats, bench), never on acceptance tasks. Acceptance tasks depend only on their milestone's core task. Rationale: phase 1 lost 17 tasks to one cascade.
- Task order vs "order the remaining milestones' tasks by measured cost" (SPEC §4.4, M0) → milestone order is fixed (M1 → M4; the fast loop needs bulk refill and table decode, output history needs a stable loop). P0-12 records the measured cost ranking. Tasks whose hypothesis may prove cheap (P1-03, P1-04, P2-03, P4-02) are **gated**: they measure first and record a negative result without code changes when the target is below max(2%, 2x noise) of samples (CLAUDE.md PT2). Rationale: PLAN cannot be reordered mid-flight, and gates give the same effect.
- Force-slow-path switch (SPEC §5 "cfg(test)-only or feature-gated") → cargo feature `test-hooks` (off by default) exposing `cleanflate::test_hooks::{DecodePath, decompress_with_path, decompressor_with_path}`. `crates/oracle` enables it only in `[dev-dependencies]`. Rationale: `cfg(test)` of cleanflate is invisible to the oracle's integration tests, which own the corpora.
- What the Reference path switches off → every decode technique: bulk refill (exact byte-at-a-time fill), table lookup (bit-serial `Huffman::decode`), the fast loop, and slice/doubling match copies (byte-by-byte copy through the same history view). Storage changes shared by both paths (table ownership, output-as-history, stored-block slice copy) are covered by conformance and chunking tests against C zlib. Checksums are not switched; they have direct oracle tests. Rationale: the reference must stay obviously correct, while structural code cannot sensibly exist twice.
- "Identical consumed" in the differential (SPEC §5) → compared **per call**, together with produced, status and the full `Error` value (Debug string), plus `finish()`, `total_in`, `total_out`, `trailing_start` and gzip member count. The Fast path therefore gives back whole unneeded bytes at every non-NeedsInput exit so its reader holds exactly the Reference reader's bits (P1-01). Rationale: the strongest reading, and it catches the phase-1 P4-08 class of bug at the call where it happens.
- Invalid-code timing (found while planning: phase-1 D-16 reports `Invalid` from the table after ≥ table-width bits but the serial walk only after 15, so the two paths disagree and the answer depends on a tunable) → one rule for both decoders: a code is `Invalid` as soon as ≥ 1 buffered bit proves no code matches or extends it. This only arises for the two incomplete distance tables RFC 1951 §3.2.7 allows (P0-04). Rationale: table-width independent, accept/reject unchanged, and the malformed corpus expectations do not move.
- Stats "time or calls spent building tables" vs C7 (no clocks in cleanflate) → the `stats` feature counts table builds and entries written, never time. Time comes from profiles. Counters are per-instance fields (no global state), and with the feature off they do not exist. Rationale: C7 and "default build bit-for-bit unaffected".
- Stats API → `#[cfg(feature = "stats")]` `cleanflate::Stats`, `Decompressor::stats()` and `decompress_with_stats()`. The oracle feature `stats` forwards to it, and `oracle bench --stats` prints it. Rationale: additive, opt-in and required by SPEC §4.3.
- Profiling entry point (SPEC §4.3) → `crates/cleanflate/examples/profile_decode.rs`. It depends only on cleanflate and std, so no C code exists in the profiled process. It reads inputs prepared by the unprofiled `oracle perf-prep` into gitignored `testdata/perf-inputs/`. `make profile`/`make flamegraph` wrap `perf record`/`cargo flamegraph` around it. Rationale: the strictest reading of "never calls C zlib in the profiled process".
- Guarding SPEC §2.2 mechanically → the PreToolUse guard denies perf/flamegraph/objdump/gdb/lldb/valgrind commands unless they target the cleanflate-only example or `target/profile/` (P0-02). A foundry constraint keeps Makefile/CI clean. Rationale: provenance is priority 1, and prose rules are not enough (phase-1 SPEC §2.3 lesson).
- Streaming-target denominator (SPEC §4.1 "≥ 60% of C zlib ... through `Decompressor` with 64 KiB chunks") → cleanflate streaming median vs C zlib **one-shot** median on the same input. Rationale: SPEC §10 forbids changing the oracle's C zlib usage beyond timing, and a harder denominator cannot overstate the result.
- Bench method (SPEC §4.2) → a warm-up decode that verifies the output, then N timed samples (full N=7, quick N=5). Each sample repeats the decode until it lasts ≥ 20 ms and runs the implementations interleaved. The bench reports median and min. The aggregate is total bytes / sum of per-file medians. The full bench covers Silesia + Canterbury, raw/zlib/gzip, levels 1/6/9, one-shot and streaming. Quick covers Canterbury + Silesia {dickens, mr, ooffice, sao, xml}, level 6. Rationale: SPEC §4.2 plus stable numbers for small files.
- Noise and verdicts (SPEC §4.1/§4.2) → noise is |run1 − run2| / run1 of cleanflate median throughput per aggregate key and per file, from two baseline runs (full and quick separately). `WIN` means the aggregate delta is > 2x its noise. A per-file `REGRESSION` means the delta is < −max(file noise, aggregate noise). Rationale: tiny Canterbury files have noisy timings, and SPEC's per-file rule should not fire on jitter.
- Performance results → `docs/perf/<task-id>.json` (plus `<task-id>-before.json` for PT1 tasks and `docs/perf/P4-03/` for the sweep), generated only by `oracle bench`/`bench-noise`. PERF.md tables come from `oracle bench-md`, and PERF.md keeps the phase-1 sections and adds `## Phase 2`. Rationale: SPEC §4.2 and no hand-transcription.
- Benchmarks are not CI gates, and acceptance-task performance criteria are recorded, not blocking → an unmet target or ratio is written as NOT MET, and the task completes. A correctness failure (ci, heavy, differential, regression) blocks with a fixture per PT5. Rationale: SPEC §4.1 "report either way" and §1 priorities.
- 3000-case rule (SPEC §5) → `make proptest-3k` (chunking + fastslow at 3000 cases) runs through `extraVerify` for any change under `crates/cleanflate/src/`. Milestone acceptance runs `make heavy` (10k cases, full-tier differential, full conformance, bomb). Rationale: mechanical rather than remembered.
- Regression fixtures (SPEC §5) → `testdata/regressions/` with `MANIFEST.tsv` (expected output length and SHA-256 from the oracle, or an expected ErrorKind) plus `oracle regress-add`/`regress-seed`. Tests run each fixture one-shot, at 1-byte chunks and with its recorded schedule, on both paths. Proptest failure dumps land in `target/regress-candidates/`. Rationale: a repro is a committed file with a test, never a log line.
- New tunables (SPEC §3, C8) → `FAST_LOOP_MIN_INPUT = 16`, `FAST_LOOP_MIN_OUTPUT = 258`, `ONESHOT_GROWTH_FLOOR = 32_768` (moves the phase-1 literal out of stream.rs), `ONESHOT_INITIAL_RATIO = 4`, `ONESHOT_MAX_PREALLOC = 64 MiB`, `CRC32_SLICES = 8`, all in config.rs. Each is introduced by the task that first needs it and re-swept in P4-03. Rationale: SPEC §3/§6 M4.
- One-shot size hints (SPEC §6 M3 "gzip ISIZE when trustworthy") → ISIZE (last 4 bytes) and `input.len() x ONESHOT_INITIAL_RATIO` are allocation hints only, capped by `max_output` and by `ONESHOT_MAX_PREALLOC`. Rationale: a hostile ISIZE must not force a large up-front allocation. Memory stays proportional to output actually produced, beyond a fixed cap.
- Toolchain, SIMD, threads, nightly → unchanged: 1.89.0, none of the others (SPEC §3/§10), enforced by constraint `no-simd-arch-or-nightly-in-cleanflate`.
- Phase-2 task IDs reuse the `P<n>-<nn>` shape and collide with phase-1 IDs in git history → the provenance summary and build report gain `--since`/`--since-commit` scoping with `PHASE_BASE ?= 599ca51` (last phase-1 commit) in P5-01. Rationale: phase-1 history must not be counted as phase-2 work.
- DESIGN numbering → every new entry takes the next free `D-NN` (D-26 at plan time). PLAN never names a number. Performance entries use `### D-NN: [perf] <title>`, enforced by design-check (P0-03). Rationale: phase-1 ID drift.
- Tagging → the implementer never tags. A human tags `v0.2.0-cleanroom` after review and merge (SPEC §6 M5).

## Conventions
- Every task is one commit. The subject is `<ID>: <title>`, and the body follows the `## Commit template` in CLAUDE.md, including `[stage:implement] [task:<ID>]`, `Defects-caught:` and, for performance tasks, `Measurement:`.
- The implementer reads CLAUDE.md, the task and the SPEC sections the task cites. `C1`–`C20`, `CR1`–`CR7` and `PT1`–`PT8` refer to CLAUDE.md.
- "Global invariants" in a task means: C1–C8, C15, C17 and SPEC §3 apply to every file under `crates/cleanflate/` the task touches. That means no unsafe, no panicking calls or `[]` indexing in library code, zero dependencies, no clocks/IO/global state, tunables only in config.rs, checked arithmetic on hostile values, and no SIMD/arch/nightly.
- cleanflate unit tests go in `crates/cleanflate/src/tests/<name>.rs`, declared in `src/tests/mod.rs`. Tests needing the oracle, proptest, the corpora or `test_hooks` go in `crates/oracle/tests/`. Tests of the `stats` feature are gated `#[cfg(feature = "stats")]` and run by `make test`.
- Test names in Acceptance tests are `file::function`. All must exist and pass.
- `make` targets are the only commands the implementer needs. `make bench` and `make heavy` exceed the 10-minute foreground limit: run them in the background and wait (PT8).
- A phase-end task writes `Manual check: NOT VERIFIED (human)` with its list in the progress log and never waits.

## Phase 0 — M0 Baseline, instrumentation, safety net

### P0-01: Feature flags, Makefile targets and ignore rules
**Goal:** Declare the opt-in `stats` and `test-hooks` features, make `make lint`/`make test` (and therefore CI) build, lint and test them, and add `make proptest-3k`.
**Files touched:** `crates/cleanflate/Cargo.toml`, `crates/oracle/Cargo.toml`, `Cargo.lock`, `Makefile`, `.gitignore`, `docs/perf/README.md` (new).
**Design constraints:** SPEC §3 (zero dependencies: features are not dependencies), §4.3 (stats compiled out by default), §5 (switch unreachable in the default build), §6 M0 ("CI: also build and clippy the stats feature"). C3, C18.
- `crates/cleanflate/Cargo.toml`: add `[features]` with `stats = []` and `test-hooks = []`. There is no `default` key naming either.
- `crates/oracle/Cargo.toml`:
  - Add `[features] stats = ["cleanflate/stats"]`.
  - Add `[dev-dependencies] cleanflate = { version = "0.1.0", path = "../cleanflate", features = ["test-hooks"] }`.
  - Leave the normal `[dependencies]` entry exactly as it is, without features.
- `Makefile`:
  - `lint`: keep the existing line and add:
    - `cargo clippy -p cleanflate --lib -- -D warnings` (default features only; catches code that is dead without a feature)
    - `cargo clippy -p cleanflate --all-targets --features stats -- -D warnings`
    - `cargo clippy -p cleanflate --all-targets --features stats,test-hooks -- -D warnings`
  - `test`: keep `cargo test --workspace`, and add `cargo test -p cleanflate` and `cargo test -p cleanflate --features stats`.
  - New `proptest-3k: CLEANFLATE_CHUNKING_CASES=3000 cargo test --release -p oracle --test chunking` (P0-05 extends it). Add it to `.PHONY`.
- `.gitignore`: add `/testdata/perf-inputs/`. Profile output goes to `target/profile/` and bench scratch to `target/bench/`, both already ignored via `/target`.
- `docs/perf/README.md`: one short section saying this directory holds bench JSON files named by task id, written only by `oracle bench`/`oracle bench-noise` and never hand-edited. The schema is added by P0-08.
- `.github/workflows/ci.yml` needs no change, because it runs `make ci`, which runs lint and test.
**Acceptance tests:**
- No new test files.
- `make lint`, `make test`, `make proptest-3k` and `make crate-checks` pass.
- `cargo metadata --format-version 1 --no-deps` lists cleanflate with 0 dependencies and features `stats` and `test-hooks`.
**Out of scope:** Any Rust source. The `test_hooks` module (P0-04). The `stats` module (P0-07).
**Verification:** `make lint && make test && make proptest-3k && make crate-checks`.
**Depends on:** none

### P0-02: Guard denies profiling of anything but cleanflate-only binaries
**Goal:** Extend the PreToolUse guard so agent Bash commands cannot profile, disassemble or debug the oracle or anything that executes C zlib. Profiling of the cleanflate-only example stays allowed.
**Files touched:**
- `tools/cleanroom/guard.py`, `tools/cleanroom/tests/test_guard.py`, `tools/cleanroom/README.md`.
- New fixtures in `tools/cleanroom/fixtures/`:
  - `deny_bash_perf_record_oracle.json`, `deny_bash_flamegraph_oracle.json`, `deny_bash_objdump_oracle.json`, `deny_bash_gdb_test_binary.json`
  - `allow_bash_perf_record_profile_decode.json`, `allow_bash_perf_report_profile_dir.json`, `allow_bash_make_profile.json`
- `tools/cleanroom/selftest.sh`, only if it lists fixtures explicitly.
**Design constraints:** SPEC §2 (the phase-1 guard applies in full and is not relaxed) and §2.2 (profiling, disassembling or stepping through C zlib is forbidden). CR6, CR7. Fail closed and log denials exactly like existing rules. Create fixture files with the Write tool, not shell heredocs, because the guard inspects Bash command text.
- The rule applies per shell segment, using the guard's existing splitting.
- **Tool words:** `perf`, `objdump`, `gdb`, `lldb`, `valgrind`, `ltrace`, `uftrace`, `flamegraph`, `cargo-flamegraph`, and `cargo` whose first non-option argument is `flamegraph`.
- **Deny** a tool-word segment if any token contains `oracle`, `libz` or `zlib`, case-insensitive.
- **Otherwise allow** a tool-word segment only if one of these holds:
  - It references `profile_decode`.
  - It is `perf report|annotate|script|diff` whose input (`-i`/`--input`) is under `target/profile/`.
  - It is a no-target query: `perf --version`, `perf list`, or `<tool> --help`.
- **Deny everything else** with reason `SPEC phase-2 §2.2: profile/disassemble/debug only cleanflate-only binaries (make profile)`.
- Segments whose command word is `make` or `cargo rustc` (e.g. `cargo rustc --release -p cleanflate -- --emit asm`, which SPEC §2.1 permits) are unaffected.
**Acceptance tests:**
- `tools/cleanroom/tests/test_guard.py::test_deny_profiling_oracle` denies each of:
  - `perf record -g target/release/oracle bench`
  - `cargo flamegraph -p oracle -- bench`
  - `objdump -d target/release/oracle`
  - `gdb target/release/deps/chunking-0123abcd`
  - `valgrind cargo test -p oracle`
  - `perf stat target/release/deps/conformance-89ab`
- `tools/cleanroom/tests/test_guard.py::test_allow_cleanflate_profiling` allows each of:
  - `perf record --call-graph dwarf -o target/profile/perf.data target/release/examples/profile_decode --dir testdata/perf-inputs/silesia/rfc1951-l6`
  - `perf report -i target/profile/perf.data --stdio`
  - `cargo flamegraph -p cleanflate --example profile_decode`
  - `make profile`
  - `perf --version`
- The seven new fixtures pass in `make cleanroom-selftest`.
**Out of scope:** Any other guard rule. The Makefile profiling targets (P0-10).
**Verification:** `make tools-check && make cleanroom-selftest`.
**Depends on:** none

### P0-03: design-check enforces the [perf] decision format
**Goal:** Make `make design-check` reject any `[perf]` DESIGN entry that lacks the clean-room statement or a measurement.
**Files touched:** `tools/ci/design-check.sh`, `tools/ci/tests/design_check_test.sh`, `docs/DESIGN.md` (the "How to read this document" section only).
**Design constraints:** SPEC §2.1 (each technique written up in the project's own words with "general technique; not derived from any specific implementation") and §9. CLAUDE.md PT7, C14.
- A heading matching `^### (D-\S+): \[perf\]` opens a perf block, which ends at the next `### D-` heading or EOF, like existing blocks.
- A perf block must contain:
  - a line starting `RFC:` (the existing rule);
  - a line starting `Measurement:`;
  - a line containing the exact phrase `general technique; not derived from any specific implementation`.
- Each failure prints the D-id and the missing element.
- Blocks without `[perf]` keep only the existing rule. The current D-01..D-25 must still pass.
- DESIGN.md "How to read this document": add one paragraph describing `[perf]` entries (the extra phrase and `Measurement:` line) and citing SPEC §2.1.
**Acceptance tests:** `tools/ci/tests/design_check_test.sh` cases:
- `case_perf_block_complete_passes`
- `case_perf_missing_phrase_fails`
- `case_perf_missing_measurement_fails`
- `case_non_perf_block_unaffected`
- all existing cases still pass.
**Out of scope:** Adding any `[perf]` entry.
**Verification:** `make tools-check && make design-check`.
**Depends on:** none

### P0-04: Decode-path switch and a uniform invalid-code rule
**Goal:** Carry `DecodePath { Fast, Reference }` through every decoder layer, expose it only under feature `test-hooks`, and route today's two fast techniques through it. Make table and bit-serial decoding agree on exactly when a code is invalid.
**Files touched:**
- `crates/cleanflate/src/`: `decode_path.rs` (new), `test_hooks.rs` (new), `lib.rs`, `inflate.rs`, `huffman.rs`, `window.rs`, `zlib.rs`, `gzip.rs`, `stream.rs`.
- `crates/cleanflate/src/tests/`: `mod.rs`, `decode_path.rs` (new), `huffman_table.rs`.
- `docs/DESIGN.md`.
**Design constraints:** SPEC §5 (reference path; a switch the default build cannot reach), §3 (public API additive only). RFC 1951 §3.2.2, §3.2.7. DESIGN D-11, D-16, D-17, D-19. Global invariants apply.
- **`decode_path.rs`:**
  - `pub enum DecodePath { Fast, Reference }`, deriving Debug, Clone, Copy, PartialEq, Eq, with `Default = Fast`.
  - Rustdoc: Fast is the production path. Reference is the simple path used as the differential oracle: exact byte-at-a-time fill, bit-serial Huffman decode, byte-by-byte match copy and no fast loop.
- **Plumbing:**
  - `Inflater` gains a `path` field and `pub(crate)` constructors taking a path, including the dictionary variant.
  - `ZlibDecoder` and `GzipDecoder` take a path and pass it to every `Inflater` they create. Gzip creates one per member.
  - `Decompressor::new(format, options)` delegates to a `pub(crate) new_with_path(.., DecodePath::Fast)`.
  - `decompress_with` delegates to a `pub(crate)` path-taking twin.
- **`test_hooks.rs`**, declared `#[cfg(feature = "test-hooks")] pub mod test_hooks;` in lib.rs:
  - `pub use crate::decode_path::DecodePath;`
  - `pub fn decompress_with_path(format: Format, input: &[u8], options: &Options, path: DecodePath) -> Result<Decoded, Error>`
  - `pub fn decompressor_with_path(format: Format, options: Options, path: DecodePath) -> Decompressor`
  - Module rustdoc says it is for tests and differential fuzzing only and is not part of the stable API.
- **Dead code in the default build:** nothing constructs `Reference` there. Items used only by it get `#[cfg_attr(not(any(test, feature = "test-hooks")), allow(dead_code))]` (allowed: C5 bans only the panic lints), so `cargo clippy -p cleanflate --lib -- -D warnings` stays clean.
- **Switched techniques:**
  - `Inflater::peek_symbol` uses `Huffman::decode_table` under Fast and `Huffman::decode` under Reference.
  - `Window::copy_match` uses the bulk non-overlap path under Fast and a byte-by-byte `copy_match_bytewise` under Reference.
- **Uniform invalid-code rule**, for both decoders, so they return the same answer for every buffer state regardless of table width:
  - A code is `Invalid` as soon as at least one buffered bit shows that it matches no code and no longer code can extend it. This only happens for the two incomplete distance tables RFC 1951 §3.2.7 allows: no codes, or one code of length 1. Both are decided after 1 bit.
  - Bit-serial: after failing to match at length `len`, return `Invalid` if no codes longer than `len` exist.
  - Table: an `Invalid` entry with `available() >= 1` is `Invalid`. It is no longer gated on `available() >= table_bits`.
  - Record a new DESIGN entry (next free D-NN; `RFC: RFC 1951 S3.2.2, S3.2.7`). It notes that the rule can only turn `Truncated` into `InvalidSymbol` for a stream that is both truncated and invalid; accept/reject is unchanged.
  - `testdata/malformed/EXPECTED.tsv` must not change.
**Acceptance tests:**
- `crates/cleanflate/src/tests/decode_path.rs`:
  - `default_path_is_fast`
  - `reference_path_same_output_fixed_dynamic_stored` (hand-built streams from the existing `bitwriter`/`dynbuilder` helpers)
  - `gzip_second_member_uses_same_path` (via a `pub(crate)` accessor)
  - `reference_byte_copy_matches_bulk_copy` (distances 1..=300 and lengths 3..=258, including ring wraparound)
- `crates/cleanflate/src/tests/huffman_table.rs`:
  - `table_decode_equals_serial_for_all_buffer_states`: `decode_table == decode` for every `available` 0..=15 and ≥ 64 bit patterns each. Tables covered: the fixed tables, both exception distance tables, and ≥ 50 random complete length sets from a fixed-seed generator written in the test.
  - `invalid_code_detected_after_one_bit` (both exception tables)
- Existing tests and `make test`, including the malformed corpus, pass unchanged.
**Out of scope:** New fast techniques. The oracle differential test (P0-05). Stats (P0-07).
**Verification:** `make lint && make test && make proptest-3k`.
**Depends on:** P0-01

### P0-05: Fast-vs-reference differential test
**Goal:** Add `crates/oracle/tests/fastslow.rs`. It decodes the conformance corpus, the malformed corpus and proptest streams on both paths and asserts identical output, errors, per-call consumed/produced/status and finish results.
**Files touched:** `crates/oracle/tests/common/mod.rs` (new), `crates/oracle/tests/fastslow.rs` (new), `crates/oracle/tests/chunking.rs` (use the shared strategies), `Makefile`.
**Design constraints:** SPEC §5 (differential in `make test`; identical output, errors, consumed and status), C13 (a missing manifest/corpus or an empty selection fails), phase-1 R1-05 (drivers must fail, never hang).
- **`tests/common/mod.rs`:**
  - `pub enum CallOutcome { Progress { consumed: usize, produced: usize, status: Status }, Err(String) }`, where the error string is `format!("{e:?}")`.
  - `pub struct Trace { calls: Vec<CallOutcome>, output: Vec<u8>, finish: Option<String>, total_in: u64, total_out: u64, trailing_start: Option<u64>, gzip_member_count: u64, stalled: bool }` (derive PartialEq, Debug).
  - `pub fn trace(format, options: &Options, path: DecodePath, input: &[u8], in_sched: &[usize], out_sched: &[usize]) -> Trace`:
    - Schedules (entries ≥ 1) are cycled.
    - Each call gets the next input chunk from the current offset (advanced by `consumed`) and a fresh zeroed output buffer of the next size.
    - The trace stops at `Done` or `Err`. After input is exhausted, it keeps calling with empty input while calls make progress.
    - It stops after 64 consecutive no-progress calls and then records `finish()`.
    - If the number of calls exceeds `16 * (input.len() + total_out + 64)`, it sets `stalled = true` and stops.
  - `pub fn oneshot(format, options, path, input) -> Result<(Vec<u8>, usize, String), String>`, returning data, consumed and the gzip headers' Debug string.
  - `pub fn dump_candidate(tag, format, input, in_sched, out_sched, note) -> PathBuf`: writes `target/regress-candidates/<tag>-<first 8 hex of sha256(input)>.bin` plus a `.txt` with the format, schedules and note.
  - Move the data/parameter proptest strategies out of `chunking.rs` into `common` and use them from both files. Keep `chunking.rs`'s behaviour and test names unchanged.
- **`fastslow.rs` tests:**
  - `fastslow_conformance_oneshot`:
    - Entries: every manifest entry for the tier (`oracle::conformance::selected(|_| true)`, honouring `CLEANFLATE_TIER`).
    - `Options.dictionary` comes from `dictionary_bytes` where the entry has one.
    - Assertion: Fast and Reference `oneshot` results are equal.
  - `fastslow_conformance_streaming`: the same entries with schedules in `[1021]` and out `[997]`. The Fast and Reference traces must be equal and not stalled.
  - `fastslow_malformed`:
    - Rows: every row of `testdata/malformed/EXPECTED.tsv`. A missing file or zero rows panics.
    - Each row is decoded one-shot and traced with schedules `[1]/[1]` and `[3,5,7]/[2,11]`. The two paths must agree.
  - `fastslow_proptest`:
    - Cases: `CLEANFLATE_FASTSLOW_CASES`, default 256.
    - Streams: the shared data/parameter strategies (all framings, levels 0–9, all strategies, windowBits 9–15, memLevel 1–9).
    - Schedules: 1..=64 per entry, with a 10% chance of all-1s.
    - Each case also runs a copy with one byte flipped at a random offset. Traces must be equal.
    - On mismatch, call `dump_candidate` first and then fail.
- Every mismatch message shows the first differing call index and both outcomes.
- **Makefile:**
  - `proptest-3k` and `proptest-10k` set `CLEANFLATE_FASTSLOW_CASES` to the same count as `CLEANFLATE_CHUNKING_CASES` and add `--test fastslow`.
  - New `fastslow-full: corpus-full` recipe: `CLEANFLATE_TIER=full cargo test --release -p oracle --test fastslow`.
  - `heavy` gains `fastslow-full`.
- If this test exposes an existing Fast/Reference difference, fix it (≤ 50 lines in cleanflate, following PT5 with a fixture once P0-06 exists, or a named test otherwise) and record it in `Defects-caught:`.
**Acceptance tests:**
- `crates/oracle/tests/fastslow.rs`:
  - `fastslow_conformance_oneshot`
  - `fastslow_conformance_streaming`
  - `fastslow_malformed`
  - `fastslow_proptest`
- `make proptest-3k` runs both proptests at 3000 cases.
- `make fastslow-full` passes; record its duration.
**Out of scope:** The regression fixture store (P0-06). New fast techniques.
**Verification:** `make test && make proptest-3k && make fastslow-full`.
**Depends on:** P0-04

### P0-06: Regression fixture store and repro tooling
**Goal:** Add committed repro fixtures under `testdata/regressions/` with a manifest, `oracle regress-add`/`regress-seed`, and a test that runs every fixture on both paths, so a failing case is never lost again.
**Files touched:**
- `crates/oracle/src/`: `regress.rs` (new), `lib.rs`, `main.rs`.
- `crates/oracle/tests/`: `regressions.rs` (new), `chunking.rs` (dump failing cases).
- `testdata/regressions/`: `README.md`, `MANIFEST.tsv`, `rfc1951/*.bin`, `rfc1950/*.bin`, `rfc1952/*.bin`.
- `Makefile`.
**Design constraints:** SPEC §5 ("never lose a repro"), C11 (paths use rfc1951/rfc1950/rfc1952), C13, C16 (expected output hashes come from `oracle::ffi`, never cleanflate).
- **`MANIFEST.tsv`:** `#` comment lines, then columns `path	format	expect	out_len	out_sha256	in_chunks	out_chunks	origin`.
  - `format` is `raw|zlib|gzip`.
  - `expect` is `ok` or `err:<ErrorKind::as_str()>`.
  - For `ok`, `out_len` and `out_sha256` come from the oracle's decode. For `err`, both are `-`.
  - Chunk lists are comma-separated positive integers (cycled), or `-`.
- **`oracle regress-add --input PATH --format raw|zlib|gzip --name NAME [--in-chunks LIST] [--out-chunks LIST] [--expect ok|err:<Kind>] --origin TEXT`:**
  - Copies the input to `testdata/regressions/<rfc195x>/<NAME>.bin`.
  - For `ok`, computes the expected output with `oracle::ffi::inflate_bytes` (raw/zlib) or `inflate_gzip_members` (gzip). It requires a clean, complete decode; otherwise it refuses and suggests `--expect err:<Kind>`.
  - Appends a row. Refuses a duplicate name.
- **`oracle regress-seed`:** deterministic (fixed seed via `oracle::prng`) and idempotent (skips existing names). It creates four seeds:
  - `rfc1951/fixed-heavy-match-11k`:
    - Data: 11,264 bytes of short repeated patterns with random literals between them.
    - Compression: Fixed strategy, level 6, raw.
    - Chunks: `1`/`1`.
    - Origin: "seed: shape of the lost phase-1 P4-08 repro (Fixed, ~11 KiB, match-heavy), not its bytes".
  - `rfc1950/fixed-heavy-match-11k`: the same data as zlib, chunks `7,1,13` / `1,258,3`.
  - `rfc1952/two-members`: two members with different data, concatenated, chunks `1`/`1`.
  - `rfc1951/dynamic-l9-64k`: 64 KiB of mixed text-like data at level 9, chunks `8,9,15,16,17` / `257,258,259`.
- **`crates/oracle/tests/regressions.rs`:**
  - `regressions_all_paths`:
    - For each row and for both Fast and Reference: decode one-shot, trace `[1]/[1]`, and trace the row's schedules when they are not `-`.
    - For `ok`: length and SHA-256 match and `finish` is Ok.
    - For `err`: the `ErrorKind` string matches.
    - Fast and Reference traces are equal.
    - The test fails if the manifest is missing, has zero rows, or names a missing file.
  - `regressions_manifest_matches_files`: every `.bin` under `testdata/regressions/` has exactly one row, and every row has its file.
- **`chunking.rs`:** on a failing case, call `common::dump_candidate("chunking", ...)` before failing. Proptest's shrinking calls the test last with the minimal case, so the final dump is the minimal one.
- **`testdata/regressions/README.md`:** how to add a fixture (PT5), what each column means, and the C16 note.
- **`Makefile`:** add `regress-seed: cargo run --release -q -p oracle -- regress-seed`.
**Acceptance tests:**
- `crates/oracle/tests/regressions.rs`:
  - `regressions_all_paths`
  - `regressions_manifest_matches_files`
- Unit tests in `crates/oracle/src/regress.rs`:
  - `manifest_roundtrip`
  - `chunk_list_parse_rejects_zero`
  - `seed_is_deterministic`
- Running `make regress-seed` a second time leaves `git status` clean.
**Out of scope:** Fixtures that need a preset dictionary.
**Verification:** `make regress-seed && make test && make crate-checks`.
**Depends on:** P0-05

### P0-07: `stats` feature counters
**Goal:** Add the opt-in `stats` feature, with per-decoder counters for the quantities SPEC §4.3 lists, exposed through `Decompressor::stats()` and `decompress_with_stats`, and compiled out entirely by default.
**Files touched:**
- `crates/cleanflate/src/`: `stats.rs` (new), `lib.rs`, `inflate.rs`, `huffman.rs` (only to report which method resolved a symbol), `window.rs`, `stream.rs`, `zlib.rs`, `gzip.rs`.
- `crates/cleanflate/src/tests/`: `mod.rs`, `stats.rs` (new, the whole module `#[cfg(feature = "stats")]`).
**Design constraints:** SPEC §4.3. Global invariants apply.
- **C7:** no clocks, so table-building cost is counted as calls and entries, never time. No global state: counters are fields of the decoder.
- **C15:** increments use `saturating_add`.
- **C18:** with the feature off, no counter fields or increment code exist. Use `#[cfg]` fields and a crate-internal macro defined twice (on/off), so default behaviour and layout are untouched.
- **`stats.rs`:** `#[cfg(feature = "stats")] pub struct Stats` (Debug, Clone, Default, PartialEq) with documented `pub u64` fields:
  - `step_calls`, `state_dispatches` (iterations of `Inflater::step`'s state loop), `output_bytes`.
  - `blocks_stored`, `blocks_fixed`, `blocks_dynamic`, `stored_bytes`.
  - `literals`, `matches`, `match_bytes`, `overlap_matches` (distance < length).
  - `match_len_hist: [u64; 7]`, bucketing lengths 3-4, 5-8, 9-16, 17-32, 33-64, 65-128 and 129-258.
  - `match_dist_hist: [u64; 9]`, bucketing distances 1, 2-4, 5-16, 17-64, 65-256, 257-1024, 1025-4096, 4097-16384 and 16385-32768.
  - `litlen_table_decodes`, `litlen_serial_decodes`, `dist_table_decodes`, `dist_serial_decodes`, `codelen_table_decodes`, `codelen_serial_decodes`. Each symbol is counted once, by the method that resolved it.
  - `refills` (fills that loaded ≥ 1 byte), `refill_bytes`, `given_back_bytes`.
  - `table_builds` (`Huffman::from_lengths` calls), `table_entries_built`, `fixed_table_clones`.
  - `window_bytes_written` (bytes written into the history window).
  - `fast_loop_entries`, `fast_loop_symbols`.
  - `oneshot_resizes`, `oneshot_bytes_allocated`, set only by `decompress_with_stats`.
  - `pub fn merge(&mut self, other: &Stats)`.
  - Fields for mechanisms that do not exist yet stay 0 until their task.
- **Gzip:** accumulates across members by merging each finished member's inflater stats.
- **Public API:**
  - `#[cfg(feature = "stats")] pub fn Decompressor::stats(&self) -> Stats`.
  - `#[cfg(feature = "stats")] pub fn decompress_with_stats(format, input, options) -> Result<(Decoded, Stats), Error>`. It shares the implementation of `decompress_with` (no copy) and also counts resizes and bytes allocated.
  - `lib.rs` re-exports both under the feature.
- **Tests:** gzip fixtures reuse the existing `tests/gzip.rs` helpers with literal CRCs (C16).
**Acceptance tests:** `crates/cleanflate/src/tests/stats.rs`:
- `stats_stored_block_counts`: an N-byte stored block gives `stored_bytes == N`, `blocks_stored == 1` and `output_bytes == N`.
- `stats_fixed_literals_and_matches`: a hand-built fixed block with known literals, lengths and distances gives exact counters, histogram buckets and overlap count.
- `stats_dynamic_table_builds`: two dynamic blocks give `blocks_dynamic == 2` and 3 table builds per block.
- `stats_symbol_decode_methods_sum`: litlen table + serial decodes == literals + matches + end-of-block symbols.
- `stats_accumulate_across_gzip_members`
- `stats_decompress_with_counts_resizes`
- `make lint` passes, including the default-feature clippy line.
**Out of scope:** Printing stats (P0-09). Any optimisation.
**Verification:** `make lint && make test && make proptest-3k`.
**Depends on:** P0-01

### P0-08: Bench harness: medians, framings, levels, modes, JSON
**Goal:** Harden `oracle bench` per SPEC §4.2. It takes a warm-up plus N timed samples and reports median and min, per file and aggregate. It covers raw/zlib/gzip, levels 1/6/9, one-shot and 64 KiB streaming, writes machine-readable JSON, and adds `make bench`/`make bench-quick`.
**Files touched:** `crates/oracle/src/bench.rs` (rewrite), `crates/oracle/src/bench_json.rs` (new), `crates/oracle/src/main.rs`, `crates/oracle/src/lib.rs`, `Makefile`, `docs/perf/README.md`.
**Design constraints:**
- SPEC §4.1 and §4.2.
- SPEC §2.1: C zlib is a black-box stopwatch. Time `oracle::ffi::inflate_bytes` and nothing else.
- SPEC §10: no change to how the oracle uses C zlib. Use the existing `deflate_bytes`/`inflate_bytes`.
- C9, C11 (JSON paths).
- C13: a missing corpus is an error that names `testdata/corpora/README.md`. Never fall back silently.
- **CLI:** `oracle bench [--kind full|quick|custom] [--corpus silesia,canterbury] [--subset quick] [--framings raw,zlib,gzip] [--levels 1,6,9] [--modes oneshot,streaming] [--iters N] [--task ID] [--json PATH]`.
  - Framing names also accept `rfc1951|rfc1950|rfc1952`.
  - `--kind full` requires `--iters >= 7`.
  - The bench refuses to run in a debug build.
- **Quick subset:** all Canterbury files plus Silesia `dickens`, `mr`, `ooffice`, `sao` and `xml`, as a constant list in bench.rs.
- **Per (corpus, file, level, framing):**
  - Compress once with the oracle (Default strategy, windowBits 15, memLevel 8).
  - Warm-up: decode once per timed implementation and mode. Verify the output is byte-identical to the original, and fail loudly otherwise.
  - Choose `reps`, the smallest power of two that makes one sample last ≥ 20 ms. `MIN_SAMPLE_MS` is a bench-local const, not a cleanflate tunable.
  - Take N samples. Within each sample index, run cleanflate one-shot, cleanflate streaming and C zlib one-shot back to back (interleaved).
  - Record per-decode ns as sample time / reps.
- **Modes:**
  - `oneshot`: `cleanflate::decompress_with(format, &compressed, &Options::default())`.
  - `streaming`: `Decompressor::new(format, Options::default())` fed 64 KiB input chunks. Output goes into a pre-sized buffer (`original.len() + 65536`) through successive windows of at most 64 KiB, then `finish()`.
  - C zlib is timed one-shot only. Its median is the denominator for both cleanflate modes (see Decisions).
- **Results:**
  - Median and min per (file, framing, level, mode, impl).
  - Aggregates per (corpus, framing, level, mode) with key `<corpus>/<framing>/l<level>/<mode>`: total bytes / sum of per-file medians, and also / sum of mins.
  - Ratio = cleanflate median MiB/s / C zlib median MiB/s.
- **JSON** (`"schema": 1`), documented field by field in `docs/perf/README.md`:
  - `kind`, `task`, `git_head`, `git_dirty`.
  - `timestamp`: UTC, formatted by hand from `SystemTime`.
  - `machine`: `uname`, `cpu_model`, `governor` (from `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor`, or `unknown`), `loadavg`, `rustc` (from `rustc --version`).
  - `c_zlib_version`, `iters`, `min_sample_ms`.
  - `stats_build`: `cfg!(feature = "stats")` of the cleanflate dependency.
  - `results[]` and `aggregates[]`.
- **Stdout:** a markdown table of aggregates: key, cleanflate MiB/s (median), cleanflate MiB/s (min-based), C zlib MiB/s, ratio.
- **Makefile:**
  - Variables: `BENCH_ID ?= adhoc`, `BENCH_OUT ?= target/bench/$(BENCH_ID).json`.
  - `bench`: `--kind full`, corpora silesia,canterbury, framings raw,zlib,gzip, levels 1,6,9, both modes, iters 7.
  - `bench-quick`: `--kind quick --subset quick`, level 6, all framings, both modes, iters 5.
  - Both create the output's parent directory.
**Acceptance tests:**
- Unit tests in `crates/oracle/src/bench.rs` and `bench_json.rs` (`#[cfg(test)]` is allowed in the oracle):
  - `median_and_min_odd_even`
  - `aggregate_is_total_bytes_over_total_median_time`
  - `reps_reach_min_sample`
  - `json_roundtrip`
  - `quick_subset_names`
  - `full_kind_requires_seven_iters`
  - existing `throughput_math`, kept
- `make bench-quick BENCH_ID=P0-08` completes and writes parseable JSON under `target/bench/`. Do not commit it.
**Out of scope:** Compare/noise/markdown/stats (P0-09). Running the full bench (P0-11).
**Verification:** `make lint && make test && make bench-quick BENCH_ID=P0-08`.
**Depends on:** none

### P0-09: bench-compare, bench-noise, bench-md and `bench --stats`
**Goal:** Add the comparison, noise, markdown and statistics tools the rest of the plan measures with.
**Files touched:** `crates/oracle/src/bench_compare.rs` (new), `crates/oracle/src/bench.rs`, `crates/oracle/src/main.rs`, `crates/oracle/src/lib.rs`, `Makefile`, `docs/perf/README.md`.
**Design constraints:** SPEC §4.1 ("no per-file regression ... by more than the measured noise"), §4.2 ("twice the measured noise", `make bench-compare A=... B=...`), §4.3 (`oracle bench --stats`). Decisions entries on noise and verdicts.
- **`oracle bench-noise A B --out PATH`:**
  - For every aggregate key and every per-file key `<corpus>/<file>/<framing>/l<level>/<mode>` present in both files, using cleanflate median throughput only: noise % = |a − b| / a × 100.
  - Writes `{schema: 1, kind, sources: [A, B], aggregate: {key: pct}, aggregate_max_pct, per_file: {key: pct}}`.
- **`oracle bench-compare A B [--noise PATH] [--fail-on-regression]`:**
  - Default noise file: `docs/perf/noise.json` when both inputs are kind `full`, and `docs/perf/noise-quick.json` when both are `quick`. If the file is missing, print `noise unknown` and use verdict `UNKNOWN`.
  - Per aggregate key, print A and B MiB/s, delta %, 2×noise % (the key's own noise, else `aggregate_max_pct`) and a verdict: `WIN` (delta > +2×noise), `LOSS` (delta < −2×noise) or `NEUTRAL`.
  - Per file: `REGRESSION` when delta < −max(file noise, aggregate noise of its key). REGRESSION lines are listed at the end.
  - Keys found in only one file are listed.
  - Error out if either JSON has `stats_build: true`.
  - Exit 0 unless `--fail-on-regression` is given and there is at least one REGRESSION.
- **`oracle bench-md JSON [--files] [--filter PREFIX]`:** markdown tables for PERF.md. Aggregates by default; `--files` adds per-file rows.
- **`oracle bench --stats`:**
  - Available only when built with oracle feature `stats`; otherwise exit with "rebuild with --features stats".
  - `--iters 0` is allowed only together with `--stats` (no timing).
  - Per (file, framing, level), one untimed `decompress_with_stats`, plus `Decompressor::stats()` after a 64 KiB-chunk decode when mode streaming is selected.
  - Prints per framing:
    - totals;
    - litlen and dist table-decode %;
    - dispatches per output byte;
    - bytes per refill;
    - overlap-match %;
    - match length and distance histograms as % of matches;
    - stored %;
    - table builds per block;
    - one-shot resizes.
  - The JSON gets a `stats` array: raw counters per file plus totals.
- **Makefile:**
  - `bench-compare`: fails with a usage message if `A` or `B` is unset; otherwise runs `oracle bench-compare $(A) $(B)`, plus `--noise $(NOISE)` if set.
  - `bench-noise A= B= OUT=`
  - `bench-md J=`
  - `bench-stats`: `cargo run --release -q -p oracle --features stats -- bench --stats --iters 0 --kind custom --corpus silesia --framings raw,zlib,gzip --levels 6 --modes oneshot,streaming --task $(BENCH_ID) --json $(BENCH_OUT)`.
  - `lint` gains `cargo clippy -p oracle --all-targets --features stats -- -D warnings`.
**Acceptance tests:**
- Unit tests in `crates/oracle/src/bench_compare.rs`:
  - `noise_from_two_runs`
  - `verdicts_win_loss_neutral`
  - `regression_threshold_is_max_of_file_and_aggregate_noise`
  - `fail_on_regression_flag`
  - `refuses_stats_build_json`
  - `markdown_render_has_all_keys`
- Unit test in `crates/oracle/src/bench.rs`: `stats_flag_requires_feature`.
- `make bench-stats BENCH_ID=P0-09` prints the table. Do not commit the output.
**Out of scope:** Baseline measurement (P0-11).
**Verification:** `make lint && make test`, then two `make bench-quick` runs into `target/bench/a.json` and `target/bench/b.json`, then `make bench-compare A=target/bench/a.json B=target/bench/b.json` and `make bench-stats BENCH_ID=P0-09`.
**Depends on:** P0-07, P0-08

### P0-10: Cleanflate-only profiling entry point and `make profile`
**Goal:** Profile cleanflate alone. Add an unprofiled input-preparation step, a cleanflate-only example binary, and `make profile`/`make flamegraph` wrapped around that binary.
**Files touched:** `crates/oracle/src/perf_prep.rs` (new), `crates/oracle/src/main.rs`, `crates/oracle/src/lib.rs`, `crates/cleanflate/examples/profile_decode.rs` (new), `Makefile`, `docs/perf/README.md` (profiling section).
**Design constraints:** SPEC §4.3 (all bullets), §2.2, CR7, C19 (rule `no-profiling-of-oracle`). C1 applies to the example (no `unsafe` token anywhere under `crates/cleanflate/`). C7 covers only `src/`, so the example may use `std::fs` and `Instant`.
- **`oracle perf-prep --corpus silesia|canterbury --framing rfc1951|rfc1950|rfc1952 --level L --out testdata/perf-inputs`:**
  - Also accepts raw/zlib/gzip as aliases.
  - Writes `<out>/<corpus>/<rfc195x>-l<L>/<file>.bin` and `index.tsv` with columns `file	compressed_len	original_len	original_crc32`. The CRC comes from `oracle::ffi::crc32`.
  - Does nothing if `index.tsv` exists and every listed file has the listed length.
  - This step is never profiled.
- **`crates/cleanflate/examples/profile_decode.rs`:**
  - Uses cleanflate and std only.
  - Arguments: `--dir DIR --format rfc1951|rfc1950|rfc1952 --iters N [--mode oneshot|streaming]`.
  - Loads every file in `index.tsv` into memory first, then decodes each one N times. Streaming uses 64 KiB chunks.
  - On the first iteration, verifies the length and the CRC-32 (`cleanflate::checksum::crc32` against the oracle value in the index). It exits non-zero on any mismatch.
  - Prints total MiB/s. Built with `--features stats`, it also prints the merged `Stats`.
- **Makefile:**
  - Variables: `PROFILE_CORPUS ?= silesia`, `PROFILE_FRAMING ?= rfc1951`, `PROFILE_LEVEL ?= 6`, `PROFILE_ITERS ?= 3`, `PROFILE_MODE ?= oneshot`, `PROFILE_PERF_FLAGS ?= --call-graph dwarf -F 499`.
  - `PROFILE_DIR := testdata/perf-inputs/$(PROFILE_CORPUS)/$(PROFILE_FRAMING)-l$(PROFILE_LEVEL)`.
  - `profile-prep` runs perf-prep.
  - `profile: profile-prep`:
    - `CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p cleanflate --example profile_decode`
    - `mkdir -p target/profile`
    - `perf record $(PROFILE_PERF_FLAGS) -o target/profile/perf.data target/release/examples/profile_decode --dir $(PROFILE_DIR) --format $(PROFILE_FRAMING) --iters $(PROFILE_ITERS) --mode $(PROFILE_MODE)`
    - `perf report -i target/profile/perf.data --stdio --no-children --sort symbol --percent-limit 0.5 > target/profile/top.txt`
    - Print the first 60 lines of `top.txt`.
  - `flamegraph: profile-prep`: `CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -p cleanflate --example profile_decode -o target/profile/flamegraph.svg -- <same args>`.
  - No recipe runs a profiler on anything but `profile_decode`.
**Acceptance tests:**
- Unit tests in `crates/oracle/src/perf_prep.rs`: `index_tsv_roundtrip`, `framing_aliases`.
- `make profile PROFILE_ITERS=1` writes `target/profile/top.txt` whose leading entries are cleanflate functions. Paste its top 10 lines into the progress log.
- `make flamegraph PROFILE_ITERS=1` writes the SVG.
- If `perf` fails for an environmental reason, record `profile NOT RUN: <reason>` and finish. Never install or reconfigure tools (CR3).
**Out of scope:** Recording findings (P0-12).
**Verification:** `make lint && make test && make profile PROFILE_ITERS=1 && make flamegraph PROFILE_ITERS=1`.
**Depends on:** P0-07

### P0-11: Baseline and noise measurement
**Goal:** Measure the phase-2 baseline twice with the full bench and twice with the quick bench, commit the JSONs and the noise estimates, and write the `## Phase 2` baseline section of PERF.md.
**Files touched:**
- `docs/perf/`: `baseline.json`, `baseline-rerun.json`, `noise.json`, `baseline-quick.json`, `baseline-quick-rerun.json`, `noise-quick.json`.
- `docs/PERF.md`.
**Design constraints:** SPEC §4.1 (baseline and targets), §4.2 (re-measure on this machine before any optimisation; noise from two runs), §8 (record conditions), C10 (no reference-implementation names in PERF.md or docs/perf). Do not touch code. Keep the phase-1 content of PERF.md; add a new top-level `## Phase 2` section after it.
- **Steps:**
  1. Record conditions: `cat /proc/loadavg`, the governor and `nproc`. Run nothing else heavy during the benches (PT8: background, wait).
  2. `make bench BENCH_ID=baseline BENCH_OUT=docs/perf/baseline.json`, then the same with `BENCH_OUT=docs/perf/baseline-rerun.json`.
  3. `make bench-noise A=docs/perf/baseline.json B=docs/perf/baseline-rerun.json OUT=docs/perf/noise.json`.
  4. Same with `make bench-quick` into `baseline-quick.json` and `baseline-quick-rerun.json`, then `noise-quick.json`.
- **PERF.md `## Phase 2`:**
  - `### Method`: the §4.2 method as implemented.
    - Warm-up plus N=7 samples, median and min, interleaving, 20 ms samples.
    - Modes, and C zlib one-shot as the denominator.
    - JSON location.
    - Noise, WIN and REGRESSION rules.
    - Cleanflate-only profiling.
  - `### Machine and conditions`.
  - `### Baseline (P0-11)`:
    - `make bench-md` tables for Silesia level 6, per framing and mode.
    - The full aggregate table.
    - A per-file table for Silesia raw level 6 one-shot.
  - `### Noise`: `aggregate_max_pct`, a per-key table, and quick noise.
  - `### Targets`: SPEC §4.1 targets against the baseline ratios.
- If aggregate noise exceeds 5%, say so prominently and add a manual check. Continue anyway.
**Acceptance tests:**
- The six JSON files exist, render with `make bench-md`, have kind `full`/`quick` and `stats_build: false`.
- `make bench-compare A=docs/perf/baseline.json B=docs/perf/baseline-rerun.json` shows no WIN, LOSS or REGRESSION (a sanity check of the noise definition).
**Out of scope:** Code changes. Hypotheses (P0-12).
**Verification:** `make bench-md J=docs/perf/baseline.json && make bench-compare A=docs/perf/baseline.json B=docs/perf/baseline-rerun.json`.
**Depends on:** P0-09

### P0-12: Hypotheses H1–H6 and baseline profile
**Goal:** Confirm or discard each SPEC §4.4 hypothesis with a number, record the baseline profile's top functions, and rank the costs.
**Files touched:** `docs/PERF.md`, `docs/perf/P0-12-stats.json`.
**Design constraints:** SPEC §4.4, §4.3 (profile cleanflate alone: only `make profile`/`make flamegraph`; CR7), §6 M0, C10. No code changes.
- **Collect:**
  - `make bench-stats BENCH_ID=P0-12 BENCH_OUT=docs/perf/P0-12-stats.json`.
  - `make profile` for `PROFILE_FRAMING=rfc1951`, `rfc1950` and `rfc1952` at level 6, plus one run with `PROFILE_MODE=streaming` for rfc1951.
- **PERF.md `### Baseline profile (P0-12)`:** the top 15 self-time symbols with percentages for each profile.
- **PERF.md `### Hypotheses (P0-12)`:** a table with the metric, the measured value, a verdict (CONFIRMED, DISCARDED or PARTIAL), the estimated share of decode time, and the tasks each hypothesis drives.
  - H1: serial / (table + serial) decodes for litlen and dist; the profile share of `Huffman::decode`.
  - H2: `state_dispatches / output_bytes`; the self share of `Inflater::step` and of State clone/drop frames.
  - H3: `window_bytes_written / output_bytes`; the share of window push/copy functions.
  - H4: overlap-match % and overlap bytes; the share of the byte-wise copy loop.
  - H5: `table_builds` and `fixed_table_clones` per block; the share of `from_lengths`, `build_table` and table clones.
  - H6: gzip vs raw and zlib vs raw aggregate delta (`baseline.json`, Silesia level 6 one-shot); the CRC/Adler share in the rfc1952/rfc1950 profiles; `oneshot_resizes` per file.
- **PERF.md `### Cost ranking (P0-12)`:** hypotheses ordered by measured share, naming which gated tasks (P1-03, P1-04, P2-03, P4-02) are likely to end as negative results. Do not edit PLAN.md.
**Acceptance tests:** PERF.md has a number and a verdict for each of H1..H6, and `docs/perf/P0-12-stats.json` is committed.
**Out of scope:** Code changes.
**Verification:** `make design-check` (DESIGN.md untouched; sanity check only).
**Depends on:** P0-09, P0-10, P0-11

### P0-13: M0 acceptance and push
**Goal:** Run the M0 acceptance commands, commit the M0 bench, snapshot provenance, push, and list the manual checks.
**Files touched:** `docs/perf/P0-13.json`, `docs/PERF.md` (`### M0 acceptance (P0-13)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 0 files.
**Design constraints:** SPEC §6 (each milestone ends with `make ci`, full `make bench` and the 10k chunking proptest, committed and pushed) and M0 acceptance. PT5 for any correctness failure: block with a fixture. PT8.
- `make ci`.
- `make heavy`: proptest-10k (chunking and fastslow), bomb, conformance-full, fastslow-full.
- `make bench BENCH_ID=P0-13 BENCH_OUT=docs/perf/P0-13.json`.
- `make bench-compare A=docs/perf/baseline.json B=docs/perf/P0-13.json`. Record any REGRESSION; expect none, since the only decoder change is P0-04's invalid-code rule.
- PERF.md: an M0 checklist with evidence:
  - baseline JSON and noise committed (paths);
  - the differential runs in `make test` (test names);
  - H1–H6 each have a number (PERF.md anchors).
- `make provenance-snapshot`, commit, `git push -u origin HEAD`.
**Acceptance tests:** `make ci` and `make heavy` pass. `docs/perf/P0-13.json` is committed.
**Out of scope:** Optimisation.
**Verification:** `make ci && make heavy && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm the GitHub Actions `ci` run is green on the pushed branch. It now also lints and tests the features.
2. Look at `docs/perf/noise.json`. If aggregate noise exceeds 3%, re-run the baseline on a quiet machine with a fixed governor (SPEC §8).
3. Run `make flamegraph` and sanity-check the P0-12 profile tables against the SVG.
4. Check that the provenance log has no unexplained denied profiling commands.
**Depends on:** P0-05, P0-06, P0-11, P0-12

## Phase 1 — M1 Bit reader and symbol decoding

### P1-01: Bulk bit refill with an exact give-back invariant
**Goal:** On the Fast path, keep the bit buffer topped up a word at a time, and redefine the reader invariant so that per-call `consumed`, trailers and resumability are exactly what the Reference path produces.
**Files touched:**
- `crates/cleanflate/src/`: `bitreader.rs`, `inflate.rs`, `stats.rs`.
- `crates/cleanflate/src/tests/`: `mod.rs`, `bitreader_bulk.rs` (new).
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P1-01-before.json`, `docs/perf/P1-01.json`.
**Design constraints:** SPEC §6 M1 bullet 1, §5 (reference path; exits restore exact state per D-07/D-22), §3 (streaming contract D-11/D-13/D-20/D-25: exact `consumed`, `trailing_start`). RFC 1951 §3.1.1, §3.2.4. Global invariants apply. PT1, PT3–PT7.
- **`BitReader::refill_bulk(&mut self, input: &[u8], pos: &mut usize)`:**
  - If at least 8 bytes remain at `*pos`, read them as one little-endian `u64` (`input.get(*pos..*pos + 8)`, `try_into`, `u64::from_le_bytes`). OR in the low k = (63 − count) / 8 whole bytes above the current count, and advance pos by k.
  - Otherwise load single bytes while count ≤ 55 and input remains.
  - count never exceeds 63, so there is never a shift by ≥ 64.
  - Update `refills` and `refill_bytes` (stats).
- **`BitReader::give_back(&mut self, keep_bits: u32) -> usize`:** k = the number of whole bytes above `keep_bits` (0 if count < keep_bits + 8). Mask off the top 8k bits, set count −= 8k, and return k.
- **Fast `peek_symbol`:** refills in bulk whenever fewer than 15 bits (the longest RFC 1951 code) are buffered, then decodes. If it still needs bits and input is exhausted, it returns NeedsInput exactly as today. Reference keeps the exact fill.
- **New invariant**, recorded as a `[perf]` DESIGN entry superseding D-07's "at most 7 bits between steps":
  - (a) NeedsInput exit: `pos == input.len()`, and the buffer holds only bits of the pending sub-step, fewer than it requires.
  - (b) OutputFull or Done exit, through one shared exit helper used by every such `return`: give back whole bytes so the buffer keeps exactly the bits of a decoded-but-unconsumed symbol. That symbol is the pending literal at OutputFull in LitLen, and `keep_bits` is its code length. Otherwise the buffer keeps ≤ 7 bits. Then `pos −=` the returned count.
  - (c) After LEN/NLEN in StoredLen, give back with `keep_bits = 0`; the count is a multiple of 8 after `align_to_byte`. StoredCopy drains any whole bytes still buffered before it reads input. This is defensive: add a `debug_assert!` that none remain.
  - Consequence, to state in DESIGN: after every `step` return, the Fast reader holds exactly the bits the Reference reader holds, so per-call `consumed` is identical (P0-05 traces check this). zlib/gzip trailers after `Done` start at the right byte.
  - Bytes given back were always loaded in the same call, because bits carried in from an earlier call are consumed by the first sub-step. Use `checked_sub` on pos with a `debug_assert!`, never a panic.
- **Error exits:** unchanged (sticky; `consumed` is not reported).
- The DESIGN entry cites the phase-1 P4-08 failure (gzip trailer corrupted by over-reading; a stream one byte short and stuck at NeedsInput) and explains why (a)–(c) make it impossible.
**Acceptance tests:**
- `crates/cleanflate/src/tests/bitreader_bulk.rs`:
  - `refill_bulk_word_and_bytewise`: the word path with ≥ 8 bytes, the byte path with < 8, and count ≤ 63 always.
  - `give_back_keeps_pending_bits`: keep 0, 9 and 15 at various counts.
  - `bulk_then_giveback_bit_sequence_equals_exact_fill`: over random streams, reading n-bit fields via bulk refill plus give-back yields the same values and positions as exact `take`.
  - `raw_done_consumed_exact_with_trailing_bytes`: a hand-built raw stream plus 0..=16 garbage bytes, at every 2-way split of the input. `consumed` at Done equals the stream length on both paths.
  - `per_call_consumed_matches_reference`: fixed and dynamic streams with output chunks of 1 and 3 bytes. Per-call (consumed, produced, status) is identical on Fast and Reference.
  - `stored_block_after_bulk_refill`: a compressed block followed by a stored block, at all 2-way input splits.
- `make test` (fastslow and regressions), `make proptest-3k` and `make fastslow-full` pass.
**Out of scope:** Table changes (P1-02). The fast loop (P2-01).
**Verification:** PT1 (before and after), `make lint && make test && make proptest-3k && make fastslow-full && make bomb`.
**Depends on:** P0-05, P0-06

### P1-02: Two-level lookup tables for every symbol
**Goal:** On the Fast path, decode every literal/length, distance and code-length symbol through lookup tables: a first-level table, plus second-level tables for longer codes. The bit-serial walk is kept only for the Reference path.
**Files touched:**
- `crates/cleanflate/src/`: `huffman.rs`, `inflate.rs`, `stats.rs`, `config.rs` (doc comments only).
- `crates/cleanflate/src/tests/`: `huffman_table.rs`, `huffman_twolevel.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P1-02-before.json`, `docs/perf/P1-02.json`.
**Design constraints:** SPEC §6 M1 bullet 2, §4.4 H1, §5. RFC 1951 §3.2.2 (canonical codes), §3.1.1 (bit order), §3.2.7 (incomplete distance tables; the code-length alphabet's lengths are 3-bit fields, so codes are at most 7 bits). Global invariants apply. PT1, PT3–PT7.
- `LITLEN_TABLE_BITS`/`DIST_TABLE_BITS` stay in config.rs (C8). The code-length table width is the RFC-fixed maximum of 7, a named const in huffman.rs citing the RFC; it is not a tunable.
- **Entry:** a compact `Copy` value (for example a `u32` or a small struct) distinguishing `Symbol{symbol, len}`, `Sub{offset, bits}` (first level only) and `Invalid`.
- **Build:**
  - The first level has `1 << table_bits` entries, built for codes ≤ table_bits as today.
  - For each `table_bits` prefix used by longer codes, append a subtable of width `max_len_under_prefix − table_bits` after the first level in the same Vec. Index it by the following bits (LSB-first, reversed codes) and fill it redundantly across don't-care bits.
  - Count entries written in stats.
- **Fast lookup,** with `a = available()` and unavailable bits reading as 0:
  - Index the first level by `peek(table_bits)`.
  - A `Symbol` with `len ≤ a` is the answer.
  - A `Sub` indexes its subtable by the next `bits` bits; a `Symbol` there with `len ≤ a` is the answer.
  - `Invalid` with `a ≥ 1` is Invalid (P0-04 rule).
  - Anything else is NeedMoreBits.
  - This equals `decode` for every buffer state. By prefix-freeness, a `Symbol` of length ≤ a reached through zero padding is the true code. State this argument in DESIGN.
- **Code-length alphabet:** a single-level 7-bit table.
- **Reference path:** keeps `decode`. The D-16 gate "only when `available() >= table_bits`" is removed from the Fast path by a `[perf]` DESIGN entry superseding D-16.
**Acceptance tests:**
- `crates/cleanflate/src/tests/huffman_twolevel.rs`:
  - `lookup_equals_serial_for_all_buffer_states`: fixed tables, both exception distance tables, and ≥ 200 fixed-seed random complete length sets including 15-bit codes; every a in 0..=15 with ≥ 64 bit patterns each.
  - `subtables_built_for_long_codes`
  - `codelen_table_decodes_all_symbols`
  - `table_widths_sweepable`: the build and lookup are correct for widths 7..=12, via an internal constructor that takes the width, so P4-03 can sweep safely.
- Existing `huffman_table.rs` tests are updated, not deleted.
- `make test`, `make proptest-3k` and `make fastslow-full` pass.
**Out of scope:** Base/extra payloads in entries (P1-04). Per-block reuse (P1-03).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full`.
**Depends on:** P0-05, P0-06

### P1-03: Per-block table reuse (gated)
**Goal:** Gated per PT2. Remove per-block table costs: build the fixed tables once and reference them instead of cloning per fixed block, and rebuild dynamic tables in place, reusing their allocations.
**Files touched:**
- `crates/cleanflate/src/`: `inflate.rs`, `huffman.rs`, `stats.rs`.
- `crates/cleanflate/src/tests/`: `inflate_block_tables.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P1-03-before.json`, `docs/perf/P1-03.json`.
**Design constraints:** SPEC §4.4 H5, §6 M1 bullet 4. RFC 1951 §3.2.6, §3.2.7. Global invariants apply. PT1, PT2, PT4–PT7.
- **Gate:** run `make bench-stats` and `make profile PROFILE_FRAMING=rfc1951`. If `from_lengths`, `build_table`, table clones and the allocation frames under them together take less than max(2%, 2x aggregate noise) of samples, record the negative result and stop.
- **Implementation:**
  - `Inflater` selects its tables through an `ActiveTables { Fixed, Dynamic }` selector (or references) instead of cloning.
  - `Huffman::rebuild(&mut self, lengths, kind)` reuses Vec capacity.
  - Completing `CodeLens` slices `dyn_lengths` instead of `to_vec`.
- This changes storage, not decoding, so both paths share it. DESIGN says so; conformance and fastslow cover it.
**Acceptance tests:**
- `crates/cleanflate/src/tests/inflate_block_tables.rs`:
  - `fixed_blocks_do_not_clone_tables`: under `#[cfg(feature = "stats")]`, `fixed_table_clones == 0` after 3 fixed blocks.
  - `dynamic_rebuild_matches_fresh_build`: after rebuilding from a different length set, lookup and serial decode equal a fresh `from_lengths`.
  - `many_dynamic_blocks_same_output`: 100 small dynamic blocks decode to the same output on both paths.
- If the gate stops the task: no tests; the PERF.md entry with the numbers is the deliverable.
**Out of scope:** Table format changes.
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k`.
**Depends on:** P0-05, P0-07, P0-10

### P1-04: Length/distance payload in table entries (gated)
**Goal:** Gated per PT2. If the profile shows length/distance base and extra-bit handling is significant, carry each length/distance symbol's base value and extra-bit count in its Fast-path table entry.
**Files touched:**
- `crates/cleanflate/src/`: `huffman.rs`, `inflate.rs`.
- `crates/cleanflate/src/tests/`: `huffman_entry_payload.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P1-04-before.json`, `docs/perf/P1-04.json`.
**Design constraints:** SPEC §6 M1 bullet 3 ("if the counters show it matters"). RFC 1951 §3.2.5 (the length and distance tables). Global invariants apply. PT1–PT7.
- **Gate:** use `perf annotate` of `Inflater::step` from `make profile`, only on the cleanflate-only binary, together with the dispatch counters. If LenExtra/DistExtra handling and base/extra table lookups take less than max(2%, 2x noise) of samples, record a negative result and stop.
- Payload values must equal `tables.rs`, which is transcribed from RFC 1951 §3.2.5.
- The Reference path keeps reading `tables.rs`.
**Acceptance tests:**
- `crates/cleanflate/src/tests/huffman_entry_payload.rs`:
  - `entry_payload_matches_rfc_tables`: all 29 length codes and 30 distance codes.
  - `payload_decode_equals_reference_all_codes`
- If gated out, the PERF.md entry is the deliverable.
**Out of scope:** The fast loop (P2-01).
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k`.
**Depends on:** P1-02, P0-10

### P1-05: M1 acceptance and push
**Goal:** Run the M1 acceptance commands, measure the table-decode ratio and the aggregate gain, commit the bench, snapshot provenance and push.
**Files touched:** `docs/perf/P1-05.json`, `docs/perf/P1-05-stats.json`, `docs/PERF.md` (`### M1 acceptance (P1-05)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 1 files.
**Design constraints:** SPEC §6 M1 acceptance (≥ 99% of litlen and distance symbols decoded by table lookup on Silesia; aggregate gain measured; all correctness gates green), §6 intro, PT5, PT8.
- Run:
  - `make ci`;
  - `make heavy`;
  - `make bench BENCH_ID=P1-05 BENCH_OUT=docs/perf/P1-05.json`;
  - `make bench-compare` against `docs/perf/baseline.json` and against `docs/perf/P0-13.json`;
  - `make bench-stats BENCH_ID=P1-05 BENCH_OUT=docs/perf/P1-05-stats.json`.
- PERF.md records:
  - the table-decode % for litlen and dist (≥ 99%: MET or NOT MET);
  - aggregate gains per framing (level 6, one-shot and streaming);
  - REGRESSION lines, if any, as findings;
  - the correctness gates.
- An unmet performance criterion is recorded, not blocking. A correctness failure blocks per PT5.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci` and `make heavy` pass. The JSONs are committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Review the P1-01 and P1-02 DESIGN `[perf]` entries for clean-room wording (SPEC §2.1).
3. Sanity-check the M1 gain on a quiet machine if the noise was high.
**Depends on:** P1-01, P1-02

## Phase 2 — M2 Fast decode loop

### P2-01: Fast literal/length/distance decode loop
**Goal:** Add the Fast-path loop for the literal/length/distance state. It is entered only with enough input and output margin, has no per-symbol state clone or outer dispatch, and exits to the state machine near buffer edges, at end of block, or on any error.
**Files touched:**
- `crates/cleanflate/src/`: `inflate.rs`, `fast_loop.rs` (new, optional), `bitreader.rs`, `config.rs`, `tables.rs` (a named `MAX_MATCH_LEN = 258` if absent), `stats.rs`, `lib.rs` (module declaration if a new file).
- `crates/cleanflate/src/tests/`: `fast_loop.rs` (new), `config.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P2-01-before.json`, `docs/perf/P2-01.json`.
**Design constraints:** SPEC §6 M2, §5 (every bullet; this is where phase-1 P4-08 failed), §3. RFC 1951 §3.2.5 (lengths 3..258, distances 1..32768, extra bits) and §3.2.6. Global invariants apply. PT1–PT7.
- **New tunables in config.rs (C8):**
  - `FAST_LOOP_MIN_INPUT: usize = 16`: input bytes that must remain to start an iteration. It must be ≥ 8, so one word refill always succeeds.
  - `FAST_LOOP_MIN_OUTPUT: usize = 258`: output bytes that must remain. It must be ≥ `MAX_MATCH_LEN`.
  - Doc comments state each constraint and say "initial value; re-swept in P4-03". Never write these numbers elsewhere.
- **Entry:** from `step` when `state == LitLen`, the path is Fast, and both margins hold.
- **Each iteration:**
  - Re-check both margins.
  - Refill in bulk if fewer than 48 bits are buffered (15 + 5 + 15 + 13, the most one literal/length plus distance sequence needs), so no sub-step inside an iteration can run out of bits.
  - A literal/length lookup (it cannot return NeedMoreBits with ≥ 48 bits buffered):
    - A literal is written, and the loop continues.
    - 256: consume it, set `state = next_block_state()`, and leave the loop.
    - 257..=285: length = base + extra bits.
      - Distance lookup. Codes 30/31 yield the same `Error::InvalidSymbol` the slow path returns, stored sticky in `State::Failed` the same way.
      - Distance = base + extra. A distance beyond the history available yields exactly the `Error::InvalidDistance { distance, available }` values the slow path computes.
      - Copy the whole match; it fits because the margin is ≥ 258.
    - 286/287, or any Invalid lookup: `InvalidSymbol`.
- **Locals:** keep the bit buffer, count and positions in locals and write them back on every exit. The exit state is `LitLen` (margin), the next-block state (EOB) or `Failed`. The outer loop then applies P1-01's exit rules unchanged.
- **Equivalence:** errors must be the same `Error` value, returned in the same call, as on the Reference path (fastslow compares per call).
- **Stats:** `fast_loop_entries`, `fast_loop_symbols`.
- **DESIGN:** a PT7 entry explaining why the margins make mid-symbol exhaustion impossible, with a pointer to D-22's negative result.
- **On any wrong result:** PT5, fixture first. If it is unresolved, block with the fixture, hypothesis and what was ruled out, and keep the fixture.
**Acceptance tests:**
- `crates/cleanflate/src/tests/fast_loop.rs`:
  - `fast_loop_same_trace_as_reference_hand_built`:
    - Streams: fixed and dynamic blocks; literals; overlapping and non-overlapping matches; EOB mid-input; several blocks; a stored block after a compressed one.
    - Schedules: whole input, 1-byte output, 1-byte input, and 7/13.
  - `fast_loop_input_margin_boundaries`: exactly FAST_LOOP_MIN_INPUT − 1, FAST_LOOP_MIN_INPUT and + 1 bytes left.
  - `fast_loop_output_margin_boundaries`: 257, 258 and 259 bytes of room.
  - `fast_loop_errors_identical_to_reference`: distance too far back, distance before stream start, fixed distance code 30, and fixed litlen 286/287. Each gives the same Error value at the same call index.
  - `fast_loop_eob_then_trailer_consumed_exact`
- `crates/cleanflate/src/tests/config.rs::fast_loop_margins_valid`: FAST_LOOP_MIN_INPUT ≥ 8 and FAST_LOOP_MIN_OUTPUT ≥ 258.
- `make test` (including the `fixed-heavy-match-11k` regression seeds), `make proptest-3k`, `make fastslow-full` and `make proptest-10k` pass.
**Out of scope:** Output-as-history and pattern doubling (P3-01, P3-02). Slow-path refactors (P2-03).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full && make proptest-10k && make bomb`.
**Depends on:** P1-01, P1-02

### P2-02: Differential fuzz target: fast vs reference
**Goal:** Add fuzz target `diff_fast_reference`. It decodes arbitrary bytes in all three formats on both paths, one-shot and under input-derived chunk schedules, and fails on any difference.
**Files touched:**
- `fuzz/`: `Cargo.toml` (the cleanflate dependency gains `features = ["test-hooks"]`; new `[[bin]]`), `Cargo.lock`, `fuzz_targets/diff_fast_reference.rs`, `seeds/diff_fast_reference/*` (≤ 10 small seeds copied from `testdata/regressions/` and `testdata/malformed/`), `README.md`.
- `Makefile` (`FUZZ_TARGETS`), `.github/workflows/fuzz-nightly.yml` (a matrix entry).
**Design constraints:** SPEC §5, §3 (fuzz targets keep passing), C9 (no libz in fuzz), C11 (names), CR3 (never install nightly or cargo-fuzz).
- **Input layout:**
  - `byte0 % 3` selects the format.
  - `byte1` and `byte2` give the input and output chunk sizes, 1..=256.
  - The rest is the stream.
- `Options.max_output` is `const FUZZ_MAX_OUTPUT`, as in the other targets.
- **Compare:**
  - one-shot results (Debug of the Result);
  - full per-call traces: consumed, produced and status, or the error's Debug string;
  - `finish()`.
- Panic on any difference, with a short description. The streaming loop has a no-progress guard.
**Acceptance tests:**
- `make fuzz-check` passes.
- `make fuzz-smoke-diff_fast_reference FUZZ_SECONDS=120` has zero crashes, or is recorded as `NOT RUN: <reason>` if nightly or cargo-fuzz is unavailable. Record the exec count.
**Out of scope:** Changing the other fuzz targets.
**Verification:** `make fuzz-check && make fuzz-smoke-diff_fast_reference FUZZ_SECONDS=120`.
**Depends on:** P0-04

### P2-03: Copyable state in the slow path (gated)
**Goal:** Gated per PT2. If the slow path still costs a significant share after the fast loop, remove the per-iteration `self.state.clone()` in `Inflater::step` by making `State` `Copy`. The sticky error moves into its own field.
**Files touched:**
- `crates/cleanflate/src/inflate.rs`.
- `crates/cleanflate/src/tests/`: `inflate_state.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P2-03-before.json`, `docs/perf/P2-03.json`.
**Design constraints:** SPEC §4.4 H2, §6 M2 bullet 2. D-11 (errors are sticky and returned as a clone on every later call). Global invariants apply. PT1, PT2, PT4–PT7.
- **Gate:** `Inflater::step` self time outside the fast loop plus State clone/drop frames. If they are below max(2%, 2x noise) of samples, record a negative result and stop.
- This is a behaviour-preserving refactor used by both paths.
**Acceptance tests:**
- `crates/cleanflate/src/tests/inflate_state.rs::errors_remain_sticky_after_refactor`: every error kind, followed by three later calls.
- Existing tests pass.
- If gated out, the PERF.md entry is the deliverable.
**Out of scope:** The fast loop.
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k`.
**Depends on:** P0-05, P0-10

### P2-04: M2 acceptance and push
**Goal:** Run the M2 acceptance commands, including the full-corpus differential and the 10k proptest, audit repro preservation, commit the bench and push.
**Files touched:** `docs/perf/P2-04.json`, `docs/PERF.md` (`### M2 acceptance (P2-04)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 2 files.
**Design constraints:** SPEC §6 M2 acceptance (the fast-vs-slow differential passes on the whole corpus; the chunking proptest passes 10k; the repro rule was followed; the aggregate gain is measured), PT5, PT8.
- Run:
  - `make ci`;
  - `make heavy`;
  - `make fuzz-smoke` if nightly is available (otherwise NOT RUN with the reason);
  - `make bench BENCH_ID=P2-04 BENCH_OUT=docs/perf/P2-04.json`;
  - `make bench-compare` against `baseline.json` and `P1-05.json`.
- PERF.md records:
  - gains per framing and mode;
  - REGRESSION lines;
  - the repro audit: every `testdata/regressions/` fixture added in phase 2 so far, with its origin, and confirmation it passes;
  - the fast-loop coverage (`fast_loop_symbols` share from `make bench-stats`).
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci` and `make heavy` pass. The JSON is committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Run `make fuzz-long` or the nightly workflow, including `diff_fast_reference`, for the 1-hour signal.
3. Review fast-loop code by hand, and mutation-sample it in review (SPEC §5).
**Depends on:** P2-01

## Phase 3 — M3 Output and match history

### P3-01: Output as match history; window carried only across calls
**Goal:** Write each output byte once. Within a call, the caller's output slice is the match history. The 32 KiB window keeps only history that must survive across calls, refreshed from the output tail on exit. Stored blocks and non-overlapping matches are copied as slices.
**Files touched:**
- `crates/cleanflate/src/`: `window.rs`, `inflate.rs`, the fast-loop file if separate, `stats.rs`.
- `crates/cleanflate/src/tests/`: `history.rs` (new), `window.rs`, `fastpath.rs`, `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P3-01-before.json`, `docs/perf/P3-01.json`.
**Design constraints:** SPEC §6 M3 bullets 1–2, §4.4 H3, §5, §3 (bounded memory; dictionaries per D-17; streaming contract). RFC 1951 §2 and §3.2.5 (a distance reaches back into all output so far, including bytes written earlier by the same match). Global invariants apply. PT1–PT7.
- **`Window`:**
  - Keeps the 32 KiB ring and `total`, the bytes ever appended including a preset dictionary.
  - New `append(&mut self, data: &[u8])`: keeps the last ≤ 32 KiB, in at most two slice copies.
  - New `read_back(&self, back: usize, dst: &mut [u8])`: copies `dst.len()` bytes starting `back` bytes before the end, in at most two pieces; `back ≥ dst.len()`.
  - `available()` is unchanged.
  - `prefill` uses `append`.
  - The per-byte `push` leaves every hot path.
- **History view within a call:** history is the window as of call entry followed by `output[..out_pos]`, and `history_available = min(window.available() + out_pos, WINDOW_SIZE)`. Distance validation uses it and reports it in `InvalidDistance { available }`; the numbers are the same as today.
- **Fast match copy,** one function shared by the slow `Copy` state and the fast loop. For n = min(remaining, room), while bytes are left, with `cur` the current output position:
  - If `dist > cur`: copy `min(left, dist − cur)` bytes from the window with `read_back(dist − cur, ..)`.
  - Otherwise: `copy_within(cur − dist .. cur − dist + chunk, cur)` with `chunk = min(left, dist)`, which is non-overlapping by construction.
- **Reference copy:** byte by byte through the same view: `output[cur − dist]` if `dist ≤ cur`, else `window.get_back(dist − cur)`.
- Literals are written only to output. The stored copy is one `copy_from_slice` per call chunk on both paths; this is structural, not a switched technique.
- **On every return from `step`** (NeedsInput, OutputFull, Done, Err), exactly once, in the shared exit helper: `window.append(&output[..out_pos])`. Only the last ≤ 32 KiB is copied.
  - The zlib/gzip wrappers need no change.
  - A 1-byte output buffer appends 1 byte per call.
  - Preset dictionaries keep their semantics.
- No new per-call heap allocation.
- `window_bytes_written` now counts bytes appended at exit; this is the H3 evidence.
- **DESIGN:** a PT7 entry superseding the copy mechanics of D-08/D-22. The semantics are unchanged.
**Acceptance tests:**
- `crates/cleanflate/src/tests/history.rs`:
  - `history_copy_matches_vec_model`: compares against an independent model that keeps every byte in a plain `Vec<u8>` and copies byte by byte.
    - Window fill states: empty, 100 bytes, exactly 32768, and wrapped after 100000.
    - `out_pos`: 0..=40 and 1000.
    - dist: 1..=40, plus {255, 256, 257, 32767, 32768} where available.
    - len: sampled from 3..=258.
    - Room: {1, 7, len}.
  - `window_append_wraps_and_truncates`: appends of 0, 1, 32767, 32768, 32769 and 100000 bytes at various fill states, against the model.
  - `dictionary_with_one_byte_output`: a hand-built raw stream whose matches reach into a preset dictionary, output chunks of 1. Fast and Reference traces are identical and the output is correct.
  - `match_resumes_across_calls_after_window_refresh`
  - `decompressor_size_not_grown`: `size_of::<Decompressor>()` ≤ the pre-change size + 1024. Measure the pre-change size first and write it in the test as a literal, with a comment.
- `fastpath.rs` is updated.
- `make test`, `make proptest-3k`, `make fastslow-full` and `make bomb` pass.
**Out of scope:** Pattern doubling (P3-02). One-shot sizing (P3-03).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full && make bomb`.
**Depends on:** P0-05, P0-06

### P3-02: Overlapping matches by pattern doubling
**Goal:** On the Fast path, expand overlapping (distance < length) matches by copying the growing repeated pattern in doubling steps instead of distance-sized chunks.
**Files touched:**
- `crates/cleanflate/src/`: `window.rs` or `inflate.rs` (wherever the match-copy function lives), `stats.rs`.
- `crates/cleanflate/src/tests/history.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P3-02-before.json`, `docs/perf/P3-02.json`.
**Design constraints:** SPEC §6 M3 bullet 2, §4.4 H4. RFC 1951 §3.2.5 (overlap semantics). Global invariants apply. PT1–PT7. Fast only; the Reference copy is unchanged.
- Once the first `dist` bytes of the pattern sit at `[s, s + dist)` in output, repeatedly `copy_within(s .. s + k, s + k)` with k starting at `dist` and doubling, capped by the remaining length. Each step is non-overlapping and preserves the period `dist`. Derive and state this in DESIGN.
- For `dist == 1`, `fill` is allowed.
- A pattern whose first bytes come from the window is first materialised in output, then doubled.
**Acceptance tests:**
- `crates/cleanflate/src/tests/history.rs`:
  - `pattern_doubling_matches_model_exhaustive`: dist 1..=64 × len 3..=258 × `out_pos` {0 (source in the window), dist, 1000} × room {1, 13, len}.
  - `pattern_doubling_straddling_window`
- `make test`, `make proptest-3k` and `make fastslow-full` pass.
**Out of scope:** Non-overlapping copies (done in P3-01).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full`.
**Depends on:** P3-01

### P3-03: One-shot output sizing from hints
**Goal:** Size the one-shot output buffer from available hints so `decompress_with` rarely regrows, always within the output limit and a preallocation cap, and move the growth-floor literal into config.rs.
**Files touched:**
- `crates/cleanflate/src/`: `stream.rs`, `config.rs`, `stats.rs`.
- `crates/cleanflate/src/tests/`: `oneshot_sizing.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P3-03-before.json`, `docs/perf/P3-03.json`.
**Design constraints:** SPEC §6 M3 bullet 3, §4.4 H6, §3 (bounded memory; the bomb test unchanged). D-13 and D-25: the growth loop and its empty-region backstop stay. RFC 1952 §2.3.1 (ISIZE). Global invariants apply. PT1, PT4–PT7.
- **New tunables in config.rs (C8):**
  - `ONESHOT_GROWTH_FLOOR: usize = 32_768`, replacing the literal in `next_len`.
  - `ONESHOT_INITIAL_RATIO: usize = 4`.
  - `ONESHOT_MAX_PREALLOC: usize = 64 << 20`.
- **`pub(crate) fn initial_len(format, input: &[u8], max_output: u64) -> usize`:**
  - Gzip with `input.len() >= 18`: the last 4 bytes as a little-endian u32. This is the last member's ISIZE, used only as a hint; multi-member streams and trailing data make it unreliable.
  - Otherwise: `input.len().saturating_mul(ONESHOT_INITIAL_RATIO)`.
  - The result is capped by `max_output` and by `ONESHOT_MAX_PREALLOC`.
  - It only sets the first allocation; the existing geometric growth continues from there. Output is never affected.
- **DESIGN:** a `[perf]` entry explaining why a hostile ISIZE cannot force a large allocation.
**Acceptance tests:**
- `crates/cleanflate/src/tests/oneshot_sizing.rs`:
  - `initial_len_gzip_uses_isize`
  - `initial_len_capped_by_limit_and_prealloc`
  - `initial_len_raw_zlib_ratio`
  - `initial_len_tiny_or_hostile_inputs`: empty, fewer than 18 bytes, and ISIZE = u32::MAX.
  - `next_len_uses_config_floor`
  - `gzip_multi_member_hint_too_small_still_correct`: a hand-built two-member gzip with literal CRCs (C16).
- `make bomb` passes.
- `oneshot_resizes` from `make bench-stats`, before and after, is recorded in PERF.md.
**Out of scope:** Streaming-API changes.
**Verification:** PT1, `make lint && make test && make proptest-3k && make bomb`.
**Depends on:** P0-07

### P3-04: M3 acceptance and push
**Goal:** Run the M3 acceptance commands, report streaming throughput separately, confirm the bomb test and memory bounds, commit the bench and push.
**Files touched:** `docs/perf/P3-04.json`, `docs/PERF.md` (`### M3 acceptance (P3-04)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 3 files.
**Design constraints:** SPEC §6 M3 acceptance (as M2, plus streaming throughput reported separately; bomb test and memory bounds unchanged), §4.1 streaming target (≥ 60%), PT5, PT8.
- Run:
  - `make ci`;
  - `make heavy`;
  - `make bench BENCH_ID=P3-04 BENCH_OUT=docs/perf/P3-04.json`;
  - `make bench-compare` against `baseline.json` and `P2-04.json`.
- PERF.md records:
  - a one-shot table and a separate streaming table (Silesia, level 6, all framings) with the 60% streaming target marked MET or NOT MET;
  - the `make bomb` result;
  - the `decompressor_size_not_grown` value;
  - REGRESSION lines.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci`, `make heavy` (including bomb) pass. The JSON is committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Watch peak RSS of `make bomb` (for example `/usr/bin/time -v`) against a phase-1 checkout, to confirm memory bounds are unchanged.
3. Review the history/copy DESIGN entries.
**Depends on:** P3-01

## Phase 4 — M4 Checksums, wrappers and tuning

### P4-01: CRC-32 several bytes at a time
**Goal:** Compute CRC-32 several bytes per step from multiple tables derived from the RFC 1952 polynomial, generated by `const fn`, and verified against the existing byte-wise implementation and the oracle.
**Files touched:**
- `crates/cleanflate/src/`: `checksum.rs`, `config.rs`.
- `crates/cleanflate/src/tests/checksum.rs`, `crates/oracle/tests/checksums.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P4-01-before.json`, `docs/perf/P4-01.json`.
**Design constraints:** SPEC §6 M4 bullet 1, §2.1, §4.4 H6. RFC 1952 §8 (reflected polynomial 0xEDB88320) and D-10. Global invariants apply. PT1, PT4–PT7.
- No `[]` indexing: use `get` with `u8`-derived indices, whose range the optimiser can prove, so it drops the check.
- No `assert!` in const fns.
- No lazily-initialised statics (rule `no-global-state-in-cleanflate`); build the tables in a `const`/`static` by `const fn`.
- **New tunable:** `CRC32_SLICES: usize = 8` in config.rs; it must be ≥ 4.
- **Tables:** `[[u32; 256]; CRC32_SLICES]`.
  - Table 0 is today's table.
  - Table k maps byte b to the CRC register after processing b followed by k zero bytes, so it follows from table k−1 by one more zero-byte step.
  - Derive and prove this in DESIGN from the CRC's linearity over GF(2), in the project's own words.
- **Update:**
  - Process `chunks_exact(CRC32_SLICES)`, folding the current register into the first 4 bytes of each chunk.
  - The tail uses the byte-wise step.
  - Keep the byte-wise update as `pub(crate)` for the tail and as the test reference.
- The public `Crc32`/`crc32` API is unchanged.
**Acceptance tests:**
- `crates/cleanflate/src/tests/checksum.rs`:
  - `crc32_slices_match_bytewise`: every length 0..=64 plus random lengths to 4096, start offsets 0..=7, and split updates.
  - `crc32_tables_follow_zero_byte_recurrence`
  - `crc32_slices_config_valid`
- `crates/oracle/tests/checksums.rs::crc32_matches_oracle_long`: random buffers up to 1 MiB, updated incrementally in random pieces.
- `Measurement:` the gzip-vs-raw aggregate overhead before and after, from bench-quick.
**Out of scope:** Adler-32 (P4-02).
**Verification:** PT1, `make lint && make test && make proptest-3k`.
**Depends on:** P0-09

### P4-02: Adler-32 revisit (gated)
**Goal:** Gated per PT2. If zlib framing still carries a significant Adler-32 cost, speed up `Adler32::update` within the proven deferred-modulo bound (D-23).
**Files touched:**
- `crates/cleanflate/src/checksum.rs`, `crates/cleanflate/src/tests/checksum.rs`, `crates/oracle/tests/checksums.rs`.
- `docs/DESIGN.md`, `docs/PERF.md`, `docs/perf/P4-02-before.json`, `docs/perf/P4-02.json`.
**Design constraints:** SPEC §6 M4 bullet 2. RFC 1950 §2.2 and §8.2. D-23. Global invariants apply. PT1–PT7.
- **Gate:** Adler functions' share in the `PROFILE_FRAMING=rfc1950` profile. Below max(2%, 2x noise), record a negative result and stop.
- **Allowed:** fixed-size sub-blocks with `chunks_exact`, so the compiler can unroll or auto-vectorise, still reducing once per `ADLER_NMAX`.
- Any change to the bound must be re-derived in DESIGN.
**Acceptance tests:**
- `crates/cleanflate/src/tests/checksum.rs`:
  - `adler32_bound_worst_case`, still passing
  - `adler32_chunked_matches_bytewise`: lengths 0..=20000, including multiples of NMAX ± 1, and split updates.
- `crates/oracle/tests/checksums.rs::adler32_matches_oracle_long`
- If gated out, the PERF.md entry is the deliverable.
**Out of scope:** CRC-32.
**Verification:** PT1/PT2, `make lint && make test`.
**Depends on:** P0-09, P0-10

### P4-03: Re-sweep tunables on the final code
**Goal:** Re-measure every performance tunable that exists on the final code, choose each value by the noise rule, and cite the measurements in PERF.md and config.rs.
**Files touched:** `crates/cleanflate/src/config.rs`, `docs/PERF.md`, `docs/perf/P4-03/*.json`.
**Design constraints:** SPEC §6 M4 bullet 3, §3 (tunables only in config.rs, with the measurement that chose them), §4.2 (noise rule). C8, PT8.
- **Keys and candidates** (sweep only keys that exist):
  - `LITLEN_TABLE_BITS` {9, 10, 11, 12}
  - `DIST_TABLE_BITS` {7, 8, 9, 10}
  - `FAST_LOOP_MIN_INPUT` {8, 16, 32}
  - `FAST_LOOP_MIN_OUTPUT` {258, 320, 512}
  - `CRC32_SLICES` {4, 8, 16}
  - `ONESHOT_INITIAL_RATIO` {2, 4, 8}
- **Procedure:**
  - Sweep one key at a time with the others at their current values.
  - For each candidate, edit config.rs, then run `make bench-quick BENCH_ID=P4-03-<key>-<value> BENCH_OUT=docs/perf/P4-03/<key>-<value>.json`.
  - `make bench-compare` each candidate against the current default's run.
  - The relevant aggregates are gzip for CRC and raw for the rest, Silesia level 6.
  - A quick `WIN` must be confirmed by `make bench` (full, background) against the latest milestone full JSON.
  - Change a default only if the full comparison is also a `WIN` with no REGRESSION. Otherwise keep it and record a measured "no change".
- **Safety bounds, not swept:** `DEFAULT_MAX_OUTPUT`, `DEFAULT_MAX_GZIP_FIELD`, `DEFAULT_MAX_GZIP_HEADERS`, `ONESHOT_GROWTH_FLOOR` and `ONESHOT_MAX_PREALLOC`. Say so in PERF.md.
- **Results:**
  - Each tunable's doc comment cites its JSON and result.
  - PERF.md `### Tunables (P4-03)` has a table of key, candidates, winner, delta and verdict.
  - Run `make test && make proptest-3k` on the final values.
**Acceptance tests:**
- Existing boundary tests (`fast_loop_margins_valid`, `crc32_slices_config_valid`, `table_widths_sweepable`) pass on the final values.
- `make test` and `make proptest-3k` pass.
**Out of scope:** Code changes outside config.rs.
**Verification:** `make lint && make test && make proptest-3k`.
**Depends on:** P0-09

### P4-04: M4 acceptance and push
**Goal:** Run the M4 acceptance commands, report zlib and gzip framing overhead relative to raw, confirm the tunables are cited, commit the bench and push.
**Files touched:** `docs/perf/P4-04.json`, `docs/PERF.md` (`### M4 acceptance (P4-04)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 4 files.
**Design constraints:** SPEC §6 M4 acceptance (gzip and zlib framing overhead relative to raw reported; tunables' measurements in PERF.md and cited in config.rs), PT5, PT8.
- Run:
  - `make ci`;
  - `make heavy`;
  - `make bench BENCH_ID=P4-04 BENCH_OUT=docs/perf/P4-04.json`;
  - `make bench-compare` against `baseline.json` and `P3-04.json`.
- PERF.md:
  - a framing-overhead table: zlib vs raw and gzip vs raw, Silesia level 6, one-shot and streaming, baseline against now;
  - REGRESSION lines.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci` and `make heavy` pass. The JSON is committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Review the CRC derivation in DESIGN.
3. Confirm the config.rs citations point at committed JSONs.
**Depends on:** P4-01

## Phase 5 — M5 Report and hand-off

### P5-01: Phase-scoped provenance summary and build report
**Goal:** Let `make provenance-summary` and `make build-report` cover only phase-2 activity, and read the phase-2 result line from PERF.md.
**Files touched:**
- `tools/cleanroom/`: `summarize.py`, `build_report.py`.
- `tools/cleanroom/tests/`: `test_summarize.py`, `test_build_report.py`, fixtures as needed.
- `Makefile`.
**Design constraints:** SPEC §6 M5 (regenerate `docs/BUILD_REPORT.md` and `provenance/SUMMARY.md` for phase 2), §2 (the logger and guard are unchanged). Stdlib Python only, as today.
- **`summarize.py --since-commit REV`:**
  - Keeps log entries with a timestamp ≥ REV's commit time (`git show -s --format=%cI REV`).
  - The header names the range.
- **`build_report.py --since REV`:**
  - Uses only commits in `REV..HEAD`.
  - `read_perf_result_line` returns the last `**Result:**` paragraph in PERF.md, not the first.
- Without the flags, behaviour is unchanged and the existing tests pass.
- **Makefile:** `PHASE_BASE ?= 599ca51` (the last phase-1 commit). `provenance-summary` passes `--since-commit $(PHASE_BASE)` and `build-report` passes `--since $(PHASE_BASE)`.
**Acceptance tests:**
- `tools/cleanroom/tests/test_summarize.py::test_since_commit_filters_entries`
- `tools/cleanroom/tests/test_build_report.py::test_since_limits_commits`
- `tools/cleanroom/tests/test_build_report.py::test_reads_last_result_line`
- `make tools-check` passes.
**Out of scope:** Changing any other summary section.
**Verification:** `make tools-check`.
**Depends on:** none

### P5-02: Final PERF.md phase-2 report and DESIGN audit
**Goal:** Run the final full bench and write the phase-2 report: numbers against every SPEC §4.1 target, per-milestone gains, hypothesis verdicts, negative results and reverted attempts with their fixtures, and the result line.
**Files touched:** `docs/PERF.md`, `docs/perf/P5-02.json`, `docs/DESIGN.md` (audit fixes only).
**Design constraints:** SPEC §6 M5 bullet 1, §4.1, §9 (every optimisation written up in DESIGN.md with its measurement), §8 (record conditions), C10, C14, PT8.
- Run `make bench BENCH_ID=P5-02 BENCH_OUT=docs/perf/P5-02.json` and `make bench-compare A=docs/perf/baseline.json B=docs/perf/P5-02.json`.
- **PERF.md `### Final (P5-02)`:**
  - Targets table: raw, zlib and gzip one-shot level-6 Silesia ratios against 70% (primary) and 90% (stretch); streaming against 60%; each MET or NOT MET.
  - Per-milestone gains from the JSONs that exist: baseline, P1-05, P2-04, P3-04, P4-04, P5-02.
  - H1–H6 final verdicts.
  - Negative results and reverted attempts, each with its fixture name.
  - Per-file regression check against the baseline.
  - A level 1 and level 9 summary.
  - Conditions.
  - If a target is missed: the measured shortfall and the next most promising step, with its counter or profile evidence.
- **Final line, exactly one:** `**Result:** phase 2: cleanflate / C zlib (Silesia, level 6, one-shot) = R% raw, Z% zlib, G% gzip (target 70%: MET|NOT MET); streaming 64 KiB = S% raw (target 60%: MET|NOT MET).`
- **DESIGN audit:** every phase-2 technique has a `[perf]` entry that passes design-check and cites its JSON.
**Acceptance tests:** `make design-check` passes, PERF.md contains the result line, and the JSON is committed.
**Out of scope:** Code changes.
**Verification:** `make design-check && make bench-md J=docs/perf/P5-02.json`.
**Depends on:** P0-11

### P5-03: Provenance, build report and hand-off push
**Goal:** Regenerate the phase-2 build report and provenance summary, update the human-interventions log, snapshot provenance, run the final gates, push, and list the manual checks.
**Files touched:** `docs/BUILD_REPORT.md` (generated), `provenance/SUMMARY.md` (generated), `provenance/human-interventions.md`, `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines anywhere, recorded in `Defects-caught:`.
**Design constraints:** SPEC §6 M5 (regenerate BUILD_REPORT and SUMMARY; update human interventions; the implementer does not tag), §9, PT8.
- Run `make provenance-snapshot`, `make provenance-summary` and `make build-report`.
- `human-interventions.md`: add rows only for interventions this phase's progress log records, or relayed operator actions the implementer knows of. If there are none, add one dated sentence under the table saying the pipeline recorded none in phase 2.
- Run `make ci` and `make heavy`, commit, and `git push -u origin HEAD`.
**Acceptance tests:** `make ci` and `make heavy` pass. SUMMARY.md and BUILD_REPORT.md are regenerated for the phase-2 range.
**Out of scope:** Tagging.
**Verification:** `make ci && make heavy && make provenance-snapshot provenance-summary build-report && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Run the final `make bench` on a quiet machine with a fixed governor (SPEC §8), record the conditions, and compare with `docs/perf/P5-02.json`.
2. Run `make fuzz-long` or the nightly workflow for all six targets and triage `make fuzz-divergences`.
3. Confirm GitHub Actions `ci` is green.
4. Review the PERF.md result line and the negative results.
5. After review and merge into `feature/perf` (and on to `main`), tag `v0.2.0-cleanroom`.
6. Complete `provenance/human-interventions.md`.
**Depends on:** P5-01, P5-02

## Spec issues
- **§4.1 streaming target denominator:** not stated. Resolved as C zlib one-shot, so no new FFI is needed (§10), and it is a conservative bar.
- **§4.3 "time or calls spent building tables" vs phase-1 C7** (no clocks in the library): resolved as counts only. Time comes from profiles.
- **§4.4 / M0 "order the remaining milestones' tasks by measured cost":** a plan executed unattended cannot be reordered. Resolved with a fixed milestone order plus gated tasks (PT2) and a recorded ranking (P0-12).
- **§5 "cfg(test)-only or feature-gated switch":** `cfg(test)` cannot reach the oracle's integration tests. Resolved with feature `test-hooks`, enabled only in the oracle's dev-dependencies.
- **§5 "identical ... consumed":** requires the Fast path to hold exactly the Reference path's buffered bits at every return. Made an explicit invariant in P1-01 and checked per call.
- **Phase-1 D-16 vs the serial walk:** they disagree on when an incomplete distance table's unused code is `Invalid`, and the table's answer depends on the `DIST_TABLE_BITS` tunable. This is not in SPEC, but it blocks a well-defined differential. Resolved in P0-04 with a width-independent rule; accept/reject and the malformed expectations are unchanged. Consider noting it in SPEC §3's "identical accept/reject behaviour" (ErrorKind may move from `Truncated` to `InvalidSymbol` only for streams that are both truncated and invalid).
- **§4.2 full bench runtime:** the full bench is about 10–15 minutes at baseline, so each run exceeds the agent's 10-minute foreground tool limit. Run in the background (PT8). A second full run for noise doubles it.
- **§4.1 "no per-file regression ... more than the measured noise":** tiny Canterbury files have high relative noise. The threshold is max(file noise, aggregate noise), and each sample lasts ≥ 20 ms.
- **§6 M3 "gzip ISIZE when trustworthy":** ISIZE is never trustworthy for allocation (multi-member streams, trailing data, hostile input). It is used only as a hint, capped by `max_output` and `ONESHOT_MAX_PREALLOC`.
- **§3 "Additive, documented changes are allowed only if a measurement shows they are needed":** the `stats` and `test-hooks` APIs are required by SPEC itself (§4.3, §5) and are opt-in features. They are treated as allowed.
- **§4.2 "Canterbury plus a fixed Silesia subset":** the subset is not named. Chosen as dickens, mr, ooffice, sao and xml (text, image, binary, low-ratio and high-ratio).
- **§6 M5 regenerating BUILD_REPORT and SUMMARY "for phase 2":** the phase-1 generators count all history and phase-2 task IDs collide with phase-1 IDs. Resolved with `--since` scoping (P5-01).
- **§8 dates:** the profiler install is dated 2026-09-30, but SPEC was committed 2026-09-29. It is cosmetic, and `perf`/`cargo flamegraph` are confirmed present at `/usr/bin/perf` and `~/.cargo/bin/cargo-flamegraph`.
- **Phase-1 carry-overs** (review notes, not SPEC): the `32768` literal in `stream.rs` is fixed by P3-03. The stale `#[allow(dead_code)]` note and the garbled R1-01 comments are left for review to judge.

## Review fixes (round 1)

### R1-01: Pin the pending-literal give-back and keep the Reference path exact-fill
**Goal:** Make the D-27 (b) exit invariant observable. An OutputFull exit must keep exactly the pending literal's code bits, and the Reference path must never give bytes back, so the fast-vs-reference differential stays an independent oracle for per-call consumed. Today, mutating `self.keep_bits = len;` (crates/cleanflate/src/inflate.rs:675) to `len.min(7)` survives make test and make proptest-3k.
**Files touched:** crates/cleanflate/src/tests/bitreader_bulk.rs, crates/cleanflate/src/tests/stats.rs, crates/cleanflate/src/inflate.rs, crates/cleanflate/src/bitreader.rs, crates/cleanflate/src/huffman.rs
**Design constraints:** SPEC 3 (exact consumed accounting), SPEC 5, CLAUDE.md PT3/C20, DESIGN D-27. Global invariants apply: no unsafe, no panicking calls or [] indexing in library code; debug_assert! is allowed.
- Library changes are limited to:
  - an optional `debug_assert!(self.path == DecodePath::Fast || k == 0, ...)` at the shared exit in `Inflater::step`;
  - the two stale doc comments: bitreader.rs:11 ('at most 7 bits', superseded by D-27) and huffman.rs:121 (the Decoded::Invalid doc, superseded by D-26).
- No behaviour change.
- The consumed model in the new test must come from the test's own bit accounting (header bits + 9 bits per literal), never from cleanflate.
**Acceptance tests:** - crates/cleanflate/src/tests/bitreader_bulk.rs::pending_literal_bits_kept_at_output_full:
  - input: a hand-built fixed block of literals 144..=255 (9-bit codes, at least 64 literals) plus EOB, decoded with 1-byte output on both DecodePath::Fast and DecodePath::Reference;
  - after every call returning OutputFull, cumulative consumed == ceil((3 header bits + 9 * literals_written + 9) / 8);
  - repeat with output chunks of 3.
- crates/cleanflate/src/tests/stats.rs::reference_path_never_gives_back (#[cfg(feature = "stats")]):
  - streams: the same fixed literal stream plus the existing hand-built fixed and dynamic streams;
  - output chunks 1 and 3;
  - on DecodePath::Reference, Decompressor stats().given_back_bytes == 0.
- Both tests must fail when crates/cleanflate/src/inflate.rs `self.keep_bits = len;` is mutated to `self.keep_bits = len.min(7);` (check with foundry_mutate or by reasoning in the commit).
**Out of scope:** Any change to decoding behaviour, the fast loop, or the give-back algorithm itself. Performance work.
**Verification:** make lint && make test && make proptest-3k. Then confirm by mutation that the keep_bits = len.min(7) mutant is killed by make test.
**Depends on:** none

### R1-02: Fast-loop error equivalence for an incomplete distance table's unused code
**Goal:** Deterministically cover the fast loop's distance-lookup Invalid exit (crates/cleanflate/src/fast_loop.rs:137). Its error value can change today without make test noticing; only a random corrupted stream in proptest-3k catches it.
**Files touched:** crates/cleanflate/src/tests/fast_loop.rs
**Design constraints:** SPEC 5 (identical errors on both paths, mutation-check the fast path), RFC 1951 S3.2.7 (a single distance code of length 1 leaves one unused code). The test is hand-built with the existing bitwriter/dynbuilder helpers. No library change.
**Acceptance tests:** - Extend crates/cleanflate/src/tests/fast_loop.rs::fast_loop_errors_identical_to_reference (or add fast_loop_incomplete_distance_table_invalid_code) with this case:
  - a dynamic block whose distance table has exactly one code of length 1;
  - at least FAST_LOOP_MIN_INPUT + 8 bytes of valid literals/matches, so the fast loop is entered;
  - then a length symbol followed by the unused distance code (bit 1).
- Assert the same Error value (InvalidSymbol) at the same call index on Fast and Reference. Use a whole-input call with >= 258 bytes of output room, plus the 7/13 schedule.
- The test must fail when fast_loop.rs line 137 `Decoded::Invalid => return FastExit::Error(Error::InvalidSymbol),` is mutated to return `Error::BadBlockType`.
**Out of scope:** Library changes; other error kinds already covered by fast_loop_errors_identical_to_reference.
**Verification:** make lint && make test && make proptest-3k. Confirm by mutation that the BadBlockType mutant of the distance-Invalid arm is killed by make test.
**Depends on:** none

## Review fixes (round 2)

### R2-01: Pin the decode path through the zlib and raw-dictionary inflater constructors
**Goal:** Make the DecodePath plumbing into ZlibDecoder (initial inflater and FDICT dictionary inflater) and into the raw-with-dictionary Decompressor backend observable by tests, so a regression cannot silently turn the SPEC 5 fast-vs-reference differential into Fast-vs-Fast for zlib-framed and dictionary streams. Today, hard-coding DecodePath::Fast at crates/cleanflate/src/zlib.rs:47 (`inflater: Inflater::new_with_path(path)`), zlib.rs:150 (`Inflater::with_dictionary(dict, self.path)`) or stream.rs:172 (`Backend::Raw(Inflater::with_dictionary(dict, path))`) survives make test; the zlib.rs:150 mutant is caught only incidentally by the dead_code lint.
**Files touched:** crates/cleanflate/src/zlib.rs, crates/cleanflate/src/stream.rs, crates/cleanflate/src/tests/decode_path.rs
**Design constraints:** SPEC 5 (differential decodes the entire corpus both ways), PLAN P0-04 plumbing ('pass it to every Inflater they create', dictionary variant takes a path). Global invariants apply: no unsafe, no panicking calls or [] indexing in library code. Library changes are limited to two pub(crate) read-only accessors: `ZlibDecoder::inflater_path(&self) -> DecodePath` (mirroring GzipDecoder::inflater_path) and a Decompressor accessor returning the current backend inflater's path; both get `#[cfg_attr(not(test), allow(dead_code))]` exactly like the gzip accessor (dead_code only; never allow clippy panic/indexing lints, C5). No behaviour change; no public API change.
**Acceptance tests:** crates/cleanflate/src/tests/decode_path.rs::zlib_decoder_uses_same_path: ZlibDecoder::new(None, DecodePath::Reference) reports Reference before and after stepping a plain zlib header; a zlib FDICT stream (hand-built, DICTID from oracle-independent hard-coded Adler-32 bytes or computed by the test's own arithmetic per C16, not crate::checksum) decoded with the matching dictionary reports Reference after the DICTID is accepted and output is produced. crates/cleanflate/src/tests/decode_path.rs::decompressor_passes_path_every_format: Decompressor::new_with_path(format, options, DecodePath::Reference) reports Reference for Raw, Raw with Options.dictionary set, Zlib and Gzip (after one decompress call on a valid stream). Each of these three mutants must be killed by make test: zlib.rs `inflater: Inflater::new_with_path(path),` -> `Inflater::new_with_path(DecodePath::Fast),`; zlib.rs `Inflater::with_dictionary(dict, self.path)` -> `Inflater::with_dictionary(dict, DecodePath::Fast)`; stream.rs `Backend::Raw(Inflater::with_dictionary(dict, path))` -> `Backend::Raw(Inflater::with_dictionary(dict, DecodePath::Fast))`.
**Out of scope:** Any change to decoding behaviour, the fast loop, or the oracle differential harness. Performance work. Changing how GzipDecoder exposes its path.
**Verification:** make lint && make test && make proptest-3k. Then confirm by foundry_mutate (commands: make test) that each of the three mutants listed under tests is killed.
**Depends on:** none

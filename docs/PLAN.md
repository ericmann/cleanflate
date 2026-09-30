# cleanflate phase 3 build plan
Derived from docs/SPEC.md v(unversioned, commit cd4a472) on 2026-09-30. SPEC.md wins over this file.

## Decisions
- Phases vs milestones → Phase N implements SPEC §6 milestone MN (M0–M5), in order, not merged. Rationale: SPEC defines the milestones and their acceptance.
- Review cadence (SPEC §6 "each milestone ends with an acceptance task") → one Foundry review loop per flight, up to 6 rounds (SPEC §7). Each phase ends with an acceptance-and-push task that runs the §5 milestone gates. Rationale: Foundry reviews once per flight; the acceptance tasks carry the per-milestone gates.
- Base branch → `main`; build branches use the `build/` prefix and PR into `main` (SPEC §7).
- Narrow dependencies (SPEC §5) → optimisation tasks depend only on the infrastructure they use (profiling tools, noise files, the sink), never on acceptance tasks; acceptance tasks depend only on their milestone's core task. Every task that changes `crates/cleanflate/src/` depends on at least P0-01, which creates the fuzz target its verify runs. Rationale: phase 1 lost 17 tasks to one cascade.
- M1 "re-order M2–M4's tasks by measured cost" → an unattended plan cannot be reordered, so the order is fixed and every task whose hotspot may prove small is **gated** (CLAUDE.md PT2): it profiles first and records a negative result without code changes when its hotspot is below max(2%, 2x noise). P1-02 writes the measured ranking and the likely gate outcome of every later task into `docs/perf/phase3-profile.md`. Rationale: gates give the effect of re-ordering without editing PLAN mid-flight.
- Bench README location (SPEC §4.2 "`crates/oracle/BENCH.md` or `docs/perf/README.md`") → `docs/perf/README.md`, which already documents schema 1. Rationale: one place.
- JSON schema → schema 2 (P0-04). "min" is renamed "best" everywhere; the loader accepts schema-1 files through serde aliases, so every committed phase-2 JSON (including `docs/perf/baselines/phase2.json`) stays comparable. Rationale: SPEC §4.2 rename without losing the phase-2 record.
- Like-for-like denominators (SPEC §4.1/§4.2) → one-shot rows use C zlib one-shot (unchanged); streaming rows use C zlib streaming with 64 KiB input and 64 KiB output windows into the same pre-sized buffer (new `oracle::ffi::inflate_streaming_into`, P0-03). The reuse mode (P3-04) has no like-for-like C denominator, because C zlib reuse would need `inflateReset`, which is not a permitted signature (SPEC §2); its `ratio` is null and it is reported against cleanflate fresh-object streaming. Rationale: SPEC §2 limits the FFI surface.
- Drift → phase 2 measured ~10% run-to-run drift on this machine. `bench-compare` (P0-07) adds a drift-corrected verdict on the like-for-like ratio (C zlib's code never changes, so a ratio move is cleanflate's own), with ratio noise from the same two-run noise files. PT1: a change is a WIN when the targeted key's throughput verdict or ratio verdict is WIN and neither is LOSS. Rationale: gains of a few percent are otherwise unprovable here.
- `bench-quick` levels → 1 and 6 (P0-01); the reuse mode joins both benches in P3-04. Rationale: M4 targets level 1, and the quick noise file must cover every key a PT1 comparison judges.
- Result file names → every phase-3 bench, stats, checksum and assembly JSON goes under `docs/perf/phase3/` (`docs/perf/phase3/<ID>.json`, `<ID>-before.json`, ...), because phase-3 task IDs repeat phase-2 IDs whose JSONs already sit in `docs/perf/` (e.g. `P2-01.json`, `P5-02.json`) and must not be overwritten. Rationale: the phase-2 record stays intact.
- Where results go → per-task write-ups accumulate in `docs/perf/PHASE3.md` under `## Task log` as `### <ID>`; profiles in `docs/perf/phase3-profile.md`; JSONs in `docs/perf/`. `docs/PERF.md` gets only the `## Phase 3` pointer section and the final `**Result:**` line (P5-02), which `build_report.py` reads. Rationale: SPEC §6 M5 names PHASE3.md as the report and PERF.md as a pointer.
- Profile evidence in DESIGN (SPEC §2 "each write-up cites the profile evidence") → `make design-check` requires a `Profile:` line in every `[perf]` block numbered D-34 or later (P0-02). D-34 is the next free number at plan time; D-27..D-33 are unaffected. Rationale: mechanical, not remembered.
- Per-task fuzz gate (SPEC §5) → `make fuzz-smoke-decoder` (never_panic_raw/rfc1950/rfc1952 and diff_fast_reference, 60 s each) runs through `extraVerify` for every change under `crates/cleanflate/src/`, beside `make proptest-3k`. Rationale: mechanical.
- Milestone fuzz gate (SPEC §5) → `make fuzz-milestone` runs `diff_oracle_roundtrip` and `diff_fast_reference` concurrently for 1800 s each (30 min wall), in the background, never while a bench runs, at every milestone acceptance including M0 and M1 (SPEC §9: gates green "at every milestone"). Rationale: SPEC's letter; M0's run is also phase 3's starting fuzz signal.
- Checksum microbench (SPEC §6 M2 "cleanflate only") → a cleanflate example `checksum_bench` with `required-features = ["test-hooks"]`, timing variants exposed through `cleanflate::test_hooks::checksum` (P2-01). Rationale: no C code in the process, profile-able, no new public API.
- Adler-32 batch (SPEC §6 M2) → a derived constant in `checksum.rs` beside `ADLER_NMAX`, not a tunable, with its bound derived in a new `[perf]` entry that extends D-23. CRC-32 slicing-16 → the existing `CRC32_SLICES` tunable, measured by the microbench and the bench. Rationale: derived bounds are not tunables (C8 covers tunables).
- Output sink (SPEC §6 M2) → a crate-private `OutputSink` trait with `SliceSink` (streaming) and `VecSink` (one-shot), generic and monomorphised through `Inflater`, the fast loop, the window copy and both wrappers. Three tasks: the refactor with only the slice sink (P2-05), the Vec sink plus the sink differential (P2-06), and the switch of the public one-shot path (P2-07), so the switch alone can be reverted under PT4. Rationale: SPEC's one-core rule, and a revertible measurement.
- Vec sink growth → the Vec's capacity grows only by `reserve_exact` to the existing `next_len` schedule, capped by `max_output`, from the same first size (`next_len(0, ..).max(initial_len(..))`). Rationale: "memory bounds unchanged" (SPEC §6 M2 acceptance).
- `reset_with` (SPEC §6 M3 "optionally") → not added; only `Decompressor::reset(&mut self)`, which keeps format, options and decode path. Rationale: the bench and every current caller reuse the same format; an API can be added later without breaking this one.
- `reset()` semantics → after `reset()`, every observable (`decompress` results, `finish`, `total_in`, `total_out`, `trailing_start`, `gzip_headers`, `gzip_member_count`, `stats()`) equals that of `Decompressor::new(format, options)` built with the same path, for any later call sequence; allocations are kept. Rationale: SPEC "freshly-constructed state while keeping its allocations".
- Wrapper overhead (SPEC §1 pattern 4, "wrapper overhead in streaming mode") → a gated M2 task (P2-04) besides the checksum tasks. Rationale: SPEC says phase 3 addresses each pattern, measured first.
- Hot-loop scope (SPEC §6 M4) → only SPEC's list: branch-light refill, bounds-check elision (including keeping the lookup and bit buffer in the loop's locals), table width re-tune and optional multi-literal entries. Phase 2's other next steps (length/distance payload in entries, a short-match copy path) are not planned (Spec issues). Rationale: SPEC says "only where the M1/M4 profiles point" and lists the techniques.
- Bounds-check evidence (SPEC §4.3) → `make asm` (`cargo rustc --release -p cleanflate --lib -- --emit asm`) plus `tools/perf/asm_branches.py`, which counts panic/bounds-failure call sites and conditional branches in the fast-loop functions (P4-01). No cargo-asm. Rationale: SPEC §4.3.
- Targets (SPEC §4.1) → computed mechanically by `oracle bench-targets` (P0-08). T2's "row" is an aggregate key (corpus/framing/level), with per-file exceptions listed as information. T4's "within 3%" is (raw − framed) / raw ≤ 3% of cleanflate median MiB/s per corpus, level and mode. T6 is `bench-compare` of `docs/perf/baselines/phase2.json` against the run with the phase-3 noise: no LOSS aggregate and no per-file REGRESSION. Performance targets are recorded, never blocking; a correctness failure blocks with a fixture (PT5). Rationale: SPEC §4.1 "report actuals whether met or not".
- M4 "a WIN verdict" on level-1 rows → `bench-compare` of the M3 acceptance JSON (`docs/perf/phase3/P3-05.json`) against the M4 acceptance JSON, level-1 keys, full-bench noise; recorded MET/NOT MET, not blocking. Rationale: as above.
- File names → flamegraphs and per-key profile directories use the slug `<corpus>-<rfc195x>-l<level>-<mode>`, never `zlib`/`gzip` framing names (C11, and the guard denies read-family commands naming a forbidden word). Rationale: provenance tooling.
- Phase scoping of reports → `PHASE_BASE ?= cd4a472` (the phase-3 SPEC commit) in P5-03, as SPEC §6 M5 says. Phase-3 task IDs collide with earlier phases' IDs in history; the range scoping handles that.
- DESIGN numbering → every new entry takes the next free `D-NN` (D-34 at plan time). PLAN never names a number except the design-check threshold 34.
- Tagging → the implementer never tags. A human tags `v0.3.0-cleanroom` after review and merge (SPEC §6 M5).

## Conventions
- Every task is one commit. The subject is `<ID>: <title>`; the body follows CLAUDE.md `## Commit template`, including `[stage:implement] [task:<ID>]`, `Defects-caught:` and, for performance tasks, `Measurement:`.
- The implementer reads CLAUDE.md, the task and the SPEC sections it cites. `C1`–`C25`, `CR1`–`CR8` and `PT1`–`PT9` refer to CLAUDE.md.
- "Global invariants" in a task means: C1–C8, C15, C17, C23 and C24 and SPEC §3 apply to every file under `crates/cleanflate/` the task touches. That is: no `unsafe` token, no panicking calls or `[]` indexing in library code, zero dependencies, no clocks/IO/global state in `src/`, tunables only in `config.rs`, checked arithmetic on hostile values, no SIMD/arch/nightly, no benchmark-specific paths, no uninitialised-memory tricks, and no behaviour change (output, accept/reject, `Error` values, streaming contract).
- cleanflate unit tests go in `crates/cleanflate/src/tests/<name>.rs`, declared in `src/tests/mod.rs`. Tests needing the oracle, proptest, corpora or `test_hooks` go in `crates/oracle/tests/`. `stats`-feature tests are gated `#[cfg(feature = "stats")]`.
- Test names in Acceptance tests are `file::function`. All must exist and pass.
- Keys: a bench aggregate key is `<corpus>/<framing>/l<level>/<mode>` with framing `raw|zlib|gzip` (as in the JSON). `PROFILE_KEY` accepts the same shape and also `rfc1951|rfc1950|rfc1952` for the framing; use the rfc forms in shell commands. The file-name slug is `<corpus>-<rfc195x>-l<level>-<mode>`.
- Phase-3 JSONs go in `docs/perf/phase3/` (never `docs/perf/<ID>.json`, where phase-2 files with the same ids live).
- Baselines and noise: `docs/perf/baselines/phase2.json` (phase-2 numbers of record), `docs/perf/baselines/phase3-start.json` (+ `-rerun`), `docs/perf/baselines/phase3-noise.json`, and the quick trio `phase3-start-quick.json`, `phase3-start-quick-rerun.json`, `phase3-noise-quick.json`. `bench-compare` picks the phase-3 noise file by kind (P0-07).
- `make bench`, `make bench-quick`, `make heavy` and `make fuzz-milestone` exceed or approach the 10-minute foreground limit: run them in the background and wait (PT8). Never run two of them at once.
- A phase-end task writes `Manual check: NOT VERIFIED (human)` with its list in the progress log and never waits.

## Phase 0 — M0 Bench hygiene

### P0-01: Phase-3 scaffolding: fuzz gates, quick-bench levels, baseline copy, report skeleton
**Goal:** Add the Make targets the phase-3 correctness gates use, widen `bench-quick` to levels 1 and 6, store the phase-2 baseline, and create the phase-3 report skeleton.
**Files touched:** `Makefile`, `tools/perf/fuzz-milestone.sh` (new), `tools/perf/tests/fuzz_milestone_test.sh` (new), `tools/perf/tests/fixtures/` (new, small log fixtures), `docs/perf/baselines/phase2.json` (new), `docs/perf/baselines/README.md` (new), `docs/perf/PHASE3.md` (new), `docs/perf/flamegraphs/README.md` (new), `docs/perf/phase3/README.md` (new), `docs/perf/README.md` (a short "Phase 3 layout" section).
**Design constraints:** SPEC §4.2 (baseline copy), §5 (per-task fuzz smoke of `never_panic_*` and `diff_fast_reference`; per-milestone ≥ 30-minute runs of `diff_oracle_roundtrip` and `diff_fast_reference`), §8 (a shortened run records its real duration), C10, C11, C19, CR3 (never install nightly or cargo-fuzz; both are present).
- `Makefile`:
  - `FUZZ_DECODER_TARGETS := never_panic_raw never_panic_rfc1950 never_panic_rfc1952 diff_fast_reference` and `fuzz-smoke-decoder: $(addprefix fuzz-smoke-,$(FUZZ_DECODER_TARGETS))`. `FUZZ_SECONDS` keeps its default of 60. Add to `.PHONY`.
  - `FUZZ_MILESTONE_SECONDS ?= 1800` and `fuzz-milestone`: `bash tools/perf/fuzz-milestone.sh $(FUZZ_MILESTONE_SECONDS) diff_oracle_roundtrip diff_fast_reference`.
  - `bench-quick`: `--levels 1,6` (was `6`). Nothing else in the bench recipes changes.
  - `tools-check`: also run every `tools/perf/tests/*_test.sh` and `python3 -m unittest discover -s tools/perf/tests -p 'test_*.py'` when that directory has Python tests.
- `tools/perf/fuzz-milestone.sh SECONDS TARGET...` (shellcheck-clean):
  - Starts each target concurrently: `cargo +nightly fuzz run <t> fuzz/corpus/<t> fuzz/seeds/<t> -- -max_total_time=SECONDS -rss_limit_mb=2048`, full output to `target/fuzz-logs/<t>.log`. Creates the corpus directory first, as `fuzz-smoke-%` does.
  - Waits for all. For each target prints `fuzz-milestone: <t> seconds=<wall> execs=<n> exit=<code> artifacts=<count>`, where `execs` is the largest `#<n>` libFuzzer status count in the log and `artifacts` counts files created under `fuzz/artifacts/<t>/` during the run.
  - Exits non-zero if any target exited non-zero or created an artifact.
  - A `--summarize LOG` mode prints the `execs` value of one log, so the parsing is testable without fuzzing.
- `docs/perf/baselines/phase2.json`: a byte-identical copy of `docs/perf/phase2-final-operator.json`. `docs/perf/baselines/README.md` says what each baselines file is (phase2.json: the operator's post-merge phase-2 run, git `e16ac6f`, the phase-2 numbers of record; the phase-3 files arrive in P0-09) and that none is hand-edited.
- `docs/perf/PHASE3.md` skeleton with these headings, each with a one-line "filled by <task>" note: `# cleanflate phase 3 performance report`, `## Method` (P0-09), `## Machine and conditions` (P0-09), `## Baselines` (P0-09), `## Profile` (P1-02, pointer to `docs/perf/phase3-profile.md`), `## Task log` (every task appends `### <ID>`), `## Targets` (acceptance tasks, P5-02), `## Attribution` (P5-02), `## Tried and reverted` (P5-02).
- `docs/perf/flamegraphs/README.md`: files are `<slug>-<before|after>.svg`, written only by `make flamegraph-save` (P1-01), from the cleanflate-only `profile_decode` example.
- `docs/perf/phase3/README.md`: every phase-3 JSON lives here, named by task id, because phase-2 JSONs with the same ids sit in `docs/perf/`. Written only by the tools, never hand-edited.
- `docs/perf/README.md` "Phase 3 layout": `baselines/`, `phase3/`, `flamegraphs/`, `PHASE3.md`, `phase3-profile.md`, one line each.
**Acceptance tests:**
- `tools/perf/tests/fuzz_milestone_test.sh`: `summarize_reads_largest_exec_count` (fixture log with several `#n` lines), `summarize_empty_log_is_zero`.
- `make tools-check` passes (shellcheck on the new script).
- `cmp docs/perf/baselines/phase2.json docs/perf/phase2-final-operator.json` succeeds.
- `make -n fuzz-smoke-decoder` lists exactly the four `cargo +nightly fuzz run` commands.
- `make fuzz-milestone FUZZ_MILESTONE_SECONDS=20` exits 0 and prints two summary lines; paste them into the progress log.
**Out of scope:** Bench code. A real 30-minute fuzz run (P0-10).
**Verification:** `make tools-check && make -n fuzz-smoke-decoder && make fuzz-milestone FUZZ_MILESTONE_SECONDS=20`.
**Depends on:** none

### P0-02: design-check requires profile evidence in phase-3 [perf] entries
**Goal:** Make `make design-check` reject a phase-3 `[perf]` DESIGN entry that does not cite its profile evidence.
**Files touched:** `tools/ci/design-check.sh`, `tools/ci/tests/design_check_test.sh`, `docs/DESIGN.md` ("How to read this document" section only).
**Design constraints:** SPEC §2 ("each write-up cites the profile evidence that motivated it"), §9, CLAUDE.md PT7, C14.
- A `### D-<n>: [perf]` block with numeric `n >= 34` must contain a line starting `Profile:`, in addition to the existing `RFC:`, `Measurement:` and clean-room-phrase rules. The threshold is one named variable in the script (`PROFILE_REQUIRED_FROM=34`, commented "first phase-3 decision number").
- The failure message names the D-id and `Profile:`.
- Blocks D-01..D-33 are checked exactly as today and must still pass.
- DESIGN.md "How to read this document": one paragraph saying phase-3 `[perf]` entries (D-34 onward) add a `Profile:` line naming the hotspot key, the frames and their share before and after, citing `docs/perf/phase3-profile.md` or a committed flamegraph.
**Acceptance tests:** `tools/ci/tests/design_check_test.sh` cases `case_phase3_perf_without_profile_fails`, `case_phase3_perf_with_profile_passes`, `case_phase2_perf_without_profile_passes`; all existing cases still pass.
**Out of scope:** Adding any DESIGN entry.
**Verification:** `make tools-check && make design-check`.
**Depends on:** none

### P0-03: Oracle FFI: C zlib build flags and bounded streaming inflate
**Goal:** Give the oracle the two C zlib capabilities phase 3 needs: reading the library's build flags and timing its streaming path with bounded input and output windows.
**Files touched:** `crates/oracle/src/ffi.rs`, `crates/oracle/tests/ffi_roundtrip.rs`.
**Design constraints:** SPEC §2 (only the phase-1 P1-01 signatures plus `zlibCompileFlags() -> uLong`; streaming uses `inflateInit2_`/`inflate`/`inflateEnd` with bounded `avail_in`/`avail_out`), §4.2, §10. CR5, C9, C22 (rule `oracle-ffi-allowlist`).
- `pub fn zlib_compile_flags() -> u64`, calling `zlibCompileFlags()`. If libz-sys does not export it (a compile error says so), declare it in an `extern "C"` block in `ffi.rs` with that signature. Never read libz-sys sources to find out (CR2, CR5).
- `pub fn inflate_streaming_into(input: &[u8], framing: Framing, in_chunk: usize, out: &mut [u8], out_chunk: usize) -> Result<usize, OracleStatus>`:
  - One `inflateInit2_` per call, with the window bits `inflate_bytes` uses (raw −15, zlib 15, gzip 31).
  - Loop: offer `avail_in = min(in_chunk, remaining input)` and the next window of `out` of at most `out_chunk` bytes, call `inflate(Z_NO_FLUSH)`, advance by what was consumed and produced.
  - `Z_STREAM_END` returns `Ok(total produced)`. Input exhausted with no progress returns `Err(OracleStatus::Truncated)`. `out` exhausted before the end returns `Err(OracleStatus::OutputLimit)`. Other codes map as in `inflate_bytes`.
  - Always call `inflateEnd`. No heap allocation inside the loop (the bench times this function). Gzip stops at the end of the first member, like `inflate_bytes`.
  - `in_chunk` or `out_chunk` of 0 is an error, not a hang.
- No other new C symbol.
**Acceptance tests:** `crates/oracle/tests/ffi_roundtrip.rs`:
- `compile_flags_stable`: two calls return the same value, formatted as hex without panicking.
- `streaming_equals_oneshot_all_framings`: 256 KiB of mixed text-like and random data at levels 1, 6 and 9, raw/zlib/gzip, with (in, out) chunk pairs (1, 1), (7, 13) and (65536, 65536). The output equals the input and equals `inflate_bytes`'s output.
- `streaming_truncated_is_error`, `streaming_output_too_small_is_error`, `streaming_zero_chunk_is_error`.
**Out of scope:** Bench changes (P0-05, P0-06).
**Verification:** `make lint && cargo test -p oracle --test ffi_roundtrip`.
**Depends on:** none

### P0-04: Bench schema 2: "best" replaces "min", every column documented
**Goal:** Rename the bench's "min" figures to "best" in every output and JSON field, keep phase-2 JSONs loadable, and document every column and definition.
**Files touched:** `crates/oracle/src/bench.rs`, `crates/oracle/src/bench_json.rs`, `crates/oracle/src/bench_compare.rs`, `docs/perf/README.md`.
**Design constraints:** SPEC §4.2 bullet 1 (rename; document "median", "best" and every column). C10, C13 (a missing or unreadable JSON is an error that says how to fix it).
- JSON `schema: 2`. Field renames: `results[].min_ns` → `best_ns`; `aggregates[].cleanflate_mib_s_min` → `cleanflate_mib_s_best`; `aggregates[].c_zlib_mib_s_min` → `c_zlib_mib_s_best`. The old names are `#[serde(alias = ...)]` so schema-1 files load. Writers always write schema 2. The loader accepts schema 1 and 2 and rejects any other value with a message.
- Rename the matching Rust fields. No printed header, column or footer of `bench`, `bench-compare`, `bench-md` or `bench-noise` contains "min" any more; use "best".
- Definitions (docs/perf/README.md, plus a two-line footer under the bench's stdout table):
  - median: throughput from the median per-decode time of the N samples;
  - best: throughput of the fastest sample (minimum time);
  - aggregate median/best: total bytes / sum of per-file median/best times;
  - ratio: cleanflate median / C zlib median.
- `docs/perf/README.md`: a `## Schema 2` section documenting every JSON field and every column of every command's output. Keep `## Schema 1`, retitled "(read-only compatibility)".
**Acceptance tests:** unit tests in `crates/oracle/src/bench_json.rs` and `bench_compare.rs`:
- `schema1_file_loads_with_best_fields` (loads `docs/perf/baselines/phase2.json` via `CARGO_MANIFEST_DIR`, and checks a best value is present);
- `schema2_roundtrip`;
- `unknown_schema_rejected`;
- `no_min_in_rendered_headers` (the markdown, comparison and stdout renderings contain no "min" column);
- `median_and_best_odd_even` (renamed from `median_and_min_odd_even`).
- Existing tests pass.
**Out of scope:** New measurements (P0-05, P0-06). Column additions (P0-07).
**Verification:** `make lint && make test`.
**Depends on:** P0-01

### P0-05: C zlib streaming rows and like-for-like ratios
**Goal:** Time C zlib's streaming path with the same 64 KiB input and output chunks as cleanflate's streaming mode, and make every aggregate's ratio compare like with like.
**Files touched:** `crates/oracle/src/bench.rs`, `crates/oracle/src/bench_json.rs`, `docs/perf/README.md`.
**Design constraints:** SPEC §4.1 ("like for like"), §4.2 bullet 2 (keep the one-shot rows as they are), §2 (C zlib is a stopwatch only), §10. C9, C22.
- New timed implementation, C zlib streaming: `oracle::ffi::inflate_streaming_into(compressed, framing, 65536, scratch, 65536)` into the same pre-sized scratch buffer cleanflate streaming uses (`original.len() + 65536`).
- Its warm-up checks the output is byte-identical to the original and fails loudly otherwise.
- Within each sample index, run the selected implementations back to back in this order: cleanflate one-shot, cleanflate streaming, C zlib one-shot, C zlib streaming. `reps` stays per implementation.
- C zlib streaming is timed only when mode `streaming` is selected, and C zlib one-shot only when `oneshot` is.
- `results[]` gains rows with `impl: "c_zlib", mode: "streaming"`.
- `aggregates[]`:
  - `c_zlib_mib_s_median`/`c_zlib_mib_s_best` come from C zlib rows of the **same mode**.
  - New field `c_zlib_mode` (the mode used for the denominator); schema-1 aggregates default to `"oneshot"`.
  - `ratio` = cleanflate median / C zlib median of the same mode.
- The stdout table labels the ratio "(like for like)".
**Acceptance tests:** unit tests in `crates/oracle/src/bench.rs`:
- `aggregate_ratio_is_like_for_like` (synthetic rows: the streaming aggregate divides by C zlib streaming);
- `schema1_aggregate_defaults_to_oneshot_denominator`;
- `sample_order_includes_c_zlib_streaming` (a pure function returns the per-sample order for a set of modes).
- `make bench-quick BENCH_ID=P0-05` (background) completes, and its JSON (under `target/bench/`, not committed) has a C zlib streaming row for every cleanflate streaming row.
**Out of scope:** Environment capture (P0-06). Report columns (P0-07).
**Verification:** `make lint && make test && make bench-quick BENCH_ID=P0-05`.
**Depends on:** P0-03, P0-04

### P0-06: Environment capture in every bench run
**Goal:** Record the machine, toolchain, build profile, C library build and git state with every bench run, in the JSON and at the top of stdout.
**Files touched:** `crates/oracle/src/bench_env.rs` (new), `crates/oracle/src/bench.rs`, `crates/oracle/src/bench_json.rs`, `crates/oracle/src/lib.rs`, `docs/perf/README.md`.
**Design constraints:** SPEC §4.2 "Environment" (all four bullets), §2 (reading the C library's version and build flags only). C9, C22.
- `machine` keeps its fields and adds:
  - `cores`: logical CPUs, from `std::thread::available_parallelism`;
  - `os`: `PRETTY_NAME` from `/etc/os-release`, else `unknown`;
  - `kernel`: `uname -r`;
  - `loadavg_end`: the load average after the run.
- `cargo_profile`:
  - `profile: "release"` (the bench already refuses debug builds);
  - `debug_assertions`: `cfg!(debug_assertions)`;
  - `release_profile_toml`: the `[profile.release]` table of the workspace `Cargo.toml`, verbatim, read at run time;
  - `env`: every `CARGO_PROFILE_*`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS` and `CARGO_BUILD_RUSTFLAGS` variable present at run time.
- `c_zlib`:
  - `version`: the existing `c_zlib_version` value, which also stays at top level;
  - `compile_flags`: `format!("0x{:x}", zlib_compile_flags())`;
  - `libz_sys_version` and `libz_sys_features`: from `cargo metadata --format-version 1`, run once at bench start. The version comes from the package list; the features from the resolve node of that package. On any failure, record `unknown` and print a warning, never abort.
- `git_head` and `git_dirty` stay.
- All new fields use `#[serde(default)]`, so older files load.
- Stdout starts with an `Environment` block listing every captured value.
**Acceptance tests:** unit tests in `crates/oracle/src/bench_env.rs`:
- `env_json_roundtrip`;
- `release_profile_section_extracted` (from an inline Cargo.toml string with several tables);
- `metadata_version_and_features_parsed` (from a small inline JSON shaped like `cargo metadata` output);
- `compile_flags_hex_format`;
- `missing_os_release_is_unknown`.
- `make bench-quick BENCH_ID=P0-06` (background) prints the Environment block. Paste it into the progress log; `compile_flags` must not be `unknown`.
**Out of scope:** Report columns (P0-07).
**Verification:** `make lint && make test && make bench-quick BENCH_ID=P0-06`.
**Depends on:** P0-03, P0-04

### P0-07: Reports: delta vs the phase-2 baseline, drift-corrected verdicts, per-file sizes
**Goal:** Make every comparison and markdown row show the delta against the phase-2 baseline and the like-for-like ratio, add a drift-corrected ratio verdict, and report files with their sizes.
**Files touched:** `crates/oracle/src/bench_compare.rs`, `crates/oracle/src/main.rs`, `Makefile`, `docs/perf/README.md`.
**Design constraints:** SPEC §4.2 "Delta columns" and "Canterbury per-file reporting", §4.1 T6. Decisions entries on drift and noise. C13.
- `oracle bench-compare A B [--noise P] [--baseline PATH] [--fail-on-regression]`. `--baseline` defaults to `docs/perf/baselines/phase2.json`; a missing baseline file prints `baseline missing: <path>` and its columns print `n/a`. Per aggregate key, it prints:
  - A MiB/s, B MiB/s, delta %, 2x noise %, and the verdict, with the existing rule;
  - `vs phase-2 %`: B median against the baseline median of the same key;
  - `B ratio`: like for like;
  - `ratio delta %` = (B ratio / A ratio − 1) × 100, and a `ratio verdict` (WIN/LOSS/NEUTRAL by the same 2x rule against the ratio noise, `UNKNOWN` without it). Where A and B used different `c_zlib_mode` for a key (a schema-1 streaming row), print `n/a (denominator changed)`.
  - Per-file REGRESSION lines are unchanged.
- `oracle bench-noise` also writes `ratio_aggregate: {key: pct}` and `ratio_aggregate_max_pct` (|a − b| / a of the like-for-like ratio).
- Default noise file, for two JSONs of the same kind:
  - `full`: `docs/perf/baselines/phase3-noise.json`, else `docs/perf/noise.json` (printing "using phase-2 noise");
  - `quick`: `docs/perf/baselines/phase3-noise-quick.json`, else `docs/perf/noise-quick.json`;
  - otherwise, or for mixed kinds: none, and verdicts `UNKNOWN`.
- `oracle bench-md J [--files] [--filter PREFIX] [--baseline PATH]`:
  - Aggregate columns: key, cleanflate MiB/s (median), cleanflate MiB/s (best), C zlib MiB/s (median, same mode), ratio (like for like), vs phase-2 %.
  - `--files` rows add `bytes`, sort by corpus and then by bytes ascending, and carry the per-file like-for-like ratio and vs-phase-2 %.
- `Makefile`: `bench-md` passes `$(if $(FILES),--files) $(if $(FILTER),--filter $(FILTER)) $(if $(BASELINE),--baseline $(BASELINE))`; `bench-compare` passes `$(if $(BASELINE),--baseline $(BASELINE))`.
**Acceptance tests:** unit tests in `crates/oracle/src/bench_compare.rs`:
- `delta_vs_baseline_column`;
- `ratio_delta_and_verdict`;
- `denominator_change_marked`;
- `per_file_rows_sorted_by_size_with_bytes`;
- `noise_includes_ratio`;
- `default_noise_prefers_phase3_then_phase2` (with a temporary directory as root);
- `missing_baseline_prints_na`.
- All existing tests pass.
**Out of scope:** The T1–T6 evaluator (P0-08).
**Verification:** `make lint && make test`, then `make bench-compare A=docs/perf/P5-02.json B=docs/perf/baselines/phase2.json` prints the new columns without error.
**Depends on:** P0-05

### P0-08: `oracle bench-targets`: T1–T6 verdicts from a bench JSON
**Goal:** Evaluate SPEC §4.1 targets T1–T6 mechanically from one bench JSON, so every acceptance task and the final report use the same numbers.
**Files touched:** `crates/oracle/src/bench_targets.rs` (new), `crates/oracle/src/main.rs`, `crates/oracle/src/lib.rs`, `Makefile`, `docs/perf/README.md`.
**Design constraints:** SPEC §4.1 (T1–T6, "report actuals whether met or not"). Decisions entry on targets.
- `oracle bench-targets J [--baseline PATH] [--noise PATH]`, and `make bench-targets J=...` (optional `BASELINE=`, `NOISE=`). The baseline defaults to `docs/perf/baselines/phase2.json`; the noise to the P0-07 default for full runs.
- Targets, over J's aggregates:
  - T1: every `silesia/{raw,zlib,gzip}/l{1,6,9}/streaming` ratio ≥ 0.80.
  - T2: for every corpus, framing and level, cleanflate one-shot median ≥ cleanflate streaming median. Per-file cases where one-shot is slower are listed as information only.
  - T3: every `canterbury/*/*/oneshot` and `canterbury/*/*/streaming` ratio ≥ 0.70. Reuse rows are excluded, having no like-for-like denominator.
  - T4: for zlib and for gzip, (raw − framed) / raw ≤ 3%, with raw and framed the cleanflate median MiB/s of the same corpus, level and mode.
  - T5: `silesia/{raw,zlib,gzip}/l6/oneshot` ratio ≥ 0.70.
  - T6: the P0-07 comparison of the baseline against J with the noise file. MET iff it has no aggregate LOSS and no per-file REGRESSION.
- Output is markdown for PHASE3.md: one line per target with MET or NOT MET, the worst row and its value; then every failing row.
- A streaming aggregate whose `c_zlib_mode` is not `streaming` (schema 1) counts as `n/a (not like for like)`.
- Exit code 0 unless J is unreadable: targets are reported, not gates.
**Acceptance tests:** unit tests in `crates/oracle/src/bench_targets.rs`, each on synthetic JSON:
- `t1_streaming_threshold`, `t2_oneshot_vs_streaming`, `t3_canterbury_all_modes`, `t4_framing_overhead`, `t5_level6_oneshot`, `t6_uses_compare`;
- `schema1_streaming_is_na`;
- `reuse_rows_ignored`.
**Out of scope:** Measuring (P0-09).
**Verification:** `make lint && make test && make bench-targets J=docs/perf/baselines/phase2.json`.
**Depends on:** P0-07

### P0-09: Phase-3 starting baseline and noise
**Goal:** Measure the phase-3 starting point twice with the full bench and twice with the quick bench, commit the JSONs and noise files, and write the method, machine and baseline sections of PHASE3.md.
**Files touched:**
- `docs/perf/baselines/`: `phase3-start.json`, `phase3-start-rerun.json`, `phase3-noise.json`, `phase3-start-quick.json`, `phase3-start-quick-rerun.json`, `phase3-noise-quick.json`, `README.md`.
- `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §4.2 ("Baseline", "Delta columns", "Environment", "Canterbury per-file reporting"), §4.1, §8, C10. No code changes. PT8: background runs, one at a time, nothing else heavy running.
- Steps:
  1. `make bench BENCH_ID=phase3-start BENCH_OUT=docs/perf/baselines/phase3-start.json`, then the same into `phase3-start-rerun.json`.
  2. `make bench-noise A=docs/perf/baselines/phase3-start.json B=docs/perf/baselines/phase3-start-rerun.json OUT=docs/perf/baselines/phase3-noise.json`.
  3. The same with `make bench-quick` into the quick pair, then `phase3-noise-quick.json`.
- PHASE3.md:
  - `## Method`: modes and like-for-like denominators, median and best, sampling and interleaving, environment capture, the WIN/LOSS/REGRESSION and ratio-verdict rules, cleanflate-only profiling.
  - `## Machine and conditions`: from the run's Environment block.
  - `## Baselines`:
    - `make bench-md` tables for all aggregates of `phase3-start.json`;
    - Canterbury per-file with sizes (`make bench-md J=... FILES=1 FILTER=canterbury/`);
    - the noise tables (throughput and ratio, full and quick);
    - `make bench-compare A=docs/perf/baselines/phase2.json B=docs/perf/baselines/phase3-start.json` (the delta from the phase-2 record);
    - `make bench-targets J=docs/perf/baselines/phase3-start.json` as "Targets at start".
- If `aggregate_max_pct` exceeds 3% in either noise file, say so prominently and add it to this phase's manual checks. Continue anyway.
**Acceptance tests:**
- The six JSONs exist, have `schema: 2`, kinds `full`/`quick` and `stats_build: false`, and every streaming aggregate has `c_zlib_mode: "streaming"`.
- `make bench-compare A=docs/perf/baselines/phase3-start.json B=docs/perf/baselines/phase3-start-rerun.json` shows no WIN, LOSS or REGRESSION (a sanity check of the noise definition). Record any exception.
**Out of scope:** Code changes. Profiling (P1-02).
**Verification:** `make bench-md J=docs/perf/baselines/phase3-start.json && make bench-targets J=docs/perf/baselines/phase3-start.json`.
**Depends on:** P0-06, P0-08

### P0-10: M0 acceptance and push
**Goal:** Run the M0 acceptance gates, commit the M0 bench, snapshot provenance, push, and list the manual checks.
**Files touched:** `docs/perf/phase3/P0-10.json`, `docs/perf/PHASE3.md` (`### P0-10 (M0 acceptance)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 0 files.
**Design constraints:** SPEC §6 intro (acceptance: §5 milestone gates, full bench, `bench-compare` against the phase-3 start and phase-2 baselines, push), §6 M0 acceptance, §5 milestone gates, §8. PT5, PT8, PT9.
- Follow PT9: `make ci`; `make heavy`; `make fuzz-milestone`; then `make bench BENCH_ID=P0-10 BENCH_OUT=docs/perf/phase3/P0-10.json`; `make bench-compare` against `docs/perf/baselines/phase3-start.json` and against `docs/perf/baselines/phase2.json`; `make bench-targets J=docs/perf/phase3/P0-10.json`.
- PHASE3.md M0 checklist with evidence:
  - the phase-3 start baseline and noise are committed (paths);
  - every streaming row has a C zlib streaming counterpart (row counts from the JSON);
  - `make bench-compare` shows deltas against the phase-2 baseline (paste the header and three rows);
  - the fuzz durations and exec counts.
- `make provenance-snapshot`, commit, `git push -u origin HEAD`.
**Acceptance tests:** `make ci`, `make heavy` and `make fuzz-milestone` pass. `docs/perf/phase3/P0-10.json` is committed.
**Out of scope:** Optimisation.
**Verification:** `make ci && make heavy && make fuzz-milestone && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm the GitHub Actions `ci` run is green on the pushed branch.
2. If the P0-09 noise exceeds 3%, re-run the baseline pair on a quiet machine with a pinned governor.
3. Spot-check the Environment block (`c_zlib.compile_flags`, `libz_sys_features`) against the build you expect.
4. Confirm the provenance log has no unexplained denials.
**Depends on:** P0-09

## Phase 1 — M1 Profile first

### P1-01: Profile any key: per-key outputs, perf stat and saved flamegraphs
**Goal:** Let `make profile`, `make flamegraph` and a new `make profile-stat` target any (corpus, framing, level, mode) key, keep each key's outputs apart, and save checked, cleanflate-only flamegraphs into `docs/perf/flamegraphs/`.
**Files touched:** `Makefile`, `crates/cleanflate/examples/profile_decode.rs` (only if its streaming windows differ from the bench's), `tools/perf/check-flamegraph.sh` (new), `tools/perf/tests/check_flamegraph_test.sh` (new), `tools/perf/tests/fixtures/*.svg` (new, tiny), `docs/perf/README.md` (profiling section), `docs/perf/flamegraphs/README.md`.
**Design constraints:** SPEC §4.3 (all bullets), §2 (profile only cleanflate-only processes; CR7), C19 (rule `no-profiling-of-oracle`), C25, C11, C1 (no `unsafe` token in the example). Create SVG fixtures with the Write tool, not shell heredocs.
- `Makefile`:
  - `PROFILE_KEY ?=`. When set to `<corpus>/<framing>/l<level>/<mode>` (framing `raw|zlib|gzip|rfc1951|rfc1950|rfc1952`; mode `oneshot|streaming`), derive `PROFILE_CORPUS`, `PROFILE_FRAMING` (always the rfc form), `PROFILE_LEVEL` and `PROFILE_MODE` from it with Make functions. Without it, the existing variables work as before.
  - `PROFILE_SLUG := $(PROFILE_CORPUS)-$(PROFILE_FRAMING)-l$(PROFILE_LEVEL)-$(PROFILE_MODE)`; outputs go to `target/profile/$(PROFILE_SLUG)/`:
    - `perf.data`;
    - `top.txt` (self time, as today);
    - `top-children.txt` (`--children`, inclusive);
    - `flamegraph.svg`.
  - `profile-stat`: `perf stat -e task-clock,page-faults,cycles:u,instructions:u,branches:u,branch-misses:u -o target/profile/$(PROFILE_SLUG)/stat.txt` on `target/release/examples/profile_decode` with the key's arguments. An event the machine lacks prints `<not supported>`, which is acceptable.
  - `flamegraph-save`: requires `TAG`.
    - It runs the flamegraph for the key, then `bash tools/perf/check-flamegraph.sh target/profile/$(PROFILE_SLUG)/flamegraph.svg`.
    - Then it copies the SVG to `docs/perf/flamegraphs/$(PROFILE_SLUG)-$(TAG).svg`.
  - `profile-required`: requires `TAG`. For each SPEC §4.3 key (`silesia/rfc1951/l1/oneshot`, `silesia/rfc1951/l1/streaming`, `canterbury/rfc1952/l1/oneshot`), it runs `profile`, `profile-stat` and `flamegraph-save`. `PROFILE_ITERS` is 3 for Silesia and 300 for Canterbury (small files need repetitions for enough samples).
  - `bench-stats` gains `STATS_CORPUS ?= silesia`, `STATS_LEVELS ?= 6` and `STATS_MODES ?= oneshot,streaming`.
  - No recipe runs a profiler on anything but `profile_decode`.
- `tools/perf/check-flamegraph.sh FILE`:
  - Fails if the SVG has no frame starting `cleanflate::`.
  - Fails if it has any frame of the oracle crate or of the C library.
  - The word list lives inside the script (see rule `flamegraphs-cleanflate-only` in `docs/foundry.json`), so no command line has to name those words: the guard denies read-family commands that do.
- `profile_decode` streaming must use 64 KiB input chunks and 64 KiB output windows, exactly as the bench's streaming mode does. Align it if it does not.
**Acceptance tests:**
- `tools/perf/tests/check_flamegraph_test.sh`: `accepts_cleanflate_svg`, `rejects_svg_without_cleanflate_frames`, `rejects_svg_with_oracle_frame`.
- `make tools-check` passes.
- `make profile PROFILE_KEY=silesia/rfc1951/l1/streaming PROFILE_ITERS=1` writes `target/profile/silesia-rfc1951-l1-streaming/top.txt`, whose leading entries are cleanflate functions.
- `make profile-stat PROFILE_KEY=canterbury/rfc1952/l1/oneshot PROFILE_ITERS=50` writes `stat.txt`.
- `make flamegraph-save PROFILE_KEY=silesia/rfc1951/l6/oneshot PROFILE_ITERS=1 TAG=smoke` succeeds. Delete that SVG before committing.
- If `perf` fails for an environmental reason, record `profile NOT RUN: <reason>` and block. Never install or reconfigure tools (CR3).
**Out of scope:** Committed flamegraphs and findings (P1-02). Reuse mode (P3-04).
**Verification:** `make tools-check && make profile PROFILE_KEY=silesia/rfc1951/l1/streaming PROFILE_ITERS=1 && make profile-stat PROFILE_KEY=canterbury/rfc1952/l1/oneshot PROFILE_ITERS=50`.
**Depends on:** P0-01

### P1-02: Before-profiles, flamegraphs and phase3-profile.md
**Goal:** Commit the before-flamegraphs for the three required keys, and write `docs/perf/phase3-profile.md`: the top hotspots per key, each SPEC §1 pattern confirmed, adjusted or discarded by evidence, and the hotspot and likely gate outcome of every later optimisation task.
**Files touched:** `docs/perf/phase3-profile.md` (new), `docs/perf/flamegraphs/silesia-rfc1951-l1-oneshot-before.svg`, `docs/perf/flamegraphs/silesia-rfc1951-l1-streaming-before.svg`, `docs/perf/flamegraphs/canterbury-rfc1952-l1-oneshot-before.svg`, `docs/perf/phase3/P1-02-stats.json`, `docs/perf/PHASE3.md` (`### P1-02`, a pointer).
**Design constraints:** SPEC §4.3, §6 M1, §1 (the four patterns), CR7, C10, C25. No code changes.
- Collect:
  - `make profile-required TAG=before`.
  - `make profile` for:
    - `silesia/rfc1951/l6/oneshot` and `silesia/rfc1951/l6/streaming` (pattern 1 at level 6);
    - `silesia/rfc1950/l6/streaming` and `silesia/rfc1952/l6/streaming` (pattern 4);
    - `canterbury/rfc1951/l1/oneshot` and `canterbury/rfc1952/l1/streaming` (pattern 2).
  - `make profile-stat` for the three required keys and `silesia/rfc1951/l6/oneshot`.
  - `make bench-stats BENCH_ID=P1-02 BENCH_OUT=docs/perf/phase3/P1-02-stats.json STATS_CORPUS=silesia,canterbury STATS_LEVELS=1,6`.
- `docs/perf/phase3-profile.md`:
  - `## How these were taken`: commands, iterations, perf flags, machine, and the fact that user-space sampling (`perf_event_paranoid = 2`) cannot see kernel page-fault time, so page faults come from `perf stat`.
  - `## Before (P1-02)`: per key, the top ≥ 10 self-time symbols with % of samples, the top inclusive frames, and the `perf stat` figures (page faults and instructions per MiB of output).
  - `## Patterns`: one row per SPEC §1 pattern, each CONFIRMED, ADJUSTED (the evidence points to another cause, named) or DISCARDED, with numbers:
    1. one-shot slower than streaming: fill/resize/grow/realloc/copy frames and page faults of one-shot against streaming; `oneshot_resizes` and `oneshot_bytes_allocated`;
    2. Canterbury setup cost: construction frames (`Inflater::new_with_path`, `Window::new`, `Huffman::from_lengths`, `build_table`, table clones, allocator and fill frames, `GzipDecoder::new`, the header parser) against per-file size;
    3. level-1 decode loop: `fast_loop::run`, `Huffman::decode_table`, `refill_bulk`, `peek`/`consume`, match copy; literal share (literals / (literals + matches)) from stats;
    4. zlib/gzip against raw: checksum frames against wrapper `step` self time.
  - `## Hotspots by task`: one row for each of P2-02, P2-03, P2-04, P2-07, P2-08, P3-01, P3-02, P4-02, P4-03 and P4-04. Each row gives the key(s), frames, current share, gate threshold max(2%, 2x the key's quick noise from `docs/perf/baselines/phase3-noise-quick.json`), the likely outcome (above or below the gate) and the ranking by measured share. PLAN order does not change; the gates decide.
  - `## After (P5-01)`: placeholder.
**Acceptance tests:**
- The three SVGs are committed and pass `tools/perf/check-flamegraph.sh`.
- phase3-profile.md has a verdict with numbers for all four patterns and a row for every task listed above.
- `docs/perf/phase3/P1-02-stats.json` is committed.
**Out of scope:** Code changes. Editing PLAN.md.
**Verification:** `make design-check` (sanity only; DESIGN.md is untouched), `bash tools/perf/check-flamegraph.sh docs/perf/flamegraphs/silesia-rfc1951-l1-oneshot-before.svg`.
**Depends on:** P1-01, P0-09

### P1-03: M1 acceptance and push
**Goal:** Run the M1 acceptance gates, commit the M1 bench, snapshot provenance and push.
**Files touched:** `docs/perf/phase3/P1-03.json`, `docs/perf/PHASE3.md` (`### P1-03 (M1 acceptance)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 1 files.
**Design constraints:** SPEC §6 M1 acceptance (flamegraphs and profile report committed; every later task names its hotspot), §5 milestone gates, PT5, PT8, PT9.
- PT9 with `BENCH_ID=P1-03`.
- PHASE3.md checklist: the SVG paths, phase3-profile.md sections, and confirmation that its hotspot table covers every gated or optimisation task of M2–M4.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci`, `make heavy` and `make fuzz-milestone` pass. `docs/perf/phase3/P1-03.json` is committed.
**Out of scope:** Optimisation.
**Verification:** `make ci && make heavy && make fuzz-milestone && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Open the three before-flamegraphs and sanity-check the phase3-profile.md tables against them.
3. Check that the SVGs show only cleanflate, std and libc frames.
**Depends on:** P1-02

## Phase 2 — M2 Checksums, then the one-shot output path

### P2-01: Checksum microbenchmark (cleanflate only)
**Goal:** Add a cleanflate-only checksum microbenchmark reporting MiB/s per algorithm and per implementation variant, and expose the variants through `test_hooks` without changing production checksums.
**Files touched:**
- `crates/cleanflate/src/`: `checksum.rs`, `test_hooks.rs`.
- `crates/cleanflate/Cargo.toml` (`[[example]] name = "checksum_bench"`, `required-features = ["test-hooks"]`), `crates/cleanflate/examples/checksum_bench.rs` (new).
- `crates/cleanflate/src/tests/checksum.rs`.
- `Makefile` (`bench-checksum`).
- `docs/perf/phase3/P2-01-checksum.json`, `docs/perf/phase3/P2-01-checksum-rerun.json`, `docs/perf/phase3/P2-01-before.json`, `docs/perf/phase3/P2-01.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M2 "Checksums" bullets 1–3, §3 (no behaviour change; no new stable API), RFC 1950 §2.2, RFC 1952 §8, D-10, D-23, D-32. Global invariants apply. C7 covers `src/` only, so the example may use `Instant` and `std::fs`. C18: `test-hooks` stays opt-in. No lazily-initialised statics: tables are `const`/`static` built by `const fn`.
- `checksum.rs`:
  - Generalise the sliced CRC-32 update to `pub(crate) fn crc32_update_sliced<const N: usize>(crc: u32, data: &[u8]) -> u32`, with `N`-table sets built by `const fn`.
  - The production `Crc32::update` calls it with `N = CRC32_SLICES`. Its output and speed are unchanged (PT1 below).
  - `N` must be at least 4. Enforce it without `assert!` (C4): for example, only 4, 8 and 16 are instantiated, through private helpers, and a unit test covers each.
  - Add `pub(crate) fn adler32_update_bytewise(adler: u32, data: &[u8]) -> u32` (modulo after every byte), the reference, if no such function exists yet.
- `test_hooks::checksum` module (rustdoc: benchmarking and tests only), with every function taking and returning the running value in the same convention as `Crc32`/`Adler32`:
  - `crc32_bytewise`, `crc32_sliced4`, `crc32_sliced8`, `crc32_sliced16`, `crc32_production`;
  - `adler32_bytewise`, `adler32_production`.
  - Later tasks add variants here.
- `examples/checksum_bench.rs`:
  - Arguments: `--sizes 64,4096,65536,1048576 --total-mib 256 --iters 7 --task ID --json PATH`.
  - Buffers are filled from a fixed-seed xorshift written in the example.
  - Per algorithm, check once that every variant gives the same value on every size (exit non-zero otherwise).
  - Then N samples per (variant, size), each processing at least `--total-mib`, reporting median and best MiB/s.
  - Stdout: a markdown table.
  - JSON, written by hand: `{schema: 1, kind: "checksum", task, git_head, git_dirty, timestamp, rows: [{algo, variant, size, median_mib_s, best_mib_s, iters}]}`. `git_head` comes from `git rev-parse` (the example may spawn processes; C7 covers `src/` only).
- `Makefile`: `bench-checksum: cargo run --release -q -p cleanflate --features test-hooks --example checksum_bench -- --task $(BENCH_ID) --json $(BENCH_OUT)`.
- Run `make bench-checksum` twice (`P2-01-checksum.json`, `-rerun.json`). Record both tables and the per-row difference, which is the microbench noise, in PHASE3.md `### P2-01`.
- PT1 on the production refactor (gzip keys): it must not be a LOSS; if it is, fix it before finishing.
**Acceptance tests:** `crates/cleanflate/src/tests/checksum.rs`:
- `crc32_sliced_variants_match_bytewise` (N = 4, 8 and 16; lengths 0..=64 plus random lengths to 4096; start offsets 0..=15; split updates);
- `adler32_bytewise_matches_production`.
- Existing checksum tests pass.
**Out of scope:** Changing any production checksum algorithm or tunable (P2-02, P2-03).
**Verification:** PT1, `make lint && make test && make bench-checksum BENCH_ID=P2-01 BENCH_OUT=docs/perf/phase3/P2-01-checksum.json`.
**Depends on:** P0-01, P0-09

### P2-02: Adler-32 over a bound-derived unrolled batch (gated)
**Goal:** Gated per PT2. If Adler-32 is a significant share of zlib decoding, speed up its inner loop with fixed-size sub-blocks whose size and accumulator bounds are derived from the `u32` limit, extending D-23.
**Files touched:**
- `crates/cleanflate/src/`: `checksum.rs`, `test_hooks.rs` (variant `adler32_batched`).
- `crates/cleanflate/src/tests/checksum.rs`, `crates/oracle/tests/checksums.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P2-02-before.json`, `docs/perf/phase3/P2-02.json`, `docs/perf/phase3/P2-02-checksum.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M2 "Checksums" (the Adler candidate; "only if the M1 profile shows checksum time behind the zlib/gzip gap"), §1 pattern 4, §4.1 T4. RFC 1950 §2.2, §8.2. D-23. Global invariants apply. PT1–PT7. C16: oracle values come from `oracle::ffi::adler32`.
- **Hotspot and gate:**
  - Frames: `Adler32::update`, or its inlined frame inside `ZlibDecoder::step`.
  - Keys: `silesia/rfc1950/l6/streaming` and `canterbury/rfc1950/l1/oneshot` (phase3-profile.md, or a fresh `make profile PROFILE_KEY=...`).
  - Below max(2%, 2x the quick noise of `silesia/zlib/l6/streaming`) on both keys: negative result, no code change.
- **Technique:** inside each `ADLER_NMAX` chunk, process `chunks_exact(B)` sub-blocks.
  - Accumulate `s1` and the weighted `s2` contribution so the compiler can unroll or auto-vectorise. For example, `s2 += B * s1_before + Σ (B − i) · x_i` over the block, or several independent partial sums.
  - Reduce modulo 65521 once per chunk, as now. The tail uses the existing loop.
  - `B` and any changed chunk bound are derived constants in `checksum.rs`, with a doc comment saying they are derived bounds, not tunables.
  - The derivation, from the worst case (all bytes 0xFF, both sums at 65520 when a chunk starts, every intermediate below 2^32), goes in a new `[perf]` DESIGN entry that says it extends D-23: RFC, Decision, Rationale, the PT7 phrase, `Profile:` and `Measurement:`.
- Add `adler32_batched` to `test_hooks::checksum`. Commit a `make bench-checksum` run as `P2-02-checksum.json` comparing it with `adler32_production` from before the change.
- PT1 (zlib keys, levels 1 and 6), PT4. In PHASE3.md, record T4's zlib rows before and after (`make bench-targets` on both quick JSONs).
**Acceptance tests:**
- `crates/cleanflate/src/tests/checksum.rs`:
  - `adler32_batch_bound_worst_case`;
  - `adler32_batched_matches_bytewise` (lengths 0..=20000, including multiples of `ADLER_NMAX` ± 1 and of `B` ± 1, and split updates);
  - `adler32_bound_worst_case` still passes.
- `crates/oracle/tests/checksums.rs::adler32_matches_oracle_long`: random buffers up to 1 MiB, updated in random pieces, against `oracle::ffi::adler32` of the whole buffer.
- If gated out, the PHASE3.md entry with the numbers is the deliverable.
**Out of scope:** CRC-32 (P2-03). Wrapper code (P2-04).
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k && make design-check`.
**Depends on:** P2-01, P1-01

### P2-03: CRC-32 slicing-by-16 against 8 (gated)
**Goal:** Gated per PT2. If CRC-32 is a significant share of gzip decoding, measure slicing-by-16 against slicing-by-8 and adopt 16 only if it wins.
**Files touched:** `crates/cleanflate/src/config.rs` (only if the value changes), `docs/perf/phase3/P2-03-before.json`, `docs/perf/phase3/P2-03.json`, `docs/perf/phase3/P2-03-checksum.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M2 "Checksums" (slicing-by-16, measured against 8), §1 pattern 4, §4.1 T4. D-32. C8 (`CRC32_SLICES` is a tunable in config.rs). PT1, PT2, PT4.
- **Hotspot and gate:**
  - Frames: `Crc32::update` and `crc32_update_sliced`, including frames inlined in `GzipDecoder::step`.
  - Keys: `silesia/rfc1952/l6/streaming` and `canterbury/rfc1952/l1/oneshot`.
  - Below the gate: negative result.
  - Restate phase 2's measurement in the entry: `docs/perf/P4-03/CRC32_SLICES-16.json`, 16 was −0.9%.
- If the gate passes:
  - Compare `crc32_sliced16` with `crc32_sliced8` in a fresh `make bench-checksum` run (`P2-03-checksum.json`) at 4 KiB, 64 KiB and 1 MiB.
  - Only if 16 is faster by more than twice the P2-01 microbench noise at those sizes: set `CRC32_SLICES = 16`, with the doc comment citing both JSONs, and run PT1 on the gzip keys; PT4 decides. Otherwise record the negative result.
- A changed tunable value is not a new technique: D-32 stays; the config.rs doc comment and PHASE3.md carry the evidence.
**Acceptance tests:** `tests/checksum.rs::crc32_slices_config_valid` and `crc32_sliced_variants_match_bytewise` pass at the final value. If gated out or not adopted, the PHASE3.md entry is the deliverable.
**Out of scope:** Adler-32. New CRC algorithms.
**Verification:** PT1/PT2, `make lint && make test`.
**Depends on:** P2-01, P1-01

### P2-04: Wrapper per-call overhead in streaming (gated)
**Goal:** Gated per PT2. If the zlib and gzip wrappers cost a significant share beyond their checksums in streaming mode, remove their per-call overhead without changing any behaviour.
**Files touched:**
- `crates/cleanflate/src/`: `zlib.rs`, `gzip.rs`.
- `crates/cleanflate/src/tests/`: `wrapper_calls.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P2-04-before.json`, `docs/perf/phase3/P2-04.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §1 pattern 4 ("wrapper overhead in streaming mode"), §4.1 T4, §3 (no behaviour change: errors, sticky errors, `consumed`, gzip multi-member and header caps). RFC 1950 §2.2, RFC 1952 §2.2–§2.3. D-17, D-18, D-19, D-20. Global invariants apply. PT1, PT2, PT4–PT7.
- **Hotspot and gate:**
  - Frames: `ZlibDecoder::step` and `GzipDecoder::step` self time, excluding inlined checksum frames (use `top-children.txt`, subtracting the checksum's inclusive share).
  - Keys: `silesia/rfc1950/l6/streaming`, `silesia/rfc1952/l6/streaming`, `canterbury/rfc1950/l1/streaming` and `canterbury/rfc1952/l1/streaming`.
  - Below the gate: negative result.
- **Allowed changes:**
  - Remove the per-call `self.state.clone()` of the wrapper state machines: match on `&mut self.state`, or use `std::mem::replace` with a cheap placeholder.
  - Update checksums once per produced range.
  - Avoid re-slicing work per call.
  - This is a structural change shared by both decode paths, not a switched technique; conformance and fastslow cover it, and the DESIGN entry says so.
**Acceptance tests:** `crates/cleanflate/src/tests/wrapper_calls.rs`:
- `zlib_one_byte_chunks_same_trace_as_whole`;
- `gzip_multi_member_one_byte_chunks_same_output` (hand-built members with literal CRCs, C16);
- `wrapper_errors_sticky_every_kind`: bad header, checksum mismatch, truncated trailer and trailing data, each returning the same `Error` on three later calls, for zlib and gzip.
- If gated out, the PHASE3.md entry is the deliverable.
**Out of scope:** Checksum algorithms. Inflater changes.
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k`.
**Depends on:** P1-01, P0-09

### P2-05: Output-sink abstraction in the decoder core (slice sink only)
**Goal:** Route every output write of the decoder core through a generic, monomorphised `OutputSink`, with only the existing caller-slice behaviour implemented, as a pure refactor that the Vec sink (P2-06) builds on.
**Files touched:**
- `crates/cleanflate/src/`: `sink.rs` (new), `lib.rs` (module declaration), `inflate.rs`, `fast_loop.rs`, `window.rs`, `zlib.rs`, `gzip.rs`, `stream.rs`.
- `crates/cleanflate/src/tests/`: `sink.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P2-05-before.json`, `docs/perf/phase3/P2-05.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M2 "One-shot output path" (the sink is generic and monomorphised; one core for both sinks; the fast/reference split applies to both; safe Rust only), §3 (no behaviour change; streaming contract; public API unchanged), §5. RFC 1951 §3.2.5 (history semantics). D-11, D-27, D-29, D-30. Global invariants apply, including C24 (no `MaybeUninit` or similar). PT1, PT3, PT4, PT6.
- **`sink.rs`:** `pub(crate) trait OutputSink` with these methods (names fixed, so later tasks rely on them; more may be added and documented):
  - `fn written(&self) -> usize`: bytes written in this call so far.
  - `fn room(&self) -> usize`: bytes that may still be written in this call.
  - `fn push(&mut self, byte: u8) -> bool`: `false` means no room, and nothing is written.
  - `fn extend_from_slice(&mut self, data: &[u8]) -> usize`: writes `min(data.len(), room())` bytes and returns the count.
  - `fn copy_within_back(&mut self, dist: usize, n: usize) -> usize`: appends `n` bytes copied from `dist` bytes back in this call's output. It requires `1 <= dist <= written()` and `n <= dist` (non-overlapping), writes at most `room()` bytes, and returns the count. It returns 0 if a precondition fails.
  - `fn get_back(&self, dist: usize) -> Option<u8>`.
  - `fn written_since(&self, start: usize) -> &[u8]`: this call's bytes from offset `start`.
- **`SliceSink<'a> { buf: &'a mut [u8], pos: usize }`:** today's behaviour, exactly.
- **Plumbing:**
  - `Inflater::step<S: OutputSink>(&mut self, input: &[u8], sink: &mut S)` records `base = sink.written()` at entry.
    - Its history is the window as at entry followed by the sink's bytes since `base`, so a gzip member that starts mid-call sees only its own bytes, as today.
    - It refreshes the window at exit from `written_since(base)`, and `produced = written() − base`.
  - `fast_loop::run` and `Window::copy_match` are generic over the sink. The Fast path copies window bytes with `read_back` into `extend_from_slice`, and in-output bytes with `copy_within_back` in chunks of at most `dist`. The Reference path copies byte by byte with `get_back`/`push`.
  - `ZlibDecoder::step` and `GzipDecoder::step` are generic, with checksums over `written_since(before)`. `Backend::step` is generic.
  - `Decompressor::decompress` wraps the allowance-trimmed output in a `SliceSink`, with its limit logic unchanged.
- **Unchanged:** the public API and signatures, no new per-call allocation, `size_of::<Decompressor>()` (the existing `decompressor_size_not_grown` test), and every `Error` value and per-call `consumed`.
- **DESIGN:** a new non-`[perf]` entry "Output sinks for the decoder core", `RFC: RFC 1951 S3.2.5`, stating that history semantics, the window refresh and the D-27 exits are unchanged.
- **PT1:** bench-quick before and after must be NEUTRAL or WIN on every key; this is a named prerequisite of P2-06/P2-07 under PT4. A LOSS on any Silesia key must be investigated (inlining of sink methods, generic bloat in the fast loop) and fixed before finishing, or the task blocks with the numbers.
- Expect roughly 250–400 changed non-test lines. Keep the change mechanical: no decoding logic changes.
**Acceptance tests:**
- `crates/cleanflate/src/tests/sink.rs`:
  - `slice_sink_push_extend_room`;
  - `slice_sink_copy_within_back_preconditions` (`dist` 0, `dist > written`, `n > dist`, and room limits each return 0 or a short count);
  - `slice_sink_get_back_and_written_since`;
  - `gzip_second_member_history_isolated_in_one_call`: two hand-built members in one call, where a match in the second member reaching back into the first member's bytes gives the same `InvalidDistance` as today, on both paths, with literal CRCs (C16).
- Every existing test passes unchanged. `make proptest-3k` and `make fuzz-smoke-decoder` pass (verify).
**Out of scope:** `VecSink` (P2-06). Changing one-shot behaviour (P2-07).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full && make bomb`.
**Depends on:** P0-01, P0-09

### P2-06: Vec sink and the sink differential
**Goal:** Add a `VecSink` that appends decoded bytes to a `Vec` without zero-filling, a one-shot driver using it, and the SPEC §5 sink differential (slice sink against Vec sink, Fast against Reference), without yet changing the public one-shot path.
**Files touched:**
- `crates/cleanflate/src/`: `sink.rs`, `stream.rs`, `test_hooks.rs`.
- `crates/cleanflate/src/tests/sink.rs`.
- `crates/oracle/tests/`: `fastslow.rs`, `common/mod.rs`.
- `fuzz/fuzz_targets/diff_fast_reference.rs`.
**Design constraints:** SPEC §6 M2 "One-shot output path" (append literals by push/extend, matches by `extend_from_within` or equivalent safe copies; one core; safe Rust), §5 ("slice sink vs Vec sink, Fast vs Reference"), §3 (bounded memory; output-limit semantics). D-13, D-25, D-31, D-33. Global invariants apply, including C24. C13, C20. PT3, PT5, PT6.
- **`VecSink<'a>`** appends to a `&mut Vec<u8>`:
  - `written()` = `len − start` (the length when the call began); `room()` = `limit − written()` (the caller's allowance for the call).
  - `push`/`extend_from_slice` grow the Vec when needed; `copy_within_back` uses `extend_from_within`.
  - No zero-fill, no `MaybeUninit`, no `set_len`.
  - Growth: only `reserve_exact` (or `try_reserve_exact`) to the next `next_len(capacity, max_output)` step, never past `max_output`. The Vec's capacity therefore follows exactly the schedule the old zero-filled buffer followed, and peak memory is unchanged.
- **Driver:** `pub(crate) fn decompress_core_vec(format, input, options, path) -> Result<Core, Error>` has the same contract as today's `decompress_core`:
  - the same first size (`next_len(0, max_output).max(initial_len(..))`, via `Vec::with_capacity`);
  - the same `consumed`/`trailing_start` correction, `finish()` and gzip headers;
  - the same errors, including `Error::Truncated` for empty input and `OutputLimitExceeded { limit }`;
  - stats: `oneshot_resizes` counts reservations and `oneshot_bytes_allocated` sums capacity added.
- **Shared limit logic:** add `pub(crate) fn decompress_vec(&mut self, input: &[u8], out: &mut Vec<u8>) -> Result<Progress, Error>` on `Decompressor`, sharing its limit, trailing-data and stickiness logic with `decompress` through one private generic function. For the Vec sink the allowance is the remaining `max_output`, so `OutputFull` always means the limit was reached.
- **`test_hooks`:** `pub enum OneShotSink { Slice, Vec }` and `pub fn decompress_with_sink(format, input, &Options, DecodePath, OneShotSink) -> Result<Decoded, Error>`. `Slice` is today's `decompress_core`, `Vec` the new driver. The public `decompress_with` is unchanged in this task.
- **Sink differential** in `crates/oracle/tests/fastslow.rs`, comparing all four combinations (Slice or Vec, Fast or Reference): equal `data`, `consumed` and gzip headers, or the same `Error` Debug string.
  - `sinks_conformance_oneshot`: every manifest entry of the tier, with its dictionary.
  - `sinks_malformed`: every `testdata/malformed/EXPECTED.tsv` row.
  - `sinks_proptest`: `CLEANFLATE_FASTSLOW_CASES` cases from the shared strategies; each case also runs with a flipped byte and with `Options.max_output` drawn from {0, len/2, len − 1, len, len + 1, default}. On mismatch, call `dump_candidate` before failing (PT5).
  - Missing files or empty selections fail (C13). `make proptest-3k`/`proptest-10k`/`fastslow-full` pick these up (same test binary).
- **Fuzz:** `diff_fast_reference` also compares one-shot `Vec` against `Slice` on both paths.
**Acceptance tests:**
- `crates/cleanflate/src/tests/sink.rs`:
  - `vec_sink_growth_follows_next_len_and_cap`: capacity never exceeds `max_output`, and the steps equal `next_len`'s;
  - `vec_sink_exact_limit_ok_one_over_errors`: a stored block of n bytes with `max_output` n and n − 1 gives `Ok` and `OutputLimitExceeded { limit: n − 1 }`, on both sinks;
  - `vec_sink_matches_slice_sink_hand_built`: fixed, dynamic, stored, dictionary and two-member gzip streams.
- `crates/oracle/tests/fastslow.rs`: `sinks_conformance_oneshot`, `sinks_malformed`, `sinks_proptest`.
- `make fuzz-check` passes.
**Out of scope:** Switching the public one-shot path (P2-07).
**Verification:** `make lint && make test && make proptest-3k && make fastslow-full && make fuzz-check && make fuzz-smoke-diff_fast_reference FUZZ_SECONDS=120`.
**Depends on:** P2-05

### P2-07: One-shot decoding appends to a Vec: no zero-fill
**Goal:** Switch every public one-shot entry point to the Vec-sink driver, so one-shot decoding no longer writes into a pre-zeroed buffer, and keep the switch only if it wins.
**Files touched:**
- `crates/cleanflate/src/`: `stream.rs`, `test_hooks.rs`.
- `crates/cleanflate/src/tests/oneshot_sizing.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P2-07-before.json`, `docs/perf/phase3/P2-07.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M2 "One-shot output path" (all bullets and acceptance), §4.1 T2, §3 (bounded memory; bomb test unchanged). D-13, D-25, D-31, D-33. Global invariants apply. PT1, PT3–PT7.
- **Hotspot:**
  - Frames: SPEC §1 pattern 1 as measured in phase3-profile.md (the fill, resize, grow and copy frames of `decompress_core`).
  - Evidence: `perf stat` page faults for `silesia/rfc1951/l1/oneshot` and `silesia/rfc1951/l6/oneshot`.
  - Before and after, record those frames' share and the page faults.
- `decompress_with`, `inflate`, `zlib_decompress`, `gzip_decompress` and `decompress_with_stats` use `decompress_core_vec`.
- The slice driver stays reachable only through `test_hooks::decompress_with_sink(.., OneShotSink::Slice)`, with `#[cfg_attr(not(any(test, feature = "test-hooks")), allow(dead_code))]` where needed (dead_code only, C5).
- PT1 with levels 1 and 6. Also run `make bench-targets` on both quick JSONs and record T2's rows.
- Memory:
  - `make bomb` passes.
  - Record `oneshot_bytes_allocated` and `oneshot_resizes` from `make bench-stats STATS_CORPUS=silesia,canterbury STATS_LEVELS=1,6`, before and after.
- **PT4:** keep the switch only if it is a WIN on the one-shot keys with no LOSS elsewhere. Otherwise revert this task's `stream.rs` switch (P2-05, P2-06 and every test stay) and record SPEC's negative result ("a sink design cannot beat the current path") with the numbers.
- **DESIGN:** a `[perf]` entry (PT7, with `Profile:`) "One-shot output appended to a Vec". It supersedes the "freshly-resized tail" wording of D-13 and the buffer-growth wording of D-31; the size hints and caps are unchanged.
**Acceptance tests:** `crates/cleanflate/src/tests/oneshot_sizing.rs`:
- `oneshot_vec_and_slice_drivers_agree` (hand-built streams, both paths);
- `oneshot_limit_semantics_unchanged` (exact limit, limit − 1, empty input `Truncated`, `TrailingData` offsets, gzip lone trailing `0x1f`);
- `oneshot_capacity_bounded_by_limit`.
- The existing sizing tests are updated, not deleted.
**Out of scope:** Changing the size hint (P2-08).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full && make bomb`.
**Depends on:** P2-06, P1-01

### P2-08: One-shot size-hint revisit (gated)
**Goal:** Gated per PT2. If one-shot decoding still spends a significant share growing its output after P2-07, re-choose the hint or growth tunables by measurement.
**Files touched:** `crates/cleanflate/src/stream.rs`, `crates/cleanflate/src/config.rs`, `crates/cleanflate/src/tests/oneshot_sizing.rs`, `docs/DESIGN.md`, `docs/perf/phase3/P2-08-before.json`, `docs/perf/phase3/P2-08.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M2 ("revisit the hint only if the profile shows growth or zeroing cost"), §3 (bounded memory). D-31, D-33. Global invariants apply. C8. PT1, PT2, PT4–PT7.
- Run after P2-07 in plan order, on whatever one-shot path is current (the Vec sink if P2-07 was kept, otherwise the slice path).
- **Hotspot and gate:**
  - Frames: growth, reallocation and copy frames under the one-shot driver.
  - Keys: `silesia/rfc1951/l1/oneshot` and `canterbury/rfc1952/l1/oneshot`.
  - Counters: `oneshot_resizes`/`oneshot_bytes_allocated` from bench-stats.
  - Below the gate: negative result.
- **Allowed:** new values of `ONESHOT_INITIAL_RATIO` or `ONESHOT_GROWTH_FLOOR` in config.rs, chosen from measured candidates (for example ratio {4, 6, 8}); each candidate gets a bench-quick run, compared by bench-compare.
- **Unchanged:** `ONESHOT_MAX_PREALLOC`, `MAX_DEFLATE_EXPANSION`, the `max_output` cap and the hostile-ISIZE behaviour.
- A changed value cites its JSONs in its doc comment. A new DESIGN entry is needed only if the rule itself changes.
**Acceptance tests:** the existing `tests/oneshot_sizing.rs` tests (including `initial_len_tiny_or_hostile_inputs`) pass at the final values, plus `oneshot_capacity_bounded_by_limit`. If gated out, the PHASE3.md entry is the deliverable.
**Out of scope:** Sink changes.
**Verification:** PT1/PT2, `make lint && make test && make bomb`.
**Depends on:** P1-01, P0-09

### P2-09: M2 acceptance and push
**Goal:** Run the M2 acceptance gates, report T2 and T4 progress, confirm the sink differential and memory bounds, commit the bench and push.
**Files touched:** `docs/perf/phase3/P2-09.json`, `docs/perf/PHASE3.md` (`### P2-09 (M2 acceptance)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 2 files.
**Design constraints:** SPEC §6 M2 acceptance (T4 progress and checksum microbench numbers committed; T2 reported; the sink differential passes; bomb test and memory bounds unchanged), §5 milestone gates, PT5, PT8, PT9.
- PT9 with `BENCH_ID=P2-09`.
- PHASE3.md records:
  - T2 and T4 rows from `make bench-targets`, against `phase3-start.json`;
  - the checksum microbench JSONs committed in this phase (paths);
  - the sink differential test names and their full-tier run in `make heavy`;
  - `make bomb`;
  - `size_of::<Decompressor>()` (from `decompressor_size_not_grown`);
  - one-shot `oneshot_bytes_allocated` against the P1-02 stats.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci`, `make heavy` (including bomb and the full-tier sink differential) and `make fuzz-milestone` pass. `docs/perf/phase3/P2-09.json` is committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make fuzz-milestone && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Compare peak RSS of `make bomb` and of a one-shot Silesia decode against the phase-3 start (`/usr/bin/time -v`).
3. Review the sink code by hand and mutation-sample it in review (SPEC §5).
4. Review the checksum DESIGN derivations, if any were added.
**Depends on:** P2-06

## Phase 3 — M3 Per-stream setup cost

### P3-01: Per-block Huffman table reuse (gated)
**Goal:** Gated per PT2. Stop cloning the fixed tables for every fixed block and rebuild dynamic tables in place, reusing their allocations.
**Files touched:**
- `crates/cleanflate/src/`: `inflate.rs`, `huffman.rs`, `stats.rs`.
- `crates/cleanflate/src/tests/`: `inflate_block_tables.rs` (new), `mod.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P3-01-before.json`, `docs/perf/phase3/P3-01.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M3 bullet 1 ("reuse ... Huffman-table allocations across blocks"), §1 pattern 2. RFC 1951 §3.2.6, §3.2.7. D-14, D-15, D-28. Global invariants apply. PT1, PT2, PT4–PT7.
- **Hotspot and gate:**
  - Frames: `Huffman::from_lengths`, `build_table`, table `clone`, and the allocator frames under them.
  - Keys: `canterbury/rfc1952/l1/oneshot` and `canterbury/rfc1951/l1/streaming`.
  - Counters: `table_builds` and `fixed_table_clones` per block, from `make bench-stats STATS_CORPUS=canterbury STATS_LEVELS=1`.
  - Below the gate: negative result.
- **Implementation:**
  - Fixed blocks select the fixed tables through a selector (e.g. `ActiveTables { Fixed, Dynamic }`) instead of cloning them, so `fixed_table_clones` becomes 0.
  - `Huffman::rebuild(&mut self, lengths: &[u8], kind: TableKind) -> Result<(), HuffmanError>` reuses the `symbols` and `table` capacity (clear and refill, no new allocation once warm), with exactly `from_lengths`'s validation and errors.
  - Completing the code-length sequence slices `dyn_lengths` instead of `to_vec`.
- This changes storage, not decoding, so both paths share it. DESIGN says so, and conformance and fastslow cover it.
**Acceptance tests:** `crates/cleanflate/src/tests/inflate_block_tables.rs`:
- `fixed_blocks_do_not_clone_tables` (`#[cfg(feature = "stats")]`: `fixed_table_clones == 0` after 3 fixed blocks);
- `dynamic_rebuild_matches_fresh_build` (≥ 200 fixed-seed random complete length sets and the two exception distance sets: lookup and serial decode equal a fresh `from_lengths` for every buffer state);
- `rebuild_after_error_then_valid` (a rejected set leaves the table usable for the next valid build);
- `many_small_dynamic_blocks_same_output_both_paths`.
- If gated out, the PHASE3.md entry is the deliverable.
**Out of scope:** Per-stream setup (P3-02). Table format changes.
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k`.
**Depends on:** P1-01, P0-09

### P3-02: Per-stream setup: reusable inflater, lazy window, no redundant fills (gated)
**Goal:** Gated per PT2. Cut the fixed cost of starting a stream or a gzip member: reuse the inflater's fixed tables and window storage across members, and stop zero-filling storage that is fully overwritten before it is read.
**Files touched:**
- `crates/cleanflate/src/`: `inflate.rs`, `window.rs`, `gzip.rs`, `zlib.rs`, `huffman.rs`, `stats.rs`.
- `crates/cleanflate/src/tests/`: `setup_reuse.rs` (new), `history.rs`, `mod.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P3-02-before.json`, `docs/perf/phase3/P3-02.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M3 bullet 1 ("reuse the window ... across streams, and avoid zeroing large tables that are about to be fully overwritten"), §1 pattern 2, §3 (bounded memory: the window stays ≤ 32 KiB; no behaviour change). RFC 1951 §2, §3.2.5; RFC 1952 §2.2. D-08, D-19, D-30. Global invariants apply, including C24. PT1, PT2, PT4–PT7.
- **Hotspot and gate:**
  - Frames: `Inflater::new_with_path`, `Window::new` and its fill, `from_lengths` under construction, `GzipDecoder::new`, `GzipHeaderParser::new`, large-struct copies (the `Decompressor` holds a 32 KiB window inline), and allocator frames.
  - Key: `canterbury/rfc1952/l1/oneshot`, plus `perf stat` instructions per file.
  - Below the gate: negative result.
- **Candidate changes.** Measure each; keep only what wins under PT4, and say which in the commit:
  - (a) `pub(crate) fn Inflater::reset(&mut self)`: returns to the freshly-constructed state (path kept) while keeping window storage, fixed tables and table capacity. `GzipDecoder` uses it at member boundaries instead of `Inflater::new_with_path`. A dictionary is re-applied after reset where the constructor applied one.
  - (b) Window storage that is not zero-filled per stream. For example, a `Vec<u8>` that grows by `append` up to `WINDOW_SIZE`, which is safe because `get_back`/`read_back` never read beyond `available()`. It keeps its allocation across resets. `size_of::<Decompressor>()` may shrink; update the `decompressor_size_not_grown` comment and limit, never raise the limit.
  - (c) Table builds that write every entry (complete codes) skip the up-front `Invalid` fill; incomplete distance codes (RFC 1951 §3.2.7) keep the explicit fill.
- C15: all index arithmetic is checked or provably bounded.
**Acceptance tests:** `crates/cleanflate/src/tests/setup_reuse.rs`:
- `inflater_reset_equals_fresh` (hand-built stored, fixed and dynamic streams; with a dictionary; after an error; after `Done`; on both paths);
- `gzip_many_tiny_members_same_output_both_paths` (1000 identical hand-built members with a literal CRC, C16);
- `window_lazy_growth_matches_model` (appends of 0, 1, 100, 32767, 32768, 32769 and 100000 bytes; `get_back`/`read_back` against a plain `Vec` model, before the first 32 KiB and after wrapping).
- `tests/history.rs` tests pass unchanged. If gated out, the PHASE3.md entry is the deliverable.
**Out of scope:** The public `reset` (P3-03). Per-block table reuse (P3-01).
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k && make fastslow-full && make bomb`.
**Depends on:** P1-01, P0-09

### P3-03: `Decompressor::reset()` and the reset differential
**Goal:** Add the public `Decompressor::reset(&mut self)`, which returns a decompressor to its freshly-constructed state while keeping its allocations, with rustdoc, a DESIGN entry and the SPEC §5 reset differential.
**Files touched:**
- `crates/cleanflate/src/`: `stream.rs`, `zlib.rs`, `gzip.rs`, `inflate.rs` (only if `Inflater::reset` does not exist yet), `lib.rs` (the streaming-contract rustdoc mentions `reset`).
- `crates/cleanflate/src/tests/`: `reset.rs` (new), `mod.rs`.
- `crates/oracle/tests/`: `reset.rs` (new), `common/mod.rs`.
- `docs/DESIGN.md`.
**Design constraints:** SPEC §3 (the only additive public API this phase), §6 M3 bullet 2, §5 ("decodes a stream after `reset()` and compares against a fresh `Decompressor`, across formats, options and mid-stream resets"). Decisions entries on `reset` semantics and `reset_with`. D-20. Global invariants apply. C4: the rustdoc example uses `?`, not `unwrap`. C13, C16.
- `pub fn reset(&mut self)`. Rustdoc: after `reset`, the decompressor behaves exactly like `Decompressor::new(format, options)` with the same format and options (and decode path), for any later sequence of calls; allocations are kept. Include an example.
- It clears:
  - the sticky error, `total_in`, `total_out`, `trailing_start`;
  - gzip headers (`clear`, keeping capacity) and the member count;
  - the stats counters (feature `stats`);
  - backend state, through `Backend::reset`, `ZlibDecoder::reset`, `GzipDecoder::reset` and `Inflater::reset`.
- It re-applies `Options::dictionary` for raw streams; zlib keeps its dictionary for FDICT.
- If P3-02 was gated out, add `Inflater::reset` here, keeping window storage and tables.
- **DESIGN:** a non-`[perf]` entry, `RFC: n/a (API) -- SPEC S6 M3`, recording the semantics and why there is no `reset_with`.
- **`crates/oracle/tests/common/mod.rs`:** a `trace_with(decompressor: &mut Decompressor, input, in_sched, out_sched) -> Trace` variant of `trace` that drives an existing decompressor.
**Acceptance tests:**
- `crates/oracle/tests/reset.rs`:
  - `reset_equals_fresh_conformance`: for every quick-tier manifest entry B (with its dictionary), first drive the decompressor on a different entry A of the same format to one of these prefixes: no call; one call; `Done`; an error (A truncated, or with one flipped byte); `OutputLimitExceeded` (small `max_output`). Then `reset()` and trace B with schedules `[1021]/[997]` (and `[1]/[1]` for entries ≤ 64 KiB). The `Trace` must equal a fresh decompressor's trace of B with the same options.
  - `reset_equals_fresh_options`: `Options` variants default, `TrailingData::Allow`, small `max_output`, `max_gzip_headers: 1`, and a dictionary (raw and zlib).
  - `reset_proptest`: shared strategies, `CLEANFLATE_FASTSLOW_CASES` cases; a random prefix stream, a random reset call index and a target stream; on both paths via `test_hooks::decompressor_with_path`.
  - `reset_repeated_same_result`: 100 resets and decodes of one stream on one decompressor.
- `crates/cleanflate/src/tests/reset.rs`:
  - `reset_preserves_path` (Reference stays Reference, via the `inflater_path` accessor);
  - `reset_clears_sticky_error`;
  - `reset_clears_trailing_and_headers`;
  - `reset_stats_zeroed` (`#[cfg(feature = "stats")]`).
**Out of scope:** `reset_with`. The reuse bench (P3-04).
**Verification:** `make lint && make test && make proptest-3k && make fastslow-full`.
**Depends on:** P0-01

### P3-04: Reuse bench case
**Goal:** Add a `reuse` bench mode that decodes each input repeatedly through one `Decompressor` using `reset()`, reported next to fresh-object streaming, and a matching profiling mode.
**Files touched:** `crates/oracle/src/bench.rs`, `crates/oracle/src/bench_json.rs`, `crates/oracle/src/bench_compare.rs`, `crates/oracle/src/bench_targets.rs`, `crates/cleanflate/examples/profile_decode.rs`, `Makefile`, `docs/perf/README.md`, `docs/perf/phase3/P3-04.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §4.2 "Reuse bench case", §6 M3. Decisions entry on like-for-like denominators (no C zlib reuse; `inflateReset` is not permitted, SPEC §2). C9, C22. C1 for the example.
- **Mode `reuse`:** per (file, framing, level), one `Decompressor::new(format, Options::default())` is built before timing. Each decode calls `reset()`, then runs the same 64 KiB streaming loop into the same pre-sized scratch buffer as `streaming`. The warm-up verifies output. Cleanflate only.
- **Aggregates for `reuse`:**
  - `ratio: null` and `c_zlib_mode: null` (schema 2 allows null);
  - new field `vs_fresh_streaming` = reuse median / streaming median of the same key, when both modes ran.
  - `bench-md` shows `vs_fresh_streaming` for reuse rows; `bench-targets` already ignores reuse.
- **Makefile:** `bench` and `bench-quick` use `--modes oneshot,streaming,reuse`; `PROFILE_KEY` accepts mode `reuse`.
- **`profile_decode`:** `--mode reuse` (one decompressor per file, `reset()` between iterations).
- Run `make bench-quick BENCH_ID=P3-04 BENCH_OUT=docs/perf/phase3/P3-04.json` (background) and commit it. PHASE3.md `### P3-04` gets a Canterbury per-file table: bytes, fresh streaming, reuse, reuse/fresh.
**Acceptance tests:** unit tests in `crates/oracle/src/bench.rs` and `bench_compare.rs`:
- `reuse_mode_parsed`;
- `reuse_aggregate_has_no_c_zlib_ratio`;
- `vs_fresh_streaming_computed`;
- `bench_md_shows_reuse_speedup`.
- `make profile PROFILE_KEY=canterbury/rfc1952/l1/reuse PROFILE_ITERS=50` writes its `top.txt`.
**Out of scope:** Library changes.
**Verification:** `make lint && make test && make bench-quick BENCH_ID=P3-04 BENCH_OUT=docs/perf/phase3/P3-04.json`.
**Depends on:** P3-03, P0-08

### P3-05: M3 acceptance and push
**Goal:** Run the M3 acceptance gates, report the Canterbury ratio change per file with sizes and T3 progress, commit the bench and push.
**Files touched:** `docs/perf/phase3/P3-05.json`, `docs/perf/PHASE3.md` (`### P3-05 (M3 acceptance)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 3 files.
**Design constraints:** SPEC §6 M3 acceptance (Canterbury ratio change per file with sizes; T3 progress), §5 milestone gates, PT5, PT8, PT9.
- PT9 with `BENCH_ID=P3-05`. The full bench now includes reuse rows.
- PHASE3.md records:
  - `make bench-md J=docs/perf/phase3/P3-05.json FILES=1 FILTER=canterbury/` beside the same table for `phase3-start.json`, with a per-file ratio change column;
  - T3 rows from `make bench-targets`;
  - reuse against fresh streaming per Canterbury file;
  - the reset differential test names.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci`, `make heavy` and `make fuzz-milestone` pass. `docs/perf/phase3/P3-05.json` is committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make fuzz-milestone && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Review the `Decompressor::reset` rustdoc and DESIGN entry as a public API addition.
3. Compare peak RSS of `make bomb` against the phase-3 start.
**Depends on:** P3-03

## Phase 4 — M4 Decode hot loop

### P4-01: Assembly evidence tool for the fast loop
**Goal:** Build the tooling that counts panic/bounds-failure call sites and conditional branches in cleanflate's own generated assembly for named functions, and record the fast loop's baseline counts.
**Files touched:** `tools/perf/asm_branches.py` (new), `tools/perf/tests/test_asm_branches.py` (new), `tools/perf/tests/fixtures/sample.s` (new), `Makefile` (`asm`, `asm-count`), `docs/perf/README.md` (assembly section), `docs/perf/phase3/P4-01-asm.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §4.3 "Bounds-check elision" (cleanflate's own assembly via `cargo rustc --release -p cleanflate --lib -- --emit asm`; cargo-asm is not installed and must not be installed), §2 (only cleanflate's own code), CR3, CR7. Stdlib-only Python, like the other tools.
- **`make asm`:** `cargo rustc --release -p cleanflate --lib -- --emit asm`, with the release profile and no extra `-C` flags, so the code is the benchmarked code. Then copy the newest emitted `cleanflate-*.s` file(s) from `target/release/deps/` into `target/asm/`. If several codegen-unit files are emitted, copy and scan all of them.
- **`make asm-count ASM_FUNCS=fast_loop ASM_OUT=target/asm/count.json`** runs `python3 tools/perf/asm_branches.py --func <each comma-separated substring> --json $(ASM_OUT) target/asm/*.s`. For every function symbol whose mangled or demangled name contains a `--func` substring, it reports:
  - instructions;
  - conditional branches (jump mnemonics other than an unconditional `jmp`);
  - calls or jumps to panic or bounds-failure symbols (names containing `panic`, `panic_bounds_check`, `slice_start_index_len_fail`, `slice_end_index_len_fail`, `slice_index_order_fail`, `unwrap_failed`, `expect_failed` or `len_mismatch`);
  - the number of distinct such call sites.
  - Output: a table, plus JSON `{schema: 1, kind: "asm", git_head, rustc, functions: [{name, instructions, cond_branches, panic_calls, panic_sites}]}`.
- The Makefile and the tool never disassemble anything but cleanflate's own `.s` (C19).
- Record the baseline for `fast_loop`, plus `decode_table` and `refill_bulk` if they have their own symbols, as `docs/perf/phase3/P4-01-asm.json` and a table in PHASE3.md `### P4-01`.
**Acceptance tests:** `tools/perf/tests/test_asm_branches.py`: `counts_on_fixture` (a hand-written `.s` with two functions and known counts), `func_filter_substring`, `json_shape`. `make tools-check` passes. `make asm && make asm-count ASM_FUNCS=fast_loop ASM_OUT=docs/perf/phase3/P4-01-asm.json` succeeds.
**Out of scope:** Changing library code.
**Verification:** `make tools-check && make asm && make asm-count ASM_FUNCS=fast_loop ASM_OUT=docs/perf/phase3/P4-01-asm.json`.
**Depends on:** P0-01

### P4-02: Branch-light bulk refill (gated)
**Goal:** Gated per PT2. When at least 8 input bytes remain, refill the bit buffer with one word read and branch-free arithmetic.
**Files touched:**
- `crates/cleanflate/src/`: `bitreader.rs`, `fast_loop.rs` (only if the call site changes).
- `crates/cleanflate/src/tests/bitreader_bulk.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P4-02-before.json`, `docs/perf/phase3/P4-02.json`, `docs/perf/phase3/P4-02-asm.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M4 bullet "Branch-light refill", §4.3. RFC 1951 §3.1.1. D-27 (count ≤ 63; exact give-back at exits; the Reference path's exact fill is untouched). Global invariants apply. PT1–PT7.
- **Hotspot and gate:**
  - Frames: `refill_bulk`, or its lines inside `fast_loop::run` via `perf annotate -i target/profile/<slug>/perf.data`, which is allowed only on the cleanflate-only profile (CR7).
  - Keys: `silesia/rfc1951/l1/oneshot` and `silesia/rfc1951/l1/streaming`.
  - Below the gate: negative result.
- **Technique:**
  - Read a fixed-size `[u8; 8]` view (`first_chunk::<8>()` or `split_first_chunk`) as `u64::from_le_bytes`, OR `word << count` into the buffer.
  - Advance `pos` by `k = (63 − count) / 8` and add `8k` to `count`, with no loop and no per-byte branch.
  - The fewer-than-8-bytes tail keeps the byte loop.
- Record `make asm-count ASM_FUNCS=refill_bulk,fast_loop` before and after.
**Acceptance tests:** `crates/cleanflate/src/tests/bitreader_bulk.rs`:
- `refill_word_path_all_counts`: every count 0..=63 × input lengths 8..=16; bits and `pos` equal a byte-by-byte model;
- `refill_never_exceeds_63_bits`.
- Existing bulk tests pass. If gated out, the PHASE3.md entry is the deliverable.
**Out of scope:** Fast-loop restructuring (P4-03).
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k && make fastslow-full`.
**Depends on:** P4-01, P1-01

### P4-03: Fast-loop bounds-check elision and inlined lookup
**Goal:** Restructure the fast loop so LLVM can prove its accesses in bounds in safe code, and keep the table lookup and bit buffer inside the loop, measured by throughput at levels 1 and 6 and by the fast loop's panic/branch counts before and after.
**Files touched:**
- `crates/cleanflate/src/`: `fast_loop.rs`, `huffman.rs` (a lookup helper the loop can inline), `bitreader.rs`, `window.rs` and `sink.rs` (copy helpers, if needed).
- `crates/cleanflate/src/tests/fast_loop.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P4-03-before.json`, `docs/perf/phase3/P4-03.json`, `docs/perf/phase3/P4-03-asm.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M4 bullets "Bounds-check elision" and "Level-1 streams are the target", §4.3 (verify via generated assembly; record before/after counts), §5. RFC 1951 §3.2.5, §3.2.6. D-27, D-28, D-29, D-30. Global invariants apply; C5 in particular: no `[]` indexing and no local allow. PT1, PT3–PT7.
- **Hotspot:**
  - Frames: `fast_loop::run` self time, `Huffman::decode_table` (a separate symbol today), `peek`/`consume` and match copy.
  - Keys: `silesia/rfc1951/l1/oneshot` and `silesia/rfc1951/l1/streaming`.
  - Record the before and after shares.
- **Restructure, Fast path only** (the Reference path is untouched):
  - Keep the bit buffer and count in locals across iterations.
  - Use a loop-specific lookup (an `#[inline(always)]` helper in huffman.rs returning the packed entry) that relies on the ≥ 48 buffered bits the loop guarantees, so it never has a "need more bits" case.
  - Index tables with values masked to a power-of-two table length, or through fixed-size array views, so `get` can be proven in bounds.
  - Hoist the output-room check to the iteration's margin check, so literal writes and match copies need no per-byte check: `split_at_mut` or fixed-size views for the slice sink, one capacity reservation per iteration for the Vec sink.
  - Read length/distance base and extra-bit tables through fixed-size arrays indexed by masked values.
- **Must keep:** identical output; the same `Error` values at the same call; per-call `consumed` (D-27 exits); stats counters with the same meaning; both sink instantiations if P2-05 exists.
- **Evidence:** `make asm && make asm-count ASM_FUNCS=fast_loop,decode_table` before (the P4-01 JSON, or a fresh one if P4-02 changed code) and after (`P4-03-asm.json`). PHASE3.md gets a table of panic sites, panic calls and conditional branches per fast-loop function, before and after.
- PT1 at levels 1 and 6, and PT4. If no WIN can be shown, record SPEC's negative result, revert the code, and still commit the assembly evidence.
- DESIGN: a `[perf]` entry (PT7, with `Profile:`).
**Acceptance tests:** `crates/cleanflate/src/tests/fast_loop.rs`:
- `fast_loop_long_codes_via_subtable` (a hand-built dynamic block with 15-bit codes, whole input and 1-byte output chunks);
- `fast_loop_all_table_widths_same_trace` (the internal width-taking constructor, widths 7..=12, if the loop's lookup depends on the width);
- `fast_loop_vec_and_slice_sinks_same_trace` (if P2-05 exists).
- All existing fast-loop tests pass.
**Out of scope:** Multi-literal entries (P4-04). Table width changes (P4-05).
**Verification:** PT1, `make lint && make test && make proptest-3k && make fastslow-full && make bomb`.
**Depends on:** P4-01

### P4-04: Multi-literal table entries (optional, measured)
**Goal:** Gated per PT2 and measured with and without: pack two literals into one first-level literal/length entry where their code lengths fit, decode both in one fast-loop step, and keep the variant that wins.
**Files touched:**
- `crates/cleanflate/src/`: `huffman.rs`, `fast_loop.rs`, `stats.rs`.
- `crates/cleanflate/src/tests/`: `huffman_multi.rs` (new), `fast_loop.rs`, `mod.rs`.
- `docs/DESIGN.md`, `docs/perf/phase3/P4-04-before.json`, `docs/perf/phase3/P4-04.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M4 bullet "Multi-literal entries (optional)" (a general technique; the table design derived in the project's own words; measure with and without; keep the winner by PT1), §2 (no named library's multi-literal design may be consulted), §5. RFC 1951 §3.2.2, §3.1.1. D-28, D-29. Global invariants apply. PT1–PT7.
- **Gate:**
  - The literal share at level 1: literals / (literals + matches) from `make bench-stats STATS_LEVELS=1`.
  - The literal branch's share of `fast_loop::run` on `silesia/rfc1951/l1/oneshot`, via `perf annotate` of the cleanflate-only profile.
  - Below the gate: negative result.
- **Design (Fast path only):**
  - At first-level table build time, an entry whose code is a literal of length `l1` is examined at the index formed by its remaining `table_bits − l1` bits. If that index resolves, in the same first level, to another literal of length `l2` with `l1 + l2 ≤ table_bits`, the entry also carries the second literal and the total length. The pass is O(table size).
  - Inside the loop, where the output margin always holds, such an entry writes both bytes and consumes `l1 + l2` bits.
  - `decode_table`, used by the state machine and at loop edges, returns exactly the single first symbol it returns today. The extra literal is ignored outside the loop, and the Reference path is unchanged.
  - Stats count each literal, so `stats_symbol_decode_methods_sum` still holds.
- **Measure** P4-03's code (before) against this change (after). PT4: keep it only on a WIN. Otherwise revert the code, keep the trace-level tests that still compile, and record the negative result.
- DESIGN: a `[perf]` entry (PT7, with `Profile:`), in the project's own words, stating why a pair entry decodes exactly what two single lookups decode (prefix-freeness, and `l1 + l2 ≤ table_bits` means both codes are fully inside the indexed bits).
**Acceptance tests:**
- `crates/cleanflate/src/tests/huffman_multi.rs`:
  - `pair_entries_decode_same_as_two_single_lookups` (the fixed table and ≥ 200 fixed-seed random complete tables, every first-level index);
  - `pair_never_exceeds_table_width`;
  - `decode_table_unchanged_by_pairs`.
- `crates/cleanflate/src/tests/fast_loop.rs::fast_loop_pairs_same_trace_as_reference`: literal-heavy hand-built fixed and dynamic blocks, whole input and 1-byte output chunks, and the 7/13 schedule.
- If gated out, the PHASE3.md entry is the deliverable.
**Out of scope:** Table width changes (P4-05).
**Verification:** PT1/PT2, `make lint && make test && make proptest-3k && make fastslow-full`.
**Depends on:** P4-01

### P4-05: Re-tune table widths on the final loop
**Goal:** Re-measure the first-level lookup widths on the final fast loop, choose each value by the noise rule with full-bench confirmation, and cite the measurements in config.rs.
**Files touched:** `crates/cleanflate/src/config.rs`, `docs/perf/phase3/P4-05/*.json`, `docs/perf/PHASE3.md`.
**Design constraints:** SPEC §6 M4 bullet "Table size" (re-tune the primary lookup width, e.g. 9–11 bits plus subtables, on the final loop), §3 (tunables only in config.rs, with the measurement that chose them). C8. PT1, PT8.
- **Candidates:** `LITLEN_TABLE_BITS` {9, 10, 11, 12} and `DIST_TABLE_BITS` {7, 8, 9}, one key at a time with the other at its current value.
- **Procedure:**
  - Run the current defaults first (`docs/perf/phase3/P4-05/default.json`); edit config.rs for each candidate; run `make bench-quick BENCH_ID=P4-05-<KEY>-<value> BENCH_OUT=docs/perf/phase3/P4-05/<KEY>-<value>.json` (background); finish with a repeat of the defaults (`default-repeat.json`) as a drift control.
  - Compare each candidate against both default runs.
  - A quick WIN (PT1 rule) on the level-1 raw keys must be confirmed by a full `make bench` (background) against `docs/perf/phase3/P3-05.json`, with no REGRESSION, before a default changes. Otherwise keep the value and record a measured "no change".
- **Results:** each tunable's doc comment cites its JSONs and outcome. PHASE3.md `### P4-05` has a table of key, candidates, winner, deltas and verdicts.
**Acceptance tests:** `tests/huffman_twolevel.rs::table_widths_sweepable` and the fast-loop tests pass at the final values. `make test` and `make proptest-3k` pass.
**Out of scope:** Code changes outside config.rs.
**Verification:** `make lint && make test && make proptest-3k`.
**Depends on:** P0-09

### P4-06: M4 acceptance and push
**Goal:** Run the M4 acceptance gates, judge the level-1 rows against M3, commit the elision evidence and the bench, and push.
**Files touched:** `docs/perf/phase3/P4-06.json`, `docs/perf/phase3/P4-06-asm.json`, `docs/perf/PHASE3.md` (`### P4-06 (M4 acceptance)`), `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines in Phase 4 files.
**Design constraints:** SPEC §6 M4 acceptance (level-1 rows improve measurably, as a WIN verdict; every §5 gate green; the elision evidence committed), §5 milestone gates, PT5, PT8, PT9. Decisions entry on the M4 WIN criterion.
- PT9 with `BENCH_ID=P4-06`.
- `make bench-compare A=docs/perf/phase3/P3-05.json B=docs/perf/phase3/P4-06.json`: the level-1 keys' verdicts (throughput and ratio) give MET or NOT MET for "a WIN verdict". This is recorded, not blocking.
- `make asm && make asm-count ASM_FUNCS=fast_loop,decode_table,refill_bulk ASM_OUT=docs/perf/phase3/P4-06-asm.json`. PHASE3.md gets the P4-01 against P4-06 table.
- `make provenance-snapshot`, commit, push.
**Acceptance tests:** `make ci`, `make heavy` and `make fuzz-milestone` pass. `docs/perf/phase3/P4-06.json` and `docs/perf/phase3/P4-06-asm.json` are committed.
**Out of scope:** New optimisation.
**Verification:** `make ci && make heavy && make fuzz-milestone && make provenance-snapshot && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Confirm GitHub Actions `ci` is green.
2. Review the fast-loop code by hand and mutation-sample it in review (SPEC §5), preferring mutants that cannot run away (SPEC §7).
3. Review the new `[perf]` DESIGN entries for clean-room wording (SPEC §2).
**Depends on:** P4-03

## Phase 5 — M5 Report and hand-off

### P5-01: After-profiles and flamegraphs
**Goal:** Commit the after-flamegraphs for the three required keys and complete `docs/perf/phase3-profile.md` with the after profiles and a before/after hotspot comparison.
**Files touched:** `docs/perf/flamegraphs/silesia-rfc1951-l1-oneshot-after.svg`, `docs/perf/flamegraphs/silesia-rfc1951-l1-streaming-after.svg`, `docs/perf/flamegraphs/canterbury-rfc1952-l1-oneshot-after.svg`, `docs/perf/phase3-profile.md`.
**Design constraints:** SPEC §4.3 (after-flamegraphs at M5; only cleanflate and std frames), §6 M5 bullet 1, CR7, C10, C25. No code changes.
- `make profile-required TAG=after`.
- phase3-profile.md `## After (P5-01)`:
  - per key, the top ≥ 10 self-time symbols and the `perf stat` figures;
  - a before/after table of every hotspot named in `## Hotspots by task`;
  - each §1 pattern's final state.
**Acceptance tests:** the three SVGs are committed and pass `tools/perf/check-flamegraph.sh`; the After section covers every key and pattern.
**Out of scope:** Code changes.
**Verification:** `bash tools/perf/check-flamegraph.sh docs/perf/flamegraphs/canterbury-rfc1952-l1-oneshot-after.svg && make design-check`.
**Depends on:** P1-01

### P5-02: Final bench, PHASE3.md report and PERF.md phase-3 section
**Goal:** Run the final full bench and complete `docs/perf/PHASE3.md`: phase-2 vs phase-3 tables including C zlib streaming, T1–T6 verdicts, flamegraphs, per-change attribution, and everything tried and reverted. Add the Phase 3 pointer section and result line to `docs/PERF.md`.
**Files touched:** `docs/perf/phase3/P5-02.json`, `docs/perf/PHASE3.md`, `docs/PERF.md`, `docs/DESIGN.md` (audit fixes only).
**Design constraints:** SPEC §6 M5 (the report's contents; the PERF.md pointer), §4.1, §9, §8 (record conditions), C10, C14, PT7, PT8.
- Run `make bench BENCH_ID=P5-02 BENCH_OUT=docs/perf/phase3/P5-02.json` (background, quiet machine), then `make bench-compare` against `docs/perf/baselines/phase2.json` and `docs/perf/baselines/phase3-start.json`, and `make bench-targets J=docs/perf/phase3/P5-02.json`.
- **PHASE3.md:**
  - `## Summary`: the headline numbers.
  - Phase-2 against phase-3 tables for both corpora, all levels, framings and modes, with C zlib one-shot and streaming.
  - `## Targets`: T1–T6 with verdicts (from bench-targets), the worst row of each, and for every missed target the shortfall and the next most promising step with its profile evidence.
  - Flamegraphs before and after: links to the six SVGs, with one sentence each.
  - `## Attribution`: which task moved which rows and by how much, from the PT1 pairs and acceptance runs.
  - `## Tried and reverted`: every gated-out or reverted task, with the reason and any regression fixture.
  - Conditions of the final run.
- **`docs/PERF.md`:** a new `## Phase 3` section after the phase-2 content, a few lines pointing to `docs/perf/PHASE3.md` and `docs/perf/phase3-profile.md`. It ends with exactly one line:
  `**Result:** phase 3: cleanflate / C zlib like for like (Silesia, level 6) = one-shot R% raw, Z% zlib, G% gzip; streaming S% raw, SZ% zlib, SG% gzip; targets met: <T-ids or none> of T1-T6.`
- **DESIGN audit:** every kept phase-3 technique has a `[perf]` entry (D-34 onward) with `Profile:` and `Measurement:` lines; `make design-check` passes.
**Acceptance tests:** `make design-check` passes, PERF.md contains the result line, `docs/perf/phase3/P5-02.json` is committed, and PHASE3.md has every section listed above.
**Out of scope:** Code changes.
**Verification:** `make design-check && make bench-md J=docs/perf/phase3/P5-02.json && make bench-targets J=docs/perf/phase3/P5-02.json`.
**Depends on:** P0-08

### P5-03: Provenance, build report and hand-off push
**Goal:** Scope the provenance summary and build report to phase 3, regenerate them, update the human-interventions log, run the final gates, push, and list the manual checks.
**Files touched:** `Makefile` (`PHASE_BASE`), `docs/BUILD_REPORT.md` (generated), `provenance/SUMMARY.md` (generated), `provenance/human-interventions.md`, `provenance/tool-log.snapshot.jsonl`, and fixes of ≤ 100 lines anywhere, recorded in `Defects-caught:`.
**Design constraints:** SPEC §6 M5 (regenerate BUILD_REPORT and SUMMARY scoped to phase 3, with `PHASE_BASE` set to the phase-3 SPEC commit; update human interventions; the implementer does not tag), §9, PT8.
- `Makefile`: `PHASE_BASE ?= cd4a472`, with the comment updated to "the phase-3 SPEC commit: phase-3 summaries and reports cover PHASE_BASE..HEAD".
- Run `make provenance-snapshot`, `make provenance-summary` and `make build-report`.
- `human-interventions.md`: add rows only for interventions this phase's progress log records, or relayed operator actions the implementer knows of. If there are none, add one dated sentence saying the pipeline recorded none in phase 3.
- Run `make ci` and `make heavy`, commit, and `git push -u origin HEAD`.
**Acceptance tests:** `make ci` and `make heavy` pass. SUMMARY.md and BUILD_REPORT.md name the `cd4a472..HEAD` range, and BUILD_REPORT.md's performance line is the phase-3 result line.
**Out of scope:** Tagging.
**Verification:** `make ci && make heavy && make provenance-snapshot provenance-summary build-report && git push -u origin HEAD`. Write `Manual check: NOT VERIFIED (human)` with:
1. Run the final `make bench` on a quiet machine with a pinned governor, record the conditions, and compare with `docs/perf/phase3/P5-02.json`.
2. Run `make fuzz-long` or the nightly workflow for all six targets and triage `make fuzz-divergences`; confirm the milestone fuzz durations recorded in PHASE3.md.
3. Confirm GitHub Actions `ci` is green.
4. Review PHASE3.md's T1–T6 verdicts, attribution and negative results, and the PERF.md result line.
5. Review the committed flamegraphs and the assembly evidence.
6. After review and merge into `main`, tag `v0.3.0-cleanroom`.
7. Complete `provenance/human-interventions.md`.
**Depends on:** P5-02

## Spec issues
- **Reuse mode and "like for like" (§4.1, §4.2):** a C zlib counterpart to `reset()` reuse would need `inflateReset`, which is not among the permitted signatures (§2). Resolved: reuse rows are cleanflate-only, with `ratio: null`, reported against cleanflate fresh-object streaming. T3's "every mode" excludes reuse.
- **T2 and the bench's streaming mode (§4.1 T2, §4.2 "keep the one-shot rows as they are"):** the streaming rows decode into one pre-sized scratch buffer that is reused, and therefore already faulted in, across repetitions. One-shot must allocate and touch fresh memory on every decode, and kernel page-fault time is invisible to user-space sampling (`perf_event_paranoid = 2`). T2 may be unreachable without changing what is measured. Resolved: the rows are kept as SPEC says, `perf stat` page-fault counts are reported beside T2, and the shortfall is explained if it remains.
- **Re-ordering M2–M4 by profile (§6 M1):** an unattended plan cannot be reordered. Resolved with gates (PT2) and a ranking in `docs/perf/phase3-profile.md`.
- **Milestone fuzz where the decoder did not change (§5, §9):** M0 and M1 change no decoder code, but SPEC's DoD asks for the gates "at every milestone". Resolved: run them anyway (30 minutes of background wall time each).
- **M4 scope (§6 M4) vs phase 2's recommended next steps:** phase 2's final report also named length/distance payloads in table entries and a short-match copy path. SPEC's M4 list does not include them, so they are not planned. If the final profile still points at them, P5-02 names them as next steps.
- **Checksum variants (§6 M2 "per implementation variant"):** measuring non-production variants needs entry points the stable API does not have. Resolved with `cleanflate::test_hooks::checksum` behind the opt-in `test-hooks` feature (not stable API, as in phase 2).
- **"cargo profile flags" and "libz-sys version and enabled features" (§4.2):** resolved as the workspace `[profile.release]` table plus `CARGO_PROFILE_*`/`RUSTFLAGS` variables at run time, and `cargo metadata`'s resolved version and features for that package, falling back to `unknown`.
- **CLAUDE.md PT1–PT8 and CR7 (§2, §3, §4.3):** SPEC cites them, but the root CLAUDE.md on `main` was the phase-1 version; they existed only in `docs/phase-2/CLAUDE.md`. Resolved: the new CLAUDE.md restores them, updated for phase 3 (PT9 added).
- **Additive API (§3):** SPEC names only `Decompressor::reset`. The `test_hooks` additions (sink selection, checksum variants) are opt-in, feature-gated test surface, treated as allowed as in phase 2.
- **Noise vs expected gains:** phase 2 measured about 6% aggregate noise and about 10% drift, larger than most remaining gains. Mitigated with the drift-corrected ratio verdict (P0-07), before/after pairs in one session, and a manual check for a quiet machine with a pinned governor.
- **Acceptance cost:** each milestone acceptance runs `make ci`, `make heavy`, 30 minutes of fuzzing and a full bench, about an hour, six times. SPEC §8 allows shortening the fuzz runs if the operator needs the machine; the actual duration is recorded.
- **§6 M2 "revisit the hint only if the profile shows growth or zeroing cost":** a conditional task, made the gated P2-08.

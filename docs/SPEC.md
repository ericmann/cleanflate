# SPEC: cleanflate phase 3 — performance round 3

> Phase 3 of cleanflate (the "round 3" performance pass). Phase 1 (the
> clean-room decoder, `v0.1.0-cleanroom`) and phase 2 (the first performance
> pass, `v0.2.0-cleanroom`) are merged to `main` and archived under
> `docs/phase-1/` and `docs/phase-2/`. Run with `/foundry:go-flight` from
> `main`.

## 1. Purpose

Phase 2 took cleanflate from about 26% to about 66–71% of C zlib's
single-thread decompression throughput on Silesia at level 6. The operator's
post-merge full bench (`docs/perf/phase2-final-operator.json`, summarised in
`docs/PERF.md` under "Operator re-run after merge") shows cleanflate at
0.54–0.78× C zlib across all rows, with four patterns:

| pattern in the phase-2 data | suspected cause (to be confirmed by profile) |
|---|---|
| one-shot is **slower** than streaming on every Silesia row (by 4–9 points of ratio) | one-shot output-buffer cost: zero-filling, growth, cache misses on a large fresh buffer |
| Canterbury (small files) trails Silesia (0.54–0.69×) | per-stream and per-block setup cost |
| level 1 is the slowest level (0.63–0.65× on Silesia) | the literal-heavy decode loop |
| zlib and gzip trail raw by about 3–10% | Adler-32 / CRC-32 cost, and wrapper overhead in streaming mode |

Phase 3 addresses each pattern, **measured first**, and fixes the benchmark
so that every row compares like with like (cleanflate streaming is currently
compared against C zlib one-shot).

Priorities are unchanged, in this order: **provenance, correctness, safety,
then performance.** A faster decoder that is wrong on one stream is a
regression. When performance conflicts with the others it loses, and the
negative result is recorded.

## 2. Clean-room rules (NON-NEGOTIABLE)

Phase 1's §2 and phase 2's §2 (`docs/phase-1/SPEC.md`, `docs/phase-2/SPEC.md`)
apply **in full**:
- **Unchanged:** the permitted and forbidden inputs, the mechanical guard and logger, the lockdown (no network, no WebFetch/WebSearch), and the commit-tag discipline.
- **Profiling:** limited to cleanflate-only processes (CLAUDE.md CR7).
- **C zlib:** a black-box stopwatch and oracle only.
- **Hooks:** they stay active. Do not relax, bypass or work around them.

Restated because round-3 work makes them tempting:
- Do **not** consult zlib, zlib-ng, zlib-rs, miniz_oxide, libdeflate or any
  other decoder's source, internals or implementation-specific write-ups
  (including articles on "how X decodes fast", multi-literal table designs
  of a named library, or benchmark write-ups that explain another
  implementation's fast path).
- Every optimisation is justified from **cleanflate's own profiles and
  counters** plus **general computer-science knowledge**, written in the
  project's own words, and each write-up cites the profile evidence that
  motivated it (DESIGN.md `[perf]` entries, CLAUDE.md PT7).
- **Do not add comparator crates** (flate2, miniz_oxide, zlib-rs, libdeflater
  or similar) to this repo, even as dev-dependencies. Comparisons against
  other implementations belong in the separate `cleanflate-bench` project.
- The oracle may use C zlib **only** through the signatures listed in
  `docs/phase-1/PLAN.md` task P1-01, plus one addition for this phase:
  `zlibCompileFlags() -> uLong` (used only to record the C library's build
  flags in the bench environment, §4.3). Streaming C zlib uses the
  already-listed `inflateInit2_` / `inflate` / `inflateEnd` with bounded
  `avail_in` / `avail_out`.

## 3. Hard constraints (carried over; none may regress)

- `crates/cleanflate`: `#![forbid(unsafe_code)]`, `[dependencies]` empty, stable
  Rust pinned at 1.89.0, the phase-1 panic lints denied in library code with
  no local `#[allow]`s. **No SIMD intrinsics, no `std::arch`, no nightly
  `std::simd`**, no multithreading. Auto-vectorisation the compiler does on
  its own is fine.
- **No behaviour changes.**
  - Output, accept/reject decisions and `Error` values are unchanged.
  - The streaming contract is unchanged: chunking invariance including 1-byte chunks, exact `consumed`, `trailing_start`, output-limit semantics and bounded memory (DESIGN D-11, D-13, D-20, D-25, D-31, D-33).
  - gzip multi-member behaviour and header caps are unchanged.
- **Public API:** no breaking changes. Additive, documented API is allowed only where this SPEC names it (`Decompressor::reset`, §6 M3).
- **No benchmark-specific code paths.** Nothing in `crates/cleanflate` may
  detect or special-case benchmark inputs, corpora, sizes or call patterns;
  every fast path must be justified for arbitrary input and exercised by the
  correctness suites.
- Tunables live only in `crates/cleanflate/src/config.rs`, each with the
  measurement that chose it (CLAUDE.md C8).
- Phase 2's correctness architecture stays: `DecodePath { Fast, Reference }`,
  the fast-vs-reference differential, the `stats` and `test-hooks` features,
  the regression-fixture store, and the performance task protocol
  (CLAUDE.md PT1–PT8).

## 4. Measurement

### 4.1 Targets (report actuals whether met or not)
All ratios are cleanflate / C zlib, **like for like** (one-shot vs one-shot,
streaming vs streaming with the same chunk sizes, §4.2), medians, levels 1,
6 and 9, raw/zlib/gzip.

| # | target |
|---|---|
| T1 | Silesia streaming ≥ 0.80× C zlib streaming, every level and framing |
| T2 | cleanflate one-shot ≥ cleanflate streaming, every row (both corpora) |
| T3 | Canterbury ≥ 0.70× C zlib, every level, framing and mode |
| T4 | zlib and gzip within 3% of raw throughput on the same corpus, level and mode |
| T5 | carried from phase 2: Silesia level-6 one-shot ≥ 0.70× C zlib for raw, zlib and gzip |
| T6 | no row slower than the phase-2 baseline (`docs/perf/baselines/phase2.json`) by more than the measured noise |

Missing a target is an honest result. Shipping incorrect code, or a
benchmark-specific path, to hit one is not.

### 4.2 Bench hygiene (M0)
- **Rename "min" to "best"** in every bench output, defined as the
  throughput of the fastest sample (minimum time). Document the definitions
  of "median" and "best", and every column, in a bench README
  (`crates/oracle/BENCH.md` or `docs/perf/README.md`).
- **Benchmark C zlib's streaming path** with the same input and output chunk
  sizes as cleanflate's streaming mode (64 KiB in, 64 KiB out), so the
  streaming rows compare like with like. Keep the one-shot rows as they are.
- **Baseline:** save the phase-2 operator run as
  `docs/perf/baselines/phase2.json` (a copy of
  `docs/perf/phase2-final-operator.json`). Then re-measure the phase-3
  starting point with the new bench (including C zlib streaming) as
  `docs/perf/baselines/phase3-start.json`, with a noise estimate from two
  runs, as phase 2 did.
- **Delta columns:** every report row shows the delta vs the phase-2 baseline
  and the ratio vs C zlib (like for like).
- **Environment** recorded with every run:
  - CPU model, core count and governor; OS and kernel; load average;
  - `rustc` version and the cargo profile flags;
  - C zlib version (`zlibVersion`) and build flags (`zlibCompileFlags`, as hex, plus the libz-sys version and enabled features);
  - git head and dirty flag.
- **Canterbury per-file reporting:** Canterbury rows are also reported per
  file, with file size, so setup-cost effects can be read against size.
- **Reuse bench case** (M3): a mode that decodes the same inputs repeatedly
  through one `Decompressor` using `reset()`, reported next to fresh-object
  streaming.

### 4.3 Profiling (M0/M1)
- `perf` 7.1.5 and `cargo flamegraph` 0.6.14 are installed; profile only the
  cleanflate-only target (`make profile`, `make flamegraph`,
  `examples/profile_decode.rs`), with `CARGO_PROFILE_RELEASE_DEBUG=true`.
- Extend `profile_decode` / `make profile` so a profile can be taken for any
  (corpus, framing, level, mode) key, including one-shot and streaming.
- **Committed flamegraphs:** unlike phase 2, the SVGs for the keys below are
  committed under `docs/perf/flamegraphs/` (before and after), because the
  phase-3 report must show them. They contain only cleanflate and std frames.
- Required profiles, at M0 (before) and at M5 (after):
  - `silesia/raw/l1/oneshot`
  - `silesia/raw/l1/streaming`
  - `canterbury/gzip/l1/oneshot`
  - plus any key a task targets.
- **`docs/perf/phase3-profile.md`** lists the top hotspots per key with
  % of samples, and maps each §1 pattern to the functions that explain it
  (or records that the profile contradicts the suspected cause).
- **Every optimisation task names the hotspot it targets** (from that file
  or a fresh profile) and shows the before/after profile share and bench
  numbers (CLAUDE.md PT1/PT2).
- **Bounds-check elision** (M4): verify the fast loop's bounds checks and
  `Option` failure branches are optimised away by inspecting cleanflate's
  own generated assembly (`cargo rustc --release -p cleanflate --lib --
  --emit asm`, or `cargo asm` if installed). `cargo-asm` is **not** installed,
  and agents may not install it. Record the before/after count of
  panic/bounds branches in the fast-loop function(s).

## 5. Correctness gates

Phase 2's §5 applies:
- **Paths:** the reference path, and fast paths layered on it with exact state restoration at every exit.
- **Differential test:** the fast-vs-reference comparison runs on the full corpus.
- **3,000-case chunking test:** runs for every decoder change (`make proptest-3k`); 10k at milestone acceptance.
- **Repros:** never lose a repro. Minimise it, add it with `oracle regress-add`, commit, then root-cause.
- **Dependencies:** keep task dependencies narrow, so one blocked task does not skip unrelated work.

In addition, for phase 3:

**Per task** (every task that changes `crates/cleanflate/src/`): all
conformance, malformed-corpus, chunking-invariance (3000 cases) and
fast-vs-reference tests green, plus a `make fuzz-smoke` run of fuzz target
(a) (`never_panic_*`, all three entry points) and of `diff_fast_reference`.

**Per milestone acceptance:**
- `make ci` and `make heavy` (10k proptests, bomb, full conformance, full fast-vs-reference) are green.
- Differential fuzz target (b) (`diff_oracle_roundtrip`) runs **≥ 30 minutes with zero divergences**.
- `diff_fast_reference` runs ≥ 30 minutes with zero divergences.
- Both run in the background (PT8), and the run length and result go in the acceptance commit.

**New code paths get new oracles:**
- An output sink (M2) that writes into a `Vec` instead of a caller slice must be covered by the differential: slice sink vs Vec sink, Fast vs Reference.
- `reset()` (M3) must be covered by a test that decodes a stream after `reset()` and compares against a fresh `Decompressor`, across formats, options and mid-stream resets.

## 6. Milestones

Each milestone ends with an acceptance task: the §5 milestone gates, a full
`make bench` (background) committed as `docs/perf/<task-id>.json`, a
`make bench-compare` against the phase-3 start and phase-2 baselines, and a
push. Order follows cost and risk: hygiene and profiling first, cheap
checksum work early, the hot loop last.

### M0 — Bench hygiene (P3.0)
- Everything in §4.2: the best/median rename and README, C zlib streaming rows, the baselines, the delta columns, environment capture, Canterbury per-file rows, and `zlibCompileFlags` in the oracle.
- **Acceptance:**
  - the phase-3 start baseline and noise estimate are committed;
  - every streaming row has a C zlib streaming counterpart;
  - `make bench-compare` shows deltas vs the phase-2 baseline.

### M1 — Profile first (P3.1)
- The §4.3 profiling extensions, and before-flamegraphs for the three required keys.
- `docs/perf/phase3-profile.md`, with each §1 pattern confirmed, adjusted or discarded by evidence.
- Re-order M2–M4's tasks inside their milestones by measured cost where the profile says so.
- A task may gate itself out with a recorded negative result if its hotspot is below max(2%, 2× noise) (PT2).
- **Acceptance:** the flamegraphs and profile report are committed, and every later task names its hotspot.

### M2 — Checksums (P3.5), then the one-shot output path (P3.2)
**Checksums.**
- CRC-32 is already slicing-by-8 from `const fn` tables (D-32), and Adler-32 already defers its modulo (D-23).
- Change them only if the M1 profile shows checksum time behind the zlib/gzip gap. Candidates:
  - slicing-by-16, measured against 8;
  - an Adler-32 inner loop unrolled over a documented largest-safe batch. The batch is derived from the u32 accumulator bound and recorded in DESIGN.md, extending D-23.
- Add checksum microbenchmarks (MiB/s per algorithm and per implementation variant, cleanflate only).
- **Acceptance:** T4 progress reported, and checksum microbench numbers committed.

**One-shot output path.**
- Pre-sizing is already done (D-31, D-33): the gzip ISIZE hint is capped by the output limit, `ONESHOT_MAX_PREALLOC` and the DEFLATE expansion bound, and raw/zlib use a ratio estimate with geometric growth.
- Revisit the hint only if the profile shows growth or zeroing cost.
- **Remove zero-fill-before-write.** One-shot decoding must stop decoding into a pre-zeroed slice. Instead it appends decoded bytes directly to the output `Vec`:
  - literals via push/extend;
  - matches via `extend_from_within` or equivalent safe copies from the already-written output.

  This needs an output-sink abstraction in the decoder core:
  - It is generic and monomorphised, so the caller-slice (streaming) and `Vec` (one-shot) sinks share one core.
  - The "one resumable decoder core" principle (CLAUDE.md) still holds.
  - The fast/reference split applies to both sinks.
  - It must stay within safe Rust. No `MaybeUninit` tricks, and no `unsafe` of any kind.
- If a sink design cannot beat the current path without breaking the one-core rule, record that as a negative result.
- **Acceptance:**
  - T2 (one-shot ≥ streaming on every row) is reported;
  - the §5 sink differential passes;
  - the bomb test and memory bounds are unchanged.

### M3 — Per-stream setup cost (P3.3)
- Reuse the window and Huffman-table allocations across blocks and across streams, and avoid zeroing large tables that are about to be fully overwritten. This is driven by the Canterbury profile.
- Add **`Decompressor::reset(&mut self)`**, and optionally `reset_with(format, options)` if the profile or API shape needs it.
  - It returns the decompressor to its freshly-constructed state while keeping its allocations.
  - It is documented in rustdoc and DESIGN.md, and covered by the §5 reset differential.
- Add the reuse bench case (§4.2).
- **Acceptance:** Canterbury ratio change reported per file with sizes, and T3 progress reported.

### M4 — Decode hot loop (P3.4; largest and riskiest, last)
- Some of this already exists:
  - the 64-bit bit buffer with bulk refill (D-27);
  - the fast loop with input/output margins (D-29);
  - two-level tables with swept widths (D-28, phase-2 P4-03).

  Phase 3 goes further, only where the M1/M4 profiles point:
  - **Branch-light refill.** When at least 8 input bytes are available, refill with a single word read and branch-free arithmetic.
  - **Bounds-check elision.** Restructure the fast loop so LLVM can prove its slice accesses in bounds in safe code (hoisted length checks, fixed-size array views, `chunks_exact`, iterators). Verify via generated assembly (§4.3).
  - **Table size.** Re-tune the primary lookup width (e.g. 9–11 bits plus subtables) on the final loop.
  - **Multi-literal entries (optional).** Pack more than one literal per table entry where the code lengths allow. Measure with and without, and keep the variant that wins by the PT1 rule. This is a general technique, and the table design must be derived in the project's own words.
- Level-1 streams are the target: literal-heavy.
- **Acceptance:** the level-1 rows improve measurably (a WIN verdict), all §5 gates are green, and the elision evidence is committed.

### M5 — Report and hand-off
- **After-flamegraphs** for the three required keys.
- **`docs/perf/PHASE3.md`** (the "round 3" report) containing:
  - phase-2 vs phase-3 tables, now including C zlib streaming;
  - T1–T6 with verdicts;
  - flamegraphs before and after;
  - per-change attribution: which task moved which rows, and by how much;
  - everything tried and reverted, with the reason and any regression fixture.
- Update `docs/PERF.md` with a "Phase 3" section pointing to PHASE3.md.
- Regenerate `docs/BUILD_REPORT.md` and `provenance/SUMMARY.md` scoped to phase 3 (set `PHASE_BASE` to the phase-3 SPEC commit).
- Update `provenance/human-interventions.md` with any human steps.
- **Tags:** the implementer does not tag. The human tags `v0.3.0-cleanroom` after review and merge.

## 7. Foundry run notes
- Base branch: `main`. The build branch uses Foundry's `build/` prefix and PRs into `main`.
- Review depth: up to **6** review rounds (`maxRounds: 6`).
- Phase-2 Foundry state, feedback, plan and reports are archived in `docs/phase-2/`. This phase starts with fresh Foundry state.
- Known Foundry friction to plan around:
  - `foundry_mutate` can hang on mutants that loop until the output limit, and can leave `crates/oracle/tests/chunking.proptest-regressions` modified.
  - Reviewers should restore that file before submitting.
  - Reviewers should prefer mutants that cannot run away.

## 8. Human prerequisites
- None new. `perf` and `cargo flamegraph` are installed; `cargo-asm` is not, and is not required (§4.3).
- Long fuzz runs (§5 milestone gates) run on the build machine during the flight, in the background. If the operator needs the machine, milestone fuzz may be shortened to what fits. Record the actual duration in the acceptance commit and list the shortfall as a manual check.

## 9. Definition of done
- Every §3 constraint holds, and every correctness gate in §5 is green at every milestone.
- The bench compares like with like, with environment capture, baselines and deltas.
- T1–T6 are met, or each shortfall is reported with measurements and the next most promising step.
- Every optimisation is written up in DESIGN.md per PT7, and cites the profile evidence that motivated it.
- `docs/perf/PHASE3.md` is complete.
- The provenance log shows no forbidden reads, and any denials are explained.

## 10. Out of scope
- `unsafe`, SIMD intrinsics, `std::arch`, nightly `std::simd`, multithreading, and toolchain upgrades.
- Comparator crates in this repo (use `cleanflate-bench`).
- Compression, `no_std`, C ABI.
- Any change to what the oracle asks C zlib to do beyond timing it (one-shot and chunked streaming) and reading its version and build flags.

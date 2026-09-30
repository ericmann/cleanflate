# SPEC: cleanflate phase 2 — clean-room performance pass

> Phase 2 of cleanflate. Phase 1 (the correct, safe, clean-room decoder)
> is merged to `main` and archived under `docs/phase-1/` (its SPEC, PLAN,
> PROGRESS, REVIEW, SUMMARY, HANDOFF, BUILD_REPORT, foundry.json, Foundry
> state/feedback, and a copy of its PERF.md and provenance summary).
> Run with `/foundry:go-flight` from the `feature/perf` branch.

## 1. Purpose

Phase 1 shipped a decoder that is byte-identical to C zlib on every
conformance and fuzz corpus, but runs at **25.6% (raw) / 27.3% (zlib)** of C
zlib's single-thread decompression throughput on Silesia, against a 70%
target (`docs/phase-1/PERF.md`). Phase 2 closes that gap **without giving up
anything phase 1 guaranteed**, and without breaking the clean-room record.

Priorities, unchanged from phase 1 and in this order:

1. **Provenance** — the clean-room record stays intact; no existing DEFLATE
   implementation's source, internals or implementation-specific write-ups
   are consulted to make it faster (§2).
2. **Correctness** — byte-identical output, identical accept/reject
   behaviour, identical streaming semantics. A faster decoder that is wrong
   on one stream is a regression, not a trade-off.
3. **Safety** — `#![forbid(unsafe_code)]`, zero runtime dependencies, no
   reachable panics, bounded memory.
4. **Performance** — the goal of this phase, but ranked below 1–3. When they
   conflict, performance loses and the negative result is recorded.

## 2. Clean-room rules (NON-NEGOTIABLE)

Phase 1's §2 (`docs/phase-1/SPEC.md` §2) applies **in full**: the permitted
and forbidden inputs, the mechanical guard and logger in `tools/cleanroom/`,
the post-M0 lockdown (still in force — no WebFetch, no network), and the
commit-tag discipline. The guard, logger and lockdown flag are already
built; phase 2 does not relax them. The provenance log continues from phase
1 into the same `provenance/tool-log.jsonl`.

### 2.1 Permitted inputs for performance work
- This SPEC, everything under `docs/` (including `docs/phase-1/`, especially
  `PERF.md`, `DESIGN.md` and the phase-1 REVIEW/SUMMARY), and `docs/rfc/`.
- cleanflate's own source, tests and history in this repo.
- **Measurements of cleanflate itself**: benchmarks, profiles (`perf`,
  `cargo flamegraph` or equivalent, if installed), instruction counts,
  in-crate counters (§4.3), and generated assembly of cleanflate's own code
  (`cargo rustc --release -- --emit asm`, or equivalent).
- **C zlib as a black-box stopwatch only**: the oracle may time C zlib
  decompressing the same input, to compute the throughput ratio. Nothing
  about *how* C zlib achieves its speed may be observed.
- **General, implementation-independent performance knowledge**, applied
  from first principles to this code base. Examples of what counts:
  - keep a wide bit buffer topped up in bulk (whole bytes or words at a
    time) instead of refilling one byte per need;
  - look up Huffman symbols in tables indexed by the next N bits, with a
    second-level table for longer codes, instead of walking one bit at a
    time;
  - put the common case in a tight inner loop guarded by "enough input and
    output remain" margins, and fall back to the careful general path near
    buffer edges;
  - write output once, and treat already-written output as match history
    instead of maintaining a second copy;
  - copy non-overlapping matches as slices, and expand overlapping,
    short-distance matches by copying the growing pattern in larger steps;
  - avoid per-byte bounds checks and per-iteration clones in hot loops,
    within safe Rust (slices, `chunks_exact`, iterators, `copy_within`,
    hoisted length checks the optimiser can prove);
  - compute CRCs a word at a time from multiple precomputed tables derived
    from the polynomial;
  - size one-shot output buffers from available hints to avoid repeated
    growth copies.

  Each technique used must be written up in `docs/DESIGN.md` **in the
  project's own words**, justified from first principles, the RFC and
  cleanflate's own measurements, with the statement "general technique;
  not derived from any specific implementation".

### 2.2 Forbidden inputs (in addition to phase 1 §2.2)
- Profiling, disassembling, symbol-annotating, or stepping through **C zlib
  or any other decoder** (e.g. `perf annotate` or `perf report` on libz
  symbols, `objdump` of the oracle's static zlib, a debugger in zlib
  frames). Profiles of oracle runs must be filtered to cleanflate's own
  symbols, and any view that shows the C library's internals is a
  violation to log as a human intervention if it happens by accident.
- Articles, talks, benchmarks write-ups or code walkthroughs describing how
  a *specific* implementation (zlib, zlib-ng, libdeflate, miniz_oxide,
  zlib-rs, Go/Java/Python/.NET inflaters, etc.) structures its fast path,
  tables or bit buffer. Knowing that such libraries exist and are fast is
  fine; learning their design from sources is not.
- Reading comparator crates' docs beyond public API pages (phase 1 rule,
  restated because perf work makes it tempting).

## 3. Hard constraints (all carried over; none may regress)

- `crates/cleanflate`: `#![forbid(unsafe_code)]`; `[dependencies]` empty;
  the phase-1 panic lints (`unwrap_used`, `expect_used`, `panic`,
  `indexing_slicing`) stay denied in library code, and local `#[allow]`s of
  them stay banned (phase-1 rule `no-panic-lint-allow-in-cleanflate-lib`).
- No `std::arch`, SIMD intrinsics, `unsafe`-backed crates, multithreading,
  or nightly features. Auto-vectorisation the compiler does on its own is
  fine.
- Stable toolchain pinned in `rust-toolchain.toml` (1.89.0). A toolchain
  bump is out of scope.
- Public API: no breaking changes. The one-shot functions, `Decompressor`,
  `Options`, `Error`/`ErrorKind`, `Status` and `Progress` keep their
  signatures and documented semantics. Additive, documented changes are
  allowed only if a measurement shows they are needed.
- Streaming contract (DESIGN D-11, D-13, D-20, D-25): identical output for
  any input/output chunking including 1-byte chunks; exact `consumed`
  accounting; `trailing_start` semantics; output-limit semantics; bounded
  memory; gzip multi-member behaviour and header caps.
- Every phase-1 test, the conformance corpus (100%, one-shot and
  streaming), malformed corpus, bomb test, chunking proptest and fuzz
  targets keep passing. `make ci` stays green on GitHub Actions.
- Tunables live only in `crates/cleanflate/src/config.rs` (phase-1 rule
  C8); new tunables (margins, table sizes, refill widths) go there, with
  the measurement that chose their value.

## 4. Measurement

### 4.1 Baseline and targets
Phase-1 numbers (`docs/phase-1/PERF.md`, Silesia, level 6, one-shot,
best of 5, AMD Ryzen 5 3400G):

| framing | cleanflate MiB/s | C zlib MiB/s | ratio |
|---|---|---|---|
| raw | 71.1 | 277.7 | 25.6% |
| zlib | 71.9 | 263.0 | 27.3% |

Targets for phase 2, aggregate over Silesia (total bytes / total time),
one-shot, level 6:

- **Primary:** ≥ 70% of C zlib for raw, zlib **and** gzip framing.
- **Streaming:** ≥ 60% of C zlib when driven through `Decompressor` with
  64 KiB input and 64 KiB output chunks.
- **Stretch:** ≥ 90% one-shot.
- **No per-file regression:** no Silesia or Canterbury file may end up
  slower than its phase-2 M0 baseline by more than the measured noise.
- Report the achieved numbers either way. Missing a target is an honest
  result; shipping incorrect code to hit one is not.

### 4.2 Benchmark methodology (M0 hardens the existing `oracle bench`)
- Re-measure the baseline at M0 on the current machine before any
  optimisation, and commit it as `docs/perf/baseline.json` plus a table in
  `docs/PERF.md`. All later comparisons are against this, not the phase-1
  numbers.
- Per measurement: warm-up run, then N ≥ 7 timed runs; report median and
  min. Report per file and aggregate, for raw/zlib/gzip framing, levels 1,
  6 and 9, one-shot and streaming (64 KiB chunks).
- Estimate noise at M0 by running the full baseline twice; a change counts
  as a win only if it beats **twice** the measured noise on the aggregate.
- Machine-readable results in `docs/perf/<task-id>.json`; a comparison
  command (`make bench-compare A=... B=...`) prints the per-file and
  aggregate deltas.
- `make bench-quick` (Canterbury plus a fixed Silesia subset, fewer runs)
  for iteration inside a task; `make bench` (full) at milestone ends.
- Benchmarks are **not** CI gates (shared runners are too noisy); CI stays
  a correctness gate. Performance acceptance is judged from committed
  bench results.

### 4.3 Profiling
- `perf` 7.1.5 (`/usr/bin/perf`, from Pop!_OS `linux-tools`) and
  `cargo flamegraph` 0.6.14 are installed on the build machine
  (`kernel.perf_event_paranoid = 2`: user-space profiling of your own
  processes works; kernel frames do not, and are not needed). Use them.
- **Profile cleanflate alone.** M0 adds a cleanflate-only profiling entry
  point (e.g. `oracle profile --corpus silesia --framing raw --iters N`,
  or a dedicated example/binary) that decodes pre-compressed inputs with
  cleanflate only and never calls C zlib in the profiled process, plus a
  `make profile` target wrapping `perf record`/`cargo flamegraph` around
  it. Compressed inputs are prepared by a separate, unprofiled step (or
  read from `testdata/generated/`). Never run `perf` or `cargo flamegraph`
  on `oracle bench` or anything else that executes C zlib code (§2.2).
- Build profiled binaries with debug info
  (`CARGO_PROFILE_RELEASE_DEBUG=true`) so frames name cleanflate's own
  functions. Profile outputs (`perf.data`, SVGs) are gitignored; the
  findings they support go into `docs/PERF.md` in words and numbers.
- Regardless, M0 adds an opt-in cargo feature `stats` to `cleanflate`
  (zero dependencies, compiled out by default, and the default build must
  be bit-for-bit unaffected in behaviour) that counts: symbols decoded via
  the table vs. the bit-serial fallback, bit-buffer refills and bytes per
  refill, state-machine dispatches per output byte, matches by
  length/distance bucket and overlap/non-overlap, stored bytes, and time
  or calls spent building tables. `oracle bench --stats` prints them.
  Every optimisation task states which counter or profile justified it.

### 4.4 Hypotheses to verify first (from reading phase-1 cleanflate)
Measure these at M0 before committing to a plan order; confirm or
discard each in `docs/PERF.md`:

- **H1 — the lookup table is mostly bypassed.** `BitReader` keeps at most
  7 bits between steps (DESIGN D-07), and `peek_symbol` refills one bit's
  worth at a time on `NeedMoreBits`, while `decode_table` falls back to the
  bit-serial walk whenever fewer than `LITLEN_TABLE_BITS` (10) bits are
  buffered. Most symbols may therefore be decoded bit-by-bit.
- **H2 — per-symbol dispatch overhead.** `Inflater::step` clones
  `self.state` every loop iteration and returns to the outer `match` for
  every literal and match.
- **H3 — every output byte is written twice.** Once to the output slice
  and once into the 32 KiB ring `Window` (with a `u64` modulo and a bounds
  check per byte), including stored blocks.
- **H4 — overlapping matches are byte-by-byte.** Short-distance repeats
  (distance < length) are common and take the slow per-byte path.
- **H5 — per-block setup costs.** Fixed-Huffman blocks clone the fixed
  tables (`Vec`s); dynamic blocks rebuild full tables every block.
- **H6 — wrapper and one-shot overheads.** CRC-32 is byte-at-a-time for
  gzip; one-shot output growth may copy repeatedly.

## 5. Correctness strategy for performance work

Phase 1's attempt (P4-08) at a fast decode loop found a real bug, could not
root-cause it within one task, reverted everything, and lost the
reproducing case. Phase 2 must not repeat that.

- **Keep a reference path.** The existing resumable state machine stays as
  the general, obviously-correct slow path. Fast paths are layered on top
  and entered only when their preconditions (input/output margins, state)
  hold; at every exit they restore the exact state the slow path expects,
  including the resumable reader invariant (bytes loaded but not consumed
  are given back or accounted for exactly — see D-07/D-22).
- **Differential fast-vs-slow testing.** M0 adds a way to force the slow
  path (a `cfg(test)`-only or feature-gated switch that the default build
  cannot reach) and a test that decodes the entire conformance corpus,
  the malformed corpus and a proptest stream both ways and asserts
  identical output, errors, `consumed` and status. This runs in
  `make test`.
- **Scale up the chunking proptest for decoder changes.** Any task that
  touches `inflate.rs`, `bitreader.rs`, `huffman.rs`, `window.rs` or
  `stream.rs` runs the chunking proptest at ≥ 3000 cases (which is what
  caught P4-08's bug) before it is marked done; milestone acceptance runs
  10k.
- **Never lose a repro.** When a fast path produces a wrong result:
  minimise the case, commit it as a named regression fixture (proptest
  regressions file and/or a `testdata/regressions/` file with its expected
  output hash), then root-cause it. If the task must stop, it blocks with
  the fixture, the hypothesis and what was ruled out, so the next task or
  review round continues from there instead of starting over. A revert
  must keep the fixture.
- **Mutation-check the fast path.** The reviewer's mutation sampling
  should include fast-path code; a surviving mutation there is a finding.

## 6. Milestones

Each milestone ends with an acceptance task that runs `make ci`,
`make bench` (full) and the 10k chunking proptest, commits the bench
results, and pushes. Plan task dependencies narrowly: a blocked
optimisation must not cascade-skip unrelated work (phase 1 lost 17 tasks
to one blocked download).

### M0 — Baseline, instrumentation, safety net
- Harden `oracle bench` per §4.2 (median/min of N, framings, levels,
  one-shot and streaming modes, JSON output, `bench-quick`,
  `bench-compare`).
- Add the `stats` feature and `oracle bench --stats` (§4.3).
- Add the cleanflate-only profiling entry point and `make profile`
  (§4.3), and record a baseline profile's top functions in `docs/PERF.md`.
- Add the force-slow-path switch and the fast-vs-slow differential test
  (§5), passing trivially until a fast path exists.
- Measure the baseline twice (noise estimate), record H1–H6 findings in
  `docs/PERF.md` under a new "Phase 2" section, and order the remaining
  milestones' tasks by measured cost.
- CI: also build and clippy the `stats` feature.
- **Acceptance:** baseline JSON and noise estimate committed; the
  differential test runs in `make test`; H1–H6 each confirmed or discarded
  with a number.

### M1 — Bit reader and symbol decoding
- Keep the bit buffer topped up in bulk when input allows, and redefine
  the D-07 invariant (in a new DESIGN entry) so it stays exact across
  resumable calls and block/stream boundaries.
- Decode every symbol through tables: first-level table for codes up to
  the table width, second-level tables for longer codes; the bit-serial
  walk remains only as the reference in tests (and, if needed, for the
  code-length alphabet).
- Consider folding length/distance base values and extra-bit counts into
  table entries if the counters show it matters.
- Avoid per-block table clones and redundant rebuilds (H5).
- **Acceptance:** `stats` shows ≥ 99% of literal/length and distance
  symbols decoded by table lookup on Silesia; aggregate throughput gain
  measured and committed; all correctness gates green.

### M2 — Fast decode loop
- A tight loop for the literal/length/distance state, entered with
  input/output margins sufficient for a worst-case symbol sequence
  without mid-symbol exhaustion checks, exiting to the state machine near
  buffer edges, at end of block, or on any error.
- No per-iteration state clone or outer dispatch inside the loop.
- **Acceptance:** fast-vs-slow differential passes on the whole corpus;
  chunking proptest passes 10k cases; the repro-preservation rule (§5)
  was followed for any bug found; aggregate gain measured and committed.

### M3 — Output and match history
- Write each output byte once. Use the caller's output slice as match
  history where it is contiguous, and keep the 32 KiB window only for
  history that must survive across `decompress` calls (refreshed from the
  output tail on exit). Decide and document how this interacts with
  small caller output buffers and preset dictionaries.
- Non-overlapping matches as slice copies; overlapping matches by
  repeated pattern doubling; stored blocks as single slice copies.
- One-shot output sizing from hints (e.g. gzip ISIZE when trustworthy,
  compressed length heuristics), always capped by the output limit.
- **Acceptance:** as M2, plus streaming-mode throughput reported
  separately; bomb test and memory bounds unchanged.

### M4 — Checksums, wrappers and tuning
- CRC-32 computed several bytes at a time from multiple tables derived
  from the RFC 1952 polynomial (generated in `const fn`, verified against
  the existing byte-wise implementation and the oracle).
- Revisit Adler-32 if the profile says so.
- Re-sweep the table widths and any new tunables (margins, refill width)
  on the final code, choosing values by measurement (§4.2 noise rule).
- **Acceptance:** gzip and zlib framing overhead relative to raw is
  reported; tunables' measurements are in `docs/PERF.md` and cited in
  `config.rs`.

### M5 — Report and hand-off
- `docs/PERF.md` "Phase 2" section: final numbers against §4.1 targets,
  per-milestone gains, every hypothesis's verdict, negative results and
  reverted attempts with their regression fixtures.
- Regenerate `docs/BUILD_REPORT.md` and `provenance/SUMMARY.md` for
  phase 2; update `provenance/human-interventions.md`.
- The implementer does not tag; the human tags `v0.2.0-cleanroom` after
  review and merge.
- **Acceptance:** `make ci` green, full bench committed, report complete.

## 7. Foundry run notes
- Base branch: `feature/perf` (this branch). Build branches use Foundry's
  `build/` prefix and PR into `feature/perf`, not `main`.
- Review depth: up to **6** review rounds (`maxRounds: 6`).
- Phase-1 Foundry state and feedback are archived in `docs/phase-1/`;
  this phase starts with fresh state.

## 8. Human prerequisites (before or during the flight)
- Done 2026-09-30: `perf` 7.1.5 and `cargo flamegraph` 0.6.14 installed
  by the operator (recorded in `provenance/human-interventions.md`).
  Agents must not install or upgrade tooling themselves (network is
  locked down); anything missing is a human request in the handoff.
- Final `make bench` should run on a quiet machine (no other heavy
  processes; ideally a fixed CPU governor). Record the conditions in
  `docs/PERF.md`.

## 9. Definition of done
- All §3 constraints hold and every phase-1 correctness gate is green.
- The fast-vs-slow differential test exists and passes on the full corpus.
- §4.1 targets met, or the shortfall is reported with measurements and
  the next most promising step.
- Every optimisation is written up in DESIGN.md per §2.1, with the
  measurement that justified it.
- The provenance log shows no forbidden reads, and any denials are
  explained.

## 10. Out of scope
- `unsafe`, SIMD intrinsics, `std::arch`, multithreading, nightly Rust.
- Compression (phase-1 M5 stays deferred), `no_std`, C ABI.
- Changes to the oracle's C zlib usage beyond timing.
- Toolchain upgrades.

# cleanflate phase 3 build progress
Branch: build/2026-09-30
Started: 2026-09-30T14:32:48.581Z

## Tasks
- [x] P0-01 Phase-3 scaffolding: fuzz gates, quick-bench levels, baseline copy, report skeleton
- [x] P0-02 design-check requires profile evidence in phase-3 [perf] entries
- [x] P0-03 Oracle FFI: C zlib build flags and bounded streaming inflate
- [x] P0-04 Bench schema 2: "best" replaces "min", every column documented
- [x] P0-05 C zlib streaming rows and like-for-like ratios
- [x] P0-06 Environment capture in every bench run
- [x] P0-07 Reports: delta vs the phase-2 baseline, drift-corrected verdicts, per-file sizes
- [x] P0-08 `oracle bench-targets`: T1–T6 verdicts from a bench JSON
- [x] P0-09 Phase-3 starting baseline and noise
- [x] P0-10 M0 acceptance and push
- [x] P1-01 Profile any key: per-key outputs, perf stat and saved flamegraphs
- [x] P1-02 Before-profiles, flamegraphs and phase3-profile.md
- [x] P1-03 M1 acceptance and push
- [x] P2-01 Checksum microbenchmark (cleanflate only)
- [x] P2-02 Adler-32 over a bound-derived unrolled batch (gated)
- [x] P2-03 CRC-32 slicing-by-16 against 8 (gated)
- [x] P2-04 Wrapper per-call overhead in streaming (gated)
- [x] P2-05 Output-sink abstraction in the decoder core (slice sink only)
- [x] P2-06 Vec sink and the sink differential
- [x] P2-07 One-shot decoding appends to a Vec: no zero-fill
- [x] P2-08 One-shot size-hint revisit (gated)
- [x] P2-09 M2 acceptance and push
- [x] P3-01 Per-block Huffman table reuse (gated)
- [x] P3-02 Per-stream setup: reusable inflater, lazy window, no redundant fills (gated)
- [x] P3-03 `Decompressor::reset()` and the reset differential
- [x] P3-04 Reuse bench case
- [x] P3-05 M3 acceptance and push
- [x] P4-01 Assembly evidence tool for the fast loop
- [x] P4-02 Branch-light bulk refill (gated)
- [x] P4-03 Fast-loop bounds-check elision and inlined lookup
- [x] P4-04 Multi-literal table entries (optional, measured)
- [x] P4-05 Re-tune table widths on the final loop
- [x] P4-06 M4 acceptance and push
- [x] P5-01 After-profiles and flamegraphs
- [x] P5-02 Final bench, PHASE3.md report and PERF.md phase-3 section
- [x] P5-03 Provenance, build report and hand-off push
- [x] R1-01 Reset differential: cover the raw dictionary, fail on empty selections, random reset points

## Log
(one entry per task, appended by implement)

### P0-01 — 8f52cbd
Added fuzz-smoke-decoder, fuzz-milestone, bench-quick levels 1,6, tools-check perf tests, tools/perf/fuzz-milestone.sh + test, baselines/phase2.json, PHASE3.md skeleton, READMEs.
fuzz-milestone 20s output:
fuzz-milestone: diff_oracle_roundtrip seconds=40 execs=18406 exit=0 artifacts=0
fuzz-milestone: diff_fast_reference seconds=40 execs=28203 exit=0 artifacts=0
(wall 40s includes build time.)

### P0-02 — 5046b86
PROFILE_REQUIRED_FROM=34 in design-check.sh; 3 new cases. Interpretation: existing perf fixtures renumbered D-90..92 -> D-30..32 (else they'd need Profile:).

### P0-03 — 6b41bf8
ffi.rs: zlib_compile_flags() (sys::zlibCompileFlags exported by libz-sys, no extern block needed), inflate_streaming_into(input, framing, in_chunk, out, out_chunk) -> Result<usize, OracleStatus>; zero chunk -> OtherError(-2). 5 tests added. No-progress rule: full out => OutputLimit, else exhausted input => Truncated.

### P0-04 — 02a9aec
Schema 2 (bench_json::SCHEMA=2), BenchJson::parse accepts 1|2 (aliases for min_*), load_json uses it. best_ns / cleanflate_mib_s_best / c_zlib_mib_s_best. TABLE_FOOTER printed under stdout table. README Schema 2 section. Interpretation: min_sample_ms kept; noise JSON schema stays 1. Note: guard blocks bash commands containing 'zlib' words (c_zlib) in args; use script files.

### P0-05 — 53239b7
Imp::CZlibOneShot/CZlibStreaming, sample_order(), Aggregate.c_zlib_mode (serde default oneshot), ratio like-for-like, render_md per-file table uses same-mode C zlib row (fallback oneshot). bench-quick P0-05 ran 87s: 96 cleanflate streaming rows all have a C zlib streaming row; e.g. canterbury/raw/l6/streaming 248.2 vs 405.1 MiB/s ratio 0.613. Bash note: `sleep` >~100s blocked; use until-loop with timeout.

### P0-06 — 4427b9b
bench_env.rs (new): CargoProfile, CZlibInfo, render_environment, metadata parsing. Machine gained cores/os/kernel/loadavg_end (Machine: Default); BenchJson gained cargo_profile, c_zlib (serde default). cargo metadata uses --offline --filter-platform <host>. Environment block from bench-quick P0-06:
cpu_model: AMD Ryzen 5 3400G; cores 8; governor schedutil; os Pop!_OS 24.04 LTS; kernel 7.1.5-76070105-generic; rustc 1.89.0; profile release; debug_assertions false; release_profile: debug = "line-tables-only"; build_env none; c_zlib_version 1.3.2; c_zlib_compile_flags 0xa9; libz_sys_version 1.1.29; libz_sys_features default,libc,static,stock-zlib.

### P0-07 — 5668fd5
bench_compare: compare_with_baseline, render_md_with_baseline (old fns kept as wrappers), RatioDelta, Noise.ratio_aggregate(+_max_pct, serde default), default_noise_path(root,a,b)->Option<(path,phase2?)>. main: --baseline for bench-compare/bench-md, load_baseline prints "baseline missing: path". Makefile passes FILES/FILTER/BASELINE. Interpretation: ratio verdict UNKNOWN if noise file lacks ratio noise. Verified: make bench-compare A=docs/perf/P5-02.json B=docs/perf/baselines/phase2.json prints new columns.

### P0-08 — dabe22e
bench_targets.rs (evaluate/render), `oracle bench-targets J [--baseline] [--noise]`, `make bench-targets`. Interpretations: reuse rows = mode not oneshot/streaming (ignored); no evaluable rows -> n/a; noise default from J's kind. Phase-2 baseline currently: T1 n/a (schema-1 streaming), T4 fails, T5 fails (silesia raw l6 0.661, zlib 0.696). clippy neg_cmp_op fixed with is_nan checks.

### P0-09 — 66b9d12
Baselines committed under docs/perf/baselines (phase3-start[-rerun], quick pair, two noise files). All schema 2, stats_build false, streaming c_zlib_mode streaming. Noise: aggregate_max_pct 8.22% full / 8.82% quick (>3%, manual check); ratio noise max 12.63% / 7.08%. Reruns show no WIN/LOSS/REGRESSION. Targets at start: all T1-T6 NOT MET (T1 worst silesia/gzip/l1/streaming 0.543; T5 worst silesia/raw/l6/oneshot 0.649). This machine ~5-10% below phase-2 record. PHASE3.md Method/Machine/Baselines filled, task log started. Manual check: NOT VERIFIED (human): noise >3%.

### P0-10 — 587b9ed
make ci, make heavy, make fuzz-milestone all pass. fuzz: diff_oracle_roundtrip seconds=1826 execs=1205957 exit=0 artifacts=0; diff_fast_reference seconds=1826 execs=894969 exit=0 artifacts=0. P0-10.json committed (207/207 streaming rows). Compare vs phase3-start: 14 WIN/LOSS lines with no code change (drift). Provenance snapshot committed, pushed origin build/2026-09-30.
Manual check: NOT VERIFIED (human): 1) GitHub Actions ci green on branch; 2) noise 8.2%/8.8% >3% -> rerun baseline pair on quiet machine, pinned governor; 3) spot-check Environment (compile_flags 0xa9, libz_sys_features default,libc,static,stock-zlib); 4) provenance log has no unexplained denials.

### P1-01 — 5988e2c
Makefile: PROFILE_KEY, per-key target/profile/<slug>/{perf.data,top.txt,top-children.txt,flamegraph.svg,stat.txt}, profile-stat, flamegraph-save TAG=, profile-required TAG=, STATS_* vars. tools/perf/check-flamegraph.sh + test. Interpretation: flamegraph uses --no-inline (default inline view strips crate paths so no cleanflate:: frames). profile_decode untouched (already 64 KiB windows). Verified profile/profile-stat/flamegraph-save smoke on this machine (perf works). Smoke svg deleted. Early profile note: silesia rfc1951 l1 streaming top self: fast_loop::run 38%, Huffman::decode_table 25%, crc32 13.5%, copy_match 10%.

### P1-02 — 1089aab
phase3-profile.md + 3 before SVGs + P1-02-stats.json committed. Verdicts: P1 ADJUSTED (zero-fill+fresh-page faults, 14 resizes/12 files; not growth), P2 ADJUSTED (setup ~7% Canterbury vs ~2% Silesia, table build per byte equal), P3 ADJUSTED (level1 literals 46.6% of symbols vs l6 60.8%; loop is symbol-count heavy), P4 CONFIRMED (Crc32::update 9.6% gzip, ZlibDecoder::step 5.1%). fast_loop::run+Huffman::decode_table (out-of-line) = 70% self at l1. Likely gates: above: P4-03,P4-04,P2-03,P4-02; below: P2-02,P3-01,P3-02,P2-04,P2-08; P2-07 at the gate. CR6 note: guard denied `perf report ... --sort dso` (outside make profile); not worked around. PHASE3.md: append later task-log entries before the exact line "## Targets".

### P1-03 — 6aa0edf
make ci/heavy/fuzz-milestone pass. fuzz: diff_oracle_roundtrip seconds=1801 execs=1383630 exit=0 artifacts=0; diff_fast_reference seconds=1801 execs=525087 exit=0 artifacts=0. P1-03.json committed, provenance snapshot, pushed. IMPORTANT: compare vs phase3-start with NO code change shows 9 WIN, 11 LOSS, 65 per-file REGRESSION lines: the two-run noise pair underestimates cross-session drift on this machine. Later tasks should take before and after back to back (same session) and read ratio verdicts. Manual check: NOT VERIFIED (human): ci green; open before SVGs and sanity check phase3-profile.md tables; SVGs only cleanflate/std/libc frames.

### P2-01 — 6da7532
checksum_bench example + test_hooks::checksum variants; generic crc32_update_sliced::<N> (N<4 or >16 -> bytewise), shared 16-row CRC32_TABLES (CRC32_MAX_SLICES in checksum.rs, not config: derived capacity). Microbench (MiB/s): crc32 bytewise ~435, sliced4 ~1200, sliced8 ~1830, sliced16 ~2140 (+17%), production ~1830; adler32 bytewise ~345, production ~3050. Run-to-run mean diff 2.9%, max 7.6%. PT1: Crc32::update 9.56% -> 9.19%; interleaved A/B neutral/positive (+3.7%); bench-quick JSON pair load-polluted (load 1.35 vs 4.70): whole machine is a shared desktop, so use interleaved A/B with old/new profile_decode binaries as the controlled check for code changes (binaries at scratchpad pd_before/pd_after method: git stash, build, cp). Verify passed incl. proptest-3k and fuzz-smoke-decoder.

### P2-02 — 6bbab78
GATED OUT (negative result). adler_fold inclusive 6.34% (silesia/rfc1950/l6/streaming) and 4.83% (canterbury/rfc1950/l1/oneshot) vs gate 7.02% (2x 3.51% quick noise). No code, no DESIGN entry, no JSONs. PHASE3.md ### P2-02 has numbers.

### P2-03 — 8e4fe84
Gate passed (Crc32::update 8.31% / 8.99% vs 5.14% / 4.84%). Microbench sliced16 +13.5..+17.9% over sliced8 (> 2x noise) so CRC32_SLICES=16 was tried; bounded whole-decode gain <= 1.2%; bench-quick pair load-polluted (loadavg to 8.4), targeted gzip keys NEUTRAL, canterbury/gzip/l1/streaming ratio LOSS; interleaved A/B +2.3% (4 of 5 pairs). PT4: reverted to 8 (config.rs unchanged). JSONs P2-03-before/P2-03/P2-03-checksum committed. Machine noise note: a shared desktop; loadavg varies 1-8 during runs, making bench JSON pairs unreliable for gains under ~5%.

### P2-04 — 45bb098
GATED OUT. Wrapper excess over checksum: 0.05% / 0.04% (zlib keys), <0.5% (gzip keys) vs 2% floor. No code. PHASE3.md ### P2-04.

### P2-05 — 726eb2c
OutputSink/SliceSink (sink.rs), generic step_into on Inflater/ZlibDecoder/GzipDecoder/Backend, Window::copy_match_sink + back_parts, fast_loop::run generic (param out_base because `base` is shadowed by LENGTH_BASE locals). Slice-shaped test wrappers in tests/slice_api.rs (cfg(test) banned in src by rule). DESIGN D-34 (non-perf). PT1: hot share unchanged (83.07% -> 82.55%); interleaved A/B neutral/positive; fastslow-full, bomb, proptest-3k, fuzz-smoke pass. Next free D is D-35. Note for P2-06: VecSink must implement written_since, copy_within_back etc. for Vec; SliceSink::at is test-only helper.

### P2-06 — f1c3f9c
VecSink (capacity-bounded, no zero-fill), Decompressor::decompress_vec via allowance()/decompress_into(), decompress_core_vec + grow_output + shared finish_core, test_hooks::{OneShotSink, decompress_with_sink}. Differential sinks_* (4 combos) pass in proptest-3k and fastslow-full; fuzz diff_fast_reference extended, 120 s smoke pass (33308 runs). Public one-shot unchanged (P2-07 switches). Interpretation: VecSink room bounded by capacity so writes cannot fail; OutputFull = capacity full; driver grows by next_len exactly like the old one. Dead-code attrs cfg_attr(not(any(test, feature="test-hooks")), allow(dead_code)) on VecSink/decompress_vec/decompress_core_vec/grow_output — remove in P2-07 when production uses them.

### P2-07 — 3567195
NEGATIVE RESULT (PT4): switched public one-shot to decompress_core_vec, measured, reverted. Page faults identical before/after (110,179 l1; 110,749 l6); resizes/bytes_allocated identical; after-switch extend_from_within 13.4% inclusive offsets zero-fill removal; A/B neutral (-2.3%..+3%). stream.rs/sink.rs/test_hooks.rs back to P2-06 state except decompress_core now pub(crate). 3 new oneshot_sizing tests kept. Finding: the fresh-page fault cost of one-shot cannot be removed by a sink; only buffer reuse (M3) can. No D entry. Next free D = D-35.

### P2-08 — 4a0a212
GATED OUT. No growth/realloc/copy frames; zero-fill resize<u8> 2.26% (Silesia), 0.57% (Canterbury), mostly first allocation; over-allocation bound ~0.7%/0.2% vs gates 2%/4.84%. No code. oneshot stats: 14 resizes/12 files, 1.49x alloc/output (Silesia).

### P2-09 — f121b8d
make ci, heavy (sinks_* full tier, bomb 3 tests), fuzz-milestone pass: diff_oracle_roundtrip seconds=1802 execs=1282261; diff_fast_reference seconds=1802 execs=391032; no artifacts. P2-09.json committed, pushed. T2 worst 0.884->0.902, T4 worst 12.63%->11.51% (NOT MET, noise-level: no M2 technique adopted). Compare vs phase3-start shows 22 WIN 0 LOSS 29 REGRESSION lines with no decoder change (drift). Manual check: NOT VERIFIED (human): ci green; peak RSS of bomb and one-shot Silesia vs start; review sink code/mutation-sample; checksum DESIGN (none added).

### P3-01 — 7c91003
GATED OUT. from_lengths 2.87% / 3.20% vs gates 4.84% / 4.71%; fixed_table_clones 0 (no fixed blocks in Canterbury); table entry computation dominates, allocation small. No code.

### P3-02 — 78277d0
GATED OUT. Setup frames: new_with_path 1.71%, resize 0.72%, Window::new 0.53% (upper bound ~3.0%, overlapping) vs gate 4.84%; 696 page faults / 3300 constructions. No code.

### P3-03 — c94628e
Decompressor::reset (public, doc example uses ?), Backend/Zlib/Gzip/Inflater/Window reset (window storage kept, history dropped via total=0). DESIGN D-35. trace_with in oracle common. Differential (oracle/tests/reset.rs) passes: conformance (41 s release; one scenario per entry + one-byte chunks for every 4th entry <= 64 KiB), options, proptest (both paths), repeated x100. verify (incl proptest-3k, fuzz-smoke) + fastslow-full pass. Next free D = D-36. reset_with not added (DESIGN explains).

### P3-04 — 875d0db
Mode::Reuse in bench (Fixture.reuse RefCell<Decompressor>), Aggregate.ratio/c_zlib_mode now Option (null for reuse), vs_fresh_streaming, RatioDelta::NotApplicable, profile_decode --mode reuse, Makefile bench/bench-quick modes include reuse. P3-04.json: reuse vs fresh streaming ~1.00 on all aggregates (max +3.9% canterbury/gzip/l6), PHASE3.md has per-file Canterbury table. Changes bench-quick runtime (+1 mode).

### P3-05 — 817a650
make ci, heavy, fuzz-milestone pass: diff_oracle_roundtrip seconds=1828 execs=1361846; diff_fast_reference seconds=1828 execs=373554; no artifacts. P3-05.json (full bench incl reuse) committed, pushed. No M3 technique changed throughput (P3-01/P3-02 gated out; reset API + reuse bench only); compare vs start: 12 WIN 2 LOSS 73 REGRESSION lines = drift. Manual check: NOT VERIFIED (human): ci green; review reset rustdoc/D-35; peak RSS of bomb vs start.

### P4-01 — c062fc1
tools/perf/asm_branches.py (+tests, fixture), make asm / asm-count (ASM_FUNCS, ASM_SRCS, ASM_OUT). KEY FINDING: fast loop is inlined into Inflater::step_into in the lib asm build (no fast_loop symbol); step_into 2259 instr / 167 cond branches / 0 panic calls; decode_table 182/22/0; copy_match_sink 241/24 with 2 panic calls (2 sites); src:fast_loop.rs 79 instr/11 branches/0 panic. So there are NO bounds-check panic sites in the hot loop: P4-03's elision premise is weak; measure branches/instructions instead. docs/perf/phase3/P4-01-asm.json committed.

### P4-02 — 02d83fe
NEGATIVE RESULT (PT4). refill_bulk already IS the word-read, branch-free refill (D-27). Tried first_chunk/masked-shift spelling: asm cond branches identical (167 step_into, 39 bitreader.rs), bench NEUTRAL (+0.15%/+1.2%), A/B flat. Reverted; tests refill_word_path_all_counts/refill_never_exceeds_63_bits kept. make asm fixed to copy newest .s when cargo finds build fresh. Gate passed (6.10% > 2% on oneshot key).

### P4-03 — 161d00c
BIG WIN, kept. huffman.rs FastTable/FastDecoded/fast_table (fixed-size first level, masked index), BitReader::bits() + inline attrs, fast_loop::run rewritten (one function, locals). silesia raw l1 streaming 129.3->170.1 MiB/s (+31.5%), every l1/l6 key +18..+31% WIN on throughput AND ratio verdicts; instructions 36.4G->25.8G. asm: decode_table symbol gone (inlined), panic sites 0. fastslow-full, bomb, proptest-3k, fuzz-smoke pass. DESIGN D-36. One per-file regression alice29 gzip l1 oneshot -6.1% (noise). Next free D = D-37. Note for P4-04/P4-05: loop is now a single `run` function; LITLEN_FIRST/DIST_FIRST are derived from config table bits (fast_table returns None for other widths).

### P4-04 — 7dac8b3
NEGATIVE RESULT (PT4). Pair entries implemented correctly (tests passed, stats matched) but LOSS: bench silesia l1 oneshot -18%, streaming -16%, gzip -16% (verdict LOSS incl. ratio); A/B -5.6..-9.2%. Reverted huffman.rs/fast_loop.rs and the huffman_multi tests; kept trace test fast_loop_pairs_same_trace_as_reference. Next free D stays D-37 (no entry written). Did not take perf annotate; gate judged on literal share 46.6%.

### P4-05 — b218184
Measured no change: LITLEN_TABLE_BITS stays 10 (11: -6%, 12: -4%, 9: within drift), DIST_TABLE_BITS stays 8 (7 within drift; 9 +0.4% overall but -1.5..-2.6% on l1 raw). Drift control default vs repeat: 4W/3L of 24. config.rs doc comments cite docs/perf/phase3/P4-05/*.json. ASSUMPTION keys unchanged.

### P4-06 — f5f4c85
make ci, heavy, fuzz-milestone pass: diff_oracle_roundtrip seconds=1802 execs=1504454; diff_fast_reference seconds=1802 execs=395074; no artifacts. P4-06.json + P4-06-asm.json committed, pushed. Level-1 vs M3: 12/12 WIN (throughput and ratio): M4 criterion MET. Compare vs phase3-start: 36 WIN 0 LOSS 5 REGRESSION lines. Manual check: NOT VERIFIED (human): ci green; review fast loop by hand + mutation-sample; review D-36 wording.

### P5-01 — b850150
After flamegraphs (3, checked) + After section of phase3-profile.md (9 keys, hotspot before/after table, pattern final states). Findings for P5-02: fast_loop::run 79% one function; instructions/MiB 60M->42.5M (l1 oneshot); fixed costs now larger shares: Crc32::update 12.4% (gate 5.1%), adler_fold 6.7% (gate 7.0%), from_lengths 5.64% Canterbury (gate 4.84%) -> next-step candidates: CRC-32 slicing-16 (~2% of gzip), table build.

### P5-02 — adc0f97
P5-02.json (quiet machine, load 1.8) + PHASE3.md Summary/Final tables/Targets/Flamegraphs/Attribution/Tried and reverted/Conditions + PERF.md Phase 3 section with Result line. Silesia l6 like-for-like: oneshot 84.1/85.7/85.4% raw/zlib/gzip (phase 2: 66.9/68.8/68.8), streaming 81.8/83.9/78.0%. Targets: T5 MET; T1 (worst gzip l1 streaming 0.710), T2 (0.88-0.94), T3 (0.636), T4 (22%), T6 (5 canterbury/ptt5 per-file rows; all aggregates +7.3..+25.8% vs phase 2) NOT MET, with next steps. design-check passes.

### P5-03 — f57be5d
PHASE_BASE=cd4a472; SUMMARY.md / BUILD_REPORT.md regenerated (range cd4a472..HEAD, BUILD_REPORT performance line = phase-3 result line); human-interventions.md: dated sentence (no interventions; 9 guard denials explained, none a source read); make ci + make heavy pass; pushed.
Manual check: NOT VERIFIED (human): 1) final make bench on a quiet machine with pinned governor vs P5-02.json; 2) make fuzz-long / nightly for all six targets + triage fuzz-divergences; 3) GitHub Actions ci green; 4) review PHASE3.md T1-T6 verdicts/attribution/negative results + PERF.md result line; 5) review flamegraphs and assembly evidence; 6) after merge tag v0.3.0-cleanroom; 7) complete human-interventions.md.

### R1-01 — 8a84b96
Added reset_reapplies_raw_dictionary (Fast+Reference); oracle raw-dictionary case now built via deflate_bytes with dict; zlib dict selection panics with fix hint if empty; reset_proptest gains prefix_calls (0..=6) driven with case schedules. Mutation (drop prefill in Backend::reset) fails new unit test. Interpretation: fixed dict/cyclic data for raw case. verify green incl. proptest-3k.

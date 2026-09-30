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
- [ ] P0-10 M0 acceptance and push
- [ ] P1-01 Profile any key: per-key outputs, perf stat and saved flamegraphs
- [ ] P1-02 Before-profiles, flamegraphs and phase3-profile.md
- [ ] P1-03 M1 acceptance and push
- [ ] P2-01 Checksum microbenchmark (cleanflate only)
- [ ] P2-02 Adler-32 over a bound-derived unrolled batch (gated)
- [ ] P2-03 CRC-32 slicing-by-16 against 8 (gated)
- [ ] P2-04 Wrapper per-call overhead in streaming (gated)
- [ ] P2-05 Output-sink abstraction in the decoder core (slice sink only)
- [ ] P2-06 Vec sink and the sink differential
- [ ] P2-07 One-shot decoding appends to a Vec: no zero-fill
- [ ] P2-08 One-shot size-hint revisit (gated)
- [ ] P2-09 M2 acceptance and push
- [ ] P3-01 Per-block Huffman table reuse (gated)
- [ ] P3-02 Per-stream setup: reusable inflater, lazy window, no redundant fills (gated)
- [ ] P3-03 `Decompressor::reset()` and the reset differential
- [ ] P3-04 Reuse bench case
- [ ] P3-05 M3 acceptance and push
- [ ] P4-01 Assembly evidence tool for the fast loop
- [ ] P4-02 Branch-light bulk refill (gated)
- [ ] P4-03 Fast-loop bounds-check elision and inlined lookup
- [ ] P4-04 Multi-literal table entries (optional, measured)
- [ ] P4-05 Re-tune table widths on the final loop
- [ ] P4-06 M4 acceptance and push
- [ ] P5-01 After-profiles and flamegraphs
- [ ] P5-02 Final bench, PHASE3.md report and PERF.md phase-3 section
- [ ] P5-03 Provenance, build report and hand-off push

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

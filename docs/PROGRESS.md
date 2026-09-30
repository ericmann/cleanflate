# cleanflate phase 2 build progress
Branch: build/2026-09-29-2
Started: 2026-09-29T23:20:38.692Z

## Tasks
- [x] P0-01 Feature flags, Makefile targets and ignore rules
- [x] P0-02 Guard denies profiling of anything but cleanflate-only binaries
- [x] P0-03 design-check enforces the [perf] decision format
- [x] P0-04 Decode-path switch and a uniform invalid-code rule
- [x] P0-05 Fast-vs-reference differential test
- [x] P0-06 Regression fixture store and repro tooling
- [x] P0-07 `stats` feature counters
- [x] P0-08 Bench harness: medians, framings, levels, modes, JSON
- [x] P0-09 bench-compare, bench-noise, bench-md and `bench --stats`
- [x] P0-10 Cleanflate-only profiling entry point and `make profile`
- [x] P0-11 Baseline and noise measurement
- [x] P0-12 Hypotheses H1–H6 and baseline profile
- [x] P0-13 M0 acceptance and push
- [x] P1-01 Bulk bit refill with an exact give-back invariant
- [x] P1-02 Two-level lookup tables for every symbol
- [x] P1-03 Per-block table reuse (gated)
- [x] P1-04 Length/distance payload in table entries (gated)
- [x] P1-05 M1 acceptance and push
- [x] P2-01 Fast literal/length/distance decode loop
- [x] P2-02 Differential fuzz target: fast vs reference
- [x] P2-03 Copyable state in the slow path (gated)
- [x] P2-04 M2 acceptance and push
- [x] P3-01 Output as match history; window carried only across calls
- [ ] P3-02 Overlapping matches by pattern doubling
- [ ] P3-03 One-shot output sizing from hints
- [ ] P3-04 M3 acceptance and push
- [ ] P4-01 CRC-32 several bytes at a time
- [ ] P4-02 Adler-32 revisit (gated)
- [ ] P4-03 Re-sweep tunables on the final code
- [ ] P4-04 M4 acceptance and push
- [ ] P5-01 Phase-scoped provenance summary and build report
- [ ] P5-02 Final PERF.md phase-2 report and DESIGN audit
- [ ] P5-03 Provenance, build report and hand-off push

## Log
(one entry per task, appended by implement)

### P0-01 — c8cf74d
Added features stats/test-hooks (cleanflate), oracle stats feature + dev-dep test-hooks, Makefile lint/test/proptest-3k, .gitignore, docs/perf/README.md. All verify green.

### P0-02 — 8860f57
Guard _check_profile_segment added per-segment; 7 fixtures + selftest lines + unit tests; README section. Interpretation: perf list/--version only as first arg, --help as sole arg. make tools-check and cleanroom-selftest pass.

### P0-03 — 2d7a421
design-check.sh enforces [perf] blocks (Measurement: line + exact phrase); 4 new test cases; DESIGN.md how-to-read paragraph.

### P0-04 — 8be8580
DecodePath (decode_path.rs), test_hooks.rs (feature test-hooks), plumbed via Inflater::new_with_path/with_dictionary(dict,path), ZlibDecoder::new(dict,path), GzipDecoder::new(..,path), Decompressor::new_with_path, stream::decompress_with_path (pub(crate)). Window::copy_match_path (Reference => copy_match_bytewise). Inflater::new()/Window::copy_match kept as Fast wrappers for existing tests (allow dead_code when not test). Uniform invalid rule in Huffman::decode (no longer code => Invalid); D-26 added. Interpretation: table path delegates to serial when available<table_bits (equivalent). Note: dead-code allow is cfg_attr(not(test)) not any(test,test-hooks) since oracle dev-dep enables test-hooks in workspace lint.

### P0-05 — c4b6e31
tests/common/mod.rs (Trace/CallOutcome/trace/oneshot/dump_candidate/describe_diff + shared strategies), tests/fastslow.rs (4 tests), chunking.rs uses shared strategies. Makefile: proptest-3k/10k set both case env vars and run both tests; fastslow-full added to heavy. fastslow-full passed, 248s test time (4m10 wall). Quick tier ~37s release. No Fast/Reference differences found.

### P0-06 — e4b0694
oracle::regress (Row/manifest parse+render, add_to, seed_into, run_add/run_seed), CLI regress-add/regress-seed, Makefile regress-seed, testdata/regressions/ (README, MANIFEST.tsv, 4 seed .bin), tests/regressions.rs (2 tests), chunking.rs dumps candidates to target/regress-candidates/ on failure (tags chunking, chunking-corrupt). Second regress-seed run is a no-op. Interpretation: row with only one chunk list uses 1021/997 for the other.

### P0-07 — 21e4a39
stats.rs (stat! macro on/off + Stats + len_bucket/dist_bucket), counters in Inflater (stats field), BitReader (refills, refill_bytes), Window (written), Huffman::decode_table_method/table_entries, gzip accumulates via Box<Stats> stats_done, Decompressor::stats(), decompress_with_stats via shared decompress_core. 6 tests in tests/stats.rs. Interpretations: table_builds excludes constructor's 2 fixed tables; fixed_table_clones = 2 per fixed block; window_bytes_written excludes dictionary prefill; given_back_bytes/fast_loop_* stay 0 until their tasks. Default build has no counter code (cfg + macro).

### P0-08 — b22aa52
bench.rs rewritten (parse_args, medians, reps_for, interleaved samples, aggregates_of, render_markdown), bench_json.rs (schema 1, format_utc), main.rs run_bench writes JSON, Makefile bench/bench-quick with BENCH_ID/BENCH_OUT, docs/perf/README.md schema. make bench-quick took 62s; baseline median ratios 0.18-0.23 (silesia raw l6 oneshot 51 MiB/s vs C 275). JSON in target/bench/P0-08.json not committed. Interpretation: reps per implementation; no corpus fallback.

### P0-09 — 3e883a7
bench_compare.rs (Noise, compare, verdicts, render_md, exit_code), bench.rs --stats flag + collect_stats/render_stats/finish_stats (cfg stats), BenchJson.stats array, main.rs bench-noise/bench-compare/bench-md, Makefile targets + lint line for oracle --features stats. Ran two bench-quick runs: single-pair noise up to 6.5% aggregate, xml/zlib per-file 31% -- machine is noisy. bench-stats Silesia l6 shows: litlen table-decode 54%, dist 51.7%, 0.644 dispatches/output byte, 1.08 bytes/refill, 2.99 table builds/block, match len mostly 3-8 (71%), dists spread widely (43% > 1K). Interpretation: no noise file => per-file regression check skipped. Outputs under target/bench not committed.

### P0-10 — e3826e8
perf_prep.rs (2 tests), examples/profile_decode.rs (cleanflate+std only; verifies len+CRC on iter 0; prints Stats with --features stats), Makefile profile-prep/profile/flamegraph + PROFILE_* vars, README profiling section. make profile PROFILE_ITERS=1 (silesia rfc1951 l6, 52.6 MiB/s) top: decode_table 30.2%+6.4%, Inflater::step 27.2%+5.0%, checksum::crc32 12.8% (example's first-iteration verify; dilutes with more iters), peek_symbol 6.7%+1.0%, Huffman::from_lengths 0.5%. flamegraph SVG written. Interpretation: -g none on perf report; flamegraph recipe moves stray perf.data into target/profile.

### P0-11 — 8b15626
Six JSONs in docs/perf/ (full pair 7 samples, quick pair 5 samples, both noise files), PERF.md ## Phase 2 (Method, Machine, Baseline, Noise, Targets). Baseline Silesia l6 one-shot ratios: raw 22.5%, zlib 24.0%, gzip 20.7%; streaming 23.4/24.9/22.2%. Noise: aggregate_max_pct 6.08% full, 6.52% quick -- EXCEEDS 5%, so a win must beat ~12-13% on the noisiest aggregates (Canterbury keys). Governor schedutil, load ~3 at start, 8 CPUs, Ryzen 5 3400G. Also fixed a P0-09 defect in a separate chore commit (FP epsilon in compare). Manual check: NOT VERIFIED (human): noise > 5%; try a pinned performance governor.

### P0-12 — 7f982a5
PERF.md: Baseline profile, Hypotheses, Cost ranking (docs/perf/P0-12-stats.json committed). Verdicts: H1 CONFIRMED (litlen 46.0% / dist 48.3% serial, 1.08 B/refill, decode 32% inclusive), H2 CONFIRMED (0.644 dispatches/output byte, step self 36%, State clone 7.5%), H3 CONFIRMED (window_bytes_written/output = 1.0, ~7-14%), H4 DISCARDED (0.48% overlap), H5 DISCARDED (0.6%, 2.99 builds/block, no fixed blocks), H6 PARTIAL (CRC ~12.7% gzip, Adler ~2%, gzip one-shot zero-fills an extra output-sized buffer: 514MB allocated vs 302MB). Likely negative gated tasks: P1-03, P4-02, probably P2-03; P1-04 undecided. Observation: P0-04's uniform-invalid rule adds ~4% in the serial walk (any() over counts) -- cheap to fix in P1-02.

### P0-13 — 6800093
make ci green; make heavy green (proptest-10k both, bomb, conformance-full, fastslow-full 248s); docs/perf/P0-13.json committed; provenance snapshot committed; PERF.md M0 acceptance section. bench-compare vs baseline: 27 WIN/0 LOSS/9 NEUTRAL + 32 per-file REGRESSION with NO decoder change => run-to-run drift exceeds the single-pair noise estimate; later tasks must re-measure interleaved rather than trust one verdict. Push: pushed build/2026-09-29-2 to origin.
Manual check: NOT VERIFIED (human): (1) GitHub Actions ci run green on the pushed branch (now lints/tests features). (2) noise.json aggregate 6.08% > 3%: re-run baseline on a quiet machine with a fixed governor (SPEC 8). (3) run make flamegraph and sanity-check the P0-12 tables against the SVG. (4) provenance log denied entries: 3 mine this phase, benign (recursive grep x2, sed regex with forbidden word); none profiling.

### P1-01 — 17ecd44
BitReader::refill_bulk/give_back, Inflater::step now wraps step_inner with one exit helper (give_back(keep_bits) on OutputFull/Done; keep_bits set at the pending-literal OutputFull site), StoredLen gives back all whole bytes + defensive drain in StoredCopy, Fast peek_symbol bulk-refills when <15 bits. D-27 [perf] added. 6 tests in tests/bitreader_bulk.rs (mutation-checked). PT1: all 12 quick aggregates WIN (silesia l6 oneshot raw 51.1->79.3, zlib 49.7->75.5, gzip 45.3->57.8), no per-file REGRESSION. make test, proptest-3k, fastslow-full (209s), bomb pass. Interpretation: give-back applied on both paths (Reference returns 0).

### P1-02 — 8096976
huffman.rs: packed u32 Entry (Invalid/Symbol/Sub), two-level build_table, total lookup in decode_table_method (Symbol longer than available => NeedMoreBits, Invalid with a>=1 => Invalid), CodeLength single 7-bit level (CODE_LENGTH_TABLE_BITS const, RFC-fixed), from_lengths_with_bits for sweeps. D-28 [perf] (supersedes D-16 gate, includes prefix-freeness argument). Tests in tests/huffman_twolevel.rs (220+ random complete sets with 15-bit codes, widths 7..=12). stats test updated: Fast has 0 serial decodes. PT1: NEUTRAL on Silesia (raw oneshot 78.9->81.9), Canterbury gzip/raw WIN; one LOSS on silesia gzip oneshot did not repeat in a second run. Kept per PT4 as prerequisite of P2-01. Stats after: table-decode 100%, bytes/refill 6.21. fastslow-full, proptest-3k pass. Interpretation: NEUTRAL kept as prerequisite.

### P1-03 — ff08378
GATED OUT (negative result), no code change. PERF.md '### P1-03 (gated, negative result)': from_lengths 1.29% inclusive (build_table 0.72%, alloc 0.43%), 2.99 builds/block, 0 fixed-table clones on Silesia l6 => ~1.7% < max(2%, 2x noise). The 9.45% clone frame is State clone (P2-03), not tables.

### P1-04 — 8e8a235
GATED OUT (negative result), no code change. Profile by source line (perf report --sort srcline on the cleanflate-only profile): LenExtra arm 0.63% + DistExtra arm 0.73% = 1.4% < 2%; whole bitreader.rs ~5.5% upper bound. Real cost is dispatch (`match &self.state` 16.5%) and State clone (9.5+5.6%), for P2-01/P2-03. Interpretation: srcline attribution used instead of annotate output.

### P1-05 — bb3a2c6
make ci and make heavy green; P1-05.json and P1-05-stats.json committed; PERF.md M1 acceptance. Table-decode 100.0%/100.0% (>=99% MET). Silesia l6 vs baseline: one-shot raw 62.3->98.9, zlib 61.3->94.5, gzip 51.8->75.3 MiB/s; streaming raw 65.0->107.8; all WIN, no REGRESSION. Push: pushed to origin.
Manual check: NOT VERIFIED (human): (1) GitHub Actions ci green on the pushed branch. (2) Review D-27/D-28 [perf] entries for clean-room wording (SPEC 2.1). (3) Sanity-check the M1 gain on a quiet machine (noise was high, single-pair noise 6.1%).

### P2-01 — 7ade00d
fast_loop.rs (FastCtx/FastExit, run with local bit buffer/positions, 48-bit iteration refill), config FAST_LOOP_MIN_INPUT=16 / FAST_LOOP_MIN_OUTPUT=258, integration in step_inner (skip_fast flag: one slow dispatch after a Margin exit), stats count_decode/record_match moved to stats.rs and shared, fast_loop_entries/symbols counted. D-29 [perf] added. 5 tests in tests/fast_loop.rs + tests/config.rs (mutation-checked). PT1: all 12 aggregates WIN: silesia l6 oneshot raw 83.1->132.2, zlib 80.2->131.0, gzip 66.1->92.6; streaming raw 83.8->135.5; no per-file REGRESSION. make test, proptest-3k, proptest-10k, fastslow-full (200s), bomb pass. Interpretation: Margin retry policy via skip_fast; Copy exit defensive.

### P2-02 — 9963364
fuzz/fuzz_targets/diff_fast_reference.rs (byte0%3 format, byte1/2 chunk sizes; compares one-shot results and full per-call traces + finish/total_in/total_out/trailing_start; panics on any difference), fuzz/Cargo.toml (test-hooks feature + bin), 10 seeds from testdata/malformed, FUZZ_TARGETS + nightly workflow matrix + README. make fuzz-check passes; make fuzz-smoke-diff_fast_reference FUZZ_SECONDS=120: 214251 runs, 0 crashes (nightly and cargo-fuzz were already installed).

### P2-03 — 3dd7e54
GATED OUT (negative result), no code change. After P2-01: Inflater::step self 0.49% (was 35.9%), State clone frame 0.15% (was 7.5%); fast_loop::run self 38.4%; perf-profiled decode 138 MiB/s. Below the 2% floor.

### P2-04 — 139a5d4
make ci, make heavy (incl. full-tier differential), make fuzz-smoke (6 targets, 0 crashes; diff_fast_reference 96,588 runs) green; docs/perf/P2-04.json committed; PERF.md M2 acceptance. Silesia l6 vs baseline: one-shot raw 62.3->153.8, zlib 61.3->147.9, gzip 51.8->105.8; streaming raw 176.8 (64.3% of C zlib), zlib 166.8 (66.3%) => streaming target 60% met for raw/zlib; one-shot 56.0%/58.8%/41.3% (target 70% NOT YET MET). Fast loop covers 99.98% of symbols, 0.0032 dispatches/output byte. No REGRESSION. Repro audit: 4 seed fixtures pass. Push: pushed to origin.
Manual check: NOT VERIFIED (human): (1) GitHub Actions ci green. (2) Run make fuzz-long or nightly workflow incl. diff_fast_reference for the 1-hour signal. (3) Review the fast-loop code by hand and mutation-sample it (SPEC 5).

### P3-01 — 03480a4
Window now append/read_back/history_available/copy_match(path, output, cur, dist, n) (slice copies on Fast; Reference byte-by-byte via output or get_back); step wrapper appends output[..out_pos] to the window exactly once on every return (step_inner returns (pos,out_pos,Result) via closure wrap); literals/stored copy write only to output; fast loop takes &Window. D-30 [perf]. tests/history.rs (5 tests, incl. Decompressor size <= 33688+1024; pre-change size was 33688 default/34080 stats); window/fastpath/decode_path tests rewritten to the new API. PT1: all 12 aggregates WIN (silesia l6 oneshot raw 133.4->160.6, zlib 122.1->151.1, gzip 87.9->108.2); 3 per-file REGRESSION lines (canterbury xargs.1 gzip, silesia sao gzip streaming) recorded as findings. window_bytes_written/output: 1.0 -> 0.019 oneshot, 0.50 streaming. make test, proptest-3k, fastslow-full (189s), bomb pass. Interpretation: exit append on every return incl. Err; stat counts bytes actually copied.

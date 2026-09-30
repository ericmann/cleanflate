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
- [~] P1-03 Per-block table reuse (gated)
- [ ] P1-04 Length/distance payload in table entries (gated)
- [ ] P1-05 M1 acceptance and push
- [ ] P2-01 Fast literal/length/distance decode loop
- [ ] P2-02 Differential fuzz target: fast vs reference
- [ ] P2-03 Copyable state in the slow path (gated)
- [ ] P2-04 M2 acceptance and push
- [ ] P3-01 Output as match history; window carried only across calls
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

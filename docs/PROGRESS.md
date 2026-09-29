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
- [ ] P0-07 `stats` feature counters
- [ ] P0-08 Bench harness: medians, framings, levels, modes, JSON
- [ ] P0-09 bench-compare, bench-noise, bench-md and `bench --stats`
- [ ] P0-10 Cleanflate-only profiling entry point and `make profile`
- [ ] P0-11 Baseline and noise measurement
- [ ] P0-12 Hypotheses H1–H6 and baseline profile
- [ ] P0-13 M0 acceptance and push
- [ ] P1-01 Bulk bit refill with an exact give-back invariant
- [ ] P1-02 Two-level lookup tables for every symbol
- [ ] P1-03 Per-block table reuse (gated)
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

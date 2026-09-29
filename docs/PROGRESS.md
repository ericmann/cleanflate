# cleanflate phase 2 build progress
Branch: build/2026-09-29-2
Started: 2026-09-29T23:20:38.692Z

## Tasks
- [x] P0-01 Feature flags, Makefile targets and ignore rules
- [x] P0-02 Guard denies profiling of anything but cleanflate-only binaries
- [x] P0-03 design-check enforces the [perf] decision format
- [ ] P0-04 Decode-path switch and a uniform invalid-code rule
- [ ] P0-05 Fast-vs-reference differential test
- [ ] P0-06 Regression fixture store and repro tooling
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

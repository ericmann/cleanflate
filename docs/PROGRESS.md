# cleanflate phase 3 build progress
Branch: (set by implement)
Started: (set by implement)

## Tasks
- [ ] P0-01 Phase-3 scaffolding: fuzz gates, quick-bench levels, baseline copy, report skeleton
- [ ] P0-02 design-check requires profile evidence in phase-3 [perf] entries
- [ ] P0-03 Oracle FFI: C zlib build flags and bounded streaming inflate
- [ ] P0-04 Bench schema 2: "best" replaces "min", every column documented
- [ ] P0-05 C zlib streaming rows and like-for-like ratios
- [ ] P0-06 Environment capture in every bench run
- [ ] P0-07 Reports: delta vs the phase-2 baseline, drift-corrected verdicts, per-file sizes
- [ ] P0-08 `oracle bench-targets`: T1–T6 verdicts from a bench JSON
- [ ] P0-09 Phase-3 starting baseline and noise
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

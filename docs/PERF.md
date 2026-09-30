# PERF.md

Single-thread decompression throughput, cleanflate vs C zlib (SPEC S5 M4). Method: `oracle bench` compresses each corpus file with C zlib at the given level (raw and zlib framing), then times best-of-5 `cleanflate::decompress_with` against `oracle::ffi::inflate_bytes` with `std::time::Instant`, single-threaded, release build only. Throughput is uncompressed MiB/s; ratio is cleanflate / zlib (>= 1.0 means cleanflate is at least as fast).

Machine: Linux 7.1.5-76070105-generic x86_64
CPU: AMD Ryzen 5 3400G with Radeon Vega Graphics
Compiler: rustc 1.89.0 (29483883e 2025-08-04)

## Baseline (P4-07)

Corpus: silesia | zlib 1.3.2

### raw

| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |
|---|---:|---:|---:|---:|
| silesia-dickens | 10192446 | 55.9 | 258.2 | 0.22 |
| silesia-mozilla | 51220480 | 58.1 | 242.0 | 0.24 |
| silesia-mr | 9970564 | 61.2 | 280.7 | 0.22 |
| silesia-nci | 33553445 | 155.1 | 500.4 | 0.31 |
| silesia-ooffice | 6152192 | 46.4 | 203.8 | 0.23 |
| silesia-osdb | 10085684 | 71.6 | 310.0 | 0.23 |
| silesia-reymont | 6627202 | 77.9 | 319.7 | 0.24 |
| silesia-samba | 21606400 | 82.2 | 315.2 | 0.26 |
| silesia-sao | 7251944 | 38.8 | 232.5 | 0.17 |
| silesia-webster | 41458703 | 66.1 | 245.0 | 0.27 |
| silesia-x-ray | 8474240 | 36.6 | 190.2 | 0.19 |
| silesia-xml | 5345280 | 145.5 | 618.4 | 0.24 |
| **aggregate** | | **67.4** | **279.1** | **0.24** |

### zlib

| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |
|---|---:|---:|---:|---:|
| silesia-dickens | 10192446 | 50.3 | 230.2 | 0.22 |
| silesia-mozilla | 51220480 | 50.8 | 215.8 | 0.24 |
| silesia-mr | 9970564 | 51.9 | 255.8 | 0.20 |
| silesia-nci | 33553445 | 108.3 | 420.3 | 0.26 |
| silesia-ooffice | 6152192 | 41.2 | 193.5 | 0.21 |
| silesia-osdb | 10085684 | 57.3 | 265.9 | 0.22 |
| silesia-reymont | 6627202 | 63.6 | 283.2 | 0.22 |
| silesia-samba | 21606400 | 66.0 | 280.5 | 0.24 |
| silesia-sao | 7251944 | 34.4 | 217.5 | 0.16 |
| silesia-webster | 41458703 | 55.9 | 224.0 | 0.25 |
| silesia-x-ray | 8474240 | 32.9 | 183.2 | 0.18 |
| silesia-xml | 5345280 | 104.3 | 450.1 | 0.23 |
| **aggregate** | | **56.8** | **250.2** | **0.23** |

## Pass 1 (P4-08)

Two changes were attempted:

1. **Window bulk match copy** (`crates/cleanflate/src/window.rs`, kept):
   when a length/distance match cannot overlap itself (`distance >=
   len`), copy it via up to two contiguous slice copies each way
   (respecting the ring buffer's wraparound point) instead of `len`
   individual `get_back`/`push` calls. Verified against a byte-by-byte
   reference implementation for a spread of distances/lengths including
   wraparound (`crates/cleanflate/src/tests/fastpath.rs`), and the full
   test suite plus 3000 `chunking` proptest cases (`CLEANFLATE_CHUNKING_CASES=3000
   cargo test --release -p oracle --test chunking`) pass unchanged.
2. **Fast decode loop with proactive bulk bit-buffer refill**
   (`crates/cleanflate/src/inflate.rs`/`bitreader.rs`, **reverted**): a
   loop entered from `Inflater::step` while `state == LitLen` and at
   least 16 input bytes / 258 output bytes remained, decoding
   literal/length+distance symbols directly without returning to the
   outer state dispatch, refilling the bit buffer to ~56 bits at a time
   instead of reactively one byte at a time. This surfaced two real bugs:
   first, over-reading whole bytes into the bit buffer beyond what a
   block actually used corrupted gzip's per-member trailer boundary
   detection (fixed by giving back unconsumed whole bytes to `consumed`
   on a `Done` exit); second, and not tracked down to its root cause
   within this task's budget, `CLEANFLATE_CHUNKING_CASES=3000 cargo test
   --release -p oracle --test chunking` found a ~11 KiB Fixed-strategy
   raw stream (heavy on length/distance matches) that decoded one byte
   short of the original, with `Decompressor` stuck reporting
   `NeedsInput` after consuming 100% of a self-produced, definitely-
   complete compressed stream -- bits were being silently dropped
   somewhere in the fast litlen/length/distance chain. Per this task's
   own "if the change ... breaks any test, revert it and record the
   negative result instead" clause, both files were reverted to their
   pre-P4-08 state in full.

Corpus: silesia | zlib 1.3.2 (window-bulk-copy change only; measured
after reverting the fast decode loop)

### raw

| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |
|---|---:|---:|---:|---:|
| silesia-dickens | 10192446 | 56.2 | 271.0 | 0.21 |
| silesia-mozilla | 51220480 | 62.7 | 248.5 | 0.25 |
| silesia-mr | 9970564 | 64.0 | 302.5 | 0.21 |
| silesia-nci | 33553445 | 173.0 | 493.2 | 0.35 |
| silesia-ooffice | 6152192 | 47.6 | 200.5 | 0.24 |
| silesia-osdb | 10085684 | 74.3 | 300.1 | 0.25 |
| silesia-reymont | 6627202 | 75.3 | 315.9 | 0.24 |
| silesia-samba | 21606400 | 75.5 | 299.8 | 0.25 |
| silesia-sao | 7251944 | 40.6 | 226.0 | 0.18 |
| silesia-webster | 41458703 | 55.2 | 243.1 | 0.23 |
| silesia-x-ray | 8474240 | 35.5 | 200.2 | 0.18 |
| silesia-xml | 5345280 | 168.2 | 637.4 | 0.26 |
| **aggregate** | | **66.5** | **280.5** | **0.24** |

### zlib

| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |
|---|---:|---:|---:|---:|
| silesia-dickens | 10192446 | 49.5 | 241.8 | 0.20 |
| silesia-mozilla | 51220480 | 52.0 | 226.7 | 0.23 |
| silesia-mr | 9970564 | 53.0 | 259.3 | 0.20 |
| silesia-nci | 33553445 | 114.1 | 362.3 | 0.32 |
| silesia-ooffice | 6152192 | 34.0 | 189.0 | 0.18 |
| silesia-osdb | 10085684 | 55.7 | 256.5 | 0.22 |
| silesia-reymont | 6627202 | 61.3 | 266.9 | 0.23 |
| silesia-samba | 21606400 | 71.4 | 278.6 | 0.26 |
| silesia-sao | 7251944 | 35.6 | 216.7 | 0.16 |
| silesia-webster | 41458703 | 59.3 | 234.6 | 0.25 |
| silesia-x-ray | 8474240 | 32.0 | 187.6 | 0.17 |
| silesia-xml | 5345280 | 118.8 | 522.6 | 0.23 |
| **aggregate** | | **57.9** | **252.6** | **0.23** |

Aggregate change vs Baseline: raw 67.4 -> 66.5 MiB/s (-1.3%, within
run-to-run noise), zlib 56.8 -> 57.9 MiB/s (+1.9%). The window bulk-copy
change alone is not a measurable win on this corpus/level; most Silesia
matches at level 6 are short and/or short-distance (the overlap-eligible
case the bulk path does not help), so this is an honest report of a
change that is safe and correct but not a meaningful performance win by
itself. The reverted fast decode loop was the change actually expected
to move the needle; it remains future work.

## Pass 2 (P4-09)

Two more changes were measured:

1. **Adler-32 deferred modulo** (`crates/cleanflate/src/checksum.rs`,
   kept): `Adler32::update` reduces `s1`/`s2` mod 65521 once per
   `ADLER_NMAX` (5552) bytes instead of once per byte, a bound derived
   from RFC 1950 S2.2's modulus alone (DESIGN.md D-23 proves it) and
   verified against a per-byte reference at lengths spanning several
   chunk boundaries (`adler32_bound_worst_case`). This only affects zlib
   framing (raw DEFLATE has no Adler-32); the raw-framing numbers below
   moved only by run-to-run noise, as expected.
2. **Table-bits sweep** (`crates/cleanflate/src/config.rs`): measured
   every `LITLEN_TABLE_BITS` in `{9,10,11}` against every
   `DIST_TABLE_BITS` in `{7,8,9}` on Canterbury (`--iters 3`, all nine
   combinations within a few percent of each other -- Canterbury is
   small enough that measurement noise dominates any real signal at this
   granularity), then re-measured the strongest three candidates on
   Silesia at `--iters 5` for a cleaner comparison. The existing
   defaults (`LITLEN_TABLE_BITS = 10`, `DIST_TABLE_BITS = 8`) came out
   tied with or ahead of every other candidate on the Silesia
   head-to-head, so they were kept unchanged (a measured "no change
   needed" result, not a skipped step). CRC-32 slice-by-N was not
   attempted in this pass: profiling via the raw-vs-zlib-framing gap
   (this task's own suggested method for isolating Adler-32's cost)
   showed Adler-32 as the larger isolated cost of the two checksums, and
   slice-by-N carries materially higher correctness risk for a
   multi-table construction that would need independent re-derivation
   from the polynomial (CR1); left as candidate future work.

Corpus: silesia | zlib 1.3.2 (Adler-32 deferred-modulo change; table
bits unchanged from Pass 1)

### raw

| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |
|---|---:|---:|---:|---:|
| silesia-dickens | 10192446 | 58.9 | 264.7 | 0.22 |
| silesia-mozilla | 51220480 | 62.8 | 248.1 | 0.25 |
| silesia-mr | 9970564 | 63.7 | 293.7 | 0.22 |
| silesia-nci | 33553445 | 185.6 | 500.2 | 0.37 |
| silesia-ooffice | 6152192 | 48.0 | 188.4 | 0.26 |
| silesia-osdb | 10085684 | 69.9 | 312.0 | 0.22 |
| silesia-reymont | 6627202 | 73.3 | 324.4 | 0.23 |
| silesia-samba | 21606400 | 79.6 | 288.6 | 0.28 |
| silesia-sao | 7251944 | 35.8 | 218.7 | 0.16 |
| silesia-webster | 41458703 | 71.9 | 247.6 | 0.29 |
| silesia-x-ray | 8474240 | 39.6 | 176.3 | 0.22 |
| silesia-xml | 5345280 | 189.4 | 646.7 | 0.29 |
| **aggregate** | | **71.1** | **277.7** | **0.26** |

### zlib

| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |
|---|---:|---:|---:|---:|
| silesia-dickens | 10192446 | 57.4 | 239.6 | 0.24 |
| silesia-mozilla | 51220480 | 60.9 | 230.1 | 0.26 |
| silesia-mr | 9970564 | 64.5 | 268.6 | 0.24 |
| silesia-nci | 33553445 | 187.8 | 447.9 | 0.42 |
| silesia-ooffice | 6152192 | 49.3 | 201.1 | 0.25 |
| silesia-osdb | 10085684 | 75.4 | 281.6 | 0.27 |
| silesia-reymont | 6627202 | 80.5 | 295.9 | 0.27 |
| silesia-samba | 21606400 | 88.3 | 298.4 | 0.30 |
| silesia-sao | 7251944 | 40.7 | 221.3 | 0.18 |
| silesia-webster | 41458703 | 71.9 | 230.3 | 0.31 |
| silesia-x-ray | 8474240 | 37.6 | 189.8 | 0.20 |
| silesia-xml | 5345280 | 183.0 | 533.2 | 0.34 |
| **aggregate** | | **71.9** | **263.0** | **0.27** |

Aggregate change vs Pass 1: raw 66.5 -> 71.1 MiB/s (+6.9%, run-to-run
variance -- Adler-32 does not run in raw framing, so this movement is
noise, not signal), zlib 57.9 -> 71.9 MiB/s (+24.2%, the Adler-32
deferred-modulo win: this framing is the one Adler-32 actually runs on
every byte of).

**Result:** cleanflate / C zlib = 25.6% (raw framing, 71.1 / 277.7 MiB/s)
and 27.3% (zlib framing, 71.9 / 263.0 MiB/s) on Silesia (target 70%):
**NOT MET**. The decode loop itself (Huffman symbol decode and the
per-state dispatch in `Inflater::step`) remains the dominant cost by a
wide margin; the attempted fix for that (P4-08's fast decode loop) had
to be reverted for a correctness bug found within this pass's budget.
Closing the remaining gap to C zlib needs that loop redone correctly
(or an equivalent), not further checksum/table tuning, which this pass's
own numbers show has much smaller headroom.

## Phase 2

Phase 2 optimises decode speed against the goals in SPEC section 4.1. Everything below was measured on the phase-2 machine, before any phase-2 optimisation; all later comparisons are against these files, not the phase-1 numbers above.

### Method

`oracle bench` (release builds only) works per file, framing (raw, zlib, gzip) and level (1, 6, 9):

- The fixture is compressed once with the oracle (Default strategy, windowBits 15, memLevel 8). Each timed implementation then decodes it once as a warm-up, and the output is checked byte-identical to the original.
- One sample lasts at least 20 ms: `reps` is the smallest power of two that reaches it. The per-decode time is sample time / `reps`.
- The full bench takes N = 7 samples (quick: 5). Within each sample index the implementations run back to back (interleaved), so drift affects all of them alike. Median and minimum are reported per file; the aggregate is total bytes / sum of per-file times.
- Modes: `oneshot` (`decompress_with`) and `streaming` (`Decompressor` fed 64 KiB input chunks into a pre-sized output through 64 KiB windows). C zlib is timed one-shot only; its median is the denominator of the ratio for both modes.
- Results are written as JSON (`docs/perf/<task>.json`, schema in `docs/perf/README.md`). `make bench-noise` derives noise as |a - b| / a from two runs of the same code; `make bench-compare` labels an aggregate WIN or LOSS only beyond twice its noise, and a per-file REGRESSION when a file drops by more than the larger of its own noise and its aggregate's noise.
- Profiling uses only cleanflate: `make profile` decodes pre-compressed inputs in `crates/cleanflate/examples/profile_decode.rs`, so no profile contains C zlib code. Counters come from the opt-in `stats` feature (`make bench-stats`).

### Machine and conditions

- CPU: AMD Ryzen 5 3400G with Radeon Vega Graphics (8 logical CPUs), Linux 7.1.5-76070105-generic x86_64, rustc 1.89.0 (29483883e 2025-08-04), C zlib 1.3.2.
- Governor: schedutil (not pinned). Load average at the start of the full run: 3.16 3.26 3.17 (the tail of the preceding build and test activity); the machine was otherwise idle and no other heavy job ran during the runs.
- Run at `a59e2e123a17`; the tree was marked dirty only because the pipeline's `docs/PROGRESS.md` had uncommitted bookkeeping, with no code difference.
- Files: `docs/perf/baseline.json` and `baseline-rerun.json` (kind `full`, 7 samples), `noise.json`; `baseline-quick.json`, `baseline-quick-rerun.json`, `noise-quick.json` (kind `quick`, 5 samples, level 6).

### Baseline (P0-11)

The phase-1 figures (raw 71.1 and zlib 71.9 MiB/s) came from a quieter session; on today's machine the same code measures about 62 MiB/s. That gap is why the baseline is re-measured here.

Silesia, level 6, per framing and mode (from `docs/perf/baseline.json`):

| key | cleanflate MiB/s (median) | cleanflate MiB/s (min) | C zlib MiB/s (median) | ratio |
|---|---:|---:|---:|---:|
| silesia/raw/l6/oneshot | 62.3 | 64.0 | 277.3 | 22.5% |
| silesia/raw/l6/streaming | 65.0 | 67.9 | 277.3 | 23.4% |
| silesia/zlib/l6/oneshot | 61.3 | 63.3 | 255.5 | 24.0% |
| silesia/zlib/l6/streaming | 63.7 | 66.9 | 255.5 | 24.9% |
| silesia/gzip/l6/oneshot | 51.8 | 54.2 | 250.7 | 20.7% |
| silesia/gzip/l6/streaming | 55.7 | 58.9 | 250.7 | 22.2% |

All aggregates:

| key | cleanflate MiB/s (median) | cleanflate MiB/s (min) | C zlib MiB/s (median) | ratio |
|---|---:|---:|---:|---:|
| canterbury/gzip/l1/oneshot | 51.9 | 55.1 | 326.7 | 15.9% |
| canterbury/gzip/l1/streaming | 53.1 | 55.8 | 326.7 | 16.3% |
| canterbury/gzip/l6/oneshot | 61.7 | 66.5 | 329.6 | 18.7% |
| canterbury/gzip/l6/streaming | 61.7 | 67.1 | 329.6 | 18.7% |
| canterbury/gzip/l9/oneshot | 63.4 | 66.3 | 346.5 | 18.3% |
| canterbury/gzip/l9/streaming | 63.7 | 67.7 | 346.5 | 18.4% |
| canterbury/raw/l1/oneshot | 59.6 | 64.0 | 350.2 | 17.0% |
| canterbury/raw/l1/streaming | 60.1 | 65.8 | 350.2 | 17.2% |
| canterbury/raw/l6/oneshot | 70.8 | 73.6 | 358.6 | 19.8% |
| canterbury/raw/l6/streaming | 70.4 | 74.9 | 358.6 | 19.6% |
| canterbury/raw/l9/oneshot | 72.3 | 76.4 | 369.8 | 19.5% |
| canterbury/raw/l9/streaming | 74.0 | 76.4 | 369.8 | 20.0% |
| canterbury/zlib/l1/oneshot | 53.8 | 58.5 | 290.3 | 18.5% |
| canterbury/zlib/l1/streaming | 55.1 | 59.5 | 290.3 | 19.0% |
| canterbury/zlib/l6/oneshot | 68.3 | 75.6 | 312.2 | 21.9% |
| canterbury/zlib/l6/streaming | 68.1 | 76.7 | 312.2 | 21.8% |
| canterbury/zlib/l9/oneshot | 70.2 | 75.7 | 317.6 | 22.1% |
| canterbury/zlib/l9/streaming | 70.1 | 77.6 | 317.6 | 22.1% |
| silesia/gzip/l1/oneshot | 45.2 | 47.0 | 244.0 | 18.5% |
| silesia/gzip/l1/streaming | 48.4 | 50.3 | 244.0 | 19.8% |
| silesia/gzip/l6/oneshot | 51.8 | 54.2 | 250.7 | 20.7% |
| silesia/gzip/l6/streaming | 55.7 | 58.9 | 250.7 | 22.2% |
| silesia/gzip/l9/oneshot | 52.9 | 55.0 | 258.7 | 20.5% |
| silesia/gzip/l9/streaming | 57.2 | 59.7 | 258.7 | 22.1% |
| silesia/raw/l1/oneshot | 51.0 | 53.3 | 255.2 | 20.0% |
| silesia/raw/l1/streaming | 53.7 | 56.4 | 255.2 | 21.0% |
| silesia/raw/l6/oneshot | 62.3 | 64.0 | 277.3 | 22.5% |
| silesia/raw/l6/streaming | 65.0 | 67.9 | 277.3 | 23.4% |
| silesia/raw/l9/oneshot | 62.6 | 65.8 | 276.0 | 22.7% |
| silesia/raw/l9/streaming | 65.9 | 69.2 | 276.0 | 23.9% |
| silesia/zlib/l1/oneshot | 51.0 | 53.7 | 239.6 | 21.3% |
| silesia/zlib/l1/streaming | 52.6 | 56.1 | 239.6 | 22.0% |
| silesia/zlib/l6/oneshot | 61.3 | 63.3 | 255.5 | 24.0% |
| silesia/zlib/l6/streaming | 63.7 | 66.9 | 255.5 | 24.9% |
| silesia/zlib/l9/oneshot | 60.7 | 63.8 | 249.9 | 24.3% |
| silesia/zlib/l9/streaming | 65.4 | 68.0 | 249.9 | 26.2% |

Per-file, Silesia raw level 6 one-shot:

| file | bytes | cleanflate MiB/s (median) | C zlib MiB/s (median) | ratio |
|---|---:|---:|---:|---:|
| dickens | 10192446 | 51.7 | 247.8 | 20.8% |
| mozilla | 51220480 | 53.3 | 238.7 | 22.3% |
| mr | 9970564 | 56.3 | 294.3 | 19.1% |
| nci | 33553445 | 182.6 | 502.0 | 36.4% |
| ooffice | 6152192 | 40.4 | 193.7 | 20.8% |
| osdb | 10085684 | 61.9 | 303.7 | 20.4% |
| reymont | 6627202 | 71.1 | 309.4 | 23.0% |
| samba | 21606400 | 78.3 | 318.2 | 24.6% |
| sao | 7251944 | 33.1 | 232.9 | 14.2% |
| webster | 41458703 | 61.7 | 242.6 | 25.5% |
| x-ray | 8474240 | 31.1 | 198.1 | 15.7% |
| xml | 5345280 | 151.1 | 602.8 | 25.1% |

### Noise

**Aggregate noise exceeds 5%: the full pair has `aggregate_max_pct` = 6.08% and the quick pair 6.52%.** A win must therefore beat about 12% on the noisiest aggregate to count (twice the noise). The noise looks like machine-level variation (unpinned governor, shared desktop), not decoder behaviour. A human should check whether a pinned governor lowers it (manual check in the handoff); the plan continues with these values.

Full-run noise per aggregate key (`docs/perf/noise.json`; the same-code pair, |a - b| / a in percent):

| key | noise % | 2x noise % |
|---|---:|---:|
| canterbury/gzip/l1/oneshot | 5.10 | 10.21 |
| canterbury/gzip/l1/streaming | 5.83 | 11.66 |
| canterbury/gzip/l6/oneshot | 1.20 | 2.41 |
| canterbury/gzip/l6/streaming | 1.09 | 2.18 |
| canterbury/gzip/l9/oneshot | 3.72 | 7.43 |
| canterbury/gzip/l9/streaming | 2.29 | 4.57 |
| canterbury/raw/l1/oneshot | 1.38 | 2.77 |
| canterbury/raw/l1/streaming | 2.31 | 4.61 |
| canterbury/raw/l6/oneshot | 6.08 | 12.16 |
| canterbury/raw/l6/streaming | 3.48 | 6.97 |
| canterbury/raw/l9/oneshot | 0.24 | 0.49 |
| canterbury/raw/l9/streaming | 0.39 | 0.78 |
| canterbury/zlib/l1/oneshot | 3.75 | 7.51 |
| canterbury/zlib/l1/streaming | 3.97 | 7.93 |
| canterbury/zlib/l6/oneshot | 2.52 | 5.04 |
| canterbury/zlib/l6/streaming | 0.97 | 1.94 |
| canterbury/zlib/l9/oneshot | 1.36 | 2.72 |
| canterbury/zlib/l9/streaming | 3.62 | 7.24 |
| silesia/gzip/l1/oneshot | 1.56 | 3.12 |
| silesia/gzip/l1/streaming | 0.89 | 1.78 |
| silesia/gzip/l6/oneshot | 1.44 | 2.88 |
| silesia/gzip/l6/streaming | 2.66 | 5.33 |
| silesia/gzip/l9/oneshot | 0.97 | 1.93 |
| silesia/gzip/l9/streaming | 1.40 | 2.79 |
| silesia/raw/l1/oneshot | 3.80 | 7.60 |
| silesia/raw/l1/streaming | 3.53 | 7.06 |
| silesia/raw/l6/oneshot | 1.84 | 3.68 |
| silesia/raw/l6/streaming | 2.03 | 4.07 |
| silesia/raw/l9/oneshot | 0.36 | 0.72 |
| silesia/raw/l9/streaming | 0.93 | 1.87 |
| silesia/zlib/l1/oneshot | 0.38 | 0.77 |
| silesia/zlib/l1/streaming | 0.33 | 0.66 |
| silesia/zlib/l6/oneshot | 2.60 | 5.19 |
| silesia/zlib/l6/streaming | 1.13 | 2.25 |
| silesia/zlib/l9/oneshot | 0.52 | 1.05 |
| silesia/zlib/l9/streaming | 0.63 | 1.25 |

Quick-run noise (`docs/perf/noise-quick.json`): `aggregate_max_pct` = 6.52%; per key: canterbury/gzip/l6/oneshot 3.02%, canterbury/gzip/l6/streaming 2.43%, canterbury/raw/l6/oneshot 2.59%, canterbury/raw/l6/streaming 2.99%, canterbury/zlib/l6/oneshot 0.12%, canterbury/zlib/l6/streaming 1.58%, silesia/gzip/l6/oneshot 0.72%, silesia/gzip/l6/streaming 1.12%, silesia/raw/l6/oneshot 3.58%, silesia/raw/l6/streaming 4.37%, silesia/zlib/l6/oneshot 6.52%, silesia/zlib/l6/streaming 3.53%.

Sanity check: `make bench-compare` of each baseline pair against its own noise file reports NEUTRAL for every aggregate and no per-file REGRESSION.

### Targets

Targets from SPEC section 4.1 (Silesia aggregate, level 6) against the baseline ratios measured here:

| target | goal | baseline | gap |
|---|---|---:|---|
| primary, raw one-shot | >= 70% | 22.5% | 47.5 points |
| primary, zlib one-shot | >= 70% | 24.0% | 46.0 points |
| primary, gzip one-shot | >= 70% | 20.7% | 49.3 points |
| streaming, raw | >= 60% | 23.4% | 36.6 points |
| streaming, zlib | >= 60% | 24.9% | 35.1 points |
| streaming, gzip | >= 60% | 22.2% | 37.8 points |
| stretch, raw one-shot | >= 90% | 22.5% | 67.5 points |

No per-file regression against these baseline files, beyond the measured noise, is allowed at the end of phase 2.

### Baseline profile (P0-12)

Cleanflate-only profiles from `make profile` (`perf record --call-graph dwarf -F 499`, Silesia level 6, 3 iterations, debug info on, 12 files, 202 MiB per pass). Self time by symbol, entries above the 0.5% report limit (fewer than 15 symbols exceed it; inlined callees are folded into their callers, see the inclusive table further down). The first iteration includes the example's own length and CRC-32 check, which is the `checksum::crc32` row of the raw and zlib profiles.

**raw (rfc1951), one-shot** (profiled run: 60.4 MiB/s under `perf`):

| self % | symbol |
|---:|---|
| 41.16 | `cleanflate::huffman::Huffman::decode_table` |
| 35.88 | `cleanflate::inflate::Inflater::step` |
| 6.25 | `cleanflate::inflate::Inflater::peek_symbol` |
| 5.39 | (addresses with no symbol name, summed) |
| 4.91 | `cleanflate::checksum::crc32` |

**zlib (rfc1950), one-shot** (profiled run: 56.0 MiB/s under `perf`):

| self % | symbol |
|---:|---|
| 40.15 | `cleanflate::huffman::Huffman::decode_table` |
| 34.99 | `cleanflate::inflate::Inflater::step` |
| 6.96 | `cleanflate::inflate::Inflater::peek_symbol` |
| 4.86 | (addresses with no symbol name, summed) |
| 4.80 | `cleanflate::checksum::crc32` |
| 2.04 | `cleanflate::zlib::ZlibDecoder::step` |

**gzip (rfc1952), one-shot** (profiled run: 50.8 MiB/s under `perf`):

| self % | symbol |
|---:|---|
| 35.52 | `cleanflate::huffman::Huffman::decode_table` |
| 31.25 | `cleanflate::inflate::Inflater::step` |
| 12.65 | `cleanflate::gzip::GzipDecoder::step` |
| 5.70 | `cleanflate::inflate::Inflater::peek_symbol` |
| 4.18 | `cleanflate::checksum::crc32` |
| 4.14 | (addresses with no symbol name, summed) |
| 0.57 | `cleanflate::huffman::Huffman::from_lengths` |

**raw (rfc1951), streaming 64 KiB** (profiled run: 60.0 MiB/s under `perf`):

| self % | symbol |
|---:|---|
| 41.45 | `cleanflate::huffman::Huffman::decode_table` |
| 35.19 | `cleanflate::inflate::Inflater::step` |
| 7.42 | `cleanflate::inflate::Inflater::peek_symbol` |
| 4.86 | `cleanflate::checksum::crc32` |
| 4.61 | (addresses with no symbol name, summed) |

Inclusive share (children included, inlined frames named) for the raw one-shot profile:

| inclusive % | self % | frame |
|---:|---:|---|
| 47.41 | 0.00 | `peek_symbol (inlined)` |
| 41.16 | 41.16 | `cleanflate::huffman::Huffman::decode_table` |
| 39.50 | 0.00 | `decode_table_method (inlined)` |
| 35.88 | 35.88 | `cleanflate::inflate::Inflater::step` |
| 32.06 | 0.00 | `decode (inlined)` |
| 13.83 | 0.00 | `copy_match_path (inlined)` |
| 12.95 | 0.00 | `copy_match_bulk (inlined)` |
| 7.53 | 0.00 | `clone (inlined)` |
| 6.82 | 0.00 | `copy_from_slice<u8> (inlined)` |
| 6.25 | 6.25 | `cleanflate::inflate::Inflater::peek_symbol` |
| 4.91 | 4.91 | `cleanflate::checksum::crc32` |
| 3.23 | 0.00 | `fill (inlined)` |
| 2.74 | 0.00 | `peek (inlined)` |
| 2.26 | 0.00 | `take (inlined)` |
| 1.81 | 0.00 | `consume (inlined)` |

The rows overlap (an inlined frame is also part of its caller), so they do not add up to 100%. `decode (inlined)` is the bit-serial Huffman walk (32.1% inclusive); about 4% of samples sit in the `any` over the code-length counts that the uniform invalid-code rule of P0-04 added to that walk.

### Hypotheses (P0-12)

Counters come from `make bench-stats` (`docs/perf/P0-12-stats.json`, Silesia level 6, identical across framings because the deflate data is the same); shares from the profiles above. Threshold reminder for gated tasks: max(2%, 2x noise) of samples, with measured aggregate noise of 6.1%.

| hyp | metric | measured | verdict | est. share of decode time | drives |
|---|---|---|---|---|---|
| H1 table mostly bypassed | serial / (table + serial) decodes | litlen 46.0%, dist 48.3%; bytes per refill 1.08; `Huffman::decode` inclusive 32.1% | CONFIRMED (about half of all symbols take the serial walk) | ~32% in the serial walk, ~41% in `decode_table` in total | P1-01, P1-02, P2-01 |
| H2 per-symbol dispatch | `state_dispatches / output_bytes` | 0.644 dispatches per output byte (136405477 total); `Inflater::step` self 35.9%; `clone (inlined)` 7.5% inclusive (the only per-iteration clone is `self.state.clone()`; Silesia l6 has 0 fixed blocks, so no table clones) | CONFIRMED | ~36% self in `step`, ~7.5% of it State clone/drop | P2-01, P2-03 |
| H3 output written twice | `window_bytes_written / output_bytes` | 1.000; `copy_match_path` 13.8% inclusive, of which `copy_from_slice` 6.8% | CONFIRMED (every output byte is also written into the ring) | ~7-14% | P3-01 |
| H4 overlapping matches | overlap matches / matches | 0.48% of 17970872 matches; no byte-wise copy frame above the 0.5% report limit | DISCARDED | <1% | P3-02 (expect a small result) |
| H5 table build cost | `table_builds`, `fixed_table_clones` per block | 8424 builds over 2817 blocks = 2.99 per block; 0 fixed clones; `Huffman::from_lengths` self 0.5-0.6% | DISCARDED for Silesia l6 (about 75 KiB of output per block amortises it) | ~0.6% | P1-03 |
| H6 wrapper costs | gzip and zlib vs raw one-shot aggregate (baseline.json, Silesia l6); checksum share; `oneshot_resizes` | raw 62.3, zlib 61.3, gzip 51.8 MiB/s (gzip -16.9% vs raw, zlib -1.6%); CRC-32 is inlined in `GzipDecoder::step` at 12.65% self; Adler-32 about 2.0% (`ZlibDecoder::step`); one-shot resizes 9-12 per file (raw) and 513928468 bytes allocated for gzip against 301989888 for raw (212 MB of output) | PARTIAL: CRC-32 CONFIRMED (~12.7% for gzip), Adler-32 small (~2%), and gzip one-shot zero-fills an extra output-sized buffer on its last loop | gzip ~13% CRC + zero-fill; zlib ~2% | P4-01, P4-02, P3-03 |

### Cost ranking (P0-12)

Ordered by measured share of decode time (Silesia level 6, raw one-shot unless noted):

1. **H1, serial Huffman decode: about 32% inclusive** (`decode_table` as a whole 41%). Half of the symbols bypass the table because the bit buffer holds fewer than 10 bits (1.08 bytes per refill). Driven by P1-01 (bulk refill) and P1-02 (two-level tables).
2. **H2, state-machine dispatch: `Inflater::step` self time 36%**, with State clone/drop about 7.5% of samples. Addressed structurally by the P2-01 fast loop; P2-03 removes the clone in whatever slow path remains.
3. **H3, second write of every byte into the ring: about 7-14%** (`copy_match_path` 13.8% inclusive). Driven by P3-01.
4. **H6, CRC-32 on gzip: about 12.7%** (gzip only), plus the extra zero-filled buffer on the last one-shot growth step (P3-03). Driven by P4-01.
5. **H6, Adler-32 on zlib: about 2%.**
6. **H5, table building: about 0.6%.**
7. **H4, overlapping matches: below 1%** (0.48% of matches).

Gated tasks and the likely outcome, against the gate max(2%, 2x noise) = max(2%, ~12%):

- **P1-03 (table reuse): likely a negative result.** Table building is 0.6% and there are no fixed blocks at level 6; it would only matter for many-tiny-block inputs. Re-check with the gate procedure before closing it.
- **P4-02 (Adler-32): likely a negative result.** About 2% is at or below the gate, and below 2x the measured noise.
- **P2-03 (State Copy): decide after the fast loop.** State clone/drop is 7.5% today, below 2x noise (~12%) even before P2-01 shrinks the slow path, so a negative result is likely unless the slow path grows.
- **P1-04 (length/distance payload in table entries): undecided.** Needs `perf annotate` of `Inflater::step` (LenExtra/DistExtra handling is folded into `step`'s 36% self time); do not assume.

The noise (6.1% aggregate) is large next to most of these shares: only H1, H2 and possibly H3/H6-CRC can clear twice the noise on their own.


### M0 acceptance (P0-13)

Commands run on the M0 head (`14ce067`): `make ci` (green, 1m40s), `make heavy` (proptest-10k for chunking and fastslow, bomb, conformance-full, fastslow-full: all green; fastslow-full 248.7 s), and `make bench BENCH_ID=P0-13 BENCH_OUT=docs/perf/P0-13.json` (12.5 min).

Checklist with evidence:

- [x] Baseline JSON and noise are committed: `docs/perf/baseline.json`, `baseline-rerun.json`, `noise.json`, `baseline-quick.json`, `baseline-quick-rerun.json`, `noise-quick.json` (Phase 2 section above).
- [x] The differential runs in `make test`: `crates/oracle/tests/fastslow.rs::{fastslow_conformance_oneshot, fastslow_conformance_streaming, fastslow_malformed, fastslow_proptest}`, the fixtures in `crates/oracle/tests/regressions.rs::{regressions_all_paths, regressions_manifest_matches_files}`, and in cleanflate `tests/decode_path.rs` and `tests/huffman_table.rs::table_decode_equals_serial_for_all_buffer_states`.
- [x] H1 to H6 each have a number and a verdict: see "Hypotheses (P0-12)" and "Cost ranking (P0-12)" above.
- [x] The `stats` and `test-hooks` features are linted and tested by `make lint` and `make test`; the default build carries no counter code.

`make bench-compare A=docs/perf/baseline.json B=docs/perf/P0-13.json`: **27 WIN, 0 LOSS, 9 NEUTRAL aggregates and 32 per-file REGRESSION lines, although no decoder code changed between the two runs** (both measure `a59e2e1`-equivalent decoder code: the only later commits are documentation, tooling and the tolerance fix in `bench-compare`). Silesia level 6 one-shot moved raw 62.3 to 66.2, zlib 61.3 to 63.7 and gzip 51.8 to 54.5 MiB/s. This is measurement drift, not a code effect, and it shows that the noise estimated from the single baseline pair (6.1% aggregate) understates run-to-run variation on this machine: the machine ran about 6% faster during the M0 run. Consequences for later tasks:

- Do not treat a single WIN or REGRESSION verdict against `baseline.json` as proof. Confirm a claimed win by an interleaved before/after comparison in the same session, or with a fresh baseline pair taken just before the change.
- Per-file REGRESSION lines from `bench-compare` are unreliable at this noise level. Treat them as prompts to re-measure, not as failures.
- The M0 run, not `baseline.json`, is the better reference for absolute numbers taken late in the session; both are committed.

Provenance: `make provenance-snapshot` was run. The log holds three denied commands from this phase, all mine and benign (two recursive `grep` at the repository root without `--exclude-dir=target`, and one `sed` whose regular expression contained a forbidden word as a path-like argument); each was re-run another way. No denied profiling command appears.

Push: see the progress log for this task.

### P1-01

Bulk bit refill with an exact give-back invariant (D-27). `make bench-quick`, before (`docs/perf/P1-01-before.json`) and after (`docs/perf/P1-01.json`), same session, `make bench-compare` with `docs/perf/noise-quick.json`:

| key | before MiB/s | after MiB/s | delta % | 2x noise % | verdict |
|---|---:|---:|---:|---:|---|
| silesia/raw/l6/oneshot | 51.1 | 79.3 | +55.1 | 7.2 | WIN |
| silesia/raw/l6/streaming | 50.6 | 78.2 | +54.6 | 8.7 | WIN |
| silesia/zlib/l6/oneshot | 49.7 | 75.5 | +52.1 | 13.0 | WIN |
| silesia/zlib/l6/streaming | 51.5 | 75.5 | +46.6 | 7.1 | WIN |
| silesia/gzip/l6/oneshot | 45.3 | 57.8 | +27.7 | 1.4 | WIN |
| silesia/gzip/l6/streaming | 47.1 | 61.6 | +30.7 | 2.2 | WIN |
| canterbury/raw/l6/oneshot | 74.6 | 104.2 | +39.7 | 5.2 | WIN |
| canterbury/zlib/l6/oneshot | 76.2 | 93.7 | +23.0 | 0.2 | WIN |
| canterbury/gzip/l6/oneshot | 65.2 | 77.5 | +18.9 | 6.0 | WIN |

All twelve aggregates are WIN and there is no per-file REGRESSION. The machine was in a slow phase for this pair (the "before" raw figure is well below the M0 baseline), so the absolute numbers are not comparable with `baseline.json`; the before/after pair was taken back to back. The effect is far larger than the noise, which is what the table-lookup hypothesis (H1) predicted: the table now serves nearly every symbol.

### P1-02

Two-level lookup tables for every symbol (D-28). `make bench-quick` before (`docs/perf/P1-02-before.json`) and after (`docs/perf/P1-02.json`), back to back, compared with `docs/perf/noise-quick.json`:

| key | before MiB/s | after MiB/s | delta % | 2x noise % | verdict |
|---|---:|---:|---:|---:|---|
| silesia/raw/l6/oneshot | 78.9 | 81.9 | +3.8 | 7.2 | NEUTRAL |
| silesia/raw/l6/streaming | 80.7 | 83.0 | +2.9 | 8.7 | NEUTRAL |
| silesia/zlib/l6/oneshot | 81.4 | 78.3 | -3.8 | 13.0 | NEUTRAL |
| silesia/zlib/l6/streaming | 81.1 | 80.1 | -1.2 | 7.1 | NEUTRAL |
| silesia/gzip/l6/oneshot | 63.5 | 61.2 | -3.6 | 1.4 | LOSS |
| silesia/gzip/l6/streaming | 67.9 | 68.3 | +0.6 | 2.2 | NEUTRAL |
| canterbury/raw/l6/oneshot | 99.2 | 108.3 | +9.2 | 5.2 | WIN |
| canterbury/gzip/l6/oneshot | 75.1 | 84.6 | +12.5 | 6.0 | WIN |
| canterbury/gzip/l6/streaming | 74.9 | 87.9 | +17.3 | 4.9 | WIN |

A second after-run (not committed) gave silesia gzip one-shot +0.3% (NEUTRAL) and silesia zlib one-shot -7.6% (NEUTRAL); the single LOSS above is inside the run-to-run drift seen in P0-13 and did not repeat, and no per-file verdict was treated as reliable. Result: **NEUTRAL on Silesia, kept under PT4 as the named prerequisite of the fast loop (P2-01)**, which needs one total lookup per symbol. `make bench-stats` after this change: litlen and dist table-decode 100.0% (was 54.0% and 51.7% at baseline), bytes per refill 6.21 (was 1.08). Most of the gain from removing the serial walk came from P1-01's bulk refill, which is why this task adds little on top.

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

### P1-03 (gated, negative result)

Gate (PT2): `make bench-stats` and `make profile PROFILE_FRAMING=rfc1951` (Silesia level 6, 3 iterations, cleanflate-only). Threshold: max(2%, 2x aggregate noise); the measured aggregate noise is 1.8% for raw one-shot (worst aggregate 6.1%), so the threshold is at least 3.7% and at most about 12%.

Measured, after P1-01 and P1-02:

| item | value |
|---|---|
| `make bench-stats` table builds | 8424 over 2817 blocks (2.99 per block: code-length, literal/length, distance), 0 fixed-table clones (there are no fixed blocks at level 6), about 75 KiB of output per block |
| `Huffman::from_lengths` (inclusive, including `build_table`) | 1.29% of samples (self 0.93%; `build_table` 0.72%) |
| table clones | none in this input (0 fixed blocks); the `clone` frame at 9.45% inclusive is `Inflater::step`'s per-iteration `self.state.clone()` (P2-03's target), not a table clone |
| allocation frames under table building (`reserve`, `grow`) | 0.43% at most |

Table costs together are about 1.7% of samples, below the 2% floor and far below twice the noise. **Negative result: no code was changed.** The two ideas (reference the fixed tables instead of cloning; rebuild dynamic tables in place) would only matter for inputs with very many tiny blocks, which neither corpus contains; the fixed-block clone is a copy of two small tables per fixed block and is not on any measured path. Revisit only if a workload of many small dynamic or fixed blocks is added to the benchmark.

### P1-04 (gated, negative result)

Gate (PT2): the cleanflate-only profile of `make profile PROFILE_FRAMING=rfc1951` (Silesia level 6, 3 iterations, after P1-01 and P1-02), read with `perf report --sort srcline` on `target/profile/perf.data` (the annotate view of `Inflater::step` attributed by source line), together with the dispatch counters. Threshold: max(2%, 2x aggregate noise), at least 3.7% for raw one-shot and about 12% for the worst aggregate.

Share of samples by state arm of `Inflater::step` (source lines of `inflate.rs`):

| region | share |
|---|---:|
| `LenExtra` arm (base and extra-bit lookup for lengths) | 0.63% |
| `DistExtra` arm (base and extra-bit lookup for distances, distance check) | 0.73% |
| `Dist` arm (decode of the distance symbol) | 1.11% |
| `Copy` arm | 0.64% |
| literal/length arm (`LitLen`) | 4.95% |
| `match &self.state` (dispatch) | 16.5% |
| `derive(Clone)` of `State` and `self.state.clone()` | 9.5% + 5.6% |

The extra-bit reads themselves go through `BitReader::take`; the whole of `bitreader.rs` (fills, takes, peeks and consumes, including the bulk refill) is about 5.5% of samples, so even attributing all of it to length and distance extras would not be the target of this task. The base and extra-bit table handling that P1-04 would move into the table entries is 0.63% + 0.73% = 1.4% of samples: **below the 2% floor and far below twice the noise. Negative result: no code was changed.** The cost that matters is the dispatch and the per-iteration `State` clone (about 31% together), which the fast loop (P2-01) and the copyable state (P2-03) address; the fast loop will read the length and distance tables directly, so no payload-in-entry step is needed first.

### M1 acceptance (P1-05)

Commands run on the M1 head: `make ci` (green, 1m36s), `make heavy` (proptest-10k for chunking and fastslow, bomb, conformance-full, fastslow-full: all green; fastslow-full 209 s), `make bench-stats BENCH_ID=P1-05 BENCH_OUT=docs/perf/P1-05-stats.json`, and `make bench BENCH_ID=P1-05 BENCH_OUT=docs/perf/P1-05.json` (10.0 min). `docs/perf/P1-05.json` and `docs/perf/P1-05-stats.json` are committed.

**Table-decode share (SPEC M1: >= 99%): MET.** On Silesia level 6, literal/length symbols 100.000% and distance symbols 100.000% are resolved by table lookup for raw, zlib and gzip framing (baseline: 54.0% and 51.7%). Bytes per refill 6.21 (baseline 1.08).

Aggregate gain against the M0 baseline (Silesia, level 6, median MiB/s; C zlib is the same-run one-shot median):

| key | baseline | M1 | gain | C zlib (M1 run) | cleanflate / C zlib |
|---|---:|---:|---:|---:|---:|
| silesia/raw/l6/oneshot | 62.3 | 98.9 | +58.7% | 277.0 | 35.7% |
| silesia/raw/l6/streaming | 65.0 | 107.8 | +66.0% | 277.0 | 38.9% |
| silesia/zlib/l6/oneshot | 61.3 | 94.5 | +54.3% | 249.5 | 37.9% |
| silesia/zlib/l6/streaming | 63.7 | 102.4 | +60.7% | 249.5 | 41.1% |
| silesia/gzip/l6/oneshot | 51.8 | 75.3 | +45.3% | 249.2 | 30.2% |
| silesia/gzip/l6/streaming | 55.7 | 82.0 | +47.2% | 249.2 | 32.9% |

`make bench-compare` of the M1 run against `baseline.json` reports WIN on every Silesia level-6 aggregate and no per-file REGRESSION line; against `P0-13.json` (which was itself about 6% faster than the baseline through drift, see M0) the gains are +38% to +54% and again no REGRESSION. The gain comes almost entirely from P1-01 (bulk refill, D-27); P1-02 (D-28) was NEUTRAL alone and is kept as the prerequisite of the fast loop; P1-03 and P1-04 were gated out with negative results (see their sections). The M1 numbers do not yet reach the primary target (70% of C zlib) or the streaming target (60%): they are recorded here for the M4 acceptance.

Correctness gates: `make ci`, `make heavy` (10k-case chunking and fast-vs-reference proptests, full-tier conformance and differential, bomb) all pass; no regression fixture was needed.

### P2-01

Fast literal/length/distance decode loop (D-29). `make bench-quick` before (`docs/perf/P2-01-before.json`, the M1 code) and after (`docs/perf/P2-01.json`), back to back, compared with `docs/perf/noise-quick.json`:

| key | before MiB/s | after MiB/s | delta % | 2x noise % | verdict |
|---|---:|---:|---:|---:|---|
| silesia/raw/l6/oneshot | 83.1 | 132.2 | +59.0 | 7.2 | WIN |
| silesia/raw/l6/streaming | 83.8 | 135.5 | +61.8 | 8.7 | WIN |
| silesia/zlib/l6/oneshot | 80.2 | 131.0 | +63.4 | 13.0 | WIN |
| silesia/zlib/l6/streaming | 81.9 | 130.9 | +59.9 | 7.1 | WIN |
| silesia/gzip/l6/oneshot | 66.1 | 92.6 | +40.0 | 1.4 | WIN |
| silesia/gzip/l6/streaming | 70.2 | 100.9 | +43.7 | 2.2 | WIN |
| canterbury/raw/l6/oneshot | 110.8 | 155.2 | +40.1 | 5.2 | WIN |
| canterbury/zlib/l6/oneshot | 104.6 | 156.8 | +49.9 | 0.2 | WIN |
| canterbury/gzip/l6/oneshot | 86.9 | 115.7 | +33.2 | 6.0 | WIN |

All twelve aggregates are WIN and there is no per-file REGRESSION. The gain is far beyond the noise: the loop removes the per-symbol `State` clone and dispatch (the two costs that together were about 31% of samples at the M0 profile). gzip gains least because its CRC-32 is untouched (P4-01).

### P2-03 (gated, negative result)

Gate (PT2): `make profile PROFILE_FRAMING=rfc1951` (Silesia level 6, 3 iterations, cleanflate-only) after the fast loop (P2-01). Threshold: max(2%, 2x aggregate noise), so at least 2%.

| item | before the fast loop (M0 profile) | after P2-01 |
|---|---:|---:|
| `Inflater::step` self time | 35.9% | 0.49% |
| `clone (inlined)` (the per-iteration `self.state.clone()`) inclusive | 7.5% | 0.15% |
| `match &self.state` dispatch and `derive(Clone)` of `State` (source lines) | 16.5% + 15.1% | not among the source lines above 0.3% |
| `fast_loop::run` self | - | 38.4% |
| decode rate under `perf` | 60 MiB/s | 138 MiB/s |

The slow path's `Inflater::step` self time plus the State clone frame is about 0.6% of samples, far below the 2% floor: the fast loop already took the per-symbol state handling out of the hot path, and the state machine now only runs at block boundaries, near buffer edges and for stored blocks. **Negative result: no code was changed.** Making `State` `Copy` and moving the sticky error into its own field would touch every error path for a saving that cannot be measured. Revisit only if a workload dominated by tiny blocks or by very small output buffers (where the loop's margins never hold) is added; the streaming target with 64 KiB windows keeps the loop active.

### M2 acceptance (P2-04)

Commands run on the M2 head: `make ci` (green, 1m30s), `make heavy` (proptest-10k for chunking and fastslow, bomb, conformance-full, fastslow-full: all green, including the full-tier fast-vs-reference differential over the whole corpus), `make fuzz-smoke` (all six targets, 60 s each, zero crashes: `diff_fast_reference` 96,588 runs, and 186,792 / 150,029 / 165,061 / 51,359 / 142,290 runs for the five phase-1 targets in Makefile order), `make bench BENCH_ID=P2-04 BENCH_OUT=docs/perf/P2-04.json` (7.8 min), and `make bench-stats` (not committed). `docs/perf/P2-04.json` is committed.

Aggregate gains, Silesia level 6, median MiB/s (C zlib is the same-run one-shot median):

| key | baseline | M1 (P1-05) | M2 | gain vs baseline | C zlib (M2 run) | cleanflate / C zlib |
|---|---:|---:|---:|---:|---:|---:|
| silesia/raw/l6/oneshot | 62.3 | 98.9 | 153.8 | +147.0% | 274.9 | 56.0% |
| silesia/raw/l6/streaming | 65.0 | 107.8 | 176.8 | +172.1% | 274.9 | 64.3% |
| silesia/zlib/l6/oneshot | 61.3 | 94.5 | 147.9 | +141.4% | 251.6 | 58.8% |
| silesia/zlib/l6/streaming | 63.7 | 102.4 | 166.8 | +161.8% | 251.6 | 66.3% |
| silesia/gzip/l6/oneshot | 51.8 | 75.3 | 105.8 | +104.2% | 255.9 | 41.3% |
| silesia/gzip/l6/streaming | 55.7 | 82.0 | 124.9 | +124.2% | 255.9 | 48.8% |

`make bench-compare` against `baseline.json` and against `P1-05.json` reports no per-file REGRESSION and no LOSS. Against the SPEC targets (recorded, not blocking): streaming for raw and zlib is above the 60% target already (64.3% and 66.3%; gzip streaming 48.8%); one-shot raw and zlib are at 56.0% and 58.8% (target 70%), gzip one-shot 41.3%, held back by its CRC-32 (P4-01) and by the one-shot output buffer (M3). These are NOT YET MET for the one-shot primary target; the M4 acceptance repeats the comparison.

Fast-loop coverage (`make bench-stats`, Silesia level 6 raw): `fast_loop_symbols` = 45,894,668 of 45,902,136 symbols (literals, matches and end-of-block symbols), 99.98%; `fast_loop_entries` = 2,918; dispatches per output byte 0.0032 (baseline 0.644).

Repro audit: every fixture in `testdata/regressions/` was added in phase 2 and passes on both decode paths (`crates/oracle/tests/regressions.rs`, run by `make test` and `make heavy`):

| fixture | origin |
|---|---|
| `rfc1951/fixed-heavy-match-11k.bin` | seed: shape of the lost phase-1 P4-08 repro (Fixed, ~11 KiB, match-heavy), not its bytes |
| `rfc1950/fixed-heavy-match-11k.bin` | same data as zlib, chunks 7,1,13 / 1,258,3 |
| `rfc1952/two-members.bin` | seed: two concatenated gzip members with different data |
| `rfc1951/dynamic-l9-64k.bin` | seed: 64 KiB text-like data at level 9 (dynamic blocks) |

No wrong output, error or `consumed` was found by any differential run, proptest or fuzz run in phase 2, so no further fixture was needed; the boundary and error tests of P2-01 (`tests/fast_loop.rs`) were mutation-checked instead.

### P3-01

Output as match history; window carried only across calls (D-30). `make bench-quick` before (`docs/perf/P3-01-before.json`, the M2 code) and after (`docs/perf/P3-01.json`), back to back, compared with `docs/perf/noise-quick.json`:

| key | before MiB/s | after MiB/s | delta % | 2x noise % | verdict |
|---|---:|---:|---:|---:|---|
| silesia/raw/l6/oneshot | 133.4 | 160.6 | +20.3 | 7.2 | WIN |
| silesia/raw/l6/streaming | 134.3 | 160.4 | +19.5 | 8.7 | WIN |
| silesia/zlib/l6/oneshot | 122.1 | 151.1 | +23.7 | 13.0 | WIN |
| silesia/zlib/l6/streaming | 124.2 | 148.2 | +19.3 | 7.1 | WIN |
| silesia/gzip/l6/oneshot | 87.9 | 108.2 | +23.1 | 1.4 | WIN |
| silesia/gzip/l6/streaming | 103.1 | 112.5 | +9.1 | 2.2 | WIN |
| canterbury/raw/l6/oneshot | 189.1 | 214.6 | +13.5 | 5.2 | WIN |
| canterbury/zlib/l6/oneshot | 171.9 | 205.8 | +19.7 | 0.2 | WIN |
| canterbury/gzip/l6/oneshot | 120.8 | 141.1 | +16.8 | 6.0 | WIN |

All twelve aggregates are WIN. Three per-file REGRESSION lines were printed (canterbury xargs.1 gzip one-shot and streaming, -4.8% and -5.2% on a 4 KiB file; silesia sao gzip streaming, -10.5%). They are findings, not failures: the aggregates for the same keys are WINs, the per-file thresholds for these keys are 1.1% to 3.0% and rest on a single noise pair, and no other framing of the same files regressed. `make bench-stats` confirms the mechanism: `window_bytes_written / output_bytes` (bytes actually copied into the ring per output byte) fell from 1.000 (every byte written twice, one at a time) to 0.019 for one-shot decoding (large output slices: at most 32 KiB per call) and 0.50 for 64 KiB streaming windows (32 KiB of each 64 KiB window), all as slice copies at call exits.

### P3-02 (negative result, code reverted)

Overlapping matches by pattern doubling (copy the growing repeated pattern in doubling steps, with `fill` for distance 1) was implemented in `Window::copy_match`, tested against an independent model, and measured twice against the M3 code (`docs/perf/P3-02-before.json`; after: `docs/perf/P3-02.json` and a repeat `docs/perf/P3-02-rerun.json`, both taken with the doubling code). Silesia level 6 one-shot medians:

| key | before MiB/s | run 1 | verdict | run 2 | verdict |
|---|---:|---:|---|---:|---|
| silesia/raw/l6/oneshot | 158.3 | 171.2 | WIN (+8.1%) | 162.6 | NEUTRAL (+2.7%) |
| silesia/zlib/l6/oneshot | 142.9 | 158.6 | NEUTRAL (+11.0%) | 156.0 | NEUTRAL (+9.2%) |
| silesia/gzip/l6/oneshot | 108.0 | 111.1 | WIN (+2.9%) | 106.5 | NEUTRAL (-1.4%) |
| silesia/gzip/l6/streaming | 121.4 | 123.8 | NEUTRAL (+2.0%) | 117.3 | LOSS (-3.3%) |

Run 1 gave 6 WIN and 6 NEUTRAL aggregates, run 2 gave 1 WIN, 1 LOSS and 10 NEUTRAL, and the per-file REGRESSION lines fell on different files each time. The effect is inside the run-to-run variation, as the M0 profile predicted: only 0.48% of matches have a distance smaller than their length (H4, DISCARDED). Under PT4 a change that is not a WIN and not the prerequisite of a later task is reverted: **the code was reverted; the tests (`tests/history.rs::pattern_doubling_matches_model_exhaustive`, `pattern_doubling_straddling_window`) were kept and pass against the slice-chunk copy of P3-01, so the overlap semantics remain covered.** No DESIGN entry was written because no technique was kept. The per-file table, the doubling derivation (a copy from `k` back with `k` a multiple of the distance keeps the period) and both JSONs are the record.

### P3-03

One-shot output sizing from hints (D-31). `make bench-quick` before (`docs/perf/P3-03-before.json`) and after (`docs/perf/P3-03.json`, and a repeat `docs/perf/P3-03-rerun.json`), plus `make bench-stats` before and after:

| counter, Silesia level 6, one-shot | before | after |
|---|---:|---:|
| `oneshot_resizes`, raw and zlib | 122 | 15 |
| `oneshot_resizes`, gzip | 134 | 12 |
| `oneshot_bytes_allocated`, raw | 301,989,888 | 304,788,200 |
| `oneshot_bytes_allocated`, gzip | 513,928,468 | 211,938,580 (the output size exactly) |

Timing, Silesia level 6 one-shot medians (MiB/s), with the untouched streaming path as a drift control:

| key | before | run 1 | run 2 |
|---|---:|---:|---:|
| gzip one-shot | 108.1 | 117.9 (WIN, +9.1%) | 109.4 (NEUTRAL) |
| gzip streaming (unchanged code) | 120.5 | 118.9 | 109.5 |
| gzip one-shot / streaming | 0.897 | 0.991 | 0.999 |
| raw one-shot | 162.8 | 165.8 (NEUTRAL) | 152.0 (NEUTRAL) |
| raw streaming (unchanged code) | 159.5 | 164.9 | 151.8 |
| raw one-shot / streaming | 1.021 | 1.005 | 1.001 |

The machine drifts by up to 10% between runs (the untouched streaming keys show the same swings, including two LOSS verdicts on Canterbury zlib and gzip streaming in the repeat), so absolute deltas are not reliable here; the one-shot to streaming ratio removes the drift. It shows the expected effect: gzip one-shot, which allocated twice its output and zero-filled the extra buffer, gains about 10% relative to streaming, and raw and zlib one-shot are unchanged (the decode dominates; the growth steps were cheap next to it). Result: **kept**, a WIN for gzip one-shot (H6) and neutral elsewhere, with the allocation counters as the direct evidence. The bomb test passes unchanged.

### M3 acceptance (P3-04)

Commands run on the M3 head: `make ci` (green, 1m34s), `make heavy` (proptest-10k for chunking and fastslow, bomb, conformance-full, fastslow-full: all green; fastslow-full 188.6 s), and `make bench BENCH_ID=P3-04 BENCH_OUT=docs/perf/P3-04.json` (7.0 min). `docs/perf/P3-04.json` is committed.

Silesia level 6, median MiB/s (C zlib is the same-run one-shot median, the denominator for both tables):

One-shot (`decompress_with`), target 70% of C zlib:

| framing | baseline | M2 (P2-04) | M3 | gain vs baseline | C zlib (M3 run) | cleanflate / C zlib | target |
|---|---:|---:|---:|---:|---:|---:|---|
| raw | 62.3 | 153.8 | 190.6 | +206.0% | 285.9 | 66.7% | NOT MET (70%) |
| zlib | 61.3 | 147.9 | 177.5 | +189.8% | 255.1 | 69.6% | NOT MET (70%), 0.4 points short |
| gzip | 51.8 | 105.8 | 135.5 | +161.5% | 253.8 | 53.4% | NOT MET (70%) |

Streaming (`Decompressor`, 64 KiB input and 64 KiB output windows), target 60% of C zlib one-shot:

| framing | baseline | M2 (P2-04) | M3 | gain vs baseline | C zlib (M3 run) | cleanflate / C zlib | target |
|---|---:|---:|---:|---:|---:|---:|---|
| raw | 65.0 | 176.8 | 212.4 | +227.0% | 285.9 | 74.3% | MET (60%) |
| zlib | 63.7 | 166.8 | 196.8 | +208.8% | 255.1 | 77.2% | MET (60%) |
| gzip | 55.7 | 124.9 | 141.7 | +154.3% | 253.8 | 55.8% | NOT MET (60%) |

`make bench-compare` against `baseline.json`: no LOSS and no per-file REGRESSION. Against `P2-04.json` (M2): every Silesia level-6 aggregate is a WIN (+18% to +28%), no LOSS, and two per-file REGRESSION lines (silesia mr and ooffice zlib streaming, -3.7% and -3.9%), recorded as findings: the aggregates for the same keys are +18.0% WINs and per-file thresholds of 1.1% rest on a single noise pair. The remaining gap is dominated by CRC-32 for gzip (P4-01) and, for raw and zlib one-shot, by decode itself; the primary targets are re-assessed at M4.

Bomb and memory: `make bomb` (run by `make heavy`) passes unchanged, 3 tests in 9.8 s. `decompressor_size_not_grown`: `size_of::<Decompressor>()` is 33,688 bytes, identical to the pre-P3-01 size (limit 33,688 + 1,024). One-shot allocation is bounded by the output limit and by `ONESHOT_MAX_PREALLOC` for hints (D-31); gzip one-shot now allocates exactly its output size (211.9 MB for the Silesia set, down from 513.9 MB).

### P4-01

CRC-32 several bytes per step (D-32). `make bench-quick` before (`docs/perf/P4-01-before.json`) and after (`docs/perf/P4-01.json`), back to back, compared with `docs/perf/noise-quick.json`:

| key | before MiB/s | after MiB/s | delta % | 2x noise % | verdict |
|---|---:|---:|---:|---:|---|
| silesia/gzip/l6/oneshot | 120.2 | 154.3 | +28.4 | 1.4 | WIN |
| silesia/gzip/l6/streaming | 119.2 | 153.0 | +28.3 | 2.2 | WIN |
| canterbury/gzip/l6/oneshot | 152.5 | 206.7 | +35.6 | 6.0 | WIN |
| canterbury/gzip/l6/streaming | 145.1 | 206.0 | +42.0 | 4.9 | WIN |
| silesia/raw/l6/oneshot | 163.8 | 161.5 | -1.4 | 7.2 | NEUTRAL |
| silesia/zlib/l6/oneshot | 151.4 | 148.8 | -1.7 | 13.0 | NEUTRAL |

The zlib and raw aggregates, which do not compute a CRC-32, stay NEUTRAL, as they should. Overhead of gzip over raw (raw / gzip - 1, medians):

| corpus, mode | before | after |
|---|---:|---:|
| silesia one-shot | 36.3% | 4.6% |
| silesia streaming | 38.5% | 4.4% |
| canterbury one-shot | 46.5% | 13.4% |
| canterbury streaming | 51.8% | 11.0% |

No per-file REGRESSION line was printed for a gzip key. Seven were printed for raw and zlib keys (for example silesia dickens zlib -18%), whose code path does not touch the CRC-32; they are run-to-run drift (this pair's noise file is from a single earlier pair) and confirm that per-file lines are prompts to re-measure, not verdicts. Result: **kept**, a WIN of about +28% on Silesia gzip, which closes almost all of the gap between gzip and raw framing.

### P4-02 (gated, negative result)

Gate (PT2): the Adler-32 share in `make profile PROFILE_FRAMING=rfc1950` (Silesia level 6, 3 iterations, cleanflate-only, after P4-01), against max(2%, 2x aggregate noise).

| item | share of samples |
|---|---:|
| `update (inlined)` of `Adler32` inside `ZlibDecoder::step` | 6.15% |
| (for comparison) the example's own CRC-32 length check, `checksum::crc32` | 3.29% |
| `fast_loop::run` / `Huffman::decode_table` / `Window::copy_match` | 41.2% / 26.7% / 9.0% |

At M0 the Adler-32 was about 2% of samples; the rest of the decoder became about three times faster, so its share rose to 6.15% while its absolute cost is unchanged. Noise thresholds for the affected keys: the full-run noise of silesia zlib one-shot is 2.6% (twice: 5.2%) and of streaming 1.1% (2.3%), but the quick bench that PT1 uses for a before/after pair has 6.5% (twice: 13.0%) for one-shot and 3.5% (7.1%) for streaming. Even removing the Adler-32 completely would speed zlib framing up by at most 1 / (1 - 0.0615) - 1 = 6.6%, which is below twice the quick noise for both keys, so no implementation of this task could be shown as a WIN under PT1 (and PT4 would then require reverting it). The measured gain the previous, larger cost centres delivered (D-27 to D-32) is where the remaining budget sits. **Negative result: no code was changed.** Revisit if the quick-bench noise falls below about 3% (a pinned governor on a quiet machine would do it): the allowed change would be fixed-size sub-blocks (`chunks_exact`) inside the existing `ADLER_NMAX` reduction, and the tests named in the plan (`adler32_chunked_matches_bytewise`, the oracle long-buffer test) would go with it.

### Tunables (P4-03)

Every performance tunable that exists on the final code was re-swept, one key at a time with the others at their defaults: `oracle` quick bench (`make bench-quick`, level 6, Silesia and Canterbury, all framings, one-shot and streaming), one run per candidate, with a run of the default first in each group (`docs/perf/P4-03/<KEY>-<value>.json`). A quick WIN must be confirmed by the full bench and leave no REGRESSION before a default may change; otherwise the default is kept and the measured "no change" is recorded.

Result: **no default changed.** Medians of the relevant aggregate (raw for the decode tunables, gzip for the CRC), default -> candidate, MiB/s:

| key | default | candidate | aggregate | one-shot | streaming |
|---|---:|---:|---|---|---|
| `LITLEN_TABLE_BITS` | 10 | 9 | Silesia raw l6 | 158.6 -> 157.0 (-1.0%) | 160.0 -> 162.8 (+1.7%) |
| `LITLEN_TABLE_BITS` | 10 | 11 | Silesia raw l6 | 158.6 -> 161.4 (+1.8%) | 160.0 -> 165.2 (+3.3%) |
| `LITLEN_TABLE_BITS` | 10 | 12 | Silesia raw l6 | 158.6 -> 161.2 (+1.6%) | 160.0 -> 161.0 (+0.7%) |
| `DIST_TABLE_BITS` | 8 | 7 | Silesia raw l6 | 156.0 -> 148.3 (-5.0%) | 160.3 -> 149.9 (-6.5%) |
| `DIST_TABLE_BITS` | 8 | 9 | Silesia raw l6 | 156.0 -> 145.6 (-6.7%) | 160.3 -> 147.2 (-8.2%) |
| `DIST_TABLE_BITS` | 8 | 10 | Silesia raw l6 | 156.0 -> 142.2 (-8.8%) | 160.3 -> 147.6 (-7.9%) |
| `FAST_LOOP_MIN_INPUT` | 16 | 8 | Silesia raw l6 | 150.8 -> 165.6 (+9.8%) | 152.7 -> 165.9 (+8.6%) |
| `FAST_LOOP_MIN_INPUT` | 16 | 32 | Silesia raw l6 | 150.8 -> 164.5 (+9.1%) | 152.7 -> 167.8 (+9.9%) |
| `FAST_LOOP_MIN_OUTPUT` | 258 | 320 | Silesia raw l6 | 153.0 -> 162.2 (+6.0%) | 154.1 -> 163.7 (+6.2%) |
| `FAST_LOOP_MIN_OUTPUT` | 258 | 512 | Silesia raw l6 | 153.0 -> 161.2 (+5.3%) | 154.1 -> 162.7 (+5.6%) |
| `CRC32_SLICES` | 8 | 4 | Silesia gzip l6 | 153.1 -> 135.6 (-11.4%) | 153.3 -> 137.0 (-10.6%) |
| `CRC32_SLICES` | 8 | 16 | Silesia gzip l6 | 153.1 -> 151.7 (-0.9%) | 153.3 -> 151.6 (-1.1%) |
| `ONESHOT_INITIAL_RATIO` | 4 | 2 | Silesia raw l6 | 161.7 -> 150.6 (-6.8%) | 167.7 -> 152.5 (-9.1%) |
| `ONESHOT_INITIAL_RATIO` | 4 | 8 | Silesia raw l6 | 161.7 -> 140.9 (-12.9%) | 167.7 -> 161.7 (-3.6%) |

How to read this table. Consecutive runs on this machine drift by up to about 10% (the M0 and M3 sections show the same; in this sweep `ONESHOT_INITIAL_RATIO` candidates of 2 and 8 print LOSS on the streaming keys, which the ratio cannot touch, and both `FAST_LOOP_MIN_INPUT` candidates print a +9% one-shot WIN against a default run that was itself a slow outlier). A repeat run of the unchanged defaults, taken at the end of the sweep (`docs/perf/P4-03/default-repeat.json`), gave Silesia level 6 raw one-shot 159.1 and streaming 162.9 MiB/s, gzip one-shot 141.4, which is the level of the "candidate" runs in the `FAST_LOOP_MIN_INPUT` and `FAST_LOOP_MIN_OUTPUT` groups. Against that repeat no candidate is a confirmed improvement. Verdicts by tunable:

- `LITLEN_TABLE_BITS` (9, 11, 12 against 10): within +-3.3%, no verdict beyond twice the noise on the raw keys; keep 10.
- `DIST_TABLE_BITS` (7, 9, 10 against 8): 9 and 10 are 7% to 9% slower on raw (the larger tables cost more than they save), 7 is 5% to 6.5% slower on raw; keep 8.
- `FAST_LOOP_MIN_INPUT` (8, 32 against 16) and `FAST_LOOP_MIN_OUTPUT` (320, 512 against 258): the quick "WINs" are the outlier default run, the effects have no consistent sign across framings (`FAST_LOOP_MIN_OUTPUT` 320 is a LOSS on gzip and a NEUTRAL +6% on raw), and none was confirmed by the full bench; keep 16 and 258. The margins matter for correctness (D-29), not for speed at these values.
- `CRC32_SLICES` (4, 16 against 8): 4 is 11% slower on gzip (a real effect: a shorter fold does less independent work per step), 16 is -1% (no gain); keep 8.
- `ONESHOT_INITIAL_RATIO` (2, 8 against 4): every difference is inside or below drift, with the streaming keys moving as much as the one-shot ones; the allocation counters of P3-03 do not favour either; keep 4.

Not swept, because they are safety bounds and not performance tunables: `DEFAULT_MAX_OUTPUT`, `DEFAULT_MAX_GZIP_FIELD`, `DEFAULT_MAX_GZIP_HEADERS`, `ONESHOT_GROWTH_FLOOR` and `ONESHOT_MAX_PREALLOC`.

### M4 acceptance (P4-04)

Commands run on the M4 head: `make ci` (green), `make heavy` (proptest-10k for chunking and fastslow, bomb, conformance-full, fastslow-full: all green; fastslow-full 177.0 s), and `make bench BENCH_ID=P4-04 BENCH_OUT=docs/perf/P4-04.json` (`docs/perf/P4-04.json` is committed). Wall times: `make ci` 4m04s, `make heavy` 6m41s; the full bench took about 8 minutes.

Silesia level 6, median MiB/s (C zlib is the same-run one-shot median, the denominator for both tables):

One-shot (`decompress_with`):

| framing | baseline | M3 (P3-04) | M4 | gain vs baseline | C zlib (M4 run) | cleanflate / C zlib |
|---|---:|---:|---:|---:|---:|---:|
| raw | 62.3 | 190.6 | 189.8 | +204.7% | 284.4 | 66.7% |
| zlib | 61.3 | 177.5 | 174.8 | +185.4% | 255.9 | 68.3% |
| gzip | 51.8 | 135.5 | 176.2 | +240.1% | 255.9 | 68.9% |

Streaming (`Decompressor`, 64 KiB windows):

| framing | baseline | M3 (P3-04) | M4 | gain vs baseline | C zlib (M4 run) | cleanflate / C zlib |
|---|---:|---:|---:|---:|---:|---:|
| raw | 65.0 | 212.4 | 211.9 | +226.1% | 284.4 | 74.5% |
| zlib | 63.7 | 196.8 | 195.7 | +207.1% | 255.9 | 76.5% |
| gzip | 55.7 | 141.7 | 189.5 | +240.1% | 255.9 | 74.1% |

Framing overhead relative to raw (raw / framing - 1 of the medians, so 0% means the checksum is free):

| framing | mode | baseline | M4 |
|---|---|---:|---:|
| zlib vs raw | one-shot | 1.7% | 8.5% |
| gzip vs raw | one-shot | 20.2% | 7.7% |
| zlib vs raw | streaming | 1.9% | 8.3% |
| gzip vs raw | streaming | 16.6% | 11.8% |

The gzip overhead fell (the CRC-32 of P4-01), and the zlib overhead is now larger in relative terms only because everything else got about three times faster while the Adler-32 (6.15% of samples in the zlib profile, P4-02) did not change.

Against the SPEC section 4.1 targets (recorded, not blocking): **primary target, one-shot at least 70% of C zlib for raw, zlib and gzip: NOT MET** (66.7%, 68.3% and 68.9%; zlib and gzip are within about two points). **Streaming at least 60%: MET** for all three (74.5%, 76.5% and 74.1%). **Stretch, one-shot at least 90%: NOT MET.** The noise of this machine (single-pair aggregate noise 6.1%, drift of about 10% between runs) is larger than the remaining gap to 70%, so a further gain would have to be demonstrated on a quiet machine with a pinned governor.

`make bench-compare` against `baseline.json`: no LOSS and no per-file REGRESSION line. Against `P3-04.json`: 15 WIN (the gzip keys, from P4-01), and 9 LOSS on raw and zlib aggregates whose code did not change since M3 (for example canterbury raw level 1 one-shot -5.1%, silesia raw level 9 one-shot -2.7%), plus 74 per-file REGRESSION lines (31 Canterbury, 43 Silesia). The verdict on those keys is run-to-run drift (single-pair noise file; the same swings appear on keys of code that did not change), recorded as findings; the comparison that matters for the "no per-file regression" rule, against the phase-2 baseline, is clean.

Tunables: every tunable's doc comment in `config.rs` cites its sweep (`docs/perf/P4-03/`, all committed) and the outcome (kept, with the measurement), and the safety bounds say that they were not swept. The CRC-32 derivation is in DESIGN D-32.

### Final (P5-02)

Final full bench of the phase-2 code: `make bench BENCH_ID=P5-02 BENCH_OUT=docs/perf/P5-02.json` (about 6.6 minutes), compared with `docs/perf/baseline.json`.

**Targets (SPEC section 4.1), Silesia aggregate, level 6:**

| target | goal | result | verdict |
|---|---|---:|---|
| primary, raw one-shot | >= 70% of C zlib | 66.9% | NOT MET |
| primary, zlib one-shot | >= 70% | 68.8% | NOT MET |
| primary, gzip one-shot | >= 70% | 68.8% | NOT MET |
| streaming, raw (64 KiB in and out) | >= 60% | 74.9% | MET |
| streaming, zlib | >= 60% | 77.1% | MET |
| streaming, gzip | >= 60% | 74.2% | MET |
| stretch, raw one-shot | >= 90% | 66.9% | NOT MET |
| no per-file regression against the phase-2 baseline | no file slower than baseline by more than the noise | 0 REGRESSION and 0 LOSS lines in `make bench-compare` (36 WIN aggregates) | MET |

The primary target is missed by 3.1 points (raw), 1.2 points (zlib) and 1.2 points (gzip); the streaming target is met with 14 to 17 points to spare. Speeds have risen from the baseline of 62.3 (raw), 61.3 (zlib) and 51.8 (gzip) MiB/s one-shot to 190.1, 177.3 and 177.7 (about 3.0x, 2.9x and 3.4x), while C zlib on this machine stayed at 258 to 284 MiB/s.

**Per-milestone gains** (Silesia level 6, median MiB/s, from the committed full-bench JSONs; C zlib in the last column is the final run's one-shot median):

| key | baseline | M1 (P1-05) | M2 (P2-04) | M3 (P3-04) | M4 (P4-04) | final (P5-02) | C zlib | final ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| raw one-shot | 62.3 | 98.9 | 153.8 | 190.6 | 189.8 | 190.1 | 284.2 | 66.9% |
| zlib one-shot | 61.3 | 94.5 | 147.9 | 177.5 | 174.8 | 177.3 | 257.8 | 68.8% |
| gzip one-shot | 51.8 | 75.3 | 105.8 | 135.5 | 176.2 | 177.7 | 258.1 | 68.8% |
| raw streaming | 65.0 | 107.8 | 176.8 | 212.4 | 211.9 | 212.8 | 284.2 | 74.9% |
| zlib streaming | 63.7 | 102.4 | 166.8 | 196.8 | 195.7 | 198.8 | 257.8 | 77.1% |
| gzip streaming | 55.7 | 82.0 | 124.9 | 141.7 | 189.5 | 191.6 | 258.1 | 74.2% |

What each milestone bought (all from the PT1 before/after pairs in the sections above): bulk refill (D-27) about +50% and the table-decode share from 54% to 100% (D-28, NEUTRAL by itself, kept as the prerequisite of the loop); the fast loop (D-29) about +60%; the output as history (D-30) about +20%; hint-sized one-shot output (D-31) about +10% for gzip one-shot; the multi-byte CRC-32 (D-32) about +28% for gzip.

**Hypotheses (SPEC 4.4), final verdicts:**

| hyp | verdict | outcome |
|---|---|---|
| H1 table mostly bypassed | CONFIRMED, fixed | serial decodes 46%/48% -> 0; bytes per refill 1.08 -> 6.21 (D-27, D-28) |
| H2 per-symbol dispatch | CONFIRMED, fixed | dispatches per output byte 0.644 -> 0.0032, `Inflater::step` self 35.9% -> 0.5%, State clone 7.5% -> 0.15% (D-29); P2-03 gated out afterwards |
| H3 output written twice | CONFIRMED, fixed | bytes copied into the ring per output byte 1.0 -> 0.019 one-shot and 0.50 streaming (D-30) |
| H4 overlapping matches | DISCARDED | 0.48% of matches; the doubling attempt was measured twice and reverted (P3-02) |
| H5 table build cost | DISCARDED | about 1.7% of samples, no fixed blocks at level 6; P1-03 gated out |
| H6 wrapper costs | PARTIAL, mostly fixed | CRC-32 fixed (D-32: gzip overhead over raw 20.2% -> 7.7% one-shot at M4) and the extra zero-filled buffer removed (D-31); Adler-32 (6.15% of samples) left as is (P4-02) |

**Negative results and reverted attempts** (none needed a bug fixture; the correctness tests written for them stay):

- P1-03 table reuse (gated out): tables are 1.7% of samples. No code, no fixture.
- P1-04 length/distance payload in table entries (gated out): 1.4% of samples. No code, no fixture.
- P2-03 copyable state (gated out): State clone 0.15% after the fast loop. No code, no fixture.
- P3-02 pattern doubling (implemented, measured twice, reverted): not a reliable WIN. Kept: `tests/history.rs::pattern_doubling_matches_model_exhaustive` and `pattern_doubling_straddling_window`, and `docs/perf/P3-02-before.json`, `P3-02.json`, `P3-02-rerun.json`.
- P4-02 Adler-32 (gated out): at most +6.6% attainable, below twice the quick-bench noise. No code, no fixture.
- P4-03 tunables: no default changed; the sweep JSONs are in `docs/perf/P4-03/`.
- Repro fixtures in `testdata/regressions/` (all passing, both decode paths): `rfc1951/fixed-heavy-match-11k`, `rfc1950/fixed-heavy-match-11k`, `rfc1952/two-members`, `rfc1951/dynamic-l9-64k`. No differential, proptest or fuzz run found a wrong output, error or `consumed` in phase 2.

**Per-file regression check against the baseline:** `make bench-compare A=docs/perf/baseline.json B=docs/perf/P5-02.json` prints no REGRESSION line, no LOSS aggregate and 36 WIN aggregates (all 12 framing and mode combinations at each of levels 1, 6 and 9, for both corpora).

**Levels 1 and 9** (Silesia, median MiB/s; ratio = cleanflate / C zlib):

| level | key | baseline | final | baseline ratio | final ratio |
|---|---|---:|---:|---:|---:|
| 1 | raw one-shot | 51.0 | 162.2 | 20.0% | 61.6% |
| 1 | raw streaming | 53.7 | 176.3 | 21.0% | 66.9% |
| 1 | zlib one-shot | 51.0 | 155.1 | 21.3% | 64.6% |
| 1 | zlib streaming | 52.6 | 166.9 | 22.0% | 69.5% |
| 1 | gzip one-shot | 45.2 | 151.8 | 18.5% | 62.6% |
| 1 | gzip streaming | 48.4 | 162.9 | 19.8% | 67.2% |
| 9 | raw one-shot | 62.6 | 194.0 | 22.7% | 66.9% |
| 9 | raw streaming | 65.9 | 219.5 | 23.9% | 75.7% |
| 9 | zlib one-shot | 60.7 | 182.8 | 24.3% | 69.8% |
| 9 | zlib streaming | 65.4 | 200.9 | 26.2% | 76.8% |
| 9 | gzip one-shot | 52.9 | 182.4 | 20.5% | 69.4% |
| 9 | gzip streaming | 57.2 | 193.1 | 22.1% | 73.5% |

**Conditions.** AMD Ryzen 5 3400G with Radeon Vega Graphics, 8 logical CPUs, Linux 7.1.5-76070105-generic x86_64, rustc 1.89.0 (29483883e 2025-08-04), C zlib 1.3.2, governor schedutil (not pinned), load average 2.66 2.60 3.05 at the start, run at `d1778c1a4f72` (the tree was marked dirty only by the pipeline's uncommitted `docs/PROGRESS.md`). Run-to-run drift on this machine is about 10% (see the M0, P3-03 and P4-03 sections), so single-run differences below that are not evidence. A human should repeat the final bench on a quiet machine with a fixed governor (SPEC section 8) before quoting the ratios.

**Shortfall and the next most promising step.** The one-shot ratios are 1.2 to 3.1 points short of 70%. The final cleanflate-only profile of Silesia level 6 raw (`make profile`, 172 MiB/s under `perf`) has `fast_loop::run` at 46.9% self, `Huffman::decode_table` (the lookup, not inlined into the loop) at 27.4% self (25.6% inclusive as `decode_table_method`), `Window::copy_match` at 8.6% self (15.8% inclusive, of which `copy_within` 9.0%), `refill_bulk` 6.1% inclusive, and `peek` plus `consume` about 13% inclusive. The evidence points to three steps, in this order: (1) inline the two-level lookup into the fast loop and keep the bit buffer and count in locals as registers (removes a call and the repeated `peek`/`consume` bounds handling per symbol: up to about 25% of samples are in that path); (2) a short-match path for the 71% of matches of length 3 to 8 (the `copy_within` call and its bounds checks cost 9.0% for copies that are a few bytes long); (3) carry the length and distance base and extra-bit counts in the table entries (P1-04's gate was closed at 1.4% only because the state machine dominated then; inside the fast loop that handling is now part of the 46.9%). The Adler-32 (6.15% of samples for zlib) would follow. Each of these is a few percent, so proving it needs the quiet machine and pinned governor named above.

**Result:** phase 2: cleanflate / C zlib (Silesia, level 6, one-shot) = 66.9% raw, 68.8% zlib, 68.8% gzip (target 70%: NOT MET); streaming 64 KiB = 74.9% raw (target 60%: MET).

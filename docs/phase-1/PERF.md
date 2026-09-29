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

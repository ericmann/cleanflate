# cleanflate phase 3 profile

This is the M1 profile of the phase-2 decoder (git `78a6eb7`, before any phase-3 optimisation). Every number comes from cleanflate-only processes (`profile_decode`, `perf stat`, `perf record`) and from `oracle bench --stats` counters; C zlib is never profiled. The report confirms, adjusts or discards the four SPEC section 1 patterns by evidence and gives each later optimisation task its hotspot and likely gate outcome.

## How these were taken

- Commands: `make profile-required TAG=before` (keys `silesia/rfc1951/l1/oneshot`, `silesia/rfc1951/l1/streaming`, `canterbury/rfc1952/l1/oneshot`: `profile`, `profile-stat`, saved flamegraph), then `make profile PROFILE_KEY=...` for `silesia/rfc1951/l6/oneshot`, `silesia/rfc1951/l6/streaming`, `silesia/rfc1950/l6/streaming`, `silesia/rfc1952/l6/streaming`, `canterbury/rfc1951/l1/oneshot` and `canterbury/rfc1952/l1/streaming`; `make profile-stat` for the three required keys and `silesia/rfc1951/l6/oneshot`; `make bench-stats BENCH_ID=P1-02 BENCH_OUT=docs/perf/phase3/P1-02-stats.json STATS_CORPUS=silesia,canterbury STATS_LEVELS=1,6`.
- Iterations: 3 passes over Silesia (606.4 MiB of output), 300 passes over Canterbury (804.2 MiB). `perf record --call-graph dwarf -F 499`, user-space cycles. Machine as in `docs/perf/PHASE3.md` (Ryzen 5 3400G, 8 logical cores, `schedutil`), release build with `debug = "line-tables-only"` plus `CARGO_PROFILE_RELEASE_DEBUG=true` for the profile build.
- `perf_event_paranoid` is 2, so sampling sees user space only. Kernel page-fault time is invisible to the samples; it shows up as `sys` time and page faults in `perf stat`, which are reported next to every key that has one.
- Self time is the exclusive share of samples of a symbol; a symbol that perf splits into several address ranges is summed. `unresolved addresses` are samples in a system library without symbols; the guard denies `perf report --sort dso`, so their owner is not confirmed (the size and copy routines of the C library are the likely candidates).
- `crc32` (the free function) in the raw and zlib keys is the harness verifying the first pass's output against the index; it is not decoder work and shrinks with more iterations. `Crc32::update` and `ZlibDecoder::step` are decoder work.
- The harness's streaming mode allocates a fresh zeroed output buffer per file, whereas the bench's streaming mode reuses one buffer across repetitions. The harness therefore cannot show the bench's one-shot versus streaming gap directly; see pattern 1.
- Flamegraphs (`docs/perf/flamegraphs/*-before.svg`) are made with `--no-inline` so frames keep their crate paths, and were checked by `tools/perf/check-flamegraph.sh`.

## Before (P1-02)

### silesia/rfc1951/l1/oneshot

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 41.79 |
| `cleanflate::huffman::Huffman::decode_table` | 28.53 |
| `cleanflate::window::Window::copy_match` | 12.75 |
| `cleanflate::checksum::crc32` | 3.10 |
| `cleanflate::huffman::Huffman::from_lengths` | 1.52 |
| unresolved addresses (system library code, summed) | 7.07 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 41.8%; `decode_table (inlined)` 28.6%; `cleanflate::huffman::Huffman::decode_table` 28.5%; `decode_table_method (inlined)` 26.7%; `copy_match (inlined)` 21.5%; `copy_match_slices (inlined)` 13.0%; `cleanflate::window::Window::copy_match` 12.8%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 10.5%; `copy<u8> (inlined)` 9.8%; `peek (inlined)` 7.0%.

`perf stat`: task-clock 3810 ms (user 3.38 s, sys 0.44 s), page faults 142600 = 235.2 per MiB of output, instructions 36.36 G = 60.0 M per MiB, cycles 12.90 G, branch misses 1.48% of 5.88 G branches.

### silesia/rfc1951/l1/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 42.89 |
| `cleanflate::huffman::Huffman::decode_table` | 27.58 |
| `cleanflate::window::Window::copy_match` | 12.97 |
| `cleanflate::checksum::crc32` | 2.94 |
| `cleanflate::huffman::Huffman::from_lengths` | 1.97 |
| `cleanflate::inflate::Inflater::step` | 1.00 |
| unresolved addresses (system library code, summed) | 6.10 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 42.9%; `cleanflate::huffman::Huffman::decode_table` 27.6%; `decode_table (inlined)` 27.6%; `decode_table_method (inlined)` 25.8%; `copy_match (inlined)` 21.7%; `copy_match_slices (inlined)` 13.1%; `cleanflate::window::Window::copy_match` 13.0%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 9.5%; `copy<u8> (inlined)` 8.8%; `consume (inlined)` 6.6%.

`perf stat`: task-clock 4140 ms (user 3.73 s, sys 0.42 s), page faults 107439 = 177.2 per MiB of output, instructions 36.49 G = 60.2 M per MiB, cycles 13.75 G, branch misses 1.58% of 5.89 G branches.

### canterbury/rfc1952/l1/oneshot

300 iterations, 804.2 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 36.66 |
| `cleanflate::huffman::Huffman::decode_table` | 24.09 |
| `cleanflate::window::Window::copy_match` | 13.00 |
| `cleanflate::checksum::Crc32::update` | 9.21 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.20 |
| `cleanflate::inflate::Inflater::step` | 0.57 |
| unresolved addresses (system library code, summed) | 8.99 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 36.7%; `cleanflate::huffman::Huffman::decode_table` 24.1%; `decode_table (inlined)` 24.1%; `copy_match (inlined)` 23.8%; `decode_table_method (inlined)` 22.7%; `copy_match_slices (inlined)` 15.6%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 13.3%; `cleanflate::window::Window::copy_match` 13.0%; `copy<u8> (inlined)` 12.5%; `cleanflate::checksum::Crc32::update` 9.2%.

`perf stat`: task-clock 4391 ms (user 4.39 s, sys 0.01 s), page faults 690 = 0.9 per MiB of output, instructions 49.01 G = 60.9 M per MiB, cycles 16.89 G, branch misses 1.26% of 7.68 G branches.

### silesia/rfc1951/l6/oneshot

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 44.10 |
| `cleanflate::huffman::Huffman::decode_table` | 27.82 |
| `cleanflate::window::Window::copy_match` | 10.52 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.79 |
| `cleanflate::checksum::crc32` | 3.61 |
| `cleanflate::inflate::Inflater::step` | 0.69 |
| unresolved addresses (system library code, summed) | 5.38 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 44.1%; `decode_table (inlined)` 27.9%; `cleanflate::huffman::Huffman::decode_table` 27.8%; `decode_table_method (inlined)` 26.6%; `copy_match (inlined)` 17.2%; `copy_match_slices (inlined)` 10.6%; `cleanflate::window::Window::copy_match` 10.5%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 8.6%; `copy<u8> (inlined)` 7.8%; `consume (inlined)` 6.6%.

`perf stat`: task-clock 3485 ms (user 2.97 s, sys 0.52 s), page faults 146522 = 241.6 per MiB of output, instructions 29.83 G = 49.2 M per MiB, cycles 10.84 G, branch misses 1.62% of 4.87 G branches.

### silesia/rfc1951/l6/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 43.18 |
| `cleanflate::huffman::Huffman::decode_table` | 28.82 |
| `cleanflate::window::Window::copy_match` | 12.01 |
| `cleanflate::checksum::crc32` | 3.61 |
| `cleanflate::huffman::Huffman::from_lengths` | 2.70 |
| `cleanflate::inflate::Inflater::step` | 0.82 |
| unresolved addresses (system library code, summed) | 3.40 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 43.2%; `decode_table (inlined)` 28.9%; `cleanflate::huffman::Huffman::decode_table` 28.8%; `decode_table_method (inlined)` 27.4%; `copy_match (inlined)` 19.3%; `copy_match_slices (inlined)` 12.3%; `cleanflate::window::Window::copy_match` 12.0%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 9.1%; `copy<u8> (inlined)` 8.4%; `consume (inlined)` 6.5%.

### silesia/rfc1950/l6/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 39.50 |
| `cleanflate::huffman::Huffman::decode_table` | 25.75 |
| `cleanflate::window::Window::copy_match` | 11.70 |
| `cleanflate::zlib::ZlibDecoder::step` | 5.12 |
| `cleanflate::checksum::crc32` | 3.17 |
| `cleanflate::huffman::Huffman::from_lengths` | 2.64 |
| `cleanflate::inflate::Inflater::step` | 0.92 |
| unresolved addresses (system library code, summed) | 5.96 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 39.5%; `cleanflate::huffman::Huffman::decode_table` 25.8%; `decode_table (inlined)` 25.8%; `decode_table_method (inlined)` 23.7%; `copy_match (inlined)` 20.8%; `copy_match_slices (inlined)` 13.6%; `cleanflate::window::Window::copy_match` 11.7%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 9.2%; `copy<u8> (inlined)` 8.5%; `consume (inlined)` 6.1%.

### silesia/rfc1952/l6/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 39.42 |
| `cleanflate::huffman::Huffman::decode_table` | 26.44 |
| `cleanflate::window::Window::copy_match` | 9.57 |
| `cleanflate::checksum::Crc32::update` | 9.56 |
| `cleanflate::checksum::crc32` | 2.94 |
| `cleanflate::huffman::Huffman::from_lengths` | 2.16 |
| `cleanflate::inflate::Inflater::step` | 0.91 |
| unresolved addresses (system library code, summed) | 3.41 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 39.4%; `cleanflate::huffman::Huffman::decode_table` 26.4%; `decode_table (inlined)` 26.4%; `decode_table_method (inlined)` 24.2%; `copy_match (inlined)` 16.8%; `copy_match_slices (inlined)` 10.9%; `cleanflate::window::Window::copy_match` 9.6%; `cleanflate::checksum::Crc32::update` 9.6%; `update (inlined)` 9.6%; `crc32_fold_chunk (inlined)` 8.4%.

### canterbury/rfc1951/l1/oneshot

300 iterations, 804.2 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 41.24 |
| `cleanflate::huffman::Huffman::decode_table` | 26.13 |
| `cleanflate::window::Window::copy_match` | 16.11 |
| `cleanflate::huffman::Huffman::from_lengths` | 2.73 |
| `cleanflate::inflate::Inflater::step` | 0.79 |
| unresolved addresses (system library code, summed) | 9.78 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 41.2%; `cleanflate::huffman::Huffman::decode_table` 26.1%; `decode_table (inlined)` 26.1%; `copy_match (inlined)` 25.5%; `decode_table_method (inlined)` 24.5%; `cleanflate::window::Window::copy_match` 16.1%; `copy_match_slices (inlined)` 15.1%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 12.5%; `copy<u8> (inlined)` 11.3%; `consume (inlined)` 6.4%.

### canterbury/rfc1952/l1/streaming

300 iterations, 804.2 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 35.48 |
| `cleanflate::huffman::Huffman::decode_table` | 26.30 |
| `cleanflate::window::Window::copy_match` | 13.30 |
| `cleanflate::checksum::Crc32::update` | 8.21 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.58 |
| `cleanflate::inflate::Inflater::step` | 0.67 |
| unresolved addresses (system library code, summed) | 8.07 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 35.5%; `decode_table (inlined)` 26.3%; `cleanflate::huffman::Huffman::decode_table` 26.3%; `decode_table_method (inlined)` 24.6%; `copy_match (inlined)` 23.1%; `copy_match_slices (inlined)` 14.8%; `cleanflate::window::Window::copy_match` 13.3%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 11.6%; `copy<u8> (inlined)` 10.9%; `cleanflate::checksum::Crc32::update` 8.2%.


## Patterns

| # | SPEC section 1 pattern | Verdict |
|---|---|---|
| 1 | one-shot slower than streaming | ADJUSTED |
| 2 | Canterbury trails Silesia: per-stream and per-block setup cost | ADJUSTED (setup is real but small) |
| 3 | level 1 is slowest: the literal-heavy decode loop | ADJUSTED (symbol-count heavy, not literal-heavy) |
| 4 | zlib and gzip trail raw: checksum and wrapper cost | CONFIRMED |

**1. One-shot slower than streaming: ADJUSTED.** In the phase-3 start bench one-shot trails streaming at level 1 (`silesia/raw/l1`: 164.4 against 181.2 MiB/s median, one-shot / streaming = 0.907; `docs/perf/baselines/phase3-start.json`). The counters rule out growth and copying as the cause: `oneshot_resizes` is 14 over 12 Silesia files (about one per file: the ratio estimate is close), `oneshot_bytes_allocated` is 316,146,272 for 211,938,580 bytes of output (1.49x), and no `realloc`, `grow` or `copy` frame reaches the top-inclusive list (the largest allocation-related frame is `resize<u8>`/`extend_with<u8>` at 2.06% inclusive in `silesia/rfc1951/l1/oneshot`, the zero-fill of the output `Vec`). The remaining difference is fresh-page first touch: one-shot writes 235.2 page faults per MiB of output against 177.2 for the harness's streaming mode (which also allocates per file), with `sys` time 0.44 s of a 3.81 s run (11%) that the user-space samples cannot see. The bench's streaming mode reuses its buffer and pays neither, which matches the 0.57 ms per MiB gap against 0.72 ms per MiB of `sys` time in the one-shot profile. Instruction counts are equal (60.0 against 60.2 million per MiB), so the decode work is the same. The candidate cause is therefore zero-fill plus fresh-allocation faults, not growth: P2-07 removes the user-space zero-fill (2.06%) and the pre-zeroing of pages, but a `Vec` that grows still touches fresh pages, so its gain is bounded.

**2. Canterbury setup cost: ADJUSTED.** Setup is visible but small. In `canterbury/rfc1952/l1/oneshot`, `Huffman::from_lengths` is 3.20% self (3.86% inclusive; `build_table` 1.81%), `Inflater::new_with_path` about 2.3% inclusive (two call sites, 1.42% and 0.91%) and `resize<u8>` 0.74%, together roughly 7%, against about 1.5% for `from_lengths` in `silesia/rfc1951/l1/oneshot`. The counters say table building is proportional to output, not to file count: 56,058 table entries for 2,810,784 output bytes (0.0199 per byte) on Canterbury, 4,327,666 for 211,938,580 (0.0204 per byte) on Silesia, with 38 dynamic blocks for 11 files on Canterbury, no fixed blocks and `fixed_table_clones` 0 on both. The 11 Canterbury files range from 3.7 KB (`grammar.lsp`) to 1.03 MB (`kennedy.xls`), so the small ones do pay per-stream costs, but the decode loop (`fast_loop::run` 36.7% self, `decode_table` 24.1%, `copy_match` 13.0%) has the same shape as Silesia, so setup cannot explain more than a few points of the 0.5 to 0.6 ratio; the larger part is the same per-symbol cost that trails C zlib everywhere.

**3. Level 1 decode loop: ADJUSTED.** The loop dominates: `fast_loop::run`, `Huffman::decode_table` and `Window::copy_match` are 41.8% + 28.5% + 12.8% = 83.1% of self time at level 1 one-shot. But level 1 is not literal-heavy. Literals are 22,545,429 of 48,346,529 symbols (46.6%) at level 1 and 27,928,456 of 45,899,328 (60.8%) at level 6 on Silesia (Canterbury: 29.5% and 42.0%). Level 1 has more symbols (48.3 million against 45.9 million), more compressed input (`refill_bytes` 77.0 against 68.1 million) and shorter matches (mean 7.3 bytes against 10.2), and it is slower per output byte (about 160 against 189 MiB/s in the harness). The cost is per symbol, and `Huffman::decode_table` is a separate, non-inlined function taking 22 to 29% of samples in every key; the fast loop itself carries the inlined refill (`refill_bulk` 5.8% inclusive), `peek` (7.0%) and `consume` (6.6%). The machine executes about 57 instructions and 20 cycles per output byte (IPC 2.8, branch misses 1.5% of branches), so the loop is instruction-bound rather than misprediction-bound. This points at P4-03 (inline the lookup, prove bounds) first, then P4-04 (multi-literal entries have 46.6% of symbols to work with at level 1) and P4-02.

**4. zlib and gzip against raw: CONFIRMED.** For gzip the CRC-32 is a first-order cost: `Crc32::update` is 9.6% self in `silesia/rfc1952/l6/streaming` and 9.2% in `canterbury/rfc1952/l1/oneshot`. For zlib the wrapper `ZlibDecoder::step` is 5.1% self in `silesia/rfc1950/l6/streaming`, which contains Adler-32 (inlined) and the wrapper's own bookkeeping; they cannot be separated at this resolution, and the raw key's `Inflater::step` (0.7 to 0.9%) bounds the per-call overhead of the decoder underneath. The phase-3 start bench shows raw to framed gaps of up to 12.6% (`silesia/zlib/l9/oneshot`, T4), the same order as these shares.

## Hotspots by task

Gate threshold is max(2%, 2 x the key's noise in `docs/perf/baselines/phase3-noise-quick.json`). PLAN order does not change; the gates decide.

| Task | Key | Frame(s) and current share | Gate | Likely outcome | Rank by share |
|---|---|---|---:|---|---:|
| P4-03 fast-loop bounds elision and inlined lookup | `silesia/rfc1951/l1/oneshot` | `fast_loop::run` 41.8% + `Huffman::decode_table` 28.5% = 70.3% self | 2% | above | 1 |
| P4-04 multi-literal entries | `silesia/rfc1951/l1/oneshot` | literals are 46.6% of symbols inside that 70.3%; decode of literal symbols only | 2% | above (gain bounded by the literal share) | 2 |
| P2-03 CRC-32 slicing-by-16 | `silesia/rfc1952/l6/streaming` | `Crc32::update` 9.6% self | 5.14% (noise 2.57%) | above | 3 |
| P4-02 branch-light bulk refill | `silesia/rfc1951/l1/oneshot` | `refill_bulk` 5.8% inclusive (`peek` 7.0%, `consume` 6.6% also inclusive) | 2% (noise 0.91%) | above | 4 |
| P2-02 Adler-32 unrolled batch | `silesia/rfc1950/l6/streaming` | `ZlibDecoder::step` 5.1% self (Adler-32 plus wrapper) | 7.02% (noise 3.51%) | below | 5 |
| P3-01 per-block table reuse | `canterbury/rfc1952/l1/oneshot` | `from_lengths` 3.9% inclusive (`build_table` 1.8%); `fixed_table_clones` 0 | 4.84% (noise 2.42%) | below | 6 |
| P3-02 per-stream setup | `canterbury/rfc1952/l1/oneshot` | `new_with_path` about 2.3% inclusive + `resize<u8>` 0.74% = about 3.0% | 4.84% | below | 7 |
| P2-07 one-shot appends to a `Vec` | `silesia/rfc1951/l1/oneshot` | `resize<u8>`/`extend_with<u8>` 2.06% inclusive user-space; 11% `sys` (page faults, only partly avoidable) | 2% (noise 0.91%) | at the gate; the wall gain is measured, not assumed | 8 |
| P2-04 wrapper per-call overhead | `silesia/rfc1950/l6/streaming` | wrapper share is at most `ZlibDecoder::step` 5.1% minus Adler-32; `Inflater::step` 0.9% | 7.02% | below | 9 |
| P2-08 one-shot size-hint revisit | `silesia/rfc1951/l1/oneshot` | growth frames absent from the profile; 14 resizes for 12 files | 2% | below | 10 |

Where the likely outcome is below the gate the task should record a negative result unless its own measurement disagrees, per PT2.

## After (P5-01)

Filled by P5-01.

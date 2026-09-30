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

Taken on the final code (git `f5f4c85`, after P4-06) with the same commands, iterations, flags and machine as the before profiles: `make profile-required TAG=after` for the three required keys (flamegraphs `docs/perf/flamegraphs/*-after.svg`, checked by `tools/perf/check-flamegraph.sh`), then `make profile` for the other six keys and `make profile-stat` for `silesia/rfc1951/l6/oneshot`. The same caveats apply: user-space samples only, the raw and zlib keys' `crc32` is the harness's output check, unresolved addresses are an unnamed system library. After P4-03 the fast loop, the table lookups and the match copy are a single function, `fast_loop::run`, so the `Huffman::decode_table` and `Window::copy_match` rows of the before tables no longer exist; `Inflater::step_into` is the renamed state machine.

### silesia/rfc1951/l1/oneshot

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 79.04 |
| `cleanflate::checksum::crc32` | 3.83 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.02 |
| `cleanflate::inflate::Inflater::step_into` | 0.93 |
| unresolved addresses (system library code, summed) | 8.29 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 79.0%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 21.4%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 20.2%; `copy_within_back (inlined)` 15.6%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 12.3%; `copy<u8> (inlined)` 9.4%; `refill_bulk (inlined)` 8.5%; `consume (inlined)` 3.9%; `from_lengths (inlined)` 3.8%; `from_lengths_with_bits (inlined)` 3.8%.

`perf stat`: task-clock 3361 ms (user 2.81 s, sys 0.56 s), page faults 142609 = 235.2 per MiB of output, instructions 25.77 G = 42.5 M per MiB, cycles 10.18 G, branch misses 2.13% of 3.87 G branches.

### silesia/rfc1951/l1/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 78.28 |
| `cleanflate::checksum::crc32` | 3.87 |
| `cleanflate::huffman::Huffman::from_lengths` | 2.62 |
| `cleanflate::inflate::Inflater::step_into` | 1.73 |
| unresolved addresses (system library code, summed) | 7.87 |

Top inclusive frames (`top-children.txt`): `profile_decode::stream_through` 96.1%; `cleanflate::fast_loop::run` 78.3%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 25.9%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 24.9%; `copy_within_back (inlined)` 16.3%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 12.8%; `copy<u8> (inlined)` 10.5%; `refill_bulk (inlined)` 6.9%; `consume (inlined)` 3.9%; `cleanflate::checksum::crc32` 3.9%.

`perf stat`: task-clock 3496 ms (user 3.00 s, sys 0.50 s), page faults 107442 = 177.2 per MiB of output, instructions 26.15 G = 43.1 M per MiB, cycles 10.61 G, branch misses 2.28% of 3.92 G branches.

### canterbury/rfc1952/l1/oneshot

300 iterations, 804.2 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 70.20 |
| `cleanflate::checksum::Crc32::update` | 10.61 |
| `cleanflate::huffman::Huffman::from_lengths` | 4.78 |
| `cleanflate::inflate::Inflater::step_into` | 0.63 |
| unresolved addresses (system library code, summed) | 8.86 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 70.2%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 23.8%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 22.9%; `copy_within_back (inlined)` 17.2%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 13.4%; `copy<u8> (inlined)` 11.2%; `cleanflate::checksum::Crc32::update` 10.6%; `crc32_update_sliced<8> (inlined)` 10.6%; `update (inlined)` 10.6%; `crc32_fold_chunk<8> (inlined)` 9.7%.

`perf stat`: task-clock 4025 ms (user 3.96 s, sys 0.07 s), page faults 696 = 0.9 per MiB of output, instructions 35.98 G = 44.7 M per MiB, cycles 14.17 G, branch misses 1.81% of 5.24 G branches.

### silesia/rfc1951/l6/oneshot

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 78.56 |
| `cleanflate::checksum::crc32` | 4.59 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.64 |
| unresolved addresses (system library code, summed) | 7.54 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 78.6%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 19.5%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 18.6%; `copy_within_back (inlined)` 14.3%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 11.9%; `copy<u8> (inlined)` 10.2%; `refill_bulk (inlined)` 7.8%; `consume (inlined)` 4.9%; `profile_decode::main` 4.6%; `cleanflate::checksum::crc32` 4.6%.

`perf stat`: task-clock 2689 ms (user 2.23 s, sys 0.47 s), page faults 146531 = 241.7 per MiB of output, instructions 21.20 G = 35.0 M per MiB, cycles 8.61 G, branch misses 2.34% of 3.20 G branches.

### silesia/rfc1951/l6/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 77.07 |
| `cleanflate::checksum::crc32` | 4.38 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.15 |
| `cleanflate::inflate::Inflater::step_into` | 1.26 |
| unresolved addresses (system library code, summed) | 6.59 |

Top inclusive frames (`top-children.txt`): `profile_decode::stream_through` 95.5%; `cleanflate::fast_loop::run` 77.1%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 22.5%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 21.7%; `copy_within_back (inlined)` 14.2%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 10.8%; `copy<u8> (inlined)` 10.1%; `refill_bulk (inlined)` 7.3%; `consume (inlined)` 5.6%; `crc32 (inlined)` 4.5%.

### silesia/rfc1950/l6/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 72.52 |
| `cleanflate::zlib::ZlibDecoder::step_into` | 6.75 |
| `cleanflate::checksum::crc32` | 4.04 |
| `cleanflate::huffman::Huffman::from_lengths` | 3.62 |
| `cleanflate::inflate::Inflater::step_into` | 1.43 |
| `cleanflate::inflate::Inflater::peek_symbol` | 0.57 |
| unresolved addresses (system library code, summed) | 5.37 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 72.5%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 19.5%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 18.6%; `copy_within_back (inlined)` 11.5%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 9.0%; `copy<u8> (inlined)` 8.0%; `refill_bulk (inlined)` 7.0%; `cleanflate::zlib::ZlibDecoder::step_into` 6.8%; `adler_fold (inlined)` 6.7%; `update (inlined)` 6.7%.

### silesia/rfc1952/l6/streaming

3 iterations, 606.4 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 68.13 |
| `cleanflate::checksum::Crc32::update` | 12.42 |
| `cleanflate::checksum::crc32` | 3.95 |
| `cleanflate::huffman::Huffman::from_lengths` | 2.31 |
| `cleanflate::inflate::Inflater::step_into` | 1.30 |
| unresolved addresses (system library code, summed) | 4.36 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 68.1%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 18.7%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 17.9%; `crc32_update_sliced<8> (inlined)` 12.5%; `update (inlined)` 12.5%; `cleanflate::checksum::Crc32::update` 12.4%; `copy_within_back (inlined)` 12.1%; `crc32_fold_chunk<8> (inlined)` 11.3%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 9.5%; `copy<u8> (inlined)` 8.3%.

### canterbury/rfc1951/l1/oneshot

300 iterations, 804.2 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 79.74 |
| `cleanflate::huffman::Huffman::from_lengths` | 4.40 |
| `cleanflate::inflate::Inflater::step_into` | 0.84 |
| unresolved addresses (system library code, summed) | 10.86 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 79.7%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 26.4%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 24.6%; `copy_within_back (inlined)` 19.6%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 15.0%; `copy<u8> (inlined)` 12.3%; `refill_bulk (inlined)` 7.1%; `from_lengths (inlined)` 4.8%; `from_lengths_with_bits (inlined)` 4.8%; `consume (inlined)` 4.6%.

### canterbury/rfc1952/l1/streaming

300 iterations, 804.2 MiB of output.

| self-time symbol | % of samples |
|---|---:|
| `cleanflate::fast_loop::run` | 69.70 |
| `cleanflate::checksum::Crc32::update` | 10.56 |
| `cleanflate::huffman::Huffman::from_lengths` | 4.02 |
| `cleanflate::inflate::Inflater::step_into` | 0.62 |
| unresolved addresses (system library code, summed) | 10.35 |

Top inclusive frames (`top-children.txt`): `cleanflate::fast_loop::run` 69.7%; `copy_match_sink<cleanflate::sink::SliceSink> (inlined)` 24.8%; `copy_match_slices<cleanflate::sink::SliceSink> (inlined)` 23.4%; `copy_within_back (inlined)` 16.3%; `copy_within<u8, core::ops::range::Range<usize>> (inlined)` 13.2%; `copy<u8> (inlined)` 11.5%; `cleanflate::checksum::Crc32::update` 10.6%; `crc32_update_sliced<8> (inlined)` 10.6%; `update (inlined)` 10.6%; `crc32_fold_chunk<8> (inlined)` 9.9%.


### Hotspots by task: before and after

Shares of samples (self unless marked inclusive), same keys as the before table. A share can rise while the time falls, because the loop got faster and the fixed costs did not (total task-clock for `silesia/rfc1951/l1/oneshot` 3,810 to 3,361 ms, instructions 36.4 G to 25.8 G, both measured with the same 3 passes).

| Task | Key | Frame | Before | After | Outcome of the task |
|---|---|---|---:|---:|---|
| P4-03 lookup inlined on table views | `silesia/rfc1951/l1/oneshot` | `fast_loop::run` + `Huffman::decode_table` + `Window::copy_match` | 83.07% | 79.04% (`fast_loop::run`, all inlined) | kept: +18% to +31% on every level-1 and level-6 key |
| P4-04 multi-literal entries | `silesia/rfc1951/l1/oneshot` | literal branch of the loop (46.6% of symbols) | - | - | reverted: -5.6% to -18% |
| P4-02 word-read refill | `silesia/rfc1951/l1/oneshot` | `refill_bulk` inclusive | 6.10% | 8.54% | already the technique (D-27); no change kept |
| P2-03 CRC-32 slicing-by-16 | `silesia/rfc1952/l6/streaming` | `Crc32::update` | 9.56% (8.31% fresh) | 12.42% | measured +13% to +18% on the microbench; reverted then (at most 1.2% of the decode); the share has since grown |
| P2-02 Adler-32 batch | `silesia/rfc1950/l6/streaming` | `adler_fold` inclusive | 6.34% | 6.70% | gated out (gate 7.02%); still just below |
| P2-04 wrapper overhead | `silesia/rfc1950/l6/streaming` | wrapper excess over `adler_fold` | 0.05% | 0.05% (6.75% against 6.70%) | gated out |
| P3-01 per-block table reuse | `canterbury/rfc1952/l1/oneshot` | `from_lengths` inclusive | 2.87% | 5.64% | gated out (gate 4.84%); now above it |
| P3-02 per-stream setup | `canterbury/rfc1952/l1/oneshot` | `new_with_path` + `Window::new` + `resize<u8>` | about 3.0% | 1.93% + 1.03% + 0.75% (overlapping) | gated out; reuse bench shows about 1% |
| P2-07 one-shot appends to a `Vec` | `silesia/rfc1951/l1/oneshot` | `resize<u8>`/`extend_with<u8>` inclusive | 2.06% | 2.83% | measured and reverted: page faults unchanged |
| P2-08 size-hint revisit | `silesia/rfc1951/l1/oneshot` | growth and realloc frames | none | none | gated out |

### Final state of the four patterns

1. **One-shot slower than streaming.** ADJUSTED to fresh-allocation zero-fill and first-touch page faults (P1-02), and left unchanged: a `Vec` sink cannot remove the faults (P2-07: page faults identical, 110,179 before and after) and the size hint has no growth cost to recover (P2-08). The one-shot and streaming bench rows stay within about 10% of each other (T2 still NOT MET).
2. **Canterbury setup cost.** ADJUSTED and mostly discounted: per-stream setup is about 1% with a reused decompressor (P3-04: reuse within 1% of fresh-object streaming on every aggregate), and per-block table building, 2.9% then, is 5.6% of the now-faster loop, the largest fixed cost left on Canterbury and the first candidate for the next phase.
3. **Level 1 is the slowest level.** ADJUSTED to a per-symbol instruction count, not literal-heaviness (P1-02), and fixed where it lived: removing the per-symbol `decode_table` call cut instructions by 29% and lifted every level-1 and level-6 row by 18% to 31% (P4-03). Multi-literal entries (P4-04) and other table widths (P4-05) did not help.
4. **zlib and gzip trail raw.** CONFIRMED (P1-02); the checksum share is now larger because the loop is faster (CRC-32 9.6% to 12.4% of samples, Adler-32 plus wrapper 6.7%), so T4 is essentially unchanged: the CRC-32 slicing-by-16 change (+13% to +18% alone) is worth about 2% of a gzip decode now and is the first candidate after the table build.

# cleanflate phase 3 performance report

## Method

Every number in this report comes from `oracle bench` (release build, `make bench` for the full tier, `make bench-quick` for the quick tier) and is written to a JSON file by the tool; nothing is hand-edited.

- **Modes and like-for-like denominators.** cleanflate is timed one-shot (`decompress_with`) and streaming (64 KiB input and output chunks into a pre-sized buffer). C zlib is timed the same two ways (`inflate` one-shot into a bounded buffer, and 64 KiB-chunk streaming). Each cleanflate aggregate's ratio divides by C zlib's aggregate of the same mode (`c_zlib_mode`). C zlib is a stopwatch only.
- **Median and best.** Each measurement takes N timed samples (7 full, 5 quick), each at least 20 ms. `median` is the throughput of the median per-decode time; `best` is the throughput of the fastest sample. Aggregates divide total bytes by the sum of per-file times.
- **Sampling.** Within one sample index the implementations run back to back (cleanflate one-shot, cleanflate streaming, C zlib one-shot, C zlib streaming), so machine drift hits all of them alike. Each is verified byte-identical to the original once before timing.
- **Environment capture.** Every JSON records CPU, cores, governor, OS, kernel, load average at start and end, rustc, the release profile and build environment, the C library version and compile flags, the `libz-sys` version and features, and git head and dirtiness.
- **Verdicts.** A change is a WIN or LOSS only beyond twice the measured noise (the relative difference of two runs of the same code); a per-file REGRESSION is a drop beyond the larger of that file's noise and its aggregate's noise. The ratio verdict applies the same rule to the like-for-like ratio (B ratio / A ratio - 1) against the ratio noise, which cancels machine drift common to cleanflate and C zlib.
- **Profiling** is cleanflate-only (`profile_decode` example); C zlib is never profiled.
- Schema-1 files (the phase-2 record) carry only one-shot C zlib rows, so their streaming ratios are marked `n/a (denominator changed)` when compared.

## Machine and conditions

From the Environment block of the `phase3-start` run:

- CPU: AMD Ryzen 5 3400G with Radeon Vega Graphics, 8 logical cores, governor `schedutil`.
- OS: Pop!_OS 24.04 LTS, kernel 7.1.5-76070105-generic; rustc 1.89.0; profile `release` (`debug = "line-tables-only"`), no build-environment overrides.
- C zlib 1.3.2, compile flags `0xa9`; `libz-sys` 1.1.29 with features `default,libc,static,stock-zlib`.
- Load average at the start of the runs: about 2 to 4.7 (a desktop machine with other processes; not idle). The working tree was dirty (the baselines being written) during these runs.
- **Noise warning.** `aggregate_max_pct` exceeds 3% in both noise files: 8.22% (full) and 8.82% (quick), with ratio noise up to 12.63% and 7.08%. The large values come from the small Canterbury files and from single aggregates such as zlib level 9. This is a manual check for this phase (see HANDOFF). The 2x-noise rule therefore rejects small effects on those keys.
- This machine measured about 5 to 10% below the phase-2 operator record for the same code (see the vs phase-2 column), so "vs phase-2 %" also contains machine drift. Ratio columns are the drift-corrected view.

## Baselines

Files: `docs/perf/baselines/phase3-start.json` and `phase3-start-rerun.json` (full), `phase3-start-quick.json` and `phase3-start-quick-rerun.json` (quick), and the noise files `phase3-noise.json`, `phase3-noise-quick.json`. All are schema 2, `stats_build: false`, and every streaming aggregate has `c_zlib_mode: "streaming"`.

### All aggregates (`make bench-md J=docs/perf/baselines/phase3-start.json`)

Bench phase3-start (full), git 6a8732c050b9 (dirty), C zlib 1.3.2, iters 7

| key | cleanflate MiB/s (median) | cleanflate MiB/s (best) | C zlib MiB/s (median, same mode) | ratio (like for like) | vs phase-2 % |
|---|---:|---:|---:|---:|---:|
| silesia/raw/l1/oneshot | 164.4 | 173.9 | 272.2 | 0.604 | -7.1 |
| silesia/raw/l1/streaming | 181.2 | 190.8 | 325.1 | 0.557 | -6.0 |
| silesia/raw/l6/oneshot | 195.6 | 204.1 | 301.6 | 0.649 | -2.0 |
| silesia/raw/l6/streaming | 215.7 | 224.8 | 353.4 | 0.610 | -5.1 |
| silesia/raw/l9/oneshot | 201.8 | 207.4 | 302.3 | 0.668 | -2.8 |
| silesia/raw/l9/streaming | 222.0 | 232.1 | 350.7 | 0.633 | -4.2 |
| silesia/zlib/l1/oneshot | 160.6 | 167.8 | 257.4 | 0.624 | -3.4 |
| silesia/zlib/l1/streaming | 174.9 | 182.8 | 300.9 | 0.581 | -4.4 |
| silesia/zlib/l6/oneshot | 181.2 | 192.9 | 272.6 | 0.665 | -5.9 |
| silesia/zlib/l6/streaming | 200.7 | 213.1 | 311.5 | 0.644 | -6.2 |
| silesia/zlib/l9/oneshot | 176.3 | 193.1 | 264.2 | 0.667 | -9.3 |
| silesia/zlib/l9/streaming | 199.5 | 213.7 | 312.8 | 0.638 | -7.5 |
| silesia/gzip/l1/oneshot | 150.1 | 162.4 | 254.3 | 0.590 | -9.9 |
| silesia/gzip/l1/streaming | 162.3 | 171.2 | 299.0 | 0.543 | -8.0 |
| silesia/gzip/l6/oneshot | 179.3 | 187.9 | 265.9 | 0.674 | -7.0 |
| silesia/gzip/l6/streaming | 191.6 | 199.7 | 320.5 | 0.598 | -6.9 |
| silesia/gzip/l9/oneshot | 184.4 | 193.1 | 278.1 | 0.663 | -5.0 |
| silesia/gzip/l9/streaming | 197.7 | 206.7 | 330.5 | 0.598 | -5.8 |
| canterbury/raw/l1/oneshot | 199.5 | 211.4 | 389.1 | 0.513 | -3.7 |
| canterbury/raw/l1/streaming | 199.4 | 211.2 | 402.3 | 0.496 | -4.4 |
| canterbury/raw/l6/oneshot | 230.0 | 243.0 | 383.4 | 0.600 | -6.6 |
| canterbury/raw/l6/streaming | 230.3 | 248.5 | 388.1 | 0.593 | -5.2 |
| canterbury/raw/l9/oneshot | 233.0 | 257.5 | 383.7 | 0.607 | -7.2 |
| canterbury/raw/l9/streaming | 232.1 | 253.7 | 398.4 | 0.583 | -8.2 |
| canterbury/zlib/l1/oneshot | 192.5 | 199.7 | 343.3 | 0.561 | -1.7 |
| canterbury/zlib/l1/streaming | 190.2 | 198.0 | 343.6 | 0.554 | -3.1 |
| canterbury/zlib/l6/oneshot | 215.0 | 221.3 | 336.1 | 0.640 | -6.9 |
| canterbury/zlib/l6/streaming | 214.6 | 222.8 | 345.8 | 0.621 | -5.7 |
| canterbury/zlib/l9/oneshot | 235.5 | 240.8 | 363.1 | 0.649 | -2.4 |
| canterbury/zlib/l9/streaming | 234.0 | 240.7 | 369.1 | 0.634 | -2.2 |
| canterbury/gzip/l1/oneshot | 180.0 | 190.6 | 347.8 | 0.517 | -5.6 |
| canterbury/gzip/l1/streaming | 177.5 | 189.0 | 356.4 | 0.498 | -5.6 |
| canterbury/gzip/l6/oneshot | 214.3 | 222.1 | 369.9 | 0.579 | -1.8 |
| canterbury/gzip/l6/streaming | 210.8 | 222.4 | 375.9 | 0.561 | -2.7 |
| canterbury/gzip/l9/oneshot | 221.1 | 231.9 | 377.2 | 0.586 | -3.2 |
| canterbury/gzip/l9/streaming | 214.6 | 228.1 | 388.7 | 0.552 | -3.7 |

### Canterbury per file, with sizes (`make bench-md J=... FILES=1 FILTER=canterbury/`)

Bench phase3-start (full), git 6a8732c050b9 (dirty), C zlib 1.3.2, iters 7

| key | cleanflate MiB/s (median) | cleanflate MiB/s (best) | C zlib MiB/s (median, same mode) | ratio (like for like) | vs phase-2 % |
|---|---:|---:|---:|---:|---:|
| canterbury/raw/l1/oneshot | 199.5 | 211.4 | 389.1 | 0.513 | -3.7 |
| canterbury/raw/l1/streaming | 199.4 | 211.2 | 402.3 | 0.496 | -4.4 |
| canterbury/raw/l6/oneshot | 230.0 | 243.0 | 383.4 | 0.600 | -6.6 |
| canterbury/raw/l6/streaming | 230.3 | 248.5 | 388.1 | 0.593 | -5.2 |
| canterbury/raw/l9/oneshot | 233.0 | 257.5 | 383.7 | 0.607 | -7.2 |
| canterbury/raw/l9/streaming | 232.1 | 253.7 | 398.4 | 0.583 | -8.2 |
| canterbury/zlib/l1/oneshot | 192.5 | 199.7 | 343.3 | 0.561 | -1.7 |
| canterbury/zlib/l1/streaming | 190.2 | 198.0 | 343.6 | 0.554 | -3.1 |
| canterbury/zlib/l6/oneshot | 215.0 | 221.3 | 336.1 | 0.640 | -6.9 |
| canterbury/zlib/l6/streaming | 214.6 | 222.8 | 345.8 | 0.621 | -5.7 |
| canterbury/zlib/l9/oneshot | 235.5 | 240.8 | 363.1 | 0.649 | -2.4 |
| canterbury/zlib/l9/streaming | 234.0 | 240.7 | 369.1 | 0.634 | -2.2 |
| canterbury/gzip/l1/oneshot | 180.0 | 190.6 | 347.8 | 0.517 | -5.6 |
| canterbury/gzip/l1/streaming | 177.5 | 189.0 | 356.4 | 0.498 | -5.6 |
| canterbury/gzip/l6/oneshot | 214.3 | 222.1 | 369.9 | 0.579 | -1.8 |
| canterbury/gzip/l6/streaming | 210.8 | 222.4 | 375.9 | 0.561 | -2.7 |
| canterbury/gzip/l9/oneshot | 221.1 | 231.9 | 377.2 | 0.586 | -3.2 |
| canterbury/gzip/l9/streaming | 214.6 | 228.1 | 388.7 | 0.552 | -3.7 |

| file key | bytes | cleanflate MiB/s | C zlib MiB/s | ratio (like for like) | vs phase-2 % |
|---|---:|---:|---:|---:|---:|
| canterbury/grammar.lsp/gzip/l1/oneshot | 3721 | 77.6 | 284.3 | 0.273 | -0.1 |
| canterbury/grammar.lsp/gzip/l1/streaming | 3721 | 70.5 | 311.9 | 0.226 | -5.7 |
| canterbury/grammar.lsp/gzip/l6/oneshot | 3721 | 76.1 | 293.4 | 0.259 | -2.3 |
| canterbury/grammar.lsp/gzip/l6/streaming | 3721 | 74.8 | 331.4 | 0.226 | -4.9 |
| canterbury/grammar.lsp/gzip/l9/oneshot | 3721 | 64.4 | 242.7 | 0.265 | -19.8 |
| canterbury/grammar.lsp/gzip/l9/streaming | 3721 | 61.5 | 275.1 | 0.223 | -22.0 |
| canterbury/grammar.lsp/raw/l1/oneshot | 3721 | 95.9 | 282.8 | 0.339 | -0.9 |
| canterbury/grammar.lsp/raw/l1/streaming | 3721 | 91.2 | 325.9 | 0.280 | -2.2 |
| canterbury/grammar.lsp/raw/l6/oneshot | 3721 | 83.3 | 247.4 | 0.337 | -15.4 |
| canterbury/grammar.lsp/raw/l6/streaming | 3721 | 78.6 | 285.0 | 0.276 | -16.6 |
| canterbury/grammar.lsp/raw/l9/oneshot | 3721 | 82.1 | 244.7 | 0.336 | -17.8 |
| canterbury/grammar.lsp/raw/l9/streaming | 3721 | 79.7 | 284.1 | 0.281 | -16.9 |
| canterbury/grammar.lsp/zlib/l1/oneshot | 3721 | 91.2 | 282.0 | 0.323 | +4.1 |
| canterbury/grammar.lsp/zlib/l1/streaming | 3721 | 91.8 | 311.1 | 0.295 | +5.6 |
| canterbury/grammar.lsp/zlib/l6/oneshot | 3721 | 96.6 | 285.6 | 0.338 | -1.4 |
| canterbury/grammar.lsp/zlib/l6/streaming | 3721 | 95.1 | 325.3 | 0.292 | +0.3 |
| canterbury/grammar.lsp/zlib/l9/oneshot | 3721 | 95.4 | 291.2 | 0.327 | -2.4 |
| canterbury/grammar.lsp/zlib/l9/streaming | 3721 | 91.3 | 330.3 | 0.276 | -3.4 |
| canterbury/xargs.1/gzip/l1/oneshot | 4227 | 71.0 | 252.9 | 0.281 | -7.3 |
| canterbury/xargs.1/gzip/l1/streaming | 4227 | 70.3 | 278.8 | 0.252 | -6.0 |
| canterbury/xargs.1/gzip/l6/oneshot | 4227 | 75.1 | 259.4 | 0.290 | -2.1 |
| canterbury/xargs.1/gzip/l6/streaming | 4227 | 73.7 | 288.5 | 0.255 | -1.1 |
| canterbury/xargs.1/gzip/l9/oneshot | 4227 | 71.7 | 258.5 | 0.278 | -6.6 |
| canterbury/xargs.1/gzip/l9/streaming | 4227 | 70.6 | 290.2 | 0.243 | -6.4 |
| canterbury/xargs.1/raw/l1/oneshot | 4227 | 90.0 | 261.5 | 0.344 | -1.5 |
| canterbury/xargs.1/raw/l1/streaming | 4227 | 87.3 | 289.2 | 0.302 | -1.6 |
| canterbury/xargs.1/raw/l6/oneshot | 4227 | 85.5 | 242.7 | 0.352 | -9.9 |
| canterbury/xargs.1/raw/l6/streaming | 4227 | 82.0 | 260.1 | 0.315 | -11.3 |
| canterbury/xargs.1/raw/l9/oneshot | 4227 | 87.5 | 260.4 | 0.336 | -5.7 |
| canterbury/xargs.1/raw/l9/streaming | 4227 | 86.3 | 289.8 | 0.298 | -4.5 |
| canterbury/xargs.1/zlib/l1/oneshot | 4227 | 76.8 | 243.1 | 0.316 | -14.1 |
| canterbury/xargs.1/zlib/l1/streaming | 4227 | 80.2 | 269.2 | 0.298 | -6.5 |
| canterbury/xargs.1/zlib/l6/oneshot | 4227 | 87.5 | 252.0 | 0.347 | -3.4 |
| canterbury/xargs.1/zlib/l6/streaming | 4227 | 86.9 | 281.0 | 0.309 | -0.1 |
| canterbury/xargs.1/zlib/l9/oneshot | 4227 | 88.3 | 252.1 | 0.350 | -6.2 |
| canterbury/xargs.1/zlib/l9/streaming | 4227 | 88.0 | 280.5 | 0.314 | -4.6 |
| canterbury/fields.c/gzip/l1/oneshot | 11150 | 105.5 | 290.6 | 0.363 | -18.4 |
| canterbury/fields.c/gzip/l1/streaming | 11150 | 107.9 | 305.9 | 0.353 | -16.4 |
| canterbury/fields.c/gzip/l6/oneshot | 11150 | 121.5 | 347.9 | 0.349 | -11.8 |
| canterbury/fields.c/gzip/l6/streaming | 11150 | 130.1 | 350.2 | 0.372 | -5.5 |
| canterbury/fields.c/gzip/l9/oneshot | 11150 | 138.5 | 381.1 | 0.364 | -3.3 |
| canterbury/fields.c/gzip/l9/streaming | 11150 | 134.6 | 397.9 | 0.338 | -4.7 |
| canterbury/fields.c/raw/l1/oneshot | 11150 | 148.6 | 366.2 | 0.406 | -1.0 |
| canterbury/fields.c/raw/l1/streaming | 11150 | 148.8 | 384.8 | 0.387 | +2.2 |
| canterbury/fields.c/raw/l6/oneshot | 11150 | 161.3 | 396.2 | 0.407 | -5.7 |
| canterbury/fields.c/raw/l6/streaming | 11150 | 161.7 | 418.8 | 0.386 | -3.1 |
| canterbury/fields.c/raw/l9/oneshot | 11150 | 170.0 | 399.8 | 0.425 | +6.1 |
| canterbury/fields.c/raw/l9/streaming | 11150 | 165.3 | 423.4 | 0.390 | +3.7 |
| canterbury/fields.c/zlib/l1/oneshot | 11150 | 118.2 | 269.5 | 0.438 | -6.0 |
| canterbury/fields.c/zlib/l1/streaming | 11150 | 116.9 | 287.7 | 0.406 | -15.1 |
| canterbury/fields.c/zlib/l6/oneshot | 11150 | 133.0 | 306.3 | 0.434 | -17.3 |
| canterbury/fields.c/zlib/l6/streaming | 11150 | 131.6 | 321.8 | 0.409 | -15.4 |
| canterbury/fields.c/zlib/l9/oneshot | 11150 | 161.2 | 369.9 | 0.436 | +0.9 |
| canterbury/fields.c/zlib/l9/streaming | 11150 | 155.8 | 392.2 | 0.397 | -0.8 |
| canterbury/cp.html/gzip/l1/oneshot | 24603 | 131.9 | 303.8 | 0.434 | -3.7 |
| canterbury/cp.html/gzip/l1/streaming | 24603 | 127.6 | 312.0 | 0.409 | -6.3 |
| canterbury/cp.html/gzip/l6/oneshot | 24603 | 152.0 | 322.5 | 0.471 | -1.4 |
| canterbury/cp.html/gzip/l6/streaming | 24603 | 150.4 | 324.8 | 0.463 | -0.1 |
| canterbury/cp.html/gzip/l9/oneshot | 24603 | 130.2 | 274.1 | 0.475 | -13.3 |
| canterbury/cp.html/gzip/l9/streaming | 24603 | 127.8 | 284.4 | 0.449 | -16.4 |
| canterbury/cp.html/raw/l1/oneshot | 24603 | 158.7 | 319.2 | 0.497 | -2.7 |
| canterbury/cp.html/raw/l1/streaming | 24603 | 158.5 | 323.7 | 0.490 | -2.6 |
| canterbury/cp.html/raw/l6/oneshot | 24603 | 170.6 | 337.5 | 0.506 | -4.8 |
| canterbury/cp.html/raw/l6/streaming | 24603 | 174.7 | 343.7 | 0.508 | -0.4 |
| canterbury/cp.html/raw/l9/oneshot | 24603 | 173.0 | 343.3 | 0.504 | +3.5 |
| canterbury/cp.html/raw/l9/streaming | 24603 | 172.8 | 358.9 | 0.481 | -2.3 |
| canterbury/cp.html/zlib/l1/oneshot | 24603 | 155.0 | 297.6 | 0.521 | +2.1 |
| canterbury/cp.html/zlib/l1/streaming | 24603 | 150.9 | 303.3 | 0.497 | -0.5 |
| canterbury/cp.html/zlib/l6/oneshot | 24603 | 136.6 | 258.6 | 0.528 | -19.1 |
| canterbury/cp.html/zlib/l6/streaming | 24603 | 138.6 | 260.7 | 0.532 | -14.6 |
| canterbury/cp.html/zlib/l9/oneshot | 24603 | 136.1 | 262.6 | 0.518 | -17.5 |
| canterbury/cp.html/zlib/l9/streaming | 24603 | 137.5 | 267.7 | 0.513 | -17.4 |
| canterbury/sum/gzip/l1/oneshot | 38240 | 132.7 | 280.6 | 0.473 | -2.8 |
| canterbury/sum/gzip/l1/streaming | 38240 | 132.1 | 287.4 | 0.460 | -3.3 |
| canterbury/sum/gzip/l6/oneshot | 38240 | 138.4 | 287.8 | 0.481 | -12.8 |
| canterbury/sum/gzip/l6/streaming | 38240 | 132.5 | 288.3 | 0.460 | -17.2 |
| canterbury/sum/gzip/l9/oneshot | 38240 | 158.2 | 320.9 | 0.493 | -3.6 |
| canterbury/sum/gzip/l9/streaming | 38240 | 160.5 | 326.9 | 0.491 | -2.1 |
| canterbury/sum/raw/l1/oneshot | 38240 | 148.4 | 300.2 | 0.494 | -2.7 |
| canterbury/sum/raw/l1/streaming | 38240 | 150.8 | 309.6 | 0.487 | -2.5 |
| canterbury/sum/raw/l6/oneshot | 38240 | 177.4 | 334.8 | 0.530 | -3.6 |
| canterbury/sum/raw/l6/streaming | 38240 | 177.7 | 345.2 | 0.515 | -3.6 |
| canterbury/sum/raw/l9/oneshot | 38240 | 191.4 | 350.7 | 0.546 | +1.8 |
| canterbury/sum/raw/l9/streaming | 38240 | 189.4 | 363.2 | 0.521 | +15.9 |
| canterbury/sum/zlib/l1/oneshot | 38240 | 147.4 | 281.2 | 0.524 | -0.1 |
| canterbury/sum/zlib/l1/streaming | 38240 | 145.5 | 288.5 | 0.504 | -2.5 |
| canterbury/sum/zlib/l6/oneshot | 38240 | 171.3 | 310.0 | 0.552 | -1.0 |
| canterbury/sum/zlib/l6/streaming | 38240 | 171.4 | 307.0 | 0.558 | +1.4 |
| canterbury/sum/zlib/l9/oneshot | 38240 | 169.5 | 290.2 | 0.584 | -4.9 |
| canterbury/sum/zlib/l9/streaming | 38240 | 163.1 | 305.1 | 0.534 | -7.7 |
| canterbury/asyoulik.txt/gzip/l1/oneshot | 125179 | 133.8 | 264.2 | 0.506 | -0.2 |
| canterbury/asyoulik.txt/gzip/l1/streaming | 125179 | 126.7 | 270.9 | 0.468 | -4.9 |
| canterbury/asyoulik.txt/gzip/l6/oneshot | 125179 | 154.5 | 274.9 | 0.562 | -8.2 |
| canterbury/asyoulik.txt/gzip/l6/streaming | 125179 | 161.1 | 270.0 | 0.597 | -1.4 |
| canterbury/asyoulik.txt/gzip/l9/oneshot | 125179 | 159.7 | 274.3 | 0.582 | -2.1 |
| canterbury/asyoulik.txt/gzip/l9/streaming | 125179 | 158.4 | 271.5 | 0.583 | -1.9 |
| canterbury/asyoulik.txt/raw/l1/oneshot | 125179 | 117.1 | 243.5 | 0.481 | -19.9 |
| canterbury/asyoulik.txt/raw/l1/streaming | 125179 | 119.8 | 244.9 | 0.489 | -17.3 |
| canterbury/asyoulik.txt/raw/l6/oneshot | 125179 | 152.4 | 240.1 | 0.635 | -16.4 |
| canterbury/asyoulik.txt/raw/l6/streaming | 125179 | 151.0 | 243.3 | 0.621 | -15.0 |
| canterbury/asyoulik.txt/raw/l9/oneshot | 125179 | 148.0 | 248.9 | 0.595 | -13.4 |
| canterbury/asyoulik.txt/raw/l9/streaming | 125179 | 144.2 | 280.8 | 0.513 | -17.6 |
| canterbury/asyoulik.txt/zlib/l1/oneshot | 125179 | 138.3 | 259.5 | 0.533 | +0.7 |
| canterbury/asyoulik.txt/zlib/l1/streaming | 125179 | 136.0 | 270.7 | 0.502 | +0.3 |
| canterbury/asyoulik.txt/zlib/l6/oneshot | 125179 | 161.5 | 245.9 | 0.657 | -7.1 |
| canterbury/asyoulik.txt/zlib/l6/streaming | 125179 | 157.5 | 256.1 | 0.615 | -7.8 |
| canterbury/asyoulik.txt/zlib/l9/oneshot | 125179 | 168.9 | 236.0 | 0.715 | -2.2 |
| canterbury/asyoulik.txt/zlib/l9/streaming | 125179 | 156.6 | 258.7 | 0.605 | -7.9 |
| canterbury/alice29.txt/gzip/l1/oneshot | 152089 | 142.8 | 271.8 | 0.525 | +0.0 |
| canterbury/alice29.txt/gzip/l1/streaming | 152089 | 137.4 | 278.9 | 0.493 | -2.9 |
| canterbury/alice29.txt/gzip/l6/oneshot | 152089 | 144.6 | 234.2 | 0.617 | -21.0 |
| canterbury/alice29.txt/gzip/l6/streaming | 152089 | 143.7 | 237.4 | 0.605 | -19.9 |
| canterbury/alice29.txt/gzip/l9/oneshot | 152089 | 174.7 | 277.7 | 0.629 | -5.8 |
| canterbury/alice29.txt/gzip/l9/streaming | 152089 | 162.8 | 293.0 | 0.556 | -9.5 |
| canterbury/alice29.txt/raw/l1/oneshot | 152089 | 157.7 | 290.3 | 0.543 | +1.1 |
| canterbury/alice29.txt/raw/l1/streaming | 152089 | 154.4 | 299.2 | 0.516 | +0.3 |
| canterbury/alice29.txt/raw/l6/oneshot | 152089 | 192.6 | 297.2 | 0.648 | -4.2 |
| canterbury/alice29.txt/raw/l6/streaming | 152089 | 188.0 | 309.1 | 0.608 | -3.4 |
| canterbury/alice29.txt/raw/l9/oneshot | 152089 | 194.4 | 301.0 | 0.646 | -3.8 |
| canterbury/alice29.txt/raw/l9/streaming | 152089 | 191.1 | 306.4 | 0.624 | -3.0 |
| canterbury/alice29.txt/zlib/l1/oneshot | 152089 | 145.6 | 264.7 | 0.550 | -4.2 |
| canterbury/alice29.txt/zlib/l1/streaming | 152089 | 147.1 | 265.9 | 0.553 | -0.4 |
| canterbury/alice29.txt/zlib/l6/oneshot | 152089 | 185.5 | 273.8 | 0.678 | -3.5 |
| canterbury/alice29.txt/zlib/l6/streaming | 152089 | 176.8 | 279.7 | 0.632 | -5.6 |
| canterbury/alice29.txt/zlib/l9/oneshot | 152089 | 186.1 | 268.3 | 0.693 | -3.7 |
| canterbury/alice29.txt/zlib/l9/streaming | 152089 | 185.1 | 274.1 | 0.675 | -2.0 |
| canterbury/lcet10.txt/gzip/l1/oneshot | 426754 | 124.6 | 231.5 | 0.538 | -19.8 |
| canterbury/lcet10.txt/gzip/l1/streaming | 426754 | 125.0 | 236.2 | 0.529 | -17.8 |
| canterbury/lcet10.txt/gzip/l6/oneshot | 426754 | 185.1 | 285.9 | 0.648 | -1.7 |
| canterbury/lcet10.txt/gzip/l6/streaming | 426754 | 177.6 | 283.7 | 0.626 | -2.5 |
| canterbury/lcet10.txt/gzip/l9/oneshot | 426754 | 191.7 | 295.7 | 0.648 | -0.5 |
| canterbury/lcet10.txt/gzip/l9/streaming | 426754 | 186.1 | 299.4 | 0.622 | +3.7 |
| canterbury/lcet10.txt/raw/l1/oneshot | 426754 | 166.1 | 304.5 | 0.546 | +2.6 |
| canterbury/lcet10.txt/raw/l1/streaming | 426754 | 163.0 | 308.1 | 0.529 | +2.3 |
| canterbury/lcet10.txt/raw/l6/oneshot | 426754 | 199.8 | 303.3 | 0.659 | -5.9 |
| canterbury/lcet10.txt/raw/l6/streaming | 426754 | 198.6 | 292.5 | 0.679 | -4.4 |
| canterbury/lcet10.txt/raw/l9/oneshot | 426754 | 203.6 | 312.9 | 0.651 | -4.6 |
| canterbury/lcet10.txt/raw/l9/streaming | 426754 | 210.7 | 294.5 | 0.716 | +0.7 |
| canterbury/lcet10.txt/zlib/l1/oneshot | 426754 | 155.2 | 261.9 | 0.593 | -4.0 |
| canterbury/lcet10.txt/zlib/l1/streaming | 426754 | 146.3 | 275.8 | 0.530 | -7.7 |
| canterbury/lcet10.txt/zlib/l6/oneshot | 426754 | 194.5 | 277.5 | 0.701 | -3.0 |
| canterbury/lcet10.txt/zlib/l6/streaming | 426754 | 191.8 | 288.9 | 0.664 | -3.3 |
| canterbury/lcet10.txt/zlib/l9/oneshot | 426754 | 203.8 | 284.6 | 0.716 | -0.1 |
| canterbury/lcet10.txt/zlib/l9/streaming | 426754 | 198.7 | 288.7 | 0.688 | +0.4 |
| canterbury/plrabn12.txt/gzip/l1/oneshot | 481861 | 130.4 | 268.1 | 0.486 | -1.7 |
| canterbury/plrabn12.txt/gzip/l1/streaming | 481861 | 125.6 | 272.9 | 0.460 | -2.8 |
| canterbury/plrabn12.txt/gzip/l6/oneshot | 481861 | 163.8 | 254.9 | 0.643 | +3.3 |
| canterbury/plrabn12.txt/gzip/l6/streaming | 481861 | 161.6 | 260.6 | 0.620 | +1.8 |
| canterbury/plrabn12.txt/gzip/l9/oneshot | 481861 | 155.2 | 246.9 | 0.628 | -4.4 |
| canterbury/plrabn12.txt/gzip/l9/streaming | 481861 | 145.4 | 251.6 | 0.578 | -9.9 |
| canterbury/plrabn12.txt/raw/l1/oneshot | 481861 | 138.6 | 286.2 | 0.484 | -0.8 |
| canterbury/plrabn12.txt/raw/l1/streaming | 481861 | 138.5 | 288.6 | 0.480 | +0.6 |
| canterbury/plrabn12.txt/raw/l6/oneshot | 481861 | 145.2 | 227.1 | 0.640 | -21.4 |
| canterbury/plrabn12.txt/raw/l6/streaming | 481861 | 143.4 | 229.2 | 0.626 | -19.1 |
| canterbury/plrabn12.txt/raw/l9/oneshot | 481861 | 184.6 | 266.9 | 0.692 | +3.6 |
| canterbury/plrabn12.txt/raw/l9/streaming | 481861 | 177.2 | 286.4 | 0.619 | -0.7 |
| canterbury/plrabn12.txt/zlib/l1/oneshot | 481861 | 131.4 | 246.1 | 0.534 | +1.7 |
| canterbury/plrabn12.txt/zlib/l1/streaming | 481861 | 126.0 | 224.3 | 0.562 | -5.4 |
| canterbury/plrabn12.txt/zlib/l6/oneshot | 481861 | 139.0 | 207.7 | 0.669 | -18.7 |
| canterbury/plrabn12.txt/zlib/l6/streaming | 481861 | 137.8 | 210.1 | 0.656 | -12.6 |
| canterbury/plrabn12.txt/zlib/l9/oneshot | 481861 | 168.0 | 252.1 | 0.667 | -4.6 |
| canterbury/plrabn12.txt/zlib/l9/streaming | 481861 | 167.5 | 245.8 | 0.682 | -1.3 |
| canterbury/ptt5/gzip/l1/oneshot | 513216 | 337.5 | 572.3 | 0.590 | +2.3 |
| canterbury/ptt5/gzip/l1/streaming | 513216 | 337.4 | 601.4 | 0.561 | +1.2 |
| canterbury/ptt5/gzip/l6/oneshot | 513216 | 297.2 | 505.8 | 0.588 | +4.9 |
| canterbury/ptt5/gzip/l6/streaming | 513216 | 290.3 | 529.6 | 0.548 | +0.7 |
| canterbury/ptt5/gzip/l9/oneshot | 513216 | 338.9 | 587.7 | 0.577 | -1.0 |
| canterbury/ptt5/gzip/l9/streaming | 513216 | 337.9 | 607.5 | 0.556 | -0.3 |
| canterbury/ptt5/raw/l1/oneshot | 513216 | 423.6 | 667.1 | 0.635 | +4.8 |
| canterbury/ptt5/raw/l1/streaming | 513216 | 420.8 | 689.7 | 0.610 | +0.9 |
| canterbury/ptt5/raw/l6/oneshot | 513216 | 344.7 | 567.8 | 0.607 | +19.6 |
| canterbury/ptt5/raw/l6/streaming | 513216 | 362.2 | 578.8 | 0.626 | +23.0 |
| canterbury/ptt5/raw/l9/oneshot | 513216 | 346.8 | 540.5 | 0.642 | -10.9 |
| canterbury/ptt5/raw/l9/streaming | 513216 | 349.3 | 581.0 | 0.601 | -12.0 |
| canterbury/ptt5/zlib/l1/oneshot | 513216 | 368.5 | 525.9 | 0.701 | +2.0 |
| canterbury/ptt5/zlib/l1/streaming | 513216 | 362.9 | 542.8 | 0.669 | +0.1 |
| canterbury/ptt5/zlib/l6/oneshot | 513216 | 311.5 | 485.3 | 0.642 | +4.0 |
| canterbury/ptt5/zlib/l6/streaming | 513216 | 315.5 | 499.5 | 0.632 | -0.8 |
| canterbury/ptt5/zlib/l9/oneshot | 513216 | 364.5 | 543.7 | 0.670 | +1.7 |
| canterbury/ptt5/zlib/l9/streaming | 513216 | 370.6 | 564.8 | 0.656 | -0.2 |
| canterbury/kennedy.xls/gzip/l1/oneshot | 1029744 | 240.1 | 463.0 | 0.519 | -2.8 |
| canterbury/kennedy.xls/gzip/l1/streaming | 1029744 | 241.1 | 472.8 | 0.510 | -1.4 |
| canterbury/kennedy.xls/gzip/l6/oneshot | 1029744 | 282.5 | 564.2 | 0.501 | -1.1 |
| canterbury/kennedy.xls/gzip/l6/streaming | 1029744 | 278.4 | 576.8 | 0.483 | -2.4 |
| canterbury/kennedy.xls/gzip/l9/oneshot | 1029744 | 287.4 | 538.9 | 0.533 | -3.6 |
| canterbury/kennedy.xls/gzip/l9/streaming | 1029744 | 284.9 | 570.1 | 0.500 | -2.1 |
| canterbury/kennedy.xls/raw/l1/oneshot | 1029744 | 243.6 | 498.1 | 0.489 | -9.5 |
| canterbury/kennedy.xls/raw/l1/streaming | 1029744 | 246.6 | 537.9 | 0.458 | -12.4 |
| canterbury/kennedy.xls/raw/l6/oneshot | 1029744 | 327.3 | 636.3 | 0.514 | -2.2 |
| canterbury/kennedy.xls/raw/l6/streaming | 1029744 | 330.6 | 664.7 | 0.497 | -1.7 |
| canterbury/kennedy.xls/raw/l9/oneshot | 1029744 | 274.6 | 525.7 | 0.522 | -15.1 |
| canterbury/kennedy.xls/raw/l9/streaming | 1029744 | 276.5 | 550.6 | 0.502 | -17.8 |
| canterbury/kennedy.xls/zlib/l1/oneshot | 1029744 | 243.1 | 452.5 | 0.537 | -4.3 |
| canterbury/kennedy.xls/zlib/l1/streaming | 1029744 | 254.1 | 462.6 | 0.549 | +0.0 |
| canterbury/kennedy.xls/zlib/l6/oneshot | 1029744 | 286.3 | 490.4 | 0.584 | -3.4 |
| canterbury/kennedy.xls/zlib/l6/streaming | 1029744 | 292.5 | 510.1 | 0.573 | -2.1 |
| canterbury/kennedy.xls/zlib/l9/oneshot | 1029744 | 298.9 | 513.7 | 0.582 | -2.5 |
| canterbury/kennedy.xls/zlib/l9/streaming | 1029744 | 302.3 | 526.1 | 0.575 | -3.0 |

### Noise

**Full noise** (`phase3-noise.json`; sources `phase3-start.json` vs `phase3-start-rerun.json`): aggregate max 8.22%, ratio max 12.63%.

| key | throughput noise % | ratio noise % |
|---|---:|---:|
| canterbury/gzip/l1/oneshot | 0.04 | 4.54 |
| canterbury/gzip/l1/streaming | 0.57 | 7.29 |
| canterbury/gzip/l6/oneshot | 1.47 | 6.97 |
| canterbury/gzip/l6/streaming | 0.88 | 4.56 |
| canterbury/gzip/l9/oneshot | 2.23 | 6.04 |
| canterbury/gzip/l9/streaming | 1.29 | 9.19 |
| canterbury/raw/l1/oneshot | 4.64 | 0.31 |
| canterbury/raw/l1/streaming | 7.59 | 0.25 |
| canterbury/raw/l6/oneshot | 0.02 | 2.47 |
| canterbury/raw/l6/streaming | 0.07 | 3.36 |
| canterbury/raw/l9/oneshot | 7.16 | 0.80 |
| canterbury/raw/l9/streaming | 5.80 | 1.81 |
| canterbury/zlib/l1/oneshot | 1.55 | 5.68 |
| canterbury/zlib/l1/streaming | 0.80 | 4.32 |
| canterbury/zlib/l6/oneshot | 1.96 | 8.12 |
| canterbury/zlib/l6/streaming | 1.84 | 9.88 |
| canterbury/zlib/l9/oneshot | 6.90 | 7.15 |
| canterbury/zlib/l9/streaming | 8.22 | 5.46 |
| silesia/gzip/l1/oneshot | 4.07 | 7.05 |
| silesia/gzip/l1/streaming | 1.20 | 4.35 |
| silesia/gzip/l6/oneshot | 0.86 | 3.23 |
| silesia/gzip/l6/streaming | 1.16 | 5.77 |
| silesia/gzip/l9/oneshot | 1.99 | 5.68 |
| silesia/gzip/l9/streaming | 2.35 | 6.20 |
| silesia/raw/l1/oneshot | 2.88 | 4.71 |
| silesia/raw/l1/streaming | 1.06 | 0.40 |
| silesia/raw/l6/oneshot | 3.09 | 2.49 |
| silesia/raw/l6/streaming | 1.97 | 0.35 |
| silesia/raw/l9/oneshot | 5.18 | 6.19 |
| silesia/raw/l9/streaming | 3.71 | 3.15 |
| silesia/zlib/l1/oneshot | 0.98 | 5.25 |
| silesia/zlib/l1/streaming | 1.27 | 7.02 |
| silesia/zlib/l6/oneshot | 0.42 | 5.58 |
| silesia/zlib/l6/streaming | 0.39 | 7.70 |
| silesia/zlib/l9/oneshot | 5.10 | 12.63 |
| silesia/zlib/l9/streaming | 1.39 | 9.59 |

**Quick noise** (`phase3-noise-quick.json`; sources `phase3-start-quick.json` vs `phase3-start-quick-rerun.json`): aggregate max 8.82%, ratio max 7.08%.

| key | throughput noise % | ratio noise % |
|---|---:|---:|
| canterbury/gzip/l1/oneshot | 2.42 | 0.13 |
| canterbury/gzip/l1/streaming | 3.85 | 0.07 |
| canterbury/gzip/l6/oneshot | 0.30 | 5.81 |
| canterbury/gzip/l6/streaming | 1.64 | 4.49 |
| canterbury/raw/l1/oneshot | 1.17 | 0.77 |
| canterbury/raw/l1/streaming | 2.35 | 2.30 |
| canterbury/raw/l6/oneshot | 0.98 | 0.31 |
| canterbury/raw/l6/streaming | 0.30 | 0.95 |
| canterbury/zlib/l1/oneshot | 4.16 | 0.61 |
| canterbury/zlib/l1/streaming | 0.88 | 3.05 |
| canterbury/zlib/l6/oneshot | 6.10 | 2.20 |
| canterbury/zlib/l6/streaming | 8.82 | 1.27 |
| silesia/gzip/l1/oneshot | 5.51 | 3.71 |
| silesia/gzip/l1/streaming | 7.70 | 1.35 |
| silesia/gzip/l6/oneshot | 0.48 | 1.93 |
| silesia/gzip/l6/streaming | 2.57 | 2.27 |
| silesia/raw/l1/oneshot | 0.91 | 1.03 |
| silesia/raw/l1/streaming | 5.61 | 6.25 |
| silesia/raw/l6/oneshot | 4.44 | 3.84 |
| silesia/raw/l6/streaming | 1.79 | 7.08 |
| silesia/zlib/l1/oneshot | 1.11 | 0.11 |
| silesia/zlib/l1/streaming | 0.99 | 4.39 |
| silesia/zlib/l6/oneshot | 2.03 | 5.16 |
| silesia/zlib/l6/streaming | 3.51 | 0.25 |

### Delta from the phase-2 record (`make bench-compare A=baselines/phase2.json B=baselines/phase3-start.json NOISE=baselines/phase3-noise.json`)

The comparison below is against the operator's phase-2 numbers of record on a differently loaded machine, so LOSS verdicts here mostly show machine drift; the ratio verdict and the `phase3-start` rerun (which shows no WIN, LOSS or REGRESSION) are the reference.

| key | A MiB/s | B MiB/s | delta % | 2x noise % | verdict | vs phase-2 % | B ratio | ratio delta % | ratio verdict |
|---|---:|---:|---:|---:|---|---:|---:|---:|---|
| canterbury/gzip/l1/oneshot | 190.6 | 180.0 | -5.55 | 0.07 | LOSS | -5.55 | 0.517 | -5.97 | NEUTRAL |
| canterbury/gzip/l1/streaming | 187.9 | 177.5 | -5.57 | 1.14 | LOSS | -5.57 | 0.498 | n/a (denominator changed) | n/a |
| canterbury/gzip/l6/oneshot | 218.2 | 214.3 | -1.78 | 2.94 | NEUTRAL | -1.78 | 0.579 | -7.23 | NEUTRAL |
| canterbury/gzip/l6/streaming | 216.7 | 210.8 | -2.70 | 1.77 | LOSS | -2.70 | 0.561 | n/a (denominator changed) | n/a |
| canterbury/gzip/l9/oneshot | 228.5 | 221.1 | -3.24 | 4.46 | NEUTRAL | -3.24 | 0.586 | -7.24 | NEUTRAL |
| canterbury/gzip/l9/streaming | 222.9 | 214.6 | -3.72 | 2.58 | LOSS | -3.72 | 0.552 | n/a (denominator changed) | n/a |
| canterbury/raw/l1/oneshot | 207.1 | 199.5 | -3.68 | 9.29 | NEUTRAL | -3.68 | 0.513 | -8.77 | LOSS |
| canterbury/raw/l1/streaming | 208.5 | 199.4 | -4.35 | 15.18 | NEUTRAL | -4.35 | 0.496 | n/a (denominator changed) | n/a |
| canterbury/raw/l6/oneshot | 246.3 | 230.0 | -6.65 | 0.03 | LOSS | -6.65 | 0.600 | -8.09 | LOSS |
| canterbury/raw/l6/streaming | 243.0 | 230.3 | -5.22 | 0.15 | LOSS | -5.22 | 0.593 | n/a (denominator changed) | n/a |
| canterbury/raw/l9/oneshot | 251.2 | 233.0 | -7.25 | 14.32 | NEUTRAL | -7.25 | 0.607 | -6.06 | LOSS |
| canterbury/raw/l9/streaming | 253.0 | 232.1 | -8.24 | 11.61 | NEUTRAL | -8.24 | 0.583 | n/a (denominator changed) | n/a |
| canterbury/zlib/l1/oneshot | 195.8 | 192.5 | -1.70 | 3.09 | NEUTRAL | -1.70 | 0.561 | -5.36 | NEUTRAL |
| canterbury/zlib/l1/streaming | 196.2 | 190.2 | -3.05 | 1.60 | LOSS | -3.05 | 0.554 | n/a (denominator changed) | n/a |
| canterbury/zlib/l6/oneshot | 231.0 | 215.0 | -6.94 | 3.93 | LOSS | -6.94 | 0.640 | -5.17 | NEUTRAL |
| canterbury/zlib/l6/streaming | 227.5 | 214.6 | -5.69 | 3.68 | LOSS | -5.69 | 0.621 | n/a (denominator changed) | n/a |
| canterbury/zlib/l9/oneshot | 241.3 | 235.5 | -2.41 | 13.80 | NEUTRAL | -2.41 | 0.649 | -6.14 | NEUTRAL |
| canterbury/zlib/l9/streaming | 239.3 | 234.0 | -2.20 | 16.44 | NEUTRAL | -2.20 | 0.634 | n/a (denominator changed) | n/a |
| silesia/gzip/l1/oneshot | 166.6 | 150.1 | -9.89 | 8.15 | LOSS | -9.89 | 0.590 | -6.49 | NEUTRAL |
| silesia/gzip/l1/streaming | 176.4 | 162.3 | -7.99 | 2.40 | LOSS | -7.99 | 0.543 | n/a (denominator changed) | n/a |
| silesia/gzip/l6/oneshot | 192.8 | 179.3 | -7.01 | 1.71 | LOSS | -7.01 | 0.674 | -4.62 | NEUTRAL |
| silesia/gzip/l6/streaming | 205.8 | 191.6 | -6.88 | 2.31 | LOSS | -6.88 | 0.598 | n/a (denominator changed) | n/a |
| silesia/gzip/l9/oneshot | 194.1 | 184.4 | -5.05 | 3.97 | LOSS | -5.05 | 0.663 | -5.46 | NEUTRAL |
| silesia/gzip/l9/streaming | 209.9 | 197.7 | -5.80 | 4.70 | LOSS | -5.80 | 0.598 | n/a (denominator changed) | n/a |
| silesia/raw/l1/oneshot | 176.9 | 164.4 | -7.06 | 5.76 | LOSS | -7.06 | 0.604 | -5.15 | NEUTRAL |
| silesia/raw/l1/streaming | 192.7 | 181.2 | -5.98 | 2.12 | LOSS | -5.98 | 0.557 | n/a (denominator changed) | n/a |
| silesia/raw/l6/oneshot | 199.5 | 195.6 | -1.96 | 6.17 | NEUTRAL | -1.96 | 0.649 | -1.84 | NEUTRAL |
| silesia/raw/l6/streaming | 227.3 | 215.7 | -5.09 | 3.95 | LOSS | -5.09 | 0.610 | n/a (denominator changed) | n/a |
| silesia/raw/l9/oneshot | 207.7 | 201.8 | -2.83 | 10.37 | NEUTRAL | -2.83 | 0.668 | -1.90 | NEUTRAL |
| silesia/raw/l9/streaming | 231.6 | 222.0 | -4.16 | 7.43 | NEUTRAL | -4.16 | 0.633 | n/a (denominator changed) | n/a |
| silesia/zlib/l1/oneshot | 166.2 | 160.6 | -3.37 | 1.95 | LOSS | -3.37 | 0.624 | -3.76 | NEUTRAL |
| silesia/zlib/l1/streaming | 183.0 | 174.9 | -4.43 | 2.55 | LOSS | -4.43 | 0.581 | n/a (denominator changed) | n/a |
| silesia/zlib/l6/oneshot | 192.6 | 181.2 | -5.90 | 0.85 | LOSS | -5.90 | 0.665 | -4.45 | NEUTRAL |
| silesia/zlib/l6/streaming | 214.0 | 200.7 | -6.21 | 0.79 | LOSS | -6.21 | 0.644 | n/a (denominator changed) | n/a |
| silesia/zlib/l9/oneshot | 194.5 | 176.3 | -9.33 | 10.20 | NEUTRAL | -9.33 | 0.667 | -5.56 | NEUTRAL |
| silesia/zlib/l9/streaming | 215.7 | 199.5 | -7.53 | 2.78 | LOSS | -7.53 | 0.638 | n/a (denominator changed) | n/a |

REGRESSION canterbury/alice29.txt/gzip/l6/oneshot: 182.9 -> 144.6 MiB/s (-21.0%, threshold -9.5%)
REGRESSION canterbury/alice29.txt/gzip/l6/streaming: 179.4 -> 143.7 MiB/s (-19.9%, threshold -8.5%)
REGRESSION canterbury/alice29.txt/gzip/l9/streaming: 179.9 -> 162.8 MiB/s (-9.5%, threshold -3.8%)
REGRESSION canterbury/asyoulik.txt/gzip/l1/streaming: 133.2 -> 126.7 MiB/s (-4.9%, threshold -3.6%)
REGRESSION canterbury/asyoulik.txt/gzip/l6/oneshot: 168.2 -> 154.5 MiB/s (-8.2%, threshold -4.8%)
REGRESSION canterbury/asyoulik.txt/raw/l1/oneshot: 146.2 -> 117.1 MiB/s (-19.9%, threshold -4.6%)
REGRESSION canterbury/asyoulik.txt/raw/l1/streaming: 144.9 -> 119.8 MiB/s (-17.3%, threshold -8.1%)
REGRESSION canterbury/asyoulik.txt/raw/l6/oneshot: 182.4 -> 152.4 MiB/s (-16.4%, threshold -2.4%)
REGRESSION canterbury/asyoulik.txt/raw/l6/streaming: 177.6 -> 151.0 MiB/s (-15.0%, threshold -6.6%)
REGRESSION canterbury/asyoulik.txt/raw/l9/streaming: 174.9 -> 144.2 MiB/s (-17.6%, threshold -15.7%)
REGRESSION canterbury/asyoulik.txt/zlib/l6/oneshot: 173.9 -> 161.5 MiB/s (-7.1%, threshold -2.0%)
REGRESSION canterbury/asyoulik.txt/zlib/l6/streaming: 170.8 -> 157.5 MiB/s (-7.8%, threshold -5.7%)
REGRESSION canterbury/cp.html/raw/l6/oneshot: 179.2 -> 170.6 MiB/s (-4.8%, threshold -2.2%)
REGRESSION canterbury/cp.html/zlib/l9/oneshot: 165.0 -> 136.1 MiB/s (-17.5%, threshold -6.9%)
REGRESSION canterbury/cp.html/zlib/l9/streaming: 166.4 -> 137.5 MiB/s (-17.4%, threshold -8.2%)
REGRESSION canterbury/fields.c/gzip/l1/oneshot: 129.3 -> 105.5 MiB/s (-18.4%, threshold -16.0%)
REGRESSION canterbury/fields.c/gzip/l1/streaming: 129.1 -> 107.9 MiB/s (-16.4%, threshold -13.5%)
REGRESSION canterbury/fields.c/gzip/l6/oneshot: 137.9 -> 121.5 MiB/s (-11.8%, threshold -6.6%)
REGRESSION canterbury/fields.c/zlib/l1/oneshot: 125.7 -> 118.2 MiB/s (-6.0%, threshold -1.5%)
REGRESSION canterbury/fields.c/zlib/l1/streaming: 137.8 -> 116.9 MiB/s (-15.1%, threshold -0.8%)
REGRESSION canterbury/fields.c/zlib/l6/oneshot: 160.8 -> 133.0 MiB/s (-17.3%, threshold -15.2%)
REGRESSION canterbury/grammar.lsp/gzip/l1/streaming: 74.7 -> 70.5 MiB/s (-5.7%, threshold -0.6%)
REGRESSION canterbury/grammar.lsp/gzip/l6/oneshot: 77.9 -> 76.1 MiB/s (-2.3%, threshold -1.5%)
REGRESSION canterbury/grammar.lsp/gzip/l6/streaming: 78.6 -> 74.8 MiB/s (-4.9%, threshold -2.4%)
REGRESSION canterbury/kennedy.xls/gzip/l6/streaming: 285.3 -> 278.4 MiB/s (-2.4%, threshold -1.7%)
REGRESSION canterbury/kennedy.xls/gzip/l9/oneshot: 298.2 -> 287.4 MiB/s (-3.6%, threshold -2.9%)
REGRESSION canterbury/kennedy.xls/raw/l1/oneshot: 269.1 -> 243.6 MiB/s (-9.5%, threshold -6.7%)
REGRESSION canterbury/kennedy.xls/raw/l1/streaming: 281.5 -> 246.6 MiB/s (-12.4%, threshold -7.6%)
REGRESSION canterbury/kennedy.xls/raw/l6/oneshot: 334.8 -> 327.3 MiB/s (-2.2%, threshold -2.1%)
REGRESSION canterbury/kennedy.xls/raw/l6/streaming: 336.2 -> 330.6 MiB/s (-1.7%, threshold -1.6%)
REGRESSION canterbury/kennedy.xls/zlib/l1/oneshot: 254.1 -> 243.1 MiB/s (-4.3%, threshold -2.3%)
REGRESSION canterbury/lcet10.txt/gzip/l1/oneshot: 155.4 -> 124.6 MiB/s (-19.8%, threshold -16.4%)
REGRESSION canterbury/lcet10.txt/gzip/l1/streaming: 152.0 -> 125.0 MiB/s (-17.8%, threshold -14.7%)
REGRESSION canterbury/lcet10.txt/gzip/l6/streaming: 182.2 -> 177.6 MiB/s (-2.5%, threshold -0.9%)
REGRESSION canterbury/lcet10.txt/raw/l6/oneshot: 212.3 -> 199.8 MiB/s (-5.9%, threshold -1.1%)
REGRESSION canterbury/lcet10.txt/raw/l6/streaming: 207.7 -> 198.6 MiB/s (-4.4%, threshold -0.6%)
REGRESSION canterbury/lcet10.txt/zlib/l1/oneshot: 161.7 -> 155.2 MiB/s (-4.0%, threshold -1.5%)
REGRESSION canterbury/lcet10.txt/zlib/l1/streaming: 158.4 -> 146.3 MiB/s (-7.7%, threshold -3.0%)
REGRESSION canterbury/plrabn12.txt/gzip/l9/oneshot: 162.2 -> 155.2 MiB/s (-4.4%, threshold -4.1%)
REGRESSION canterbury/plrabn12.txt/gzip/l9/streaming: 161.5 -> 145.4 MiB/s (-9.9%, threshold -5.5%)
REGRESSION canterbury/plrabn12.txt/raw/l6/oneshot: 184.7 -> 145.2 MiB/s (-21.4%, threshold -15.7%)
REGRESSION canterbury/plrabn12.txt/raw/l6/streaming: 177.2 -> 143.4 MiB/s (-19.1%, threshold -14.7%)
REGRESSION canterbury/plrabn12.txt/zlib/l1/streaming: 133.2 -> 126.0 MiB/s (-5.4%, threshold -0.8%)
REGRESSION canterbury/xargs.1/gzip/l1/oneshot: 76.6 -> 71.0 MiB/s (-7.3%, threshold -2.8%)
REGRESSION canterbury/xargs.1/gzip/l1/streaming: 74.7 -> 70.3 MiB/s (-6.0%, threshold -2.9%)
REGRESSION canterbury/xargs.1/raw/l6/oneshot: 95.0 -> 85.5 MiB/s (-9.9%, threshold -4.2%)
REGRESSION canterbury/xargs.1/raw/l6/streaming: 92.4 -> 82.0 MiB/s (-11.3%, threshold -8.3%)
REGRESSION canterbury/xargs.1/zlib/l1/streaming: 85.8 -> 80.2 MiB/s (-6.5%, threshold -6.0%)
REGRESSION silesia/dickens/gzip/l1/oneshot: 140.3 -> 132.3 MiB/s (-5.7%, threshold -4.1%)
REGRESSION silesia/dickens/gzip/l1/streaming: 139.6 -> 128.9 MiB/s (-7.6%, threshold -2.2%)
REGRESSION silesia/dickens/gzip/l6/oneshot: 178.0 -> 167.5 MiB/s (-5.9%, threshold -1.6%)
REGRESSION silesia/dickens/gzip/l6/streaming: 176.9 -> 158.6 MiB/s (-10.3%, threshold -5.7%)
REGRESSION silesia/dickens/gzip/l9/oneshot: 178.1 -> 154.6 MiB/s (-13.2%, threshold -13.0%)
REGRESSION silesia/dickens/gzip/l9/streaming: 176.7 -> 158.3 MiB/s (-10.5%, threshold -5.7%)
REGRESSION silesia/dickens/raw/l1/streaming: 146.7 -> 140.2 MiB/s (-4.4%, threshold -2.8%)
REGRESSION silesia/dickens/raw/l9/streaming: 189.1 -> 175.6 MiB/s (-7.1%, threshold -3.7%)
REGRESSION silesia/dickens/zlib/l1/oneshot: 142.2 -> 133.1 MiB/s (-6.4%, threshold -4.2%)
REGRESSION silesia/dickens/zlib/l1/streaming: 142.3 -> 132.3 MiB/s (-7.1%, threshold -6.1%)
REGRESSION silesia/dickens/zlib/l9/oneshot: 183.5 -> 171.7 MiB/s (-6.5%, threshold -5.1%)
REGRESSION silesia/dickens/zlib/l9/streaming: 182.8 -> 168.6 MiB/s (-7.8%, threshold -4.3%)
REGRESSION silesia/mozilla/gzip/l1/oneshot: 149.9 -> 131.0 MiB/s (-12.6%, threshold -9.2%)
REGRESSION silesia/mozilla/gzip/l1/streaming: 163.4 -> 143.7 MiB/s (-12.1%, threshold -4.6%)
REGRESSION silesia/mozilla/gzip/l6/oneshot: 165.6 -> 155.5 MiB/s (-6.1%, threshold -0.9%)
REGRESSION silesia/mozilla/gzip/l6/streaming: 182.9 -> 174.2 MiB/s (-4.8%, threshold -1.2%)
REGRESSION silesia/mozilla/gzip/l9/oneshot: 168.6 -> 156.4 MiB/s (-7.2%, threshold -2.0%)
REGRESSION silesia/mozilla/raw/l1/streaming: 175.8 -> 168.9 MiB/s (-3.9%, threshold -1.6%)
REGRESSION silesia/mozilla/raw/l6/oneshot: 171.3 -> 163.6 MiB/s (-4.5%, threshold -3.1%)
REGRESSION silesia/mozilla/raw/l6/streaming: 203.1 -> 189.9 MiB/s (-6.5%, threshold -2.0%)
REGRESSION silesia/mozilla/raw/l9/streaming: 206.7 -> 191.7 MiB/s (-7.3%, threshold -3.7%)
REGRESSION silesia/mozilla/zlib/l1/oneshot: 147.6 -> 142.0 MiB/s (-3.8%, threshold -1.0%)
REGRESSION silesia/mozilla/zlib/l1/streaming: 169.6 -> 163.1 MiB/s (-3.8%, threshold -1.6%)
REGRESSION silesia/mozilla/zlib/l6/oneshot: 164.8 -> 155.4 MiB/s (-5.7%, threshold -3.2%)
REGRESSION silesia/mozilla/zlib/l6/streaming: 190.5 -> 174.4 MiB/s (-8.5%, threshold -6.1%)
REGRESSION silesia/mozilla/zlib/l9/oneshot: 166.6 -> 156.0 MiB/s (-6.4%, threshold -5.1%)
REGRESSION silesia/mr/gzip/l1/oneshot: 156.9 -> 149.4 MiB/s (-4.8%, threshold -4.1%)
REGRESSION silesia/mr/gzip/l1/streaming: 158.4 -> 150.9 MiB/s (-4.7%, threshold -3.0%)
REGRESSION silesia/mr/gzip/l6/oneshot: 170.0 -> 150.0 MiB/s (-11.8%, threshold -8.5%)
REGRESSION silesia/mr/gzip/l6/streaming: 168.3 -> 151.0 MiB/s (-10.3%, threshold -6.5%)
REGRESSION silesia/mr/gzip/l9/oneshot: 176.2 -> 162.1 MiB/s (-8.0%, threshold -4.8%)
REGRESSION silesia/mr/gzip/l9/streaming: 177.8 -> 157.9 MiB/s (-11.2%, threshold -9.0%)
REGRESSION silesia/mr/raw/l1/oneshot: 169.7 -> 163.4 MiB/s (-3.7%, threshold -3.0%)
REGRESSION silesia/mr/raw/l1/streaming: 172.6 -> 165.4 MiB/s (-4.2%, threshold -1.1%)
REGRESSION silesia/mr/zlib/l1/oneshot: 157.8 -> 139.6 MiB/s (-11.5%, threshold -10.0%)
REGRESSION silesia/mr/zlib/l1/streaming: 159.3 -> 133.2 MiB/s (-16.4%, threshold -16.1%)
REGRESSION silesia/nci/gzip/l1/oneshot: 311.3 -> 278.4 MiB/s (-10.6%, threshold -4.1%)
REGRESSION silesia/nci/gzip/l1/streaming: 390.6 -> 356.0 MiB/s (-8.8%, threshold -1.2%)
REGRESSION silesia/nci/gzip/l6/oneshot: 395.4 -> 371.1 MiB/s (-6.1%, threshold -0.9%)
REGRESSION silesia/nci/gzip/l6/streaming: 525.6 -> 495.9 MiB/s (-5.6%, threshold -1.8%)
REGRESSION silesia/nci/gzip/l9/streaming: 555.1 -> 509.7 MiB/s (-8.2%, threshold -2.7%)
REGRESSION silesia/nci/raw/l1/oneshot: 346.2 -> 287.4 MiB/s (-17.0%, threshold -9.9%)
REGRESSION silesia/nci/raw/l1/streaming: 474.1 -> 380.7 MiB/s (-19.7%, threshold -13.2%)
REGRESSION silesia/nci/raw/l6/streaming: 711.6 -> 663.9 MiB/s (-6.7%, threshold -2.0%)
REGRESSION silesia/nci/raw/l9/oneshot: 431.5 -> 404.9 MiB/s (-6.2%, threshold -5.2%)
REGRESSION silesia/nci/raw/l9/streaming: 768.5 -> 693.9 MiB/s (-9.7%, threshold -3.7%)
REGRESSION silesia/nci/zlib/l1/streaming: 425.2 -> 391.9 MiB/s (-7.8%, threshold -7.2%)
REGRESSION silesia/nci/zlib/l6/oneshot: 355.1 -> 340.8 MiB/s (-4.0%, threshold -0.4%)
REGRESSION silesia/nci/zlib/l6/streaming: 588.3 -> 558.0 MiB/s (-5.2%, threshold -2.7%)
REGRESSION silesia/nci/zlib/l9/oneshot: 378.8 -> 328.2 MiB/s (-13.4%, threshold -7.2%)
REGRESSION silesia/nci/zlib/l9/streaming: 624.4 -> 542.1 MiB/s (-13.2%, threshold -4.3%)
REGRESSION silesia/ooffice/gzip/l1/oneshot: 118.8 -> 113.0 MiB/s (-4.8%, threshold -4.1%)
REGRESSION silesia/ooffice/gzip/l6/oneshot: 139.6 -> 132.3 MiB/s (-5.2%, threshold -0.9%)
REGRESSION silesia/ooffice/gzip/l6/streaming: 138.6 -> 130.3 MiB/s (-5.9%, threshold -1.2%)
REGRESSION silesia/ooffice/gzip/l9/oneshot: 138.0 -> 131.2 MiB/s (-5.0%, threshold -2.9%)
REGRESSION silesia/ooffice/raw/l1/oneshot: 124.2 -> 120.2 MiB/s (-3.2%, threshold -2.9%)
REGRESSION silesia/ooffice/raw/l1/streaming: 125.0 -> 120.3 MiB/s (-3.7%, threshold -1.1%)
REGRESSION silesia/ooffice/zlib/l1/oneshot: 120.3 -> 115.3 MiB/s (-4.2%, threshold -1.0%)
REGRESSION silesia/ooffice/zlib/l1/streaming: 119.4 -> 113.3 MiB/s (-5.1%, threshold -1.3%)
REGRESSION silesia/ooffice/zlib/l6/oneshot: 141.6 -> 110.9 MiB/s (-21.7%, threshold -14.7%)
REGRESSION silesia/ooffice/zlib/l9/oneshot: 141.3 -> 132.6 MiB/s (-6.1%, threshold -5.1%)
REGRESSION silesia/ooffice/zlib/l9/streaming: 140.7 -> 132.4 MiB/s (-5.9%, threshold -2.7%)
REGRESSION silesia/osdb/gzip/l1/oneshot: 173.3 -> 153.7 MiB/s (-11.4%, threshold -7.1%)
REGRESSION silesia/osdb/gzip/l1/streaming: 172.3 -> 155.6 MiB/s (-9.7%, threshold -5.9%)
REGRESSION silesia/osdb/gzip/l6/streaming: 200.8 -> 177.1 MiB/s (-11.8%, threshold -11.3%)
REGRESSION silesia/osdb/raw/l1/oneshot: 186.1 -> 152.2 MiB/s (-18.2%, threshold -2.9%)
REGRESSION silesia/osdb/raw/l6/streaming: 222.0 -> 192.8 MiB/s (-13.1%, threshold -6.6%)
REGRESSION silesia/osdb/zlib/l1/oneshot: 177.9 -> 170.3 MiB/s (-4.3%, threshold -1.8%)
REGRESSION silesia/osdb/zlib/l1/streaming: 178.9 -> 168.1 MiB/s (-6.1%, threshold -5.2%)
REGRESSION silesia/osdb/zlib/l6/oneshot: 208.4 -> 193.9 MiB/s (-7.0%, threshold -2.4%)
REGRESSION silesia/osdb/zlib/l6/streaming: 209.6 -> 193.7 MiB/s (-7.6%, threshold -3.3%)
REGRESSION silesia/osdb/zlib/l9/oneshot: 206.7 -> 189.2 MiB/s (-8.4%, threshold -6.5%)
REGRESSION silesia/osdb/zlib/l9/streaming: 209.9 -> 186.2 MiB/s (-11.3%, threshold -8.7%)
REGRESSION silesia/reymont/gzip/l1/oneshot: 171.3 -> 159.8 MiB/s (-6.7%, threshold -4.1%)
REGRESSION silesia/reymont/gzip/l1/streaming: 173.3 -> 158.4 MiB/s (-8.6%, threshold -1.2%)
REGRESSION silesia/reymont/gzip/l6/oneshot: 216.6 -> 190.2 MiB/s (-12.2%, threshold -7.6%)
REGRESSION silesia/reymont/gzip/l6/streaming: 212.5 -> 197.3 MiB/s (-7.2%, threshold -1.3%)
REGRESSION silesia/reymont/gzip/l9/oneshot: 222.0 -> 206.5 MiB/s (-6.9%, threshold -4.7%)
REGRESSION silesia/reymont/gzip/l9/streaming: 219.5 -> 208.5 MiB/s (-5.0%, threshold -2.4%)
REGRESSION silesia/reymont/raw/l1/oneshot: 182.9 -> 172.2 MiB/s (-5.8%, threshold -2.9%)
REGRESSION silesia/reymont/raw/l1/streaming: 185.0 -> 169.8 MiB/s (-8.2%, threshold -7.1%)
REGRESSION silesia/reymont/raw/l6/oneshot: 249.1 -> 224.4 MiB/s (-9.9%, threshold -7.0%)
REGRESSION silesia/reymont/raw/l6/streaming: 247.4 -> 218.1 MiB/s (-11.8%, threshold -10.8%)
REGRESSION silesia/reymont/raw/l9/oneshot: 247.1 -> 230.7 MiB/s (-6.6%, threshold -5.2%)
REGRESSION silesia/reymont/zlib/l6/oneshot: 226.9 -> 221.9 MiB/s (-2.2%, threshold -1.8%)
REGRESSION silesia/reymont/zlib/l6/streaming: 227.7 -> 216.9 MiB/s (-4.7%, threshold -2.5%)
REGRESSION silesia/reymont/zlib/l9/oneshot: 229.1 -> 209.4 MiB/s (-8.6%, threshold -5.1%)
REGRESSION silesia/reymont/zlib/l9/streaming: 230.6 -> 214.8 MiB/s (-6.8%, threshold -1.4%)
REGRESSION silesia/samba/gzip/l1/oneshot: 222.6 -> 208.9 MiB/s (-6.2%, threshold -4.1%)
REGRESSION silesia/samba/gzip/l1/streaming: 223.1 -> 212.5 MiB/s (-4.7%, threshold -1.2%)
REGRESSION silesia/samba/gzip/l6/oneshot: 254.6 -> 236.2 MiB/s (-7.2%, threshold -3.1%)
REGRESSION silesia/samba/gzip/l6/streaming: 254.2 -> 240.3 MiB/s (-5.4%, threshold -1.2%)
REGRESSION silesia/samba/gzip/l9/streaming: 258.6 -> 249.0 MiB/s (-3.7%, threshold -3.0%)
REGRESSION silesia/samba/raw/l1/oneshot: 249.2 -> 236.9 MiB/s (-5.0%, threshold -2.9%)
REGRESSION silesia/samba/raw/l1/streaming: 251.3 -> 237.9 MiB/s (-5.4%, threshold -3.4%)
REGRESSION silesia/samba/raw/l6/streaming: 297.4 -> 266.9 MiB/s (-10.3%, threshold -6.5%)
REGRESSION silesia/samba/raw/l9/oneshot: 300.7 -> 285.0 MiB/s (-5.2%, threshold -5.2%)
REGRESSION silesia/samba/raw/l9/streaming: 304.7 -> 282.9 MiB/s (-7.1%, threshold -3.7%)
REGRESSION silesia/samba/zlib/l1/oneshot: 228.6 -> 215.2 MiB/s (-5.9%, threshold -1.8%)
REGRESSION silesia/samba/zlib/l1/streaming: 234.7 -> 220.2 MiB/s (-6.2%, threshold -1.3%)
REGRESSION silesia/samba/zlib/l6/oneshot: 270.3 -> 241.7 MiB/s (-10.6%, threshold -1.0%)
REGRESSION silesia/samba/zlib/l6/streaming: 271.7 -> 260.8 MiB/s (-4.0%, threshold -3.0%)
REGRESSION silesia/samba/zlib/l9/oneshot: 278.6 -> 225.2 MiB/s (-19.2%, threshold -13.4%)
REGRESSION silesia/samba/zlib/l9/streaming: 278.5 -> 225.6 MiB/s (-19.0%, threshold -13.6%)
REGRESSION silesia/sao/gzip/l1/oneshot: 101.7 -> 96.7 MiB/s (-5.0%, threshold -4.1%)
REGRESSION silesia/sao/gzip/l1/streaming: 103.2 -> 98.2 MiB/s (-4.8%, threshold -1.2%)
REGRESSION silesia/sao/gzip/l6/oneshot: 114.2 -> 108.2 MiB/s (-5.2%, threshold -4.0%)
REGRESSION silesia/sao/gzip/l9/streaming: 115.2 -> 110.0 MiB/s (-4.5%, threshold -2.4%)
REGRESSION silesia/sao/raw/l1/oneshot: 105.1 -> 101.1 MiB/s (-3.8%, threshold -2.9%)
REGRESSION silesia/sao/zlib/l1/oneshot: 101.7 -> 96.0 MiB/s (-5.6%, threshold -3.0%)
REGRESSION silesia/sao/zlib/l1/streaming: 104.9 -> 96.3 MiB/s (-8.2%, threshold -3.7%)
REGRESSION silesia/sao/zlib/l6/oneshot: 115.2 -> 109.2 MiB/s (-5.2%, threshold -4.0%)
REGRESSION silesia/sao/zlib/l6/streaming: 116.6 -> 111.5 MiB/s (-4.4%, threshold -2.5%)
REGRESSION silesia/sao/zlib/l9/streaming: 113.9 -> 109.2 MiB/s (-4.1%, threshold -1.4%)
REGRESSION silesia/webster/gzip/l1/oneshot: 152.6 -> 131.7 MiB/s (-13.7%, threshold -12.5%)
REGRESSION silesia/webster/gzip/l1/streaming: 164.0 -> 151.6 MiB/s (-7.6%, threshold -4.2%)
REGRESSION silesia/webster/gzip/l6/oneshot: 186.2 -> 172.1 MiB/s (-7.6%, threshold -2.4%)
REGRESSION silesia/webster/gzip/l6/streaming: 207.0 -> 182.6 MiB/s (-11.8%, threshold -5.2%)
REGRESSION silesia/webster/gzip/l9/streaming: 211.5 -> 198.3 MiB/s (-6.3%, threshold -2.4%)
REGRESSION silesia/webster/raw/l1/oneshot: 158.8 -> 150.9 MiB/s (-5.0%, threshold -2.9%)
REGRESSION silesia/webster/raw/l1/streaming: 183.1 -> 176.3 MiB/s (-3.7%, threshold -1.9%)
REGRESSION silesia/webster/zlib/l6/oneshot: 189.6 -> 178.6 MiB/s (-5.8%, threshold -0.9%)
REGRESSION silesia/webster/zlib/l6/streaming: 212.7 -> 203.6 MiB/s (-4.2%, threshold -0.5%)
REGRESSION silesia/webster/zlib/l9/streaming: 208.6 -> 193.1 MiB/s (-7.4%, threshold -6.1%)
REGRESSION silesia/x-ray/gzip/l9/streaming: 103.5 -> 93.1 MiB/s (-10.0%, threshold -2.4%)
REGRESSION silesia/x-ray/raw/l1/streaming: 109.6 -> 93.2 MiB/s (-15.0%, threshold -1.6%)
REGRESSION silesia/x-ray/raw/l6/streaming: 109.2 -> 104.4 MiB/s (-4.4%, threshold -3.5%)
REGRESSION silesia/xml/raw/l6/oneshot: 521.0 -> 447.0 MiB/s (-14.2%, threshold -11.7%)
REGRESSION silesia/xml/raw/l6/streaming: 525.1 -> 453.7 MiB/s (-13.6%, threshold -7.2%)
REGRESSION silesia/xml/zlib/l9/oneshot: 458.4 -> 370.7 MiB/s (-19.1%, threshold -10.4%)
REGRESSION silesia/xml/zlib/l9/streaming: 469.2 -> 392.4 MiB/s (-16.4%, threshold -12.1%)

#

### Targets at start (`make bench-targets J=docs/perf/baselines/phase3-start.json NOISE=baselines/phase3-noise.json`)

- T1 (Silesia streaming >= 0.80x C zlib streaming): NOT MET; worst: silesia/gzip/l1/streaming ratio 0.543 (threshold >= 0.80)
- T2 (cleanflate one-shot >= streaming, every row): NOT MET; worst: silesia/zlib/l9 one-shot / streaming 0.884 (threshold >= 1.00)
- T3 (Canterbury >= 0.70x C zlib, every mode): NOT MET; worst: canterbury/raw/l1/streaming ratio 0.496 (threshold >= 0.70)
- T4 (zlib and gzip within 3% of raw throughput): NOT MET; worst: silesia/zlib/l9/oneshot overhead 12.63% (threshold <= 3%)
- T5 (Silesia level-6 one-shot >= 0.70x C zlib): NOT MET; worst: silesia/raw/l6/oneshot ratio 0.649 (threshold >= 0.70)
- T6 (no row slower than the phase-2 baseline beyond noise): NOT MET; worst: silesia/gzip/l1/oneshot -9.89% vs baseline

The full list of failing rows is reproduced by the command above.

## Profile
See `docs/perf/phase3-profile.md`: before-profiles for the three required keys and six more, the four SPEC section 1 patterns with verdicts (1, 2 and 3 ADJUSTED, 4 CONFIRMED), and the hotspot and likely gate outcome of every later optimisation task. Before-flamegraphs: `docs/perf/flamegraphs/*-before.svg`.

## Task log
Every task appends `### <ID>`.

### P0-01
Fuzz gate targets (`fuzz-smoke-decoder`, `fuzz-milestone`), `bench-quick` levels 1 and 6, phase-2 baseline copy, report skeleton.

### P0-02
`design-check` requires a `Profile:` line in `[perf]` DESIGN entries from D-34.

### P0-03
Oracle FFI: `zlib_compile_flags`, bounded `inflate_streaming_into`.

### P0-04
Bench schema 2: `best` replaces `min`; every column documented in `docs/perf/README.md`.

### P0-05
C zlib streaming rows (64 KiB chunks); aggregate ratios divide by the same-mode C zlib figure.

### P0-06
Environment capture in every JSON and at the top of stdout.

### P0-07
Comparison columns: vs phase-2, like-for-like ratio, ratio delta and verdict; per-file rows with sizes.

### P0-08
`oracle bench-targets`: T1 to T6 verdicts from one bench JSON.

### P0-09
Phase-3 starting baseline measured twice (full and quick) plus noise files; sections above filled. Noise exceeds 3% (8.22% full, 8.82% quick): manual check. Reruns show no WIN, LOSS or REGRESSION against each other.

### P0-10 (M0 acceptance)

Gates run in this order: `make ci` (pass), `make heavy` (pass: proptest-10k, bomb, conformance-full, fastslow-full), `make fuzz-milestone` (pass), then the full bench `docs/perf/phase3/P0-10.json`.

M0 checklist:

- [x] Phase-3 start baseline and noise are committed: `docs/perf/baselines/phase3-start.json`, `phase3-start-rerun.json`, `phase3-noise.json`, `phase3-start-quick.json`, `phase3-start-quick-rerun.json`, `phase3-noise-quick.json`.
- [x] Every streaming row has a C zlib streaming counterpart: `docs/perf/phase3/P0-10.json` has 207 cleanflate streaming rows and 207 C zlib streaming rows.
- [x] `make bench-compare` shows deltas against the phase-2 baseline (header and first rows, full table by `make bench-compare A=docs/perf/baselines/phase2.json B=docs/perf/phase3/P0-10.json`):

```
| key | A MiB/s | B MiB/s | delta % | 2x noise % | verdict | vs phase-2 % | B ratio | ratio delta % | ratio verdict |
|---|---:|---:|---:|---:|---|---:|---:|---:|---|
| canterbury/gzip/l1/oneshot | 190.6 | 184.8 | -3.03 | 0.07 | LOSS | -3.03 | 0.519 | -5.64 | NEUTRAL |
| canterbury/gzip/l1/streaming | 187.9 | 182.9 | -2.68 | 1.14 | LOSS | -2.68 | 0.507 | n/a (denominator changed) | n/a |
| canterbury/gzip/l6/oneshot | 218.2 | 213.6 | -2.12 | 2.94 | NEUTRAL | -2.12 | 0.582 | -6.74 | NEUTRAL |
```

- [x] Fuzz milestone runs (`make fuzz-milestone`, 1800 s target each, run concurrently):

```
fuzz-milestone: diff_oracle_roundtrip seconds=1826 execs=1205957 exit=0 artifacts=0
fuzz-milestone: diff_fast_reference seconds=1826 execs=894969 exit=0 artifacts=0
```

No code changed between `phase3-start` and this run (Phase 0 changed tooling only), so any WIN or LOSS between them is measurement drift. `make bench-compare A=baselines/phase3-start.json B=phase3/P0-10.json` reports 14 WIN or LOSS aggregate lines; two runs from the noise pair under-estimate run-to-run drift on this shared machine (see the noise warning above).

Targets at M0 (`make bench-targets J=docs/perf/phase3/P0-10.json`), all reported, none gating:

- T1 (Silesia streaming >= 0.80x C zlib streaming): NOT MET; worst: silesia/raw/l1/streaming ratio 0.546 (threshold >= 0.80)
- T2 (cleanflate one-shot >= streaming, every row): NOT MET; worst: silesia/raw/l6 one-shot / streaming 0.879 (threshold >= 1.00)
- T3 (Canterbury >= 0.70x C zlib, every mode): NOT MET; worst: canterbury/raw/l1/streaming ratio 0.499 (threshold >= 0.70)
- T4 (zlib and gzip within 3% of raw throughput): NOT MET; worst: canterbury/gzip/l9/streaming overhead 17.40% (threshold <= 3%)
- T5 (Silesia level-6 one-shot >= 0.70x C zlib): NOT MET; worst: silesia/raw/l6/oneshot ratio 0.639 (threshold >= 0.70)
- T6 (no row slower than the phase-2 baseline beyond noise): NOT MET; worst: canterbury/zlib/l1/oneshot -8.58% vs baseline

### P1-01
Per-key profiling (`PROFILE_KEY`), `profile-stat`, checked `flamegraph-save`, `profile-required`. Flamegraphs use `--no-inline` so frames carry crate paths.

### P1-02
Before-profiles and flamegraphs; `docs/perf/phase3-profile.md`. Headline: `fast_loop::run` plus the out-of-line `Huffman::decode_table` are 70% of self time at level 1; level 1 is symbol-count heavy, not literal-heavy (literals 46.6% of symbols against 60.8% at level 6); one-shot's gap is fresh-page zero-fill and faults, not growth; CRC-32 is 9.6% of gzip, Adler-32 plus wrapper 5.1% of zlib.

### P1-03 (M1 acceptance)

Gates run in order: `make ci` (pass), `make heavy` (pass), `make fuzz-milestone` (pass), then the full bench `docs/perf/phase3/P1-03.json`.

M1 checklist:

- [x] Before-flamegraphs committed: `docs/perf/flamegraphs/silesia-rfc1951-l1-oneshot-before.svg`, `silesia-rfc1951-l1-streaming-before.svg`, `canterbury-rfc1952-l1-oneshot-before.svg` (all pass `tools/perf/check-flamegraph.sh`).
- [x] `docs/perf/phase3-profile.md` has `How these were taken`, `Before (P1-02)` (per-key tables, inclusive frames, `perf stat`), `Patterns` (four verdicts with numbers) and `Hotspots by task`.
- [x] The hotspot table covers every gated or optimisation task of M2 to M4: P2-02, P2-03, P2-04, P2-07, P2-08, P3-01, P3-02, P4-02, P4-03, P4-04.
- [x] `docs/perf/phase3/P1-03.json` is committed.

Fuzz milestone:

```
fuzz-milestone: diff_oracle_roundtrip seconds=1801 execs=1383630 exit=0 artifacts=0
fuzz-milestone: diff_fast_reference seconds=1801 execs=525087 exit=0 artifacts=0
```

`make bench-compare A=baselines/phase3-start.json B=phase3/P1-03.json` (noise `phase3-noise.json`): 9 WIN, 11 LOSS, 65 per-file REGRESSION lines. Against the phase-2 record (first rows; the full table is `make bench-compare A=docs/perf/baselines/phase2.json B=docs/perf/phase3/P1-03.json`):

```
| key | A MiB/s | B MiB/s | delta % | 2x noise % | verdict | vs phase-2 % | B ratio | ratio delta % | ratio verdict |
|---|---:|---:|---:|---:|---|---:|---:|---:|---|
| canterbury/gzip/l1/oneshot | 190.6 | 174.1 | -8.63 | 0.07 | LOSS | -8.63 | 0.515 | -6.35 | NEUTRAL |
| canterbury/gzip/l1/streaming | 187.9 | 174.0 | -7.39 | 1.14 | LOSS | -7.39 | 0.500 | n/a (denominator changed) | n/a |
| canterbury/gzip/l6/oneshot | 218.2 | 203.5 | -6.73 | 2.94 | LOSS | -6.73 | 0.582 | -6.86 | NEUTRAL |
```

Targets (`make bench-targets J=docs/perf/phase3/P1-03.json`), reported, not gates:

- T1 (Silesia streaming >= 0.80x C zlib streaming): NOT MET; worst: silesia/gzip/l1/streaming ratio 0.552 (threshold >= 0.80)
- T2 (cleanflate one-shot >= streaming, every row): NOT MET; worst: silesia/raw/l6 one-shot / streaming 0.891 (threshold >= 1.00)
- T3 (Canterbury >= 0.70x C zlib, every mode): NOT MET; worst: canterbury/gzip/l1/streaming ratio 0.500 (threshold >= 0.70)
- T4 (zlib and gzip within 3% of raw throughput): NOT MET; worst: canterbury/gzip/l1/streaming overhead 13.22% (threshold <= 3%)
- T5 (Silesia level-6 one-shot >= 0.70x C zlib): NOT MET; worst: silesia/raw/l6/oneshot ratio 0.641 (threshold >= 0.70)
- T6 (no row slower than the phase-2 baseline beyond noise): NOT MET; worst: canterbury/gzip/l9/oneshot -9.76% vs baseline

### P2-01
Checksum microbenchmark (`make bench-checksum`, example `checksum_bench`, cleanflate only) and the variants behind `cleanflate::test_hooks::checksum`: CRC-32 bytewise, sliced 4, 8, 16 and production; Adler-32 bytewise and production. The CRC-32 slice tables are now one set of 16 (`CRC32_MAX_SLICES`; table `k` does not depend on the width) and `Crc32::update` calls the generic `crc32_update_sliced::<CRC32_SLICES>`; the production width is still 8.

Two runs (`docs/perf/phase3/P2-01-checksum.json`, `P2-01-checksum-rerun.json`), 7 samples of at least 256 MiB per row. The per-row difference between the runs is the microbench noise: mean 2.9%, maximum 7.6%.

| algo | variant | size | run 1 median MiB/s | run 2 median MiB/s | diff % |
|---|---|---:|---:|---:|---:|
| crc32 | bytewise | 64 | 436 | 407 | -6.7 |
| crc32 | bytewise | 4096 | 428 | 437 | +2.0 |
| crc32 | bytewise | 65536 | 432 | 439 | +1.5 |
| crc32 | bytewise | 1048576 | 439 | 438 | -0.3 |
| crc32 | sliced4 | 64 | 1134 | 1155 | +1.8 |
| crc32 | sliced4 | 4096 | 1248 | 1153 | -7.6 |
| crc32 | sliced4 | 65536 | 1195 | 1236 | +3.4 |
| crc32 | sliced4 | 1048576 | 1174 | 1230 | +4.8 |
| crc32 | sliced8 | 64 | 1773 | 1810 | +2.0 |
| crc32 | sliced8 | 4096 | 1852 | 1894 | +2.3 |
| crc32 | sliced8 | 65536 | 1843 | 1868 | +1.4 |
| crc32 | sliced8 | 1048576 | 1803 | 1709 | -5.2 |
| crc32 | sliced16 | 64 | 2183 | 2187 | +0.2 |
| crc32 | sliced16 | 4096 | 2092 | 2210 | +5.7 |
| crc32 | sliced16 | 65536 | 2131 | 1979 | -7.1 |
| crc32 | sliced16 | 1048576 | 2139 | 2167 | +1.3 |
| crc32 | production | 64 | 1765 | 1819 | +3.1 |
| crc32 | production | 4096 | 1797 | 1876 | +4.4 |
| crc32 | production | 65536 | 1833 | 1880 | +2.5 |
| crc32 | production | 1048576 | 1823 | 1875 | +2.8 |
| adler32 | bytewise | 64 | 339 | 324 | -4.6 |
| adler32 | bytewise | 4096 | 348 | 344 | -1.1 |
| adler32 | bytewise | 65536 | 345 | 336 | -2.6 |
| adler32 | bytewise | 1048576 | 347 | 344 | -1.0 |
| adler32 | production | 64 | 2449 | 2530 | +3.3 |
| adler32 | production | 4096 | 3041 | 3036 | -0.2 |
| adler32 | production | 65536 | 3093 | 3038 | -1.8 |
| adler32 | production | 1048576 | 3052 | 3066 | +0.4 |

Reading: production CRC-32 (8 slices) runs at about 1.8 GiB/s, sliced-16 about 2.1 GiB/s (+17%), sliced-4 about 1.2 GiB/s; production Adler-32 about 3.0 GiB/s against 0.35 GiB/s bytewise. At about 150 MiB/s of decode, CRC-32 at 1.8 GiB/s accounts for roughly 8% of gzip time, which matches the profile share below. These are inputs for P2-02 and P2-03, not changes.

PT1 on the production refactor (key `silesia/rfc1952/l6/streaming`, hotspot `Crc32::update`): self-time share 9.56% before (P1-02) to 9.19% after (`make profile`). `bench-quick` before/after (`docs/perf/phase3/P2-01-before.json`, `P2-01.json`) shows every key, raw included, 4% to 10% slower: the machine load average was 1.35 at the start of the before run and 4.70 at the start of the after run (a shared desktop), so the JSON comparison cannot judge the change, and its LOSS lines on gzip keys are matched by equal LOSS lines on raw keys that the change cannot touch. The controlled check is an interleaved A/B of the old and new `profile_decode` binaries on `silesia/rfc1952/l6/streaming` (4 alternating pairs, MiB/s): before 148.4, 146.2, 154.2, 135.2; after 150.0, 152.8, 156.6, 146.2. After is at least as fast in all four pairs (mean 151.4 against 146.0, +3.7%), so the refactor is neutral, not a LOSS.

### P2-02
**Negative result (gated out, PT2). No code change.**

Frames: `Adler32::update` (inlined as `adler_fold` inside `ZlibDecoder::step`). Fresh profiles, `make profile`:

| key | frame | share (inclusive, `top-children.txt`) | `ZlibDecoder::step` self |
|---|---|---:|---:|
| `silesia/rfc1950/l6/streaming` | `adler_fold` | 6.34% | 4.37% (5.1% summed over address ranges in P1-02) |
| `canterbury/rfc1950/l1/oneshot` | `adler_fold` | 4.83% | 4.35% |

Gate: max(2%, 2 x the quick noise of `silesia/zlib/l6/streaming`) = max(2%, 2 x 3.51%) = 7.02%. Both keys are below it (6.34% and 4.83%), so the task records a negative result and changes nothing. The margin on Silesia is small, and the microbench suggests where it comes from: production Adler-32 runs at about 3.0 GiB/s (P2-01), which at about 200 MiB/s of decode is roughly 6.5% of time. Even a batched loop that doubled Adler-32 speed would return about 3% of zlib decode time, below the 7% this machine's noise lets a bench prove, so a WIN verdict could not be shown. The zlib to raw gap (T4) is therefore left to the decode loop work (P4-02 to P4-04), which speeds every framing.

### P2-03
**Gate passed; change measured and reverted (PT4). `CRC32_SLICES` stays 8.**

Gate (PT2), `make profile`: `Crc32::update` (with `crc32_update_sliced<8>` inlined) is 8.31% of samples in `silesia/rfc1952/l6/streaming` and 8.99% in `canterbury/rfc1952/l1/oneshot`, against gates of 5.14% (2 x 2.57%) and 4.84% (2 x 2.42%): both above, so the change was tried. Phase 2's whole-decode sweep found 16 slices no faster than 8 (`docs/perf/P4-03/CRC32_SLICES-16.json`, -0.9%).

Microbench (`docs/perf/phase3/P2-03-checksum.json`, fresh run; noise from the two P2-01 runs):

| size | sliced8 MiB/s | sliced16 MiB/s | gain | 2 x max row noise |
|---:|---:|---:|---:|---:|
| 4096 | 2002 | 2361 | +17.9% | 11.3% |
| 65536 | 2000 | 2308 | +15.4% | 14.2% |
| 1048576 | 1974 | 2240 | +13.5% | 10.4% |

The gain exceeds twice the microbench noise at every size, so `CRC32_SLICES = 16` was set and measured. Bounded expectation: CRC-32 is about 8.3% of gzip decode and 16 slices make it about 14% faster, so at most 1.2% of decode time comes back, four times smaller than the 5.1% this machine's noise lets a bench prove.

PT1 (`docs/perf/phase3/P2-03-before.json`, `P2-03.json`, bench-quick; hotspot share `Crc32::update` 8.31% before, re-profile not repeated after the revert): the JSON pair cannot judge the change. The before run saw the machine's load average climb from 2.9 to 8.4 (a shared desktop with a browser and a cluster service) and the after run started at 7.6, so unchanged raw keys show "WIN" deltas of up to +66%. On the targeted gzip keys the verdicts are NEUTRAL on `silesia/gzip/l6/streaming` (+4.5% throughput, ratio +2.5%, both inside 2 x noise) and a ratio LOSS on `canterbury/gzip/l1/streaming` (-3.7%); no targeted key has a clean WIN. The controlled check is five interleaved pairs of old and new `profile_decode` binaries on `silesia/rfc1952/l6/streaming` (MiB/s): before 160.2, 166.1, 165.0, 170.4, 165.8; after 169.4, 174.3, 170.8, 162.4, 169.6 (mean 165.5 to 169.3, +2.3%, four of five pairs faster), which is consistent with the bounded +1.2% and inside the pair-to-pair spread.

PT4: neither a WIN verdict nor a named prerequisite of a later task, so the value is reverted to 8. The microbench numbers, the tests (`crc32_sliced_variants_match_bytewise` covers width 16) and the fixtures stay. Width 16 remains a one-line change if the checksum share grows after the M4 loop work shrinks everything else.

### P2-04
**Negative result (gated out, PT2). No code change.**

Named frames: wrapper `step` time excluding its inlined checksum frames (inclusive `ZlibDecoder::step` minus inclusive `adler_fold`, from `top-children.txt` of fresh `make profile` runs; `GzipDecoder::step` does not reach the profile's 0.5% display floor and its checksum, `Crc32::update`, is a separate symbol):

| key | wrapper `step` inclusive | checksum inclusive | wrapper excess |
|---|---:|---:|---:|
| `silesia/rfc1950/l6/streaming` | 6.39% | 6.34% (`adler_fold`) | 0.05% |
| `silesia/rfc1952/l6/streaming` | below 0.5% (not listed) | 8.31% (`Crc32::update`) | below 0.5% |
| `canterbury/rfc1950/l1/streaming` | 5.45% | 5.41% (`adler_fold`) | 0.04% |
| `canterbury/rfc1952/l1/streaming` | below 0.5% (not listed) | 8.21% (`Crc32::update`) | below 0.5% |

Gate: max(2%, 2 x quick noise of the key); the floor alone is 2%, and every key is under it by a wide margin. The wrapper's own work (header and trailer state, output bookkeeping, the per-call state handling) is unmeasurable beyond the checksum it calls, and `Inflater::step` underneath is 0.7% to 1.2% inclusive. Pattern 4's gap in streaming mode is therefore checksum cost (see P2-02 and P2-03), not per-call wrapper overhead, and no code changes.

### P2-05
Output-sink abstraction (`sink.rs`: `OutputSink`, `SliceSink`; DESIGN D-34): the state machine, fast loop, window copy and both wrappers write through a generic, monomorphised sink; only the slice sink exists. Pure refactor; test-only slice-shaped wrappers (`Inflater::step`, `ZlibDecoder::step`, `GzipDecoder::step`, `Window::copy_match`) keep the existing unit tests unchanged, and the production entry points are `step_into` and `copy_match_sink`.

PT1 (prerequisite of P2-06 and P2-07, so the bar is NEUTRAL or WIN). Hotspot key `silesia/rfc1951/l1/oneshot`, `make profile`: before, `fast_loop::run` 41.79% + `Huffman::decode_table` 28.53% + `Window::copy_match` 12.75% = 83.07% of samples; after, `fast_loop::run_loop` 51.76% (the match copy is now inlined into it) + `decode_table` 30.79% = 82.55%. The hot functions and their combined share are unchanged.

bench-quick (`docs/perf/phase3/P2-05-before.json`, `P2-05.json`; load average at start 3.08 and 4.75, a shared desktop): most keys are NEUTRAL; the LOSS lines are scattered over raw, zlib and gzip keys alike (for example raw is -3.3% and zlib -9.5% on `silesia/*/l6/streaming`) and follow the load, not a code path. The controlled check is interleaved runs of the old and new `profile_decode` binaries (MiB/s, means over 5 to 6 alternating pairs):

| key | before | after | change | after faster in |
|---|---:|---:|---:|---:|
| silesia/rfc1951 l1 oneshot (2 passes) | 150.9 | 152.0 | +0.7% | 3 of 5 |
| silesia/rfc1951 l1 streaming | 161.1 | 163.5 | +1.5% | 4 of 5 |
| silesia/rfc1952 l6 streaming | 157.7 | 166.2 | +5.4% | 5 of 5 |
| canterbury/rfc1951 l1 oneshot (200 passes) | 185.9 | 195.4 | +5.1% | 5 of 5 |
| silesia/rfc1950 l6 streaming | 164.7 | 171.5 | +4.1% | 4 of 6 |
| silesia/rfc1950 l6 oneshot | 164.0 | 170.3 | +3.8% | 6 of 6 |

No key is slower on average and every key is at least as fast in most pairs, so the refactor is NEUTRAL (the small positive means are within run-to-run spread) and stays as the prerequisite of P2-06. `make fastslow-full` and `make bomb` pass; `size_of::<Decompressor>()` is unchanged (`decompressor_size_not_grown` passes).

### P2-07
**Measured and reverted (PT4): the public one-shot path stays on the slice driver.** P2-05 (sink abstraction), P2-06 (`VecSink`, `decompress_core_vec`, the sink differential) and the new `oneshot_sizing` tests stay; only the `stream.rs` switch of the public entry points to the Vec driver was undone. SPEC's negative result applies: on this machine a sink design that appends to a `Vec` cannot beat the current path.

Hotspot and evidence. Pattern 1's candidate cost is the zero-fill (`resize<u8>`/`extend_with<u8>`, 2.06% inclusive in `silesia/rfc1951/l1/oneshot`, P1-02) plus first-touch page faults. `perf stat` (`make profile-stat`), before and after the switch:

| key | page faults before | after | task-clock before | after |
|---|---:|---:|---:|---:|
| `silesia/rfc1951/l1/oneshot` | 110,179 | 110,179 | 3,701 ms | 3,679 ms |
| `silesia/rfc1951/l6/oneshot` | 110,749 | 110,748 | 3,249 ms | 3,230 ms |

The fault count does not move at all: appending to fresh capacity touches exactly the pages that zero-filling touched, so the kernel's first-touch cost (the part of pattern 1 that user-space samples cannot see) is not something a sink can remove, only a reused buffer can (M3). Memory is unchanged by design: `oneshot_resizes` and `oneshot_bytes_allocated` (`make bench-stats STATS_CORPUS=silesia,canterbury STATS_LEVELS=1,6`) are identical before and after: Silesia level 1, 14 resizes and 316,146,272 bytes; level 6, 15 and 304,788,200; Canterbury level 1, 13 and 4,753,344; level 6, 14 and 4,492,008. After the switch the profile shows `extend_from_within` at 13.37% inclusive (`spec_extend_from_within` 9.91%) in the same key: the safe-Rust match copy into a `Vec` is slightly costlier than `copy_within` on a slice, which offsets the removed zero-fill.

PT1 (`docs/perf/phase3/P2-07-before.json`, `P2-07.json`, bench-quick): the JSON pair is load-polluted again (Canterbury raw and gzip one-shot keys show -6% to -12% together), so the controlled check is interleaved runs of the old and new `profile_decode` binaries (MiB/s, means over 6 alternating pairs):

| key | before | after | change | after faster in |
|---|---:|---:|---:|---:|
| silesia/rfc1951 l1 oneshot | 164.2 | 160.4 | -2.3% | 2 of 6 |
| silesia/rfc1951 l6 oneshot | 181.5 | 187.7 | +3.4% | 5 of 6 |
| silesia/rfc1952 l6 oneshot | 175.2 | 175.1 | -0.0% | 4 of 6 |
| canterbury/rfc1951 l1 oneshot | 200.6 | 203.8 | +1.6% | 3 of 6 |
| canterbury/rfc1952 l1 oneshot | 182.9 | 180.1 | -1.6% | 3 of 6 |

The one-shot keys the switch was meant to help are within run-to-run spread (-2.3% to +3.0% on Silesia, both signs on Canterbury), with Silesia level 1 slightly negative and level 6 slightly positive; none is a WIN, and the quick-bench verdicts on the one-shot keys are LOSS or NEUTRAL except `silesia/zlib/l6/oneshot`. PT4 therefore reverts the switch. The zero-fill is not where the one-shot gap to streaming (T2) lives on this machine; that gap is the fresh-allocation faults, which M3's buffer reuse can address.

### P2-08
**Negative result (gated out, PT2). No code change.**

Current one-shot path: the slice driver (P2-07's switch was reverted). Fresh `make profile` runs and the P1-02 stats counters:

| key | growth, realloc and copy frames | `resize<u8>` (zero-fill of the output buffer), inclusive | `oneshot_resizes` | `oneshot_bytes_allocated` / output |
|---|---|---:|---:|---:|
| `silesia/rfc1951/l1/oneshot` | none in the profile | 2.26% | 14 for 12 files | 316,146,272 / 211,938,580 = 1.49x |
| `canterbury/rfc1952/l1/oneshot` | none in the profile | 0.57% | 13 for 11 files | 4,753,344 / 2,810,784 = 1.69x |

There is no growth cost to revisit: about one allocation per file means the ratio estimate is close enough that growth steps are rare, and no reallocation or copy frame reaches the profile's display floor. What is visible is the zero-fill `resize<u8>` of the output buffer, and that is mostly the first allocation (14 resizes for 12 files), which no hint can avoid (P2-07 showed that removing the zero-fill does not change throughput or page faults either). Re-choosing `ONESHOT_INITIAL_RATIO` could only trim the over-allocation: the buffer is 1.49x the output on Silesia, so at most a third of the 2.26% zero-fill (about 0.7%) and a third of 0.57% on Canterbury are attributable to the excess. That is below the gate, max(2%, 2 x the quick noise of the key) = 2% for `silesia/raw/l1/oneshot` (noise 0.91%) and 4.84% for `canterbury/gzip/l1/oneshot` (noise 2.42%), and far below what this machine can measure. No candidate ratio was run; `ONESHOT_INITIAL_RATIO`, `ONESHOT_GROWTH_FLOOR` and D-31/D-33 stay as they are.

### P2-09 (M2 acceptance)

Gates run in order: `make ci` (pass), `make heavy` (pass), `make fuzz-milestone` (pass), then the full bench `docs/perf/phase3/P2-09.json`.

M2 checklist:

- [x] T2 and T4 reported (targets below; at the start: T2 worst `silesia/zlib/l9` one-shot / streaming 0.884, T4 worst `silesia/zlib/l9/oneshot` overhead 12.63%; now T2 worst `silesia/raw/l9` 0.902 and T4 worst `canterbury/gzip/l9/streaming` 11.51%: both still NOT MET, and unchanged within noise because no M2 technique was adopted).
- [x] Checksum microbench numbers committed: `docs/perf/phase3/P2-01-checksum.json`, `P2-01-checksum-rerun.json`, `P2-03-checksum.json` (CRC-32 production ~1.8 GiB/s, sliced-16 ~2.1 GiB/s, Adler-32 production ~3.0 GiB/s).
- [x] Checksum techniques: P2-02 (Adler-32 batch) gated out (6.34% and 4.83% against a 7.02% gate); P2-03 (CRC-32 slicing-by-16) measured and reverted by PT4 (+13% to +18% on the microbench but at most 1.2% of a whole decode); P2-04 (wrapper overhead) gated out (under 0.1% beyond the checksum).
- [x] Sink differential: `sinks_conformance_oneshot`, `sinks_malformed`, `sinks_proptest` (slice and Vec sinks, Fast and Reference) pass in `make heavy` (full-tier conformance, 7 tests in the fastslow binary) and in `make proptest-3k`; fuzz `diff_fast_reference` also compares the Vec sink.
- [x] `make bomb` passes (3 tests, including the streaming default limit and the small one-shot limit).
- [x] Memory bounds unchanged: `decompressor_size_not_grown` (`size_of::<Decompressor>()` no more than 1024 bytes above the recorded baseline) passes; the public one-shot path still uses the slice driver (P2-07 reverted), so `oneshot_bytes_allocated` is identical to P1-02 (Silesia level 1: 316,146,272 for 211,938,580 bytes of output, 14 resizes).
- [x] `docs/perf/phase3/P2-09.json` is committed.

M2 outcome in one line: the sink abstraction (P2-05) and the Vec sink with its differential (P2-06) are in; the one-shot Vec switch (P2-07) and CRC slicing-by-16 (P2-03) were measured and reverted; Adler, wrapper and size-hint tasks were gated out. No throughput change is claimed for M2.

Fuzz milestone:

```
fuzz-milestone: diff_oracle_roundtrip seconds=1802 execs=1282261 exit=0 artifacts=0
fuzz-milestone: diff_fast_reference seconds=1802 execs=391032 exit=0 artifacts=0
```

`make bench-compare A=baselines/phase3-start.json B=phase3/P2-09.json` (noise `phase3-noise.json`): 22 WIN, 0 LOSS, 29 per-file REGRESSION lines. Against the phase-2 record (first rows; the full table is `make bench-compare A=docs/perf/baselines/phase2.json B=docs/perf/phase3/P2-09.json`):

```
| key | A MiB/s | B MiB/s | delta % | 2x noise % | verdict | vs phase-2 % | B ratio | ratio delta % | ratio verdict |
|---|---:|---:|---:|---:|---|---:|---:|---:|---|
| canterbury/gzip/l1/oneshot | 190.6 | 189.9 | -0.35 | 0.07 | LOSS | -0.35 | 0.535 | -2.75 | NEUTRAL |
| canterbury/gzip/l1/streaming | 187.9 | 187.8 | -0.05 | 1.14 | NEUTRAL | -0.05 | 0.511 | n/a (denominator changed) | n/a |
| canterbury/gzip/l6/oneshot | 218.2 | 217.6 | -0.25 | 2.94 | NEUTRAL | -0.25 | 0.599 | -4.10 | NEUTRAL |
```

Targets (`make bench-targets J=docs/perf/phase3/P2-09.json`), reported, not gates:

- T1 (Silesia streaming >= 0.80x C zlib streaming): NOT MET; worst: silesia/gzip/l1/streaming ratio 0.560 (threshold >= 0.80)
- T2 (cleanflate one-shot >= streaming, every row): NOT MET; worst: silesia/raw/l9 one-shot / streaming 0.902 (threshold >= 1.00)
- T3 (Canterbury >= 0.70x C zlib, every mode): NOT MET; worst: canterbury/gzip/l1/streaming ratio 0.511 (threshold >= 0.70)
- T4 (zlib and gzip within 3% of raw throughput): NOT MET; worst: canterbury/gzip/l9/streaming overhead 11.51% (threshold <= 3%)
- T5 (Silesia level-6 one-shot >= 0.70x C zlib): NOT MET; worst: silesia/raw/l6/oneshot ratio 0.663 (threshold >= 0.70)
- T6 (no row slower than the phase-2 baseline beyond noise): NOT MET; worst: canterbury/zlib/l9/streaming -5.15% vs baseline

### P3-01
**Negative result (gated out, PT2). No code change.**

Named frames: `Huffman::from_lengths` (with `build_table` and the allocator frames under it), table clones. Fresh `make profile` runs, inclusive shares from `top-children.txt`, and the bench-stats counters for Canterbury level 1 (P1-02):

| key | `from_lengths` inclusive | of which `build_table` | allocator frames | gate (2 x quick noise of the key) |
|---|---:|---:|---:|---:|
| `canterbury/rfc1952/l1/oneshot` | 2.87% | 1.64% | under 0.6% (`resize<u8>` 0.53%) | 4.84% (noise 2.42%) |
| `canterbury/rfc1951/l1/streaming` | 3.20% | 1.77% | 0.85% (`from_elem` of a zeroed buffer) | 4.71% (noise 2.35%) |

Counters: 38 dynamic blocks, 0 fixed blocks, 114 table builds (3 per block) and `fixed_table_clones` 0 over the 11 Canterbury files, in every framing and mode. There are no fixed blocks to stop cloning for, and rebuilding the dynamic tables in place would save only the allocation part of `from_lengths`: the computation of the table entries (56,058 entries for 2.8 MB of output) remains, and that computation is most of the 2.9% to 3.2%. Both keys are under the gate, so the task records a negative result and leaves `Huffman::from_lengths` and the block-table storage unchanged.

### P3-02
**Negative result (gated out, PT2). No code change.**

Named frames on `canterbury/rfc1952/l1/oneshot` (fresh `make profile`, 300 passes over 11 files, inclusive shares from `top-children.txt`): `Inflater::new_with_path` 1.06% + 0.65% = 1.71% (two call sites; includes the two fixed-table builds and the window), `resize<u8>` 0.72%, `Window::new` (the `new` frame) 0.53%; no `GzipDecoder::new` or header-parser frame reaches the profile's display floor. Together, at most about 3.0%, and the three overlap (the window and fixed tables sit inside `new_with_path`), so the true total is lower. `perf stat` for the key: 47.18 G instructions, 696 page faults for 3,300 stream constructions, about 14.3 M instructions per file, of which the setup frames are roughly 0.4 M.

Gate: max(2%, 2 x the quick noise of `canterbury/gzip/l1/oneshot`) = 4.84%. The named frames are below it, so the task records a negative result: no `Inflater::reset`, no lazy window and no skipped table fill were tried. The 696 page faults (none per file) show that allocation of these structures is already not faulting fresh pages on every stream. `Decompressor::reset` (P3-03) is still built for its API value and for the reuse bench mode (P3-04), which measures the reuse case directly.

### P3-04
Reuse bench mode: one `Decompressor` per (file, framing, level), `reset()` before every decode, the same 64 KiB streaming loop and scratch buffer as `streaming`. Cleanflate only: `ratio` and `c_zlib_mode` are `null` for reuse aggregates (there is no like-for-like C zlib reuse row; `inflateReset` is not a permitted FFI signature), and each carries `vs_fresh_streaming` (reuse median over streaming median of the same key). `profile_decode --mode reuse` and `PROFILE_KEY=.../reuse` profile the same loop (`make profile PROFILE_KEY=canterbury/rfc1952/l1/reuse PROFILE_ITERS=50` writes its `top.txt`; `fast_loop::run_loop` 45.3% self and `decode_table` 24.4%, `Crc32::update` 6% in that profile).

Run: `docs/perf/phase3/P3-04.json` (bench-quick). Aggregates, reuse against fresh-object streaming:

| key | reuse MiB/s (median) | vs fresh streaming |
|---|---:|---:|
| silesia/raw/l1/reuse | 149.9 | 1.015 |
| silesia/raw/l6/reuse | 174.6 | 0.996 |
| silesia/zlib/l1/reuse | 143.4 | 1.006 |
| silesia/zlib/l6/reuse | 162.5 | 0.994 |
| silesia/gzip/l1/reuse | 135.9 | 0.990 |
| silesia/gzip/l6/reuse | 149.7 | 0.993 |
| canterbury/raw/l1/reuse | 201.1 | 1.008 |
| canterbury/raw/l6/reuse | 242.3 | 1.000 |
| canterbury/zlib/l1/reuse | 178.3 | 1.006 |
| canterbury/zlib/l6/reuse | 217.1 | 1.008 |
| canterbury/gzip/l1/reuse | 169.7 | 1.007 |
| canterbury/gzip/l6/reuse | 199.3 | 1.039 |

The aggregate difference is within about one percent everywhere (largest: `canterbury/gzip/l6`, +3.9%), which matches the gated-out setup tasks (P3-01, P3-02): with one decompressor reused, the fixed per-stream setup of a fresh object is not measurable. Canterbury per file, with sizes (per-file medians; differences below the noise of a per-file row, about 3% to 5% on this machine, are not meaningful):

| file | bytes | framing | level | fresh streaming MiB/s | reuse MiB/s | reuse / fresh |
|---|---:|---|---:|---:|---:|---:|
| grammar.lsp | 3721 | gzip | 1 | 66.9 | 86.3 | 1.291 |
| grammar.lsp | 3721 | gzip | 6 | 72.3 | 93.8 | 1.297 |
| grammar.lsp | 3721 | raw | 1 | 81.8 | 116.1 | 1.420 |
| grammar.lsp | 3721 | raw | 6 | 90.6 | 133.2 | 1.469 |
| grammar.lsp | 3721 | zlib | 1 | 83.2 | 109.9 | 1.320 |
| grammar.lsp | 3721 | zlib | 6 | 88.8 | 126.7 | 1.427 |
| xargs.1 | 4227 | gzip | 1 | 68.2 | 87.7 | 1.286 |
| xargs.1 | 4227 | gzip | 6 | 67.0 | 82.5 | 1.231 |
| xargs.1 | 4227 | raw | 1 | 82.2 | 109.1 | 1.326 |
| xargs.1 | 4227 | raw | 6 | 75.7 | 111.1 | 1.466 |
| xargs.1 | 4227 | zlib | 1 | 81.1 | 107.6 | 1.327 |
| xargs.1 | 4227 | zlib | 6 | 82.4 | 113.1 | 1.372 |
| fields.c | 11150 | gzip | 1 | 96.3 | 110.1 | 1.143 |
| fields.c | 11150 | gzip | 6 | 113.8 | 134.9 | 1.186 |
| fields.c | 11150 | raw | 1 | 115.0 | 139.8 | 1.215 |
| fields.c | 11150 | raw | 6 | 148.8 | 180.5 | 1.213 |
| fields.c | 11150 | zlib | 1 | 116.6 | 141.0 | 1.210 |
| fields.c | 11150 | zlib | 6 | 154.0 | 188.7 | 1.225 |
| cp.html | 24603 | gzip | 1 | 141.6 | 149.3 | 1.054 |
| cp.html | 24603 | gzip | 6 | 123.3 | 128.1 | 1.039 |
| cp.html | 24603 | raw | 1 | 160.1 | 175.4 | 1.095 |
| cp.html | 24603 | raw | 6 | 178.4 | 195.4 | 1.095 |
| cp.html | 24603 | zlib | 1 | 133.3 | 152.1 | 1.141 |
| cp.html | 24603 | zlib | 6 | 168.9 | 186.4 | 1.104 |
| sum | 38240 | gzip | 1 | 135.2 | 133.6 | 0.987 |
| sum | 38240 | gzip | 6 | 130.7 | 147.2 | 1.126 |
| sum | 38240 | raw | 1 | 131.3 | 137.1 | 1.044 |
| sum | 38240 | raw | 6 | 183.8 | 195.4 | 1.063 |
| sum | 38240 | zlib | 1 | 146.3 | 151.7 | 1.037 |
| sum | 38240 | zlib | 6 | 141.9 | 151.6 | 1.068 |
| asyoulik.txt | 125179 | gzip | 1 | 110.0 | 109.7 | 0.997 |
| asyoulik.txt | 125179 | gzip | 6 | 162.2 | 165.8 | 1.022 |
| asyoulik.txt | 125179 | raw | 1 | 142.5 | 142.0 | 0.997 |
| asyoulik.txt | 125179 | raw | 6 | 175.8 | 165.9 | 0.944 |
| asyoulik.txt | 125179 | zlib | 1 | 112.1 | 112.7 | 1.006 |
| asyoulik.txt | 125179 | zlib | 6 | 146.4 | 170.7 | 1.166 |
| alice29.txt | 152089 | gzip | 1 | 115.4 | 116.5 | 1.010 |
| alice29.txt | 152089 | gzip | 6 | 145.5 | 162.2 | 1.115 |
| alice29.txt | 152089 | raw | 1 | 153.1 | 151.5 | 0.990 |
| alice29.txt | 152089 | raw | 6 | 195.0 | 199.7 | 1.024 |
| alice29.txt | 152089 | zlib | 1 | 138.7 | 120.6 | 0.869 |
| alice29.txt | 152089 | zlib | 6 | 152.8 | 151.1 | 0.989 |
| lcet10.txt | 426754 | gzip | 1 | 123.3 | 135.9 | 1.102 |
| lcet10.txt | 426754 | gzip | 6 | 135.8 | 159.5 | 1.174 |
| lcet10.txt | 426754 | raw | 1 | 165.9 | 164.9 | 0.994 |
| lcet10.txt | 426754 | raw | 6 | 209.0 | 209.1 | 1.000 |
| lcet10.txt | 426754 | zlib | 1 | 124.8 | 124.9 | 1.001 |
| lcet10.txt | 426754 | zlib | 6 | 193.7 | 192.1 | 0.992 |
| plrabn12.txt | 481861 | gzip | 1 | 122.7 | 128.0 | 1.043 |
| plrabn12.txt | 481861 | gzip | 6 | 158.5 | 158.2 | 0.998 |
| plrabn12.txt | 481861 | raw | 1 | 131.4 | 131.4 | 1.001 |
| plrabn12.txt | 481861 | raw | 6 | 171.7 | 176.5 | 1.028 |
| plrabn12.txt | 481861 | zlib | 1 | 130.6 | 129.1 | 0.989 |
| plrabn12.txt | 481861 | zlib | 6 | 165.7 | 164.8 | 0.995 |
| ptt5 | 513216 | gzip | 1 | 327.3 | 335.9 | 1.026 |
| ptt5 | 513216 | gzip | 6 | 272.3 | 264.8 | 0.973 |
| ptt5 | 513216 | raw | 1 | 398.6 | 395.9 | 0.993 |
| ptt5 | 513216 | raw | 6 | 314.9 | 315.4 | 1.002 |
| ptt5 | 513216 | zlib | 1 | 317.0 | 333.6 | 1.052 |
| ptt5 | 513216 | zlib | 6 | 290.5 | 289.5 | 0.996 |
| kennedy.xls | 1029744 | gzip | 1 | 224.8 | 204.4 | 0.909 |
| kennedy.xls | 1029744 | gzip | 6 | 253.0 | 249.6 | 0.987 |
| kennedy.xls | 1029744 | raw | 1 | 254.0 | 259.5 | 1.022 |
| kennedy.xls | 1029744 | raw | 6 | 331.0 | 318.0 | 0.961 |
| kennedy.xls | 1029744 | zlib | 1 | 240.9 | 248.8 | 1.033 |
| kennedy.xls | 1029744 | zlib | 6 | 275.7 | 272.5 | 0.988 |

### P3-05 (M3 acceptance)

Gates run in order: `make ci` (pass), `make heavy` (pass), `make fuzz-milestone` (pass), then the full bench `docs/perf/phase3/P3-05.json`.

M3 checklist:

- [x] Reset differential passes: `crates/oracle/tests/reset.rs::reset_equals_fresh_conformance`, `reset_equals_fresh_options`, `reset_proptest`, `reset_repeated_same_result` (in `make ci`/`make test` and `make proptest-3k`), plus the unit tests `crates/cleanflate/src/tests/reset.rs::reset_preserves_path`, `reset_clears_sticky_error`, `reset_clears_trailing_and_headers`, `reset_stats_zeroed`.
- [x] Reuse against fresh-object streaming per Canterbury file: see `### P3-04` (reuse / fresh within about 1% on every aggregate; per-file table there) and the P3-05 run, whose reuse aggregates are in `docs/perf/phase3/P3-05.json`.
- [x] Per-stream setup tasks P3-01 and P3-02 were gated out (setup frames 2.9% to 3.2% and about 3.0% against gates near 4.8%); only the API (`reset`) and the bench mode were added, so no Canterbury throughput change is expected or claimed.
- [x] `docs/perf/phase3/P3-05.json` is committed.

Canterbury one-shot like-for-like ratio per file, with sizes (raw and gzip, levels 1 and 6), `phase3-start` against this run (the change is machine drift plus per-row noise of about 3% to 5%, since no decoder code changed):

| file | bytes | framing | level | ratio at start | ratio now | change |
|---|---:|---|---:|---:|---:|---:|
| grammar.lsp | 3721 | gzip | 1 | 0.273 | 0.246 | -9.8% |
| grammar.lsp | 3721 | gzip | 6 | 0.259 | 0.243 | -6.3% |
| grammar.lsp | 3721 | raw | 1 | 0.339 | 0.321 | -5.1% |
| grammar.lsp | 3721 | raw | 6 | 0.337 | 0.308 | -8.5% |
| xargs.1 | 4227 | gzip | 1 | 0.281 | 0.282 | +0.4% |
| xargs.1 | 4227 | gzip | 6 | 0.290 | 0.278 | -4.0% |
| xargs.1 | 4227 | raw | 1 | 0.344 | 0.332 | -3.6% |
| xargs.1 | 4227 | raw | 6 | 0.352 | 0.339 | -3.7% |
| fields.c | 11150 | gzip | 1 | 0.363 | 0.362 | -0.4% |
| fields.c | 11150 | gzip | 6 | 0.349 | 0.362 | +3.7% |
| fields.c | 11150 | raw | 1 | 0.406 | 0.407 | +0.2% |
| fields.c | 11150 | raw | 6 | 0.407 | 0.429 | +5.3% |
| cp.html | 24603 | gzip | 1 | 0.434 | 0.475 | +9.4% |
| cp.html | 24603 | gzip | 6 | 0.471 | 0.475 | +0.9% |
| cp.html | 24603 | raw | 1 | 0.497 | 0.524 | +5.5% |
| cp.html | 24603 | raw | 6 | 0.506 | 0.518 | +2.5% |
| sum | 38240 | gzip | 1 | 0.473 | 0.508 | +7.3% |
| sum | 38240 | gzip | 6 | 0.481 | 0.541 | +12.5% |
| sum | 38240 | raw | 1 | 0.494 | 0.547 | +10.7% |
| sum | 38240 | raw | 6 | 0.530 | 0.576 | +8.8% |
| asyoulik.txt | 125179 | gzip | 1 | 0.506 | 0.521 | +2.8% |
| asyoulik.txt | 125179 | gzip | 6 | 0.562 | 0.640 | +13.9% |
| asyoulik.txt | 125179 | raw | 1 | 0.481 | 0.566 | +17.7% |
| asyoulik.txt | 125179 | raw | 6 | 0.635 | 0.648 | +2.1% |
| alice29.txt | 152089 | gzip | 1 | 0.525 | 0.558 | +6.2% |
| alice29.txt | 152089 | gzip | 6 | 0.617 | 0.664 | +7.5% |
| alice29.txt | 152089 | raw | 1 | 0.543 | 0.569 | +4.7% |
| alice29.txt | 152089 | raw | 6 | 0.648 | 0.690 | +6.5% |
| lcet10.txt | 426754 | gzip | 1 | 0.538 | 0.573 | +6.4% |
| lcet10.txt | 426754 | gzip | 6 | 0.648 | 0.686 | +6.0% |
| lcet10.txt | 426754 | raw | 1 | 0.546 | 0.577 | +5.9% |
| lcet10.txt | 426754 | raw | 6 | 0.659 | 0.717 | +8.8% |
| plrabn12.txt | 481861 | gzip | 1 | 0.486 | 0.535 | +10.0% |
| plrabn12.txt | 481861 | gzip | 6 | 0.643 | 0.659 | +2.6% |
| plrabn12.txt | 481861 | raw | 1 | 0.484 | 0.535 | +10.6% |
| plrabn12.txt | 481861 | raw | 6 | 0.640 | 0.691 | +8.0% |
| ptt5 | 513216 | gzip | 1 | 0.590 | 0.622 | +5.4% |
| ptt5 | 513216 | gzip | 6 | 0.588 | 0.532 | -9.4% |
| ptt5 | 513216 | raw | 1 | 0.635 | 0.649 | +2.3% |
| ptt5 | 513216 | raw | 6 | 0.607 | 0.512 | -15.6% |
| kennedy.xls | 1029744 | gzip | 1 | 0.519 | 0.582 | +12.2% |
| kennedy.xls | 1029744 | gzip | 6 | 0.501 | 0.572 | +14.2% |
| kennedy.xls | 1029744 | raw | 1 | 0.489 | 0.589 | +20.4% |
| kennedy.xls | 1029744 | raw | 6 | 0.514 | 0.556 | +8.0% |

Fuzz milestone:

```
fuzz-milestone: diff_oracle_roundtrip seconds=1828 execs=1361846 exit=0 artifacts=0
fuzz-milestone: diff_fast_reference seconds=1828 execs=373554 exit=0 artifacts=0
```

`make bench-compare A=baselines/phase3-start.json B=phase3/P3-05.json` (noise `phase3-noise.json`): 12 WIN, 2 LOSS, 73 per-file REGRESSION lines. Against the phase-2 record (first rows; the full table is `make bench-compare A=docs/perf/baselines/phase2.json B=docs/perf/phase3/P3-05.json`):

```
| key | A MiB/s | B MiB/s | delta % | 2x noise % | verdict | vs phase-2 % | B ratio | ratio delta % | ratio verdict |
|---|---:|---:|---:|---:|---|---:|---:|---:|---|
| canterbury/gzip/l1/oneshot | 190.6 | 184.6 | -3.13 | 0.07 | LOSS | -3.13 | 0.560 | +1.85 | NEUTRAL |
| canterbury/gzip/l1/streaming | 187.9 | 181.4 | -3.48 | 1.14 | LOSS | -3.48 | 0.534 | n/a (denominator changed) | n/a |
| canterbury/gzip/l6/oneshot | 218.2 | 208.8 | -4.30 | 2.94 | LOSS | -4.30 | 0.611 | -2.14 | NEUTRAL |
```

Targets (`make bench-targets J=docs/perf/phase3/P3-05.json`), reported, not gates:

- T1 (Silesia streaming >= 0.80x C zlib streaming): NOT MET; worst: silesia/gzip/l1/streaming ratio 0.582 (threshold >= 0.80)
- T2 (cleanflate one-shot >= streaming, every row): NOT MET; worst: silesia/zlib/l9 one-shot / streaming 0.903 (threshold >= 1.00)
- T3 (Canterbury >= 0.70x C zlib, every mode): NOT MET; worst: canterbury/gzip/l1/streaming ratio 0.534 (threshold >= 0.70)
- T4 (zlib and gzip within 3% of raw throughput): NOT MET; worst: silesia/gzip/l6/reuse overhead 12.73% (threshold <= 3%)
- T5 (Silesia level-6 one-shot >= 0.70x C zlib): NOT MET; worst: silesia/raw/l6/oneshot ratio 0.680 (threshold >= 0.70)
- T6 (no row slower than the phase-2 baseline beyond noise): NOT MET; worst: canterbury/zlib/l9/streaming -7.83% vs baseline

### P4-01
Assembly evidence tool: `make asm` and `make asm-count` (`tools/perf/asm_branches.py`, documented in `docs/perf/README.md`, tests in `tools/perf/tests/test_asm_branches.py`). Baseline on the current code (`docs/perf/phase3/P4-01-asm.json`, `rustc 1.89.0 (29483883e 2025-08-04)`):

| row | instructions | cond branches | panic calls | panic sites |
|---|---:|---:|---:|---:|
| `cleanflate::inflate::Inflater::step_into` | 2259 | 167 | 0 | 0 |
| `src:crates/cleanflate/src/inflate.rs` | 1380 | 83 | 0 | 0 |
| `src:crates/cleanflate/src/bitreader.rs` | 582 | 39 | 0 | 0 |
| `src:crates/cleanflate/src/huffman.rs` | 507 | 40 | 0 | 0 |
| `cleanflate::window::Window::copy_match_sink` | 241 | 24 | 2 | 2 |
| `cleanflate::huffman::Huffman::decode_table` | 182 | 22 | 0 | 0 |
| `src:crates/cleanflate/src/window.rs` | 169 | 21 | 0 | 0 |
| `src:crates/cleanflate/src/fast_loop.rs` | 79 | 11 | 0 | 0 |

Findings for P4-02 and P4-03:

- The fast loop has no symbol of its own in the library assembly: it is inlined into `Inflater::step_into` (the build that `perf` profiles keeps `fast_loop::run_loop` as a separate function, so the assembly build and the profile build inline differently; the `src:` rows are the stable measure). Hence the `src:` rows for the files that make up the loop.
- The fast loop itself, `Inflater::step_into` and `Huffman::decode_table` contain **no** panic or bounds-failure call sites at all (0 and 0 above). The only two such sites anywhere in the decode path are in `Window::copy_match_sink` (2 calls at 2 sites). So the SPEC's bounds-check hypothesis (bounds checks hiding in the hot loop) does not hold for this code: its table and slice accesses already compile to branch-and-fallback code through `get`, which is the conditional-branch count, not a panic call. What remains is branch volume: `step_into` has 167 conditional branches over 2,259 instructions, `decode_table` 22 over 182, and `fast_loop.rs` alone 11 over 79.
- P4-03's measure should therefore be conditional branches and instructions per fast-loop iteration, with panic sites expected to stay 0 (and the two in `copy_match_sink` as the only ones to try to remove).

### P4-02
**Measured and reverted (PT4). `refill_bulk` stays as it is: the technique was already in place.**

Gate (PT2): `refill_bulk` is 6.10% inclusive in `silesia/rfc1951/l1/oneshot` and 5.79% in `silesia/rfc1951/l1/streaming` (P1-02 profiles), against gates of 2% (quick noise 0.91%) and 11.2% (noise 5.6%). The one-shot key is above its gate, so the change was tried.

What the task asks for is what D-27 (phase 2) already does: with at least 8 input bytes left, `refill_bulk` reads one little-endian 64-bit word, ORs `word & mask` shifted by the buffered count, and advances the position by `k = (63 - count) / 8` bytes with no per-byte loop and no data-dependent branch beyond the 8-byte availability test; only the last bytes of the input take the byte loop. The new tests `refill_word_path_all_counts` (every count 0 to 63 against input lengths 8 to 16, bits and position equal a bit-by-bit model) and `refill_never_exceeds_63_bits` pass on the existing implementation unchanged. The only variant left to try was a spelling change: a `first_chunk::<8>()` view instead of `get(..).try_from`, `count & 63` and a mask that is provably below 64 bits, so that no guard is needed.

| function (`make asm-count`, `docs/perf/phase3/P4-02-asm.json`) | instructions before | after | cond branches before | after |
|---|---:|---:|---:|---:|
| `Inflater::step_into` (fast loop inlined) | 2259 | 2303 | 167 | 167 |
| `src:bitreader.rs` | 582 | 580 | 39 | 39 |
| `Window::copy_match_sink` | 241 | 241 | 24 | 24 |
| `Huffman::decode_table` | 182 | 182 | 22 | 22 |
| `src:fast_loop.rs` | 79 | 79 | 11 | 11 |

The compiler already produced the same branch structure: the conditional branch counts are identical and no panic site changed (0 before and after in the fast loop).

PT1 (`docs/perf/phase3/P4-02-before.json`, `P4-02.json`; load averages at start 2.7 and 3.5): `silesia/raw/l1/oneshot` +0.15% (NEUTRAL, ratio -1.4% NEUTRAL) and `silesia/raw/l1/streaming` +1.2% (NEUTRAL). Interleaved runs of the old and new `profile_decode` binaries (6 alternating pairs each, MiB/s means): silesia level 1 one-shot 155.2 to 154.4, streaming 162.8 to 161.9, level 6 one-shot 181.8 to 177.9, canterbury level 1 one-shot 199.6 to 198.6: no change beyond the pair-to-pair spread. The change is neither a WIN nor a named prerequisite of a later task, so PT4 reverts it; the two tests stay.

### P4-03
Fast-loop lookup inlined on fixed-size table views (DESIGN D-36): the loop looks codes up itself in a `FastTable` (first-level size `1 << TABLE_BITS`, masked index, same rule for the second level), relies on the 48 buffered bits every iteration already guarantees, and keeps its bit buffer and tables in locals in one function. The general `decode_table`, the serial walk and the Reference path are unchanged. New tests (`crates/cleanflate/src/tests/fast_loop.rs`): `fast_loop_long_codes_via_subtable`, `fast_table_lookup_matches_decode_table`, `fast_loop_vec_and_slice_sinks_same_trace` (the requested `fast_loop_all_table_widths_same_trace` has no runtime width to vary, since the widths are config constants; the width dependence is pinned by `fast_table_lookup_matches_decode_table`, which also checks that a view exists only for the size the table was built for).

Profile (`silesia/rfc1951/l1/oneshot`, `make profile`, `make profile-stat`): before `fast_loop::run` 41.79% + `Huffman::decode_table` 28.53% + `Window::copy_match` 12.75% of samples; after a single `fast_loop::run` at 78.56%. `instructions:u` 36.36 G to 25.77 G (-29%), cycles 12.90 G to 10.23 G (-21%), task-clock 3,810 to 3,150 ms.

Assembly (`make asm`, `make asm-count`, `docs/perf/phase3/P4-03-asm.json`; before is the P4-01 baseline measured again on the unchanged code, after the new loop). The `Huffman::decode_table` symbol disappears from the library build (it is inlined into the loop, which is itself inside `Inflater::step_into`); the conditional-branch total of the decode path stays close while the instructions per symbol fall, and panic sites stay at 0 in the loop (the two sites in `Window::copy_match_sink` are unchanged):

| function or source | instructions before | after | cond branches before | after | panic calls before | after | panic sites before | after |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `cleanflate::inflate::Inflater::step_into` | 2259 | 2394 | 167 | 172 | 0 | 0 | 0 | 0 |
| `src:crates/cleanflate/src/inflate.rs` | 1380 | 1422 | 83 | 80 | 0 | 0 | 0 | 0 |
| `src:crates/cleanflate/src/bitreader.rs` | 582 | 593 | 39 | 44 | 0 | 0 | 0 | 0 |
| `src:crates/cleanflate/src/huffman.rs` | 507 | 569 | 40 | 46 | 0 | 0 | 0 | 0 |
| `cleanflate::window::Window::copy_match_sink` | 241 | 241 | 24 | 24 | 2 | 2 | 2 | 2 |
| `cleanflate::huffman::Huffman::decode_table` | 182 | - | 22 | - | 0 | - | 0 | - |
| `src:crates/cleanflate/src/window.rs` | 169 | 162 | 21 | 21 | 0 | 0 | 0 | 0 |
| `src:crates/cleanflate/src/fast_loop.rs` | 79 | 50 | 11 | 9 | 0 | 0 | 0 | 0 |

PT1 (`docs/perf/phase3/P4-03-before.json`, `P4-03.json`, bench-quick; load average 2.8 to 3.6 in both runs): every Silesia and Canterbury key at levels 1 and 6, in every framing and both modes, is a WIN on the throughput verdict and on the like-for-like ratio verdict, from +18% to +31%; there is no LOSS. Raw keys and the gzip level-1 keys:

| key | before MiB/s | after MiB/s | delta % | throughput verdict | ratio delta % | ratio verdict |
|---|---:|---:|---:|---|---:|---|
| canterbury/gzip/l1/oneshot | 178.5 | 210.7 | +18.06 | WIN | +17.77 | WIN |
| canterbury/gzip/l1/reuse | 178.8 | 218.0 | +21.98 | WIN | n/a | n/a |
| canterbury/gzip/l1/streaming | 175.5 | 214.6 | +22.23 | WIN | +21.24 | WIN |
| canterbury/raw/l1/oneshot | 202.7 | 247.0 | +21.86 | WIN | +24.03 | WIN |
| canterbury/raw/l1/streaming | 199.4 | 243.9 | +22.33 | WIN | +23.37 | WIN |
| canterbury/raw/l6/oneshot | 235.7 | 287.7 | +22.06 | WIN | +22.92 | WIN |
| canterbury/raw/l6/streaming | 226.8 | 279.3 | +23.17 | WIN | +23.07 | WIN |
| silesia/gzip/l1/oneshot | 125.0 | 155.2 | +24.15 | WIN | +26.50 | WIN |
| silesia/gzip/l1/reuse | 123.8 | 152.5 | +23.16 | WIN | n/a | n/a |
| silesia/gzip/l1/streaming | 124.6 | 155.3 | +24.63 | WIN | +21.51 | WIN |
| silesia/raw/l1/oneshot | 132.2 | 164.1 | +24.13 | WIN | +20.50 | WIN |
| silesia/raw/l1/streaming | 129.3 | 170.1 | +31.50 | WIN | +31.36 | WIN |
| silesia/raw/l6/oneshot | 156.1 | 194.7 | +24.70 | WIN | +23.02 | WIN |
| silesia/raw/l6/streaming | 151.9 | 193.1 | +27.11 | WIN | +26.85 | WIN |

One per-file line regresses (`canterbury/alice29.txt/gzip/l1/oneshot` -6.1%, threshold -4.2%): a 150 KB file among 11 whose aggregate is +18%, so it is a per-row noise excursion, not a code-path loss. Interleaved runs of the old and new `profile_decode` binaries agree with the bench: Silesia level 1 one-shot 161.4 to 189.4 MiB/s (4 of 4 pairs), streaming 157.8 to 189.0, level 6 one-shot 176.8 to 220.6, Canterbury level 1 one-shot 195.2 to 236.5. The change is a WIN and is kept (PT4).

### P4-04
**Measured and reverted (PT4): multi-literal table entries lose on this machine.** The pair-entry code and its unit tests that depend on it were removed; `fast_loop_pairs_same_trace_as_reference` (trace-level, literal-heavy dynamic blocks where every literal and the end-of-block code take 3 bits) stays.

Gate (PT2): literals are 46.6% of the symbols at level 1 (`literals / (literals + matches)`, `make bench-stats` counters in `docs/perf/phase3/P1-02-stats.json`, Silesia), so a literal is decoded on almost every other loop iteration; above any gate. (The per-line `perf annotate` share was not taken: the literal share of symbols already answers the gate, and the measurement below decides.)

Design tried, Fast path only: at table build time a first-level literal/length entry whose code is a literal of `l1` bits, followed in the stream by a second literal's whole code of `l2` bits with `l1 + l2 <= LITLEN_TABLE_BITS`, became a `Pair` entry carrying both literals and both lengths; the loop wrote both bytes and consumed `l1 + l2` bits in one step, `decode_table` still returned only the first literal, and the build added a top-down pass over the first level. Its tests (pair entries decode the same as two single lookups on the fixed table and 240 random complete tables at every first-level index with random unknown high bits, the width bound, `decode_table` equal to the bit-serial decode) all passed, and the stats counters matched, so the design was correct, only slower.

Measurement (`docs/perf/phase3/P4-04-before.json`, `P4-04.json`, bench-quick, against the P4-03 code): every level-1 Silesia key lost, for example `silesia/raw/l1/oneshot` 181.3 to 148.3 MiB/s (-18.2%, LOSS, ratio -11.0% LOSS), `silesia/raw/l1/streaming` 178.8 to 149.5 (-16.4%, LOSS), `silesia/gzip/l1/oneshot` 170.2 to 143.4 (-15.8%, LOSS). Interleaved runs of the old and new `profile_decode` binaries (5 alternating pairs, MiB/s means) agree:

| key | before | after | change | after faster in |
|---|---:|---:|---:|---:|
| silesia/rfc1951 l1 oneshot | 203.1 | 189.4 | -6.8% | 0 of 5 |
| silesia/rfc1951 l1 streaming | 198.9 | 187.8 | -5.6% | 0 of 5 |
| silesia/rfc1951 l6 oneshot | 205.7 | 192.1 | -6.6% | 1 of 5 |
| canterbury/rfc1951 l1 oneshot | 244.3 | 221.8 | -9.2% | 0 of 5 |
| canterbury/rfc1952 l1 oneshot | 215.6 | 198.0 | -8.2% | 0 of 5 |

Why it loses, from the design: with a dynamic level-1 code the literals that matter have codes of 4 to 9 bits, so a pair fits in 10 bits only for the most frequent few, and every lookup gains one more outcome (single, pair, sub-table, match) to branch on; the branch between "pair" and "single" is data-dependent and poorly predicted, costing more than the saved lookup, and the extra build pass adds to every block's table cost. The literal branch is already cheap after P4-03 (one inlined lookup, one store). SPEC's "keep the variant that wins" applies: the variant without pairs stays.

## Targets
Filled by P5-02 (acceptance tasks).

## Attribution
Filled by P5-02.

## Tried and reverted
Filled by P5-02.

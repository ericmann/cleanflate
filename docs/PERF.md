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


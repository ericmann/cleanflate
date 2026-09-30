# Performance results

This directory holds bench JSON files named by task id. They are written only by `oracle bench` and `oracle bench-noise` and are never hand-edited. The schema is added by P0-08.

## Schema 2

Written by `oracle bench --json PATH` (the `make bench` and `make bench-quick` targets, with `BENCH_ID` naming the task and the file). Never hand-edited. The loader reads schema 1 (old `min_*` names accepted as aliases) and 2, and rejects any other value.

Definitions:

- median: throughput from the median per-decode time of the N samples.
- best: throughput of the fastest sample (minimum time).
- aggregate median/best: total bytes / sum of per-file median/best times.
- ratio: cleanflate median / C zlib median.

Top-level fields: `schema` (2), `kind` (`full|quick|custom`), `task`, `git_head`, `git_dirty`, `timestamp` (UTC), `machine` (`uname`, `cpu_model`, `governor`, `loadavg` at start, `rustc`, plus `cores`, `os` (`PRETTY_NAME`), `kernel` (`uname -r`), `loadavg_end`), `cargo_profile` (`profile`, `debug_assertions`, `release_profile_toml` (the workspace `[profile.release]` table verbatim), `env` (every `CARGO_PROFILE_*`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS` present)), `c_zlib` (`version`, `compile_flags` as `0x<hex>`, `libz_sys_version`, `libz_sys_features` from `cargo metadata`; `unknown` with a warning if that fails), `c_zlib_version` (same as `c_zlib.version`), `iters` (timed samples per measurement), `min_sample_ms` (each sample lasts at least this long), `stats_build`, `results[]`, `aggregates[]`, and `stats[]` (only `--stats` runs).

`results[]` row fields: `corpus`, `file`, `framing` (`raw|zlib|gzip`), `level`, `mode` (`oneshot|streaming|reuse`), `impl` (`cleanflate|c_zlib`), `bytes` (uncompressed), `reps` (decodes per sample), `median_ns` (median ns per decode), `best_ns` (fastest sample, ns per decode). C zlib rows exist for `oneshot` and `streaming` (`impl: c_zlib`). The `reuse` mode is cleanflate only: one `Decompressor` per (file, framing, level) is built before timing, and every decode calls `reset()` and then runs the same 64 KiB streaming loop into the same scratch buffer as `streaming`; there is no C zlib reuse row (the `inflateReset` call is not a permitted FFI signature).

`aggregates[]` fields: `key` (`<corpus>/<framing>/l<level>/<mode>`), `total_bytes`, `cleanflate_mib_s_median`, `cleanflate_mib_s_best`, `c_zlib_mib_s_median`, `c_zlib_mib_s_best`, `c_zlib_mode` (mode of the C zlib rows used as denominator; schema-1 files default to `oneshot`; `null` for `reuse`), `ratio` (like for like: cleanflate median / C zlib median of the same mode; `null` for `reuse`), `vs_fresh_streaming` (`reuse` aggregates only: reuse median / fresh-object streaming median of the same key, when both modes ran). C zlib is timed one-shot and streaming (64 KiB input and output chunks, same as cleanflate's streaming mode), each only when that mode is selected. Per sample, the order is cleanflate one-shot, cleanflate streaming, C zlib one-shot, C zlib streaming.

The bench prints an `Environment` block listing every captured value at the top of stdout. All environment fields are `#[serde(default)]`, so older files load.

Output columns:

- `bench` stdout table: `key`, `cleanflate MiB/s (median)`, `cleanflate MiB/s (best)`, `C zlib MiB/s (median)`, `ratio (like for like)`, `vs fresh streaming` (`n/a` where not applicable), followed by a footer with the definitions above.
- `bench-md` (`make bench-md J=file [FILES=1] [FILTER=prefix] [BASELINE=path]`) aggregates table: `key`, `cleanflate MiB/s (median)`, `cleanflate MiB/s (best)`, `C zlib MiB/s (median, same mode)`, `ratio (like for like)`, `vs fresh streaming` (reuse rows), `vs phase-2 %` (this file's cleanflate median against the baseline's median of the same key). `reuse` rows show `n/a` for the C zlib columns. With `--files`, a second table: `file key`, `bytes`, `cleanflate MiB/s`, `C zlib MiB/s`, `ratio (like for like)`, `vs phase-2 %`, sorted by corpus and then bytes ascending.
- `bench-compare` (`make bench-compare A= B= [NOISE=] [BASELINE=]`): `key`, `A MiB/s`, `B MiB/s` (cleanflate median), `delta %`, `2x noise %` (`-` when unknown), `verdict` (`WIN|LOSS|NEUTRAL|UNKNOWN`), `vs phase-2 %` (B against the baseline, `n/a` if absent), `B ratio` (like for like), `ratio delta %` (B ratio / A ratio - 1), `ratio verdict` (same 2x rule against the ratio noise; `UNKNOWN` when the noise file has none; `n/a (denominator changed)` when A and B divided by different C zlib modes, as for schema-1 streaming keys). Then per-file `REGRESSION` lines. `--baseline` defaults to `docs/perf/baselines/phase2.json`; if missing it prints `baseline missing: <path>`.
- Default noise file for two files of the same kind: `full` uses `docs/perf/baselines/phase3-noise.json` else `docs/perf/noise.json` (printing "using phase-2 noise"); `quick` uses `docs/perf/baselines/phase3-noise-quick.json` else `docs/perf/noise-quick.json`; mixed kinds get none.
- `bench-targets` (`make bench-targets J=file [BASELINE=] [NOISE=]`): evaluates SPEC targets T1-T6 over one bench JSON and prints markdown for PHASE3.md, one line per target (`MET`, `NOT MET`, or `n/a`) with the worst row and its value, then every failing row. T1: Silesia streaming ratio >= 0.80 (every framing, level 1/6/9). T2: cleanflate one-shot median >= streaming median per corpus/framing/level (per-file exceptions listed as information). T3: Canterbury ratio >= 0.70 in one-shot and streaming. T4: zlib and gzip within 3% of raw. T5: Silesia level-6 one-shot ratio >= 0.70. T6: the `bench-compare` of the baseline against the file has no aggregate LOSS and no per-file REGRESSION. Rows whose `c_zlib_mode` differs from their mode (schema 1 streaming) and modes other than one-shot/streaming (reuse) are excluded from ratio targets. Exit code is 0 unless the file is unreadable: targets are reported, not gates.
- `bench-noise`: writes a JSON of noise percentages (`aggregate`, `aggregate_max_pct`, `per_file`, and `ratio_aggregate`, `ratio_aggregate_max_pct` for the like-for-like ratio), no table.

## Schema 1 (read-only compatibility)

Written by phase-2 builds of `oracle bench --json PATH` (the `make bench` and `make bench-quick` targets, with `BENCH_ID` naming the task and the file). Never hand-edited.

Top-level fields:

- `schema`: always 1.
- `kind`: `full`, `quick` or `custom`.
- `task`: the task id (`--task`).
- `git_head`, `git_dirty`: `git rev-parse HEAD`, and whether the tree had uncommitted changes.
- `timestamp`: UTC, `YYYY-MM-DDTHH:MM:SSZ`.
- `machine`: `uname`, `cpu_model`, `governor` (cpu0 scaling governor or `unknown`), `loadavg` (at the start), `rustc`.
- `c_zlib_version`: the linked C zlib.
- `iters`: timed samples per measurement. `min_sample_ms`: each sample lasts at least this long.
- `stats_build`: whether cleanflate was built with the `stats` feature.
- `results[]`: one row per (corpus, file, framing, level, mode, impl): `corpus`, `file`, `framing` (`raw|zlib|gzip`), `level`, `mode` (`oneshot|streaming`), `impl` (`cleanflate|c_zlib`), `bytes` (uncompressed), `reps` (decodes per sample), `median_ns` and `min_ns` (now `best_ns`) (per decode). C zlib is timed one-shot only.
- `aggregates[]`: one per (corpus, framing, level, cleanflate mode), `key` = `<corpus>/<framing>/l<level>/<mode>`: `total_bytes`, `cleanflate_mib_s_median` (total bytes over the sum of per-file medians), `cleanflate_mib_s_min` (now `..._best`) (over the sum of minimums), `c_zlib_mib_s_median`, `c_zlib_mib_s_min` (now `..._best`), `ratio` (cleanflate median over C zlib median). The C zlib figures are the one-shot denominator for both modes.

Method: each file is compressed once with the oracle (Default strategy, windowBits 15, memLevel 8). After a verified warm-up decode, each implementation runs in interleaved samples, so drift hits all of them alike.

## Comparison tools

- `make bench-noise A=run1.json B=run2.json OUT=docs/perf/noise.json` writes the noise between two runs of the same code: `|a - b| / a` in percent, cleanflate median throughput only, per aggregate key, per file key (`<corpus>/<file>/<framing>/l<level>/<mode>`) and the largest aggregate value (`aggregate_max_pct`). Use `docs/perf/noise.json` for `full` runs and `docs/perf/noise-quick.json` for `quick` runs.
- `make bench-compare A=before.json B=after.json [NOISE=file]` prints per-aggregate deltas with the verdict `WIN` (better than +2x noise), `LOSS` (worse than -2x noise) or `NEUTRAL`, `UNKNOWN` when no noise file exists, and lists per-file `REGRESSION` lines (a drop beyond the larger of the file's noise and its aggregate's noise). It refuses a JSON whose `stats_build` is true. `oracle bench-compare ... --fail-on-regression` exits 1 on any regression.
- `make bench-md J=results.json` prints the aggregates as markdown for PERF.md (`oracle bench-md J --files --filter PREFIX` for per-file rows).
- `make bench-stats BENCH_ID=id` runs `oracle bench --stats` (needs oracle feature `stats`): untimed decodes that print derived statistics per framing and mode, and write the raw counters to a `stats` array in the JSON (per file, plus `*total*` rows). Stats builds are never timed or compared.

## Profiling (cleanflate only)

Profiles must never include C zlib (SPEC 2.2). The flow is split so the profiled process cannot execute it:

1. `make profile-prep` runs `oracle perf-prep`, which uses C zlib to compress a corpus into `testdata/perf-inputs/<corpus>/<rfc195x>-l<level>/` with an `index.tsv` (`file`, `compressed_len`, `original_len`, `original_crc32`). This step is never profiled and is skipped when the directory is already complete.
2. `crates/cleanflate/examples/profile_decode.rs` (cleanflate and std only, debug info on) is the only profiled program. Its streaming mode uses 64 KiB input chunks and 64 KiB output windows, exactly like the bench.

Selecting a key: `PROFILE_KEY=<corpus>/<framing>/l<level>/<mode>` (framing `raw|zlib|gzip|rfc1951|rfc1950|rfc1952`, mode `oneshot|streaming|reuse`), or the variables `PROFILE_CORPUS` (silesia), `PROFILE_FRAMING` (rfc1951), `PROFILE_LEVEL` (6), `PROFILE_MODE` (oneshot), plus `PROFILE_ITERS` (3) and `PROFILE_PERF_FLAGS`. Outputs go to `target/profile/<corpus>-<rfc>-l<level>-<mode>/` (gitignored):

- `make profile`: `perf.data`, `top.txt` (self time) and `top-children.txt` (inclusive, `--children`).
- `make profile-stat`: `stat.txt` from `perf stat` (task-clock, page-faults, cycles, instructions, branches and branch-misses in user mode; an event the machine lacks prints `<not supported>`).
- `make flamegraph`: `flamegraph.svg`, made with `--no-inline` so frames carry their crate path (`cleanflate::...`).
- `make flamegraph-save TAG=<tag>`: runs the flamegraph, checks it with `tools/perf/check-flamegraph.sh` (needs a `cleanflate::` frame and no frame of any other decompressor process), and copies it to `docs/perf/flamegraphs/<slug>-<tag>.svg`.
- `make profile-required TAG=<tag>`: `profile`, `profile-stat` and `flamegraph-save` for the three SPEC 4.3 keys (`silesia/rfc1951/l1/oneshot`, `silesia/rfc1951/l1/streaming`, `canterbury/rfc1952/l1/oneshot`; Canterbury uses 300 iterations for enough samples).
- `make bench-stats` takes `STATS_CORPUS` (silesia), `STATS_LEVELS` (6) and `STATS_MODES` (oneshot,streaming).

The first iteration verifies length and CRC-32 against the index. Findings go into `docs/perf/phase3-profile.md` and `docs/PERF.md` in words and numbers. Never run a profiler on `oracle bench` or any test binary; the guard denies it.

## Phase 3 layout

- `baselines/`: phase baselines (`phase2.json` is the phase-2 number of record); never hand-edited.
- `phase3/`: every phase-3 JSON, named by task id; written only by the tools.
- `flamegraphs/`: `<slug>-<before|after>.svg`, written only by `make flamegraph-save`.
- `PHASE3.md`: the phase-3 performance report; every task appends a `### <ID>` entry.
- `phase3-profile.md`: the phase-3 profile of the phase-2 decoder (P1-02).

## Assembly evidence (cleanflate only)

`make asm` runs `cargo rustc --release -p cleanflate --lib -- --emit asm` (the release profile, no extra `-C` flags) and copies the freshly emitted `cleanflate-*.s` files from `target/release/deps/` to `target/asm/`. Only cleanflate's own generated assembly is ever read (no disassembler, nothing from C code).

`make asm-count [ASM_FUNCS=a,b] [ASM_SRCS=f.rs,g.rs] [ASM_OUT=path.json]` runs `tools/perf/asm_branches.py` over `target/asm/*.s` and prints a table and a JSON report (`{schema: 1, kind: "asm", git_head, rustc, functions: [...]}`). Per row: `instructions`, `cond_branches` (jump mnemonics other than an unconditional `jmp`), `panic_calls` (call or jump instructions to panic or bounds-failure routines: names containing `panic`, `panic_bounds_check`, `slice_start_index_len_fail`, `slice_end_index_len_fail`, `slice_index_order_fail`, `unwrap_failed`, `expect_failed`, `len_mismatch`) and `panic_sites` (distinct target and source-location pairs, the location being the last `.Lanon` operand loaded just before the call).

- `ASM_FUNCS` (default `fast_loop`) selects function symbols by substring of the mangled or demangled name.
- `ASM_SRCS` (default `fast_loop.rs`) reports by source file instead: every instruction is attributed to the file of the most recent `.loc` directive. This is how to measure a function the optimiser inlined into another. In the library build the fast loop is inlined into `Inflater::step_into`, which has no `fast_loop` symbol; the `src:` row for `fast_loop.rs` counts its own instructions and the `huffman.rs`, `bitreader.rs` and `window.rs` rows count what was inlined from those files.


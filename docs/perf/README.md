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

`results[]` row fields: `corpus`, `file`, `framing` (`raw|zlib|gzip`), `level`, `mode` (`oneshot|streaming`), `impl` (`cleanflate|c_zlib`), `bytes` (uncompressed), `reps` (decodes per sample), `median_ns` (median ns per decode), `best_ns` (fastest sample, ns per decode). C zlib rows exist for both modes (`impl: c_zlib`, `mode: oneshot|streaming`).

`aggregates[]` fields: `key` (`<corpus>/<framing>/l<level>/<mode>`), `total_bytes`, `cleanflate_mib_s_median`, `cleanflate_mib_s_best`, `c_zlib_mib_s_median`, `c_zlib_mib_s_best`, `c_zlib_mode` (mode of the C zlib rows used as denominator; schema-1 files default to `oneshot`), `ratio` (like for like: cleanflate median / C zlib median of the same mode). C zlib is timed one-shot and streaming (64 KiB input and output chunks, same as cleanflate's streaming mode), each only when that mode is selected. Per sample, the order is cleanflate one-shot, cleanflate streaming, C zlib one-shot, C zlib streaming.

The bench prints an `Environment` block listing every captured value at the top of stdout. All environment fields are `#[serde(default)]`, so older files load.

Output columns:

- `bench` stdout table: `key`, `cleanflate MiB/s (median)`, `cleanflate MiB/s (best)`, `C zlib MiB/s (median)`, `ratio`, followed by a two-line footer with the definitions above.
- `bench-md` (`make bench-md J=file [FILES=1] [FILTER=prefix] [BASELINE=path]`) aggregates table: `key`, `cleanflate MiB/s (median)`, `cleanflate MiB/s (best)`, `C zlib MiB/s (median, same mode)`, `ratio (like for like)`, `vs phase-2 %` (this file's cleanflate median against the baseline's median of the same key). With `--files`, a second table: `file key`, `bytes`, `cleanflate MiB/s`, `C zlib MiB/s`, `ratio (like for like)`, `vs phase-2 %`, sorted by corpus and then bytes ascending.
- `bench-compare` (`make bench-compare A= B= [NOISE=] [BASELINE=]`): `key`, `A MiB/s`, `B MiB/s` (cleanflate median), `delta %`, `2x noise %` (`-` when unknown), `verdict` (`WIN|LOSS|NEUTRAL|UNKNOWN`), `vs phase-2 %` (B against the baseline, `n/a` if absent), `B ratio` (like for like), `ratio delta %` (B ratio / A ratio - 1), `ratio verdict` (same 2x rule against the ratio noise; `UNKNOWN` when the noise file has none; `n/a (denominator changed)` when A and B divided by different C zlib modes, as for schema-1 streaming keys). Then per-file `REGRESSION` lines. `--baseline` defaults to `docs/perf/baselines/phase2.json`; if missing it prints `baseline missing: <path>`.
- Default noise file for two files of the same kind: `full` uses `docs/perf/baselines/phase3-noise.json` else `docs/perf/noise.json` (printing "using phase-2 noise"); `quick` uses `docs/perf/baselines/phase3-noise-quick.json` else `docs/perf/noise-quick.json`; mixed kinds get none.
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
2. `make profile` builds `crates/cleanflate/examples/profile_decode.rs` (cleanflate and std only, debug info on), runs it under `perf record` and writes `target/profile/top.txt` (top symbols). `make flamegraph` writes `target/profile/flamegraph.svg` the same way.

Variables: `PROFILE_CORPUS` (silesia), `PROFILE_FRAMING` (rfc1951), `PROFILE_LEVEL` (6), `PROFILE_ITERS` (3), `PROFILE_MODE` (oneshot), `PROFILE_PERF_FLAGS`. The first iteration verifies length and CRC-32 against the index. Outputs stay under `target/profile/` (gitignored); findings go into `docs/PERF.md` in words and numbers. Never run a profiler on `oracle bench` or any test binary; the guard denies it.

## Phase 3 layout

- `baselines/`: phase baselines (`phase2.json` is the phase-2 number of record); never hand-edited.
- `phase3/`: every phase-3 JSON, named by task id; written only by the tools.
- `flamegraphs/`: `<slug>-<before|after>.svg`, written only by `make flamegraph-save`.
- `PHASE3.md`: the phase-3 performance report; every task appends a `### <ID>` entry.
- `phase3-profile.md`: the phase-3 profile of the phase-2 decoder (P1-02).

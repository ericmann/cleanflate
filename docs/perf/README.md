# Performance results

This directory holds bench JSON files named by task id. They are written only by `oracle bench` and `oracle bench-noise` and are never hand-edited. The schema is added by P0-08.

## Schema 1

Written by `oracle bench --json PATH` (the `make bench` and `make bench-quick` targets, with `BENCH_ID` naming the task and the file). Never hand-edited.

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
- `results[]`: one row per (corpus, file, framing, level, mode, impl): `corpus`, `file`, `framing` (`raw|zlib|gzip`), `level`, `mode` (`oneshot|streaming`), `impl` (`cleanflate|c_zlib`), `bytes` (uncompressed), `reps` (decodes per sample), `median_ns` and `min_ns` (per decode). C zlib is timed one-shot only.
- `aggregates[]`: one per (corpus, framing, level, cleanflate mode), `key` = `<corpus>/<framing>/l<level>/<mode>`: `total_bytes`, `cleanflate_mib_s_median` (total bytes over the sum of per-file medians), `cleanflate_mib_s_min` (over the sum of minimums), `c_zlib_mib_s_median`, `c_zlib_mib_s_min`, `ratio` (cleanflate median over C zlib median). The C zlib figures are the one-shot denominator for both modes.

Method: each file is compressed once with the oracle (Default strategy, windowBits 15, memLevel 8). After a verified warm-up decode, each implementation runs in interleaved samples, so drift hits all of them alike.

## Comparison tools

- `make bench-noise A=run1.json B=run2.json OUT=docs/perf/noise.json` writes the noise between two runs of the same code: `|a - b| / a` in percent, cleanflate median throughput only, per aggregate key, per file key (`<corpus>/<file>/<framing>/l<level>/<mode>`) and the largest aggregate value (`aggregate_max_pct`). Use `docs/perf/noise.json` for `full` runs and `docs/perf/noise-quick.json` for `quick` runs.
- `make bench-compare A=before.json B=after.json [NOISE=file]` prints per-aggregate deltas with the verdict `WIN` (better than +2x noise), `LOSS` (worse than -2x noise) or `NEUTRAL`, `UNKNOWN` when no noise file exists, and lists per-file `REGRESSION` lines (a drop beyond the larger of the file's noise and its aggregate's noise). It refuses a JSON whose `stats_build` is true. `oracle bench-compare ... --fail-on-regression` exits 1 on any regression.
- `make bench-md J=results.json` prints the aggregates as markdown for PERF.md (`oracle bench-md J --files --filter PREFIX` for per-file rows).
- `make bench-stats BENCH_ID=id` runs `oracle bench --stats` (needs oracle feature `stats`): untimed decodes that print derived statistics per framing and mode, and write the raw counters to a `stats` array in the JSON (per file, plus `*total*` rows). Stats builds are never timed or compared.

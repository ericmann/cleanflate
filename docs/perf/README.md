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

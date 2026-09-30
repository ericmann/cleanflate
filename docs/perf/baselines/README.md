# Baselines

- `phase2.json`: the operator's post-merge phase-2 bench run (git `e16ac6f`), the phase-2 numbers of record (schema 1). Byte-identical copy of `docs/perf/phase2-final-operator.json`.
- `phase3-start.json`, `phase3-start-rerun.json`: two full-tier runs of the phase-3 starting code (schema 2, like-for-like denominators).
- `phase3-noise.json`: the noise between those two runs (`make bench-noise`), used as the default noise file for full runs.
- `phase3-start-quick.json`, `phase3-start-quick-rerun.json`, `phase3-noise-quick.json`: the same for the quick tier (levels 1 and 6).

None of these files is hand-edited.

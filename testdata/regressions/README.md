# Regression fixtures

Every input that ever made cleanflate disagree with C zlib, with its own
reference path, or with a chunking schedule is committed here so it is
never lost. `crates/oracle/tests/regressions.rs` decodes every row on both
decode paths (Fast and Reference), one-shot and streamed, on each run of
`make test`.

Paths use `rfc1951/` (raw), `rfc1950/` (zlib) and `rfc1952/` (gzip).

## Adding a fixture

1. A failing proptest or fuzz case is saved under `target/regress-candidates/`
   (`<tag>-<hash>.bin` plus a `.txt` with the format and schedules).
2. Promote it:

   ```
   cargo run --release -p oracle -- regress-add --input FILE --format raw|zlib|gzip \
       --name NAME [--in-chunks 1,3] [--out-chunks 2] [--expect ok|err:<Kind>] \
       --origin "where it came from"
   ```

3. Fix cleanflate, and commit the fixture with the fix.

`regress-seed` (`make regress-seed`) creates the four deterministic seed
fixtures and is idempotent.

## MANIFEST.tsv columns

`path`, `format` (`raw|zlib|gzip`), `expect` (`ok` or
`err:<ErrorKind::as_str()>`), `out_len` and `out_sha256` (`-` for `err`
rows), `in_chunks` and `out_chunks` (comma-separated positive integers,
cycled, or `-`), `origin`.

## Independence (C16)

For `ok` rows, `out_len` and `out_sha256` come from C zlib through
`oracle::ffi` at `regress-add` time, never from cleanflate. A fixture the
oracle cannot decode cleanly must be added with `--expect err:<Kind>`.

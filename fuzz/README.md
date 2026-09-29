# cleanflate fuzz targets

This is a separate `cargo-fuzz` workspace (excluded from the main
workspace; see the root `Cargo.toml`'s `exclude`), so `cleanflate`
itself never gains a dependency on `libfuzzer-sys` or any fuzzing
infrastructure.

## Targets

Each target feeds arbitrary bytes to both the one-shot decoder
(`decompress_with`) and the streaming `Decompressor`, chunked according
to the first two input bytes (1-256 bytes per input/output call). The
decoded result (`Ok`/`Err`) is ignored entirely -- these are
**never-panic** targets (SPEC S5 M4 (a)): the only failures are a panic
or a libFuzzer timeout/OOM.

- `never_panic_raw` -- `Format::Raw` (RFC 1951, no wrapper).
- `never_panic_rfc1950` -- `Format::Zlib` (RFC 1950).
- `never_panic_rfc1952` -- `Format::Gzip` (RFC 1952).

Each target's `Options::max_output` is capped at `FUZZ_MAX_OUTPUT` (`1
<< 24`, a target-local constant, not one of `cleanflate`'s own tunables)
so a crafted decompression-bomb-shaped seed can't make a single fuzz
iteration spend unbounded time producing output; that behavior is
exercised deliberately and at realistic scale in
`crates/oracle/tests/bomb.rs` instead.

Two differential targets compare `cleanflate` against the oracle's own
C zlib wrapper (SPEC S5 M4 (b)/(c); C9: always through `oracle::ffi`,
never the raw C bindings directly):

- `diff_oracle_roundtrip`: the first 5 input bytes select
  framing/level/strategy/windowBits/memLevel; the rest (capped at 64
  KiB) is compressed with `oracle::ffi::deflate_bytes` and must decode
  back to exactly that data through `cleanflate`'s one-shot API, its
  streaming API (chunked by a size derived from the data), and the
  oracle's own `inflate_bytes` -- any mismatch `panic!`s, which *is*
  the fuzz failure here: a real DEFLATE stream that any two of three
  independent decoders disagree on is a genuine bug.
- `diff_oracle_arbitrary`: arbitrary bytes (not necessarily anything
  real DEFLATE data), decoded under `TrailingData::Allow` by both
  `cleanflate` and the oracle, for each of the three framings.
  Divergences are classified (`both_ok_same`, `both_ok_differ`,
  `both_err`, `clean_ok_oracle_err`, `clean_err_oracle_ok`) and every
  class except `both_ok_same`/`both_err` is appended as a JSON line to
  `$CLEANFLATE_DIVERGENCE_LOG` (default `fuzz/artifacts/divergences.jsonl`).
  This target never panics on a divergence -- two independently-written
  decoders disagreeing at the ragged edge of malformed input is
  expected background noise, not automatically a bug; `make
  fuzz-divergences` summarizes counts per class for manual triage.
  A 60-second smoke run reliably surfaces `gzip/clean_err_oracle_ok`
  and `gzip/both_ok_differ`: these are the *expected* consequence of
  `cleanflate` decoding every concatenated gzip member (RFC 1952 S2.2)
  while the oracle's `inflateInit2_` with `windowBits=31` stops after
  the first member (documented in `crates/oracle/src/ffi.rs` and its
  P1-01 log entry) -- not a `cleanflate` bug.

## Seeds

`fuzz/seeds/<target>/` holds up to 10 small fixtures copied from
`testdata/malformed/`, covering a mix of valid and malformed streams
for that target's format, to give libFuzzer a useful starting corpus.
This directory is read-only input, checked into git; `make fuzz-smoke`
passes it as a *second* corpus directory after a git-ignored
`fuzz/corpus/<target>/` (already covered by `.gitignore`'s
`/fuzz/corpus`), which is where libFuzzer actually writes newly
discovered interesting inputs -- pointing it at `fuzz/seeds/<target>`
directly would have libFuzzer mutate the checked-in seed corpus on
every run.

## Running

```sh
make fuzz-check   # cargo check --bins (works on stable; no nightly needed)
make fuzz-smoke   # cargo +nightly fuzz run <target>, FUZZ_SECONDS each (default 60)
```

`fuzz-smoke` requires a nightly toolchain and `cargo-fuzz`:

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked
```

If the required rustc version for the latest `cargo-fuzz` release is
newer than the active stable toolchain, install `cargo-fuzz` itself
with the nightly toolchain instead (`cargo +nightly install cargo-fuzz
--locked`); the resulting binary works the same regardless of which
toolchain built it.

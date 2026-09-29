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

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

One more differential target compares the two decode paths of cleanflate
itself (SPEC S5, phase 2):

- `diff_fast_reference`: `byte0 % 3` selects the format, `byte1` and
  `byte2` give the input and output chunk sizes (1..=256), and the rest is
  the stream. Both `DecodePath::Fast` and `DecodePath::Reference` (via the
  `test-hooks` feature, enabled only in this workspace's dependency) decode
  it one-shot and through the streaming API; any difference in output,
  error, per-call consumed/produced/status or `finish()` panics. Seeds
  come from `testdata/malformed/`.

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
  fuzz-divergences` summarizes counts per class for manual triage. For
  `Framing::Gzip` the oracle side uses `oracle::ffi::inflate_gzip_members`
  (R1-07), not a single `inflate_bytes` call: plain C zlib's
  `inflateInit2_` with `windowBits=31` only ever decodes one member and
  has no notion of RFC 1952 S2.2's "members are simply concatenated", so
  comparing against it directly used to make every multi-member gzip
  input `cleanflate` correctly decodes in full look like a divergence
  (`gzip/clean_err_oracle_ok`/`gzip/both_ok_differ`) that meant nothing.
  `inflate_gzip_members` calls `inflate_bytes` once per member for as
  long as the remainder starts with `1f 8b` and the previous member
  reached `StreamEnd`, so what's left in the divergence log now reflects
  real disagreements, not multi-member background noise.

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
make fuzz-long    # fuzz-smoke with FUZZ_SECONDS=3600 (1 hour per target, 5 hours total)
```

`.github/workflows/fuzz-nightly.yml` runs each of the five targets for
3600 seconds as a matrix job, on a daily schedule and on manual
`workflow_dispatch`; it is separate from the regular push/PR CI
workflow (SPEC S5 M4 acceptance: targets (a) and (b) each need a full
hour to be a meaningful pass/fail signal, far too slow for per-push
CI). It uploads `fuzz/artifacts/<target>/` on failure and always
uploads `diff_oracle_arbitrary`'s divergence log.

`fuzz-smoke`/`fuzz-long` require a nightly toolchain and `cargo-fuzz`:

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked
```

If the required rustc version for the latest `cargo-fuzz` release is
newer than the active stable toolchain, install `cargo-fuzz` itself
with the nightly toolchain instead (`cargo +nightly install cargo-fuzz
--locked`); the resulting binary works the same regardless of which
toolchain built it.

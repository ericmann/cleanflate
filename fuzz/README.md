# fuzz

`cargo-fuzz` targets for `cleanflate`. This directory sits outside the
Cargo workspace (see the root `Cargo.toml` `exclude`).

Targets (added in Phase 4):
- `never_panic_raw`, `never_panic_rfc1950`, `never_panic_rfc1952`: arbitrary
  bytes into each entry point must never panic.
- `diff_oracle_roundtrip`, `diff_oracle_arbitrary`: differential fuzzing
  against the C zlib oracle in `crates/oracle`.

# cleanflate

Clean-room DEFLATE/zlib/gzip decompressor in safe Rust, built from
RFC 1950/1951/1952 alone. See `docs/SPEC.md` for the full specification and
`docs/DESIGN.md` for design decisions with RFC citations.

## Building and CI

- `make fmt-check` — `cargo fmt --all -- --check`
- `make lint` — `cargo clippy --workspace --all-targets -- -D warnings`
- `make test` — `cargo test --workspace` (quick tier)
- `make corpus` / `make corpus-full` — generate/verify the conformance corpus
- `make crate-checks` — zero deps, no `unsafe` token, forbidden path names
- `make cleanroom-selftest` — guard/logger end-to-end self-test
- `make tools-check` — shellcheck, python unittest, script tests
- `make design-check` — every `docs/DESIGN.md` decision has an `RFC:` line
- `make ci` — all of the above, in order
- `make proptest-10k bomb heavy` — heavy suites (release)
- `make fuzz-check fuzz-smoke fuzz-long fuzz-divergences` — cargo-fuzz
- `make bench` — cleanflate vs C zlib throughput
- `make provenance-snapshot provenance-summary build-report hooks-install`

Fresh clones should run `make hooks-install` once to enable the commit-msg
hook that tags commits with `[stage:...] [task:...]`.

# testdata/malformed

The hand-built malformed (and control-valid) corpus (SPEC S4), generated
by `crates/oracle/src/malformed.rs` via `make malformed` (`cargo run -q -p
oracle -- malformed`). Every raw-DEFLATE (RFC 1951) case lives under
`rfc1951/` and is listed in `EXPECTED.tsv`.

`EXPECTED.tsv` format: `#`-prefixed comment lines, then one row per case:
`path<TAB>expected<TAB>rule`. `expected` is a `cleanflate::ErrorKind`
variant name (via `Error::kind().as_str()`) or the literal `OK` for a
deliberately valid control case. A `sweep:<path>` row (expected always
`Truncated`) asserts that every strict prefix of the file at `<path>`
fails with `Truncated` -- used for the three `valid_small_*` control
streams to exercise truncation at every byte offset.

The builder in `malformed.rs` is a fully independent DEFLATE-stream
encoder: it does not import `cleanflate`'s (test-only, private) encoding
helpers, only RFC 1951's own constant tables and canonical-code
construction algorithm, so this corpus and the decoder it tests stay
independent (CLAUDE.md C16's spirit, extended to structural
independence, not just checksums).

`crates/oracle/tests/malformed.rs::malformed_raw_cases` runs every row
through `cleanflate::inflate` and checks the result. `::
malformed_corpus_is_up_to_date` regenerates into a temp directory and
byte-compares against the committed files, so a decoder or builder change
that would silently drift the corpus fails CI instead.

Zlib and gzip malformed cases arrive in P3-05.

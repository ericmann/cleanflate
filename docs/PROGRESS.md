# cleanflate build progress
Branch: build/2026-09-29
Started: 2026-09-29T14:33:40.844Z

## Tasks
- [ ] P0-01 Workspace scaffold, toolchain pin and Makefile skeleton
- [ ] P0-02 Crate checks script (zero deps, no unsafe token, forbidden path names)
- [ ] P0-03 Provenance library and PostToolUse logger
- [ ] P0-04 PreToolUse clean-room guard
- [ ] P0-05 Cleanroom self-test and hook wiring
- [ ] P0-06 commit-msg hook adding stage and task tags
- [ ] P0-07 Vendor RFC 1950/1951/1952 texts
- [ ] P0-08 Download Canterbury and Silesia corpora
- [ ] P0-09 CI entry point and GitHub Actions workflow
- [ ] P0-10 Lockdown flag, provenance docs and log snapshot
- [ ] P0-11 M0 acceptance and push
- [ ] P1-01 Oracle FFI wrappers over libz-sys
- [ ] P1-02 Deterministic synthetic inputs and generation matrix
- [ ] P1-03 Corpus generator CLI and MANIFEST
- [ ] P1-04 Adler-32 and CRC-32 in cleanflate
- [ ] P1-05 Error types, options, config constants, RFC tables, DESIGN.md skeleton
- [ ] P1-06 Resumable bit reader and 32 KiB window
- [ ] P1-07 Canonical Huffman construction and bit-serial decoder
- [ ] P1-08 Inflater state machine with block headers and stored blocks
- [ ] P1-09 Fixed-Huffman blocks, length/distance decoding and window copies
- [ ] P1-10 Public raw API, Decompressor, output limit, trailing policy and early invariants
- [ ] P1-11 Conformance harness and M1 conformance test
- [ ] P1-12 M1 acceptance and push
- [ ] P2-01 Huffman table validation, including single-code edge cases
- [ ] P2-02 Dynamic block header and code-length alphabet decoding
- [ ] P2-03 Table-driven Huffman decode path
- [ ] P2-04 Malformed corpus builder, raw cases and malformed test
- [ ] P2-05 Full raw conformance
- [ ] P2-06 M2 acceptance and push
- [ ] P3-01 Preset dictionary support and the zlib wrapper
- [ ] P3-02 Gzip member header parser
- [ ] P3-03 Gzip members, trailer, multi-member streams and gzip API
- [ ] P3-04 Zlib and gzip conformance, dictionary and multi-member tests
- [ ] P3-05 Malformed zlib and gzip cases
- [ ] P3-06 M3 acceptance and push
- [ ] P4-01 Finalise the streaming contract for all formats
- [ ] P4-02 Chunking-invariance property test (proptest)
- [ ] P4-03 Output-limit hardening and zip-bomb fixture
- [ ] P4-04 Fuzz crate and never-panic targets
- [ ] P4-05 Differential fuzz targets against the oracle
- [ ] P4-06 Nightly fuzz workflow and long-run target
- [ ] P4-07 Benchmark harness and baseline
- [ ] P4-08 Performance pass 1 — bulk refill and fast decode loop
- [ ] P4-09 Performance pass 2 — measured hotspot and table-bits tuning
- [ ] P4-10 M4 acceptance and push
- [ ] P6-01 Provenance summary generator
- [ ] P6-02 Build report generator and DESIGN.md audit
- [ ] P6-03 Freeze and hand-off push

## Log
(one entry per task, appended by implement)

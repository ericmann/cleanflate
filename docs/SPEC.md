# SPEC: cleanflate — clean-room DEFLATE/zlib/gzip decompressor in safe Rust

> Drop into `docs/SPEC.md` of a fresh repo. Run with `/go-flight`.
> Companion spec: `cleanflate-bench-SPEC.md` (runs only after this build is tagged).

## 1. Purpose

Produce a memory-safe, zero-dependency Rust implementation of DEFLATE decompression (RFC 1951) with zlib (RFC 1950) and gzip (RFC 1952) wrappers, built **clean-room from the RFCs alone** by an AI-assisted pipeline, with a machine-generated provenance record proving what the pipeline did and did not consult.

This project is the demonstration artifact for a white paper on AI-assisted clean-room reimplementation of legacy software. The artifact's credibility depends on three properties, in priority order:

1. **Provenance** — an auditable record that no existing implementation source was read.
2. **Correctness** — byte-identical output to reference zlib across conformance and fuzz corpora.
3. **Safety** — `#![forbid(unsafe_code)]`, no panics on any input, bounded memory.

Performance is measured and reported but is a secondary goal.

## 2. Clean-room rules (NON-NEGOTIABLE)

These rules apply to every Foundry stage (plan, implement, review, summarize).

### 2.1 Permitted inputs
- This SPEC and anything under `docs/` authored by the human or produced by M0.
- RFC 1950, 1951, 1952 plain text, vendored into `docs/rfc/` during M0.
- Rust std documentation and general Rust language knowledge.
- The **public API surface** (function signatures, docs) of `libz-sys`, used only inside `crates/oracle` (see §4).
- Test corpora downloaded in M0 (Canterbury, Silesia) as opaque binary data.

### 2.2 Forbidden inputs
Stages MUST NOT read, fetch, search for, or paste source code or implementation-level docs from any existing DEFLATE/zlib implementation, including but not limited to:
- zlib (madler/zlib, zlib.net source), zlib-ng
- zlib-rs / libz-rs-sys (Trifecta Tech), miniz, miniz_oxide, flate2 internals
- libdeflate, Go `compress/flate`, Python `zlib` C source, Java `Inflater` source, Zopfli, puff.c
- Any blog post or tutorial walking through an existing implementation's code

### 2.3 Mechanical enforcement (M0 builds this; later stages depend on it)
Prose rules are insufficient. M0 MUST implement Claude Code hooks in `.claude/settings.json` plus scripts in `tools/cleanroom/`:

- **PreToolUse guard** (`tools/cleanroom/guard.sh` or equivalent), matched on `Read|Grep|Glob|WebFetch|WebSearch|Bash`:
  - Deny (exit 2 with a stderr reason) any path under `~/.cargo/registry/src/**`, `~/.cargo/git/**`, `target/**/build/**`, and any path or glob containing `zlib`, `miniz`, `flate2`, `libdeflate`, `libz-sys`, or `zopfli` outside this repo's own `crates/cleanflate` and `crates/oracle` sources.
  - Deny `WebSearch` entirely.
  - Deny `WebFetch` except an explicit allowlist, used only in M0: `rfc-editor.org` RFC 1950/1951/1952 text URLs, and the Canterbury and Silesia corpus download URLs. After M0 completes, `WebFetch` is denied entirely. Implement this with a flag file written at M0 completion.
  - Deny Bash commands invoking `curl`, `wget`, `git clone`, `cargo vendor`, `cargo download`, or `gh`, and any `grep`/`rg`/`find`/`cat`/`less` targeting forbidden paths. The only exception is M0 corpus downloads.
- **PostToolUse logger** (`tools/cleanroom/log.sh`) appends one JSON line per tool call to `provenance/tool-log.jsonl`:
  `{ts, foundry_stage, task_id, model, tool, target, decision: "allowed"|"denied", reason}`.
  `foundry_stage`, `task_id` and `model` come from Foundry's state if exposed. Otherwise record `null` and note the gap in `provenance/README.md`.
- Denied attempts are logged, never silently dropped. The white paper reports them.
- M0 includes a self-test (`tools/cleanroom/selftest.sh`) proving the guard blocks a sample of forbidden reads and fetches and allows permitted ones.

### 2.4 Commit discipline
- Every commit message carries `[stage:<plan|implement|review|fix|summarize>] [task:<id>]`.
- Commits are small and task-scoped so the bench spec can compute per-stage effort from git history.

## 3. Product requirements (`crates/cleanflate`)

### 3.1 Crate constraints
- `#![forbid(unsafe_code)]` at the crate root. Zero `unsafe` anywhere in `crates/cleanflate`.
- **Zero runtime dependencies.** `[dependencies]` must be empty. Dev-dependencies are allowed only in `crates/oracle` and `fuzz/`.
- Stable Rust, MSRV pinned in `rust-toolchain.toml`, edition 2021 or later.
- `std` by default. Optional `no_std` + `alloc` support via feature flag is a stretch goal (M5), not required.
- No panics reachable from any public API on any input. Enforce with `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]` in library code, using explicit bounds-checked access patterns. Tests are exempt.

### 3.2 Formats
- **Raw DEFLATE (RFC 1951):** stored, fixed-Huffman and dynamic-Huffman blocks; 32 KiB window; full length/distance code handling; BFINAL across multiple blocks.
- **zlib wrapper (RFC 1950):** CMF/FLG validation including the FCHECK mod-31 test, CM=8, CINFO ≤ 7, Adler-32 verification. A stream with FDICT set is rejected with a distinct error unless a preset dictionary is supplied via API.
- **gzip wrapper (RFC 1952):** magic, CM=8, FLG bits (FTEXT, FHCRC, FEXTRA, FNAME, FCOMMENT), reserved-bit rejection, MTIME/XFL/OS parsed and exposed, CRC-32 and ISIZE verification, and **multi-member** streams (concatenated members decode as one output).
- Checksums (Adler-32, CRC-32) implemented in-crate, derived from the RFCs' definitions.

### 3.3 API
Design freedom is allowed, but these capabilities are required and documented in `docs/DESIGN.md`:
- **One-shot:** `inflate(input) -> Result<Vec<u8>>`, `zlib_decompress(...)`, `gzip_decompress(...)`.
- **Streaming:** a decompressor object that accepts arbitrary input chunks, including 1-byte chunks, and writes into caller-provided output buffers, reporting bytes consumed, bytes produced and status (`NeedsInput`, `OutputFull`, `Done`). It must produce identical output regardless of how input and output are chunked.
- **Limits:** configurable maximum output size (decompression-bomb guard) with a typed error when exceeded; default documented.
- **Errors:** a typed error enum with distinct variants for bad header, bad block type, invalid Huffman table, invalid distance (too far back), checksum mismatch, truncated input, trailing garbage policy, and output limit exceeded. No stringly-typed errors.
- Trailing-data behavior is explicit and configurable (error vs. return remaining bytes).

### 3.4 Design documentation
`docs/DESIGN.md` records each non-trivial decision, citing the RFC section it implements (e.g. "RFC 1951 §3.2.7"). This document is evidence that design flowed from the spec rather than from a prior implementation.

## 4. Test oracle (`crates/oracle`, dev-only)

- Links reference C zlib via `libz-sys` (C backend) **only through its public API**. Agents may read `libz-sys` docs.rs API pages if needed, but never its source. Prefer the zlib manual's documented function signatures written in the SPEC below.
  - Oracle functions needed: `compress2`/`deflateInit2`/`deflate`/`deflateEnd` for generating corpora at levels 0–9, strategies (default, filtered, huffman-only, RLE, fixed), windowBits 9–15, memLevel 1–9, and raw/zlib/gzip framing; `uncompress`/`inflate` for reference output.
- The oracle generates the **conformance corpus** into `testdata/generated/`, deterministically seeded, and records the generation manifest (parameters, input source, SHA-256) in `testdata/MANIFEST.json`.
- Inputs for generation: Canterbury corpus, Silesia corpus, and synthetic inputs (empty, 1 byte, all-zeros up to 16 MiB, random, highly repetitive, window-boundary patterns at 32767/32768/32769 distances, max-length matches of 258).
- **Malformed corpus** in `testdata/malformed/`, hand-built from RFC rules: bad headers, reserved block type 3, over-subscribed and incomplete Huffman code sets, distance beyond window or before stream start, truncation at every byte offset of a small valid stream, bad checksums, invalid gzip flags. Each file records its expected error variant.

## 5. Milestones

Each milestone ends with a Foundry review stage. A milestone is complete only when its acceptance criteria pass in CI.

### M0 — Clean-room harness and scaffolding
- Cargo workspace: `crates/cleanflate`, `crates/oracle`, `fuzz/`, `tools/cleanroom/`, `provenance/`, `docs/rfc/`, `testdata/`.
- Vendor the RFC 1950/1951/1952 text into `docs/rfc/`, recording source URL and SHA-256.
- Download corpora into `testdata/corpora/`, recording URLs and SHA-256.
- Implement hooks, logger and self-test (§2.3). Write the post-M0 lockdown flag.
- CI (GitHub Actions or a local `make ci`): fmt, clippy (deny warnings), test, cleanroom self-test, and a check that `crates/cleanflate` has zero dependencies and contains no `unsafe` token.
- **Acceptance:** the self-test passes, CI is green on an empty crate, the provenance log is populating, and WebFetch is locked after the flag is written.

### M1 — Bit reader, stored and fixed-Huffman blocks, checksums
- LSB-first bit reader per RFC 1951 §3.1.1, with Huffman codes packed MSB-first as specified.
- Stored blocks (LEN/NLEN validation), fixed Huffman tables (§3.2.6), length/distance decoding (§3.2.5), window copy including overlapping copies.
- Adler-32 and CRC-32 with unit tests from RFC examples and oracle values.
- **Acceptance:** all oracle-generated level-0 and fixed-strategy streams decode byte-identically, and checksum unit tests pass.

### M2 — Dynamic Huffman and complete raw inflate
- Code-length alphabet decoding (§3.2.7), canonical Huffman construction (§3.2.2), validation rejecting over-subscribed and incomplete tables (single-code edge cases documented in DESIGN.md), and a table-driven fast decode path.
- **Acceptance:** 100% of the raw-DEFLATE conformance corpus decodes byte-identically, and every malformed-corpus file returns its expected error variant with no panic.

### M3 — zlib and gzip wrappers
- Full §3.2 behavior. Preset dictionary support for zlib. Multi-member gzip.
- **Acceptance:** 100% of the zlib and gzip conformance corpus passes, multi-member tests pass, and wrapper malformed cases match their expected errors.

### M4 — Streaming, limits, fuzzing, performance pass
- Streaming API with the chunking-invariance property test (proptest or equivalent in dev-deps): random input and output chunk sizes including 1 must produce output identical to one-shot.
- Output limit enforcement, verified against a zip-bomb-style fixture (e.g. 1 KiB → >1 GiB of zeros) that fails fast with bounded memory.
- `cargo-fuzz` targets:
  - (a) arbitrary bytes into each of the three entry points: must never panic.
  - (b) differential: oracle-compress random data with random parameters, decompress with both cleanflate and C zlib, assert identical output.
  - (c) differential on arbitrary bytes: both implementations agree on accept/reject, with divergences logged for triage rather than auto-failed, since trailing-data policy may legitimately differ.
- A performance pass guided only by profiling (e.g. `cargo flamegraph`) and the RFC, **not** by reference implementations. Target at least 70% of C zlib single-thread decompression throughput on Silesia. Report the actual number either way.
- **Acceptance:** fuzz targets (a) and (b) run 1 hour each in CI-nightly or locally with zero crashes and zero divergences, and the chunking property test passes 10k cases.

### M5 (optional) — Stretch
- `no_std` + `alloc` feature.
- DEFLATE **compression** (stored, fixed, dynamic; greedy then lazy matching; levels 0–9), verified by round-trip and by C zlib successfully inflating its output. Treat this as a separate clean-room scope with the same rules.

### M6 — Freeze and hand-off
- Tag `v0.1.0-cleanroom`.
- Write `provenance/SUMMARY.md` with: total tool calls, denied calls with reasons, every file and URL read (grouped), per-stage wall clock, models used per stage, and any human interventions (what and why). Human interventions must be logged manually in `provenance/human-interventions.md` as they happen.
- Write `docs/BUILD_REPORT.md`: milestones, review rounds per milestone, defects caught in review vs. by tests vs. by fuzzing, known limitations.
- **Acceptance:** tag exists, provenance summary is complete, and the repo is ready for the bench spec.

## 6. Definition of done
- `crates/cleanflate`: zero deps, zero `unsafe`, no reachable panics per fuzz target (a).
- 100% conformance on generated corpora, and the malformed corpus matches expected errors.
- Differential fuzzing: zero divergences over at least 1 hour per target.
- Provenance log complete from M0 onward, and the summary is published.
- DESIGN.md cites RFC sections for all core decisions.

## 7. Out of scope
- C ABI compatibility (`libz.so` drop-in). This is noted as future work in the white paper.
- SIMD, multithreading, and bit-exact reproduction of zlib's *compressor* output.
- Any change to comparator libraries.

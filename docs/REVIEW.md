# Review
Round: 2

**Verdict: CHANGES REQUESTED**

Branch `build/2026-09-29` against base `c4afeb796ddf`, head `87e33ef`. The branch has 56 tasks done, 0 blocked, 0 skipped and 0 open. This round covers everything after `review: round 1` (`dae4ac0`): the 17 formerly-skipped phase tasks P0-08..P6-03 and the 8 R1 fix tasks. I also re-checked the whole branch against the constraints.

I checked the following myself:
- `foundry_verify` with no file filter: all 10 constraint rules are ok with 0 hits and no fixture failures. fmt, lint, corpus (quick tier, 2494 entries up to date), test, crate-checks and cleanroom-selftest (22/22) all pass.
- Every commit body since `dae4ac0` carries `[stage:] [task:]` tags (C12) and a good signature.
- I read every R1 diff and the library-code diffs from P4-08 (`window.rs`) and P4-09 (`checksum.rs`, `config.rs`). I read the conformance harness (C13), the CI workflow, the P0-10 lockdown/selftest changes, `summarize.py`, and the live `provenance/tool-log.jsonl` (1775 entries, 9 denied).
- I wrote a scratch probe crate outside the repo against the public API to confirm F2 and F5. I also copied the crate into the scratchpad to reproduce F3's hang with a hard timeout.

## Findings (most severe first)

### F1: C5 violated: indexing in cleanflate library code, hidden from clippy by a local `allow` (category 1, constraint)
- **Where:** `crates/cleanflate/src/checksum.rs:100-117`. `#[allow(clippy::indexing_slicing)]` sits on `const fn crc32_table()`, which does `table[n] = c;`. It was introduced in P1-04 (`d2bbfd4`) and missed in round 1.
- **What is wrong:** C5 says: "No `[]` indexing or slicing in cleanflate library code … Checked by clippy `indexing_slicing`." The item-level `allow` overrides the crate-level `deny` in `lib.rs`, so `make lint` cannot see this. It is the only such suppression in library code.
- **What would break:** The table is only evaluated in const context (`CRC32_TABLE`), so today it cannot panic at runtime. But the mechanical check C5 relies on can be switched off per item, and no rule notices. Nothing stops the next `#[allow(clippy::indexing_slicing)]` or `#[allow(clippy::unwrap_used)]` from landing on a runtime function.
- **Minimal fix:**
  - Build the table without indexing. Options: an array literal expanded by a macro from a `const fn crc32_entry(n: u32) -> u32`, or a const walk with `split_first_mut` (const-stable on the pinned 1.89 toolchain). Then drop the `allow`.
  - Add a `docs/foundry.json` rule that forbids `allow(... clippy::{unwrap_used,expect_used,panic,indexing_slicing})` in `crates/cleanflate/src/` outside `src/tests/`. The rule's `shouldMatch` must include this diff's own line, `#[allow(clippy::indexing_slicing)]`.
  - Tag C5 with the new rule in CLAUDE.md.
- **Task:** P1-04 → R2-01.

### F2: `max_gzip_headers = 0` makes every valid gzip stream fail with `Truncated` (category 3, correctness regression from R1-04)
- **Where:** `crates/cleanflate/src/gzip.rs:715`, in `GzipDecoder::finish`:

  ```rust
  if matches!(self.state, GState::Boundary) && !self.headers.is_empty() {
  ```

- **What is wrong:** The code uses `headers.is_empty()` as a stand-in for "at least one member completed". Since R1-04, `headers` is capped by `max_gzip_headers`, so with a cap of 0 it is always empty.
- **Evidence:** In my probe, `decompress_with(Format::Gzip, <valid 25-byte member>, &Options { max_gzip_headers: 0, .. })` returns `Err(Truncated)`, and so do two concatenated members. With a cap of 1 the same input decodes correctly.
- **What would break:** A caller who sets the cap to 0 ("I don't need header metadata"), which is the natural bounded-memory setting, cannot decode any gzip input.
- **Minimal fix:** Test `self.member_count > 0`, not the retained-header vector. Audit `gzip.rs` for any other use of `headers` as a member-seen proxy.
- **Task:** R1-04 → R2-02.

### F3: `decompress_with` spins forever instead of failing when the buffer cannot grow; R1-02's own would-have-caught mutation hangs `make test` (category 3)
- **Where:** `crates/cleanflate/src/stream.rs:372`, `Status::OutputFull => continue`.
- **What is wrong:** When `next_len` caps the buffer at `max_output`, the next call gets an empty `write_region`. If `Decompressor` ever answers that call with `Ok(OutputFull)` rather than `OutputLimitExceeded`, the loop never exits.
- **Evidence:** I applied PLAN R1-02's would-have-caught mutation (`max_output.saturating_add(1).saturating_sub(already_out)`) in a scratchpad copy. `streaming_limit_with_large_buffer` failed, but `stream_raw::limit_plus_one_err` never terminated, and the test binary had to be killed after 60 s. Under `foundry_mutate` that is a 30-minute timeout, not a red test. The R1-02 commit body itself says it "never terminated". That is the same hang-instead-of-fail class that R1-05 fixed in the test drivers, except this loop is in library code.
- **Minimal fix:** In `decompress_with`, if the write region offered was empty and the call returns `OutputFull`, return `Err(OutputLimitExceeded { limit: options.max_output })` rather than `continue`.
- **Task:** R1-02 → R2-03.

### F4: The oracle's multi-member `1f 8b` test is untested; a mutation survived (category 3)
- **Where:** `crates/oracle/src/ffi.rs:521`, `starts_new_member = next.first() == Some(&0x1f) && next.get(1) == Some(&0x8b)`.
- **Evidence:** `foundry_mutate` dropped the `0x8b` half of the check, and the mutation **survived** `make test`.
- **Test gap:** `gzip_multi_member_matches_concatenation` only uses a `b"XYZ"` trailer, so a trailer starting `0x1f` followed by a non-`0x8b` byte is never exercised.
- **What would break:** That is exactly the shape where cleanflate (under `Allow`) reports trailing data. An oracle regression there would make fuzz target (c) log spurious gzip divergences again, the noise R1-07 removed.
- **Minimal fix:** Add a `[0x1f, b'Z']` trailer case asserting `StreamEnd`, the two-member output, and `consumed == both.len()`.
- **Task:** R1-07 → R2-04.

### F5: One-shot `Decoded.consumed` overshoots `trailing_start()` when the input ends in a lone `0x1f` (category 5, R1-03 constraint not met)
- **Where:** `crates/cleanflate/src/stream.rs:384` (`consumed: in_pos`).
- **What the plan requires:** PLAN R1-03's design constraints: "decompress_with's Decoded.consumed equals trailing_start() whenever that is set."
- **Evidence:** In my probe, a 25-byte member followed by a single `0x1f` under `TrailingData::Allow` gives `decompress_with(..).consumed == 26`, while `trailing_start() == Some(25)`.
- **Why this is not the documented exception:** The lone-`0x1f` exception documented in `lib.rs` and D-19 is justified only for *streaming*, where the next byte has not arrived yet. `decompress_with` sees the whole input, and R1-03 did not touch `stream.rs` although PLAN listed it. The new test only covers `XYZ` and `[0x1f, 'Z']` trailers.
- **Minimal fix:** After `finish()`, set `consumed` to `trailing_start()` when it is `Some`.
- **Task:** R1-03 → R2-03.

### F6: `provenance/SUMMARY.md` says "URLs fetched: None recorded" although the log holds allowed `curl` fetches (category 5, SPEC §5 M6)
- **Where:** `tools/cleanroom/summarize.py:170`. `render_urls` only counts the `WebFetch` tool.
- **Evidence:** The live log has allowed Bash `curl` calls to `https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz` and rfc-editor.org during P0-08, including the browser-`User-Agent` retry at 2026-09-29T14:53:46Z. Yet the generated summary says no URL was fetched.
- **Why it matters:** SPEC §5 M6 requires "every file and URL read (grouped)", and provenance is the project's first priority. A summary that claims no URL was fetched is a false statement in the artifact the white paper cites.
- **Minimal fix:**
  - Also extract `http(s)://` URLs from allowed `Bash` targets and list them grouped by tool.
  - Add a README "Known gaps" bullet saying that files read via Bash commands (`cat`, `sed`, `grep`) appear only as commands, not under "Files read".
  - Regenerate SUMMARY.md.
- **Task:** P6-01 → R2-05.

## Spec issues
- **SPEC §5 M4's 70% throughput target** is reported as NOT MET: 25.6% raw and 27.3% zlib on Silesia (`docs/PERF.md`). SPEC says "Report the actual number either way", so this is compliant as written. Whether M4 counts as complete with that number is a human call. It is not a finding.
- **Round 1's spec issues** still stand (C16 vs C3 test placement, pre-hook C12 bootstrap gap, multi-member "zero divergences"). R1-01, R1-08 and R1-07 addressed each of them in PLAN-compatible ways.

## Manual checks still owed (from HANDOFF.md, round 1 section)
1. Run `make fuzz-long`, or trigger `.github/workflows/fuzz-nightly.yml`, for the 1-hour-per-target signal SPEC §5 M4 requires. Only 60 s smoke runs have happened.
2. Confirm the pushed branch's GitHub Actions `ci` run is green.
   - Note that `.github/workflows/ci.yml` fetches Canterbury with `fetch-corpora.sh canterbury` from the host that returned Cloudflare 403 to every automated request in P0-08. Expect that step to be the likely failure point.
   - `make corpus` (quick tier) needs Canterbury.
3. Spot-check `docs/rfc/*.txt` against rfc-editor.org.
4. Re-run `make bench` on a quiet machine and sanity-check `docs/PERF.md`.
5. After the review converges and the branch merges:
   - Run `make provenance-snapshot provenance-summary build-report` and commit.
   - Then tag `v0.1.0-cleanroom` and push the tag.
   - `provenance/SUMMARY.md`, `docs/BUILD_REPORT.md` and the snapshot were all generated at P6-03, before the R1 fixes. BUILD_REPORT's "Defects caught" and "Open divergence classes" sections are stale.
6. Complete `provenance/human-interventions.md` with anything from this run not already captured. For example, the round-1 re-opening of P0-08 and its skipped cascade, which HANDOFF attributes to the operator editing `docs/PROGRESS.md`.

## Notes
- **CR6 note:** during this review the guard denied my read-only `grep` of `tools/cleanroom/fetch-corpora.sh` (reason "fetch script denied: post-M0 lockdown"). This is a false positive, because the script was named in the command, not run. I did not work around it, and that is why the CI fetch question above is a manual check rather than a finding. The denial is in `tool-log.jsonl` at 2026-09-29T21:02:40Z with stage `review`.
- **HANDOFF.md is stale in its top half.** The round-1 section was appended rather than replacing the round-0 text, so the file still says "30 tasks done, 1 blocked", "`fuzz/Cargo.lock` is gitignored" and that `make fuzz-long` passed. All three are superseded.
- **R1-01's comments are garbled** in three files: "this crate's own this crate's own checksum module's". This is cosmetic.
- **`tables.rs:12,49` carries stale `#[allow(dead_code)] // used once dynamic-Huffman decoding needs it (P2-02)`.** P2-02 has long since landed.
- **R1-01's would-have-caught check** used a substitute mutation (the `s1` init) because P4-09 rewrote the named line. Small inputs cannot reach the deferred `% ADLER_MOD`, so the substitute is reasonable.
- **Mutation sample this round:**
  - `gzip.rs` header cap `<` → `<=`: killed.
  - `gzip.rs` R1-03 peek (`resolve_trailing(pos + 1, ..)`): killed.
  - `window.rs` bulk copy source offset +1: killed (8 tests).
  - `sources-check.sh` intervention-row check removed: killed.
  - `oracle/ffi.rs` `1f 8b` check: **survived** (F4).
  - `stream.rs` R1-02 limit +1: **hangs** (F3).

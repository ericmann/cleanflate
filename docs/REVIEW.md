# Review
Round: 1

**Verdict: CHANGES REQUESTED**

Branch `build/2026-09-29` against base `c4afeb796ddf`. The branch has 30 tasks done, 1 blocked (`P0-08`) and 17 skipped (all of them cascade from `P0-08`).

I checked the following myself:
- `foundry_verify` passes: all 9 constraint rules show 0 hits, and fmt, lint, corpus (still a stub), test, crate-checks and cleanroom-selftest all pass.
- I read every cleanflate library module in full: lib, config, options, bitreader, window, huffman, inflate, zlib, gzip and stream.
- I read the fuzz targets, the Makefile, `crate-checks.sh` and the guard entry point.
- I read the live `provenance/tool-log.jsonl`. It is populating, with 981 entries and 8 denied.
- I wrote a scratch probe crate (outside the repo) against the public API to confirm the two runtime defects below.

## Findings (most severe first)

### F1: C16 violated: cleanflate unit-test fixtures use cleanflate's own checksums (category 1, constraint)
- **Where:**
  - `crates/cleanflate/src/tests/stream_contract.rs:7` (`use crate::checksum::{adler32, crc32};`), used at :28 and :36
  - `crates/cleanflate/src/tests/zlib.rs:1`, used at :33, :36, :112 and :132-133
  - `crates/cleanflate/src/tests/gzip.rs:1`, used at :33
  - `crates/cleanflate/src/tests/gzip_header.rs:1`, used at :44
- **What is wrong:** C16 says fixture checksums must come from `oracle::ffi`, never from cleanflate. These fixture builders compute trailer and header checksums with the decoder's own `crate::checksum`, so a checksum bug hides itself.
- **Evidence:** I ran `foundry_mutate` on `checksum.rs` and changed Adler-32's `s2` modulus from 65521 to 65520. Every zlib wrapper test still passed: `zlib_stored_hello`, `zlib_one_byte_chunks_same_output`, `zlib_trailing_*` and `zlib_fdict_*`. Only the dedicated `checksum::adler32_large_buffer_of_ff` test caught it.
- **Why no rule caught it:** C16 is "checked by reading" and has no mechanical rule, so nothing flagged this.
- **Minimal fix:**
  - Replace the self-computed checksums with literals computed once via `oracle::ffi` (commented as such), or move the fixture-dependent cases to `crates/oracle/tests/`.
  - Add a `docs/foundry.json` rule that forbids `crate::checksum` under `crates/cleanflate/src/tests/` except in `tests/checksum.rs`. Use this diff's own lines as `shouldMatch` fixtures.
- **Task:** P3-01, P3-02, P3-03 and P4-01 → R1-01.

### F2: Provenance record is incomplete: guard denials not noted (CR6), human intervention not logged, untagged commits (C12) (category 1, clean-room/provenance)
- **The P1-01 denial:** `provenance/tool-log.jsonl` at 2026-09-29T14:54:47Z shows the guard **denied** this P1-01 command:

  ```
  find / -path '*/libz-sys-*/src/lib.rs'
  ```

  That is an attempt to locate libz-sys source, which CR5 forbids. The guard worked. But the P1-01 progress entry and commit say only "no libz-sys source/rustdoc-src read" and never mention the denial, although CR6 requires it.
- **The P0-08 denials:** P0-08's non-allowlisted `curl https://corpus.canterbury.ac.nz/` denial is also not noted. Its log does mention retrying the allowlisted URL with a spoofed browser User-Agent to get past Cloudflare.
- **The human intervention:** The operator's browser download of the Canterbury archive is recorded only in `testdata/corpora/SOURCES.json` and a commit body. SPEC §5 M6 requires `provenance/human-interventions.md`. That file does not exist, because P0-10 was skipped, and P0-10 as planned would create it with no rows.
- **Untagged commits:** Six commits lack `[stage:] [task:]` tags because they predate the P0-06 hook: `c1bbaa9`, `4015214`, `8edc633`, `956d6d7`, `44f624e` and `f776879`. Rewriting signed history is not the fix; documenting the gap is.
- **Unlocked phases:** The LOCKDOWN flag was never written, so Phases 1–4 ran with network access limited by the allowlist but not locked. That gap also needs documenting.
- **What would break:** The white paper's provenance claim, which is priority 1.
- **Minimal fix:** After P0-10 runs:
  - Add the intervention row.
  - Add a "Known gaps" / "Denied calls during build" section to `provenance/README.md`.
  - Make `sources-check` fail when a `SOURCES.json` entry has a non-automated `fetched_by` and no matching intervention row.
- **Task:** P0-08, P1-01 and P0-10 → R1-08.

### F3: A zero-length output slice raises a sticky `OutputLimitExceeded`, and the streaming limit arithmetic is untested (category 3, correctness)
- **Where:** `crates/cleanflate/src/stream.rs:202`: `if allowance_bound_output || allowance == 0`.
- **What is wrong:** `allowance == 0` is true whenever the *caller's* output slice is empty, not only when the limit is exhausted.
- **Evidence:** My probe calls `Decompressor::decompress(&stored_abc, &mut [])` and gets `Err(OutputLimitExceeded { limit: 1073741824 })`. The error is sticky, so the next call with a real 16-byte buffer fails the same way. The streaming contract (SPEC §3.3) requires `OutputFull` here, with the stream resumable.
- **Test gap:** A `foundry_mutate` that lets the streaming limit go one byte over (`max_output.saturating_add(1)`) **survived** `make test`. `limit_plus_one_err` and `limit_exact_ok` go through `decompress_with`, whose buffer growth caps at the limit, so they never exercise `Decompressor`'s own allowance check. The only test that would catch it (`bomb_streaming_default_limit`) is `#[ignore]` and outside verify.
- **Minimal fix:** Raise the limit error only when the remaining allowance, not the caller's slice, bounded the call. Add streaming tests with a buffer larger than the limit.
- **Task:** P1-10 and P4-01 → R1-02.

### F4: Under `Allow`, gzip's `consumed` at `Done` counts trailing bytes (category 5, spec drift)
- **Where:** `crates/cleanflate/src/gzip.rs:599-615` (the Boundary state consumes its lookahead bytes before deciding) and `resolve_trailing` at :626.
- **Evidence:** In my probe, a 26-byte gzip member followed by `XYZ` under `TrailingData::Allow` gives `decompress_with(..).consumed == 27`, and a streaming `Progress { consumed: 27, status: Done }`. `trailing_start()` is correctly `Some(26)`.
- **What the documents say:**
  - PLAN Decisions: "Under `Allow`, `consumed` and `trailing_start()` report the exact offset."
  - P4-01's contract, `lib.rs:32-34` and DESIGN.md D-20 (`docs/DESIGN.md:469`): "`Done` ⇒ `consumed` stops exactly at its logical end".
  - A trailing `1f` followed by a non-`8b` byte overshoots by 2.
- **Why no test caught it:** `done_consumed_exact_raw_and_zlib` excludes gzip.
- **Minimal fix:**
  - Decide at the boundary by peeking within the current call's input, and consume only a lone `0x1f` that is the last byte of the call. Document that as the one unavoidable exception.
  - Make `Decoded.consumed` equal the trailing offset for gzip.
- **Task:** P3-03 and P4-01 → R1-03.

### F5: Gzip member headers are retained without bound during streaming (category 5, SPEC §1 "bounded memory")
- **Where:** `crates/cleanflate/src/gzip.rs:497` (`self.headers.push(header)`) runs for every member forever.
- **What would break:** Each header can hold up to 3 × `max_gzip_field` (192 KiB by default). A hostile or very long multi-member stream (for example, millions of empty members, or members with 64 KiB FEXTRA each) makes a streaming `Decompressor` grow memory linearly with total input, while producing no output that would trip `max_output`. The field cap (a PLAN decision) bounds each header but not their count.
- **Minimal fix:** Add a config.rs tunable plus an `Options` field that caps the number of retained headers. Keep decoding every member, and expose the total member count.
- **Task:** P3-03 → R1-04.

### F6: Test loops in `stream_contract.rs` spin forever instead of failing (category 3)
- **Where:** `crates/cleanflate/src/tests/stream_contract.rs:264` (`error_sticky_all_formats`), and the similar `loop {}` drivers in that file.
- **What is wrong:** The loops have no no-progress guard.
- **Evidence:** A `foundry_mutate` that disables the gzip ISIZE check (`gzip.rs`) made `make test` hang: the decoder parks at a member boundary returning `Ok(NeedsInput)` on empty input forever. It happened twice, and each hang cost 30 minutes and left orphaned spinning test binaries.
- **What would break:** In an unattended flight, a regression turns into a stall rather than a red test.
- **Minimal fix:** Fail when a call on empty input makes no progress and is not `Done`.
- **Task:** P4-01 → R1-05.

### F7: `fuzz/Cargo.lock` is gitignored to dodge a constraint rule (category 2)
- **Where:** `.gitignore:7-12`.
- **What is wrong:** No task called for this. The implementer hid the lockfile because it lists `oracle`'s transitive `libz-sys`, which would trip `libz-only-in-oracle`.
- **What would break:** Fuzz builds (local and the nightly workflow) resolve dependencies fresh, so the bundled C zlib used by the fuzz oracle can drift from the one pinned in the workspace `Cargo.lock`. That undermines reproducible differential results and crash reproduction.
- **Minimal fix:** Commit `fuzz/Cargo.lock` and exclude that one path from the rule. The rule's intent is fuzz's own source and manifest.
- **Task:** P4-04 → R1-06.

### F8: Differential target (c) cannot see real gzip divergences (category 3)
- **Where:** `fuzz/fuzz_targets/diff_oracle_arbitrary.rs:99`.
- **What is wrong:** It compares cleanflate's multi-member gzip decoding (RFC 1952 §2.2) with a single-member oracle call.
- **Evidence:** The progress log reports `gzip/clean_err_oracle_ok: 666` and `both_ok_differ: 15` from a single 60-second run, all waved off as "expected". At that noise level a genuine gzip disagreement is invisible, so the target does not do its job (SPEC §5 M4 (c): "divergences logged for triage").
- **Other issues:**
  - There is no divergence log on disk to verify the triage claim.
  - HANDOFF.md also states that `make fuzz-long` (5 × 1 h) "passes". The commit timestamps (P4-06 at 10:01:15, handoff at 10:02:41) show it cannot have run.
- **Minimal fix:** Give the oracle side RFC 1952 §2.2 concatenation, using a tested helper in `oracle::ffi`.
- **Task:** P4-05 → R1-07.

### F9: `P0-08` is marked blocked, but the blocker is resolved (category 7)
- **Evidence:** Commit `361e93c` implements P0-08, adding `fetch-corpora.sh --from-file` and an honest `fetched_by` record. The Canterbury and Silesia files are on disk, and `make sources-check` passes locally.
- **Why it matters:** The `[!]` state is stale. It alone caused 17 tasks to be skipped, among them the M0–M4 acceptance, conformance, CI workflow, LOCKDOWN, corpus generator and benchmark tasks. That means SPEC's M1–M3 conformance acceptance (100% of the generated corpus) has **not been demonstrated**.
- **Action:** Unblock P0-08 so the implementer can close it against the existing commit, and reset the cascade.

## Spec issues
- **C16 conflicts with C3 plus PLAN's test placement.** PLAN puts zlib/gzip unit tests with checksummed fixtures in `crates/cleanflate/src/tests/`. cleanflate cannot depend on `oracle` (C3), so C16 can only be met with pinned literals or by moving those tests to `crates/oracle/tests/`. PLAN should say which. R1-01 allows both.
- **C12 (SPEC §2.4) cannot hold for commits made before the tag hook exists.** The first 5 progress commits and Foundry's own run-start commit are necessarily untagged. SPEC/PLAN should acknowledge this bootstrap gap (R1-08 documents it).
- **SPEC §5 M4 / §6 "zero divergences" vs gzip multi-member.** Any single-member reference decoder disagrees with an RFC 1952 §2.2-compliant multi-member decoder, so the differential needs a multi-member oracle loop (R1-07) for "zero divergences" to be meaningful.

## Manual checks still owed (from HANDOFF.md)
1. Flip `P0-08` back to open. This review's `unblock` does it through the tool, but confirm that the cascade of skipped tasks actually re-opens. The corpus is already fetched and hash-verified on disk, and it is gitignored.
2. `P0-09`/`P0-10`/`P0-11` never ran, so there is no `.github/workflows/ci.yml`, no `tools/cleanroom/LOCKDOWN`, and WebFetch lockdown after M0 was never activated.
3. Confirm `provenance/tool-log.jsonl` keeps accumulating entries after a Claude Code restart. It is live in this session: 981 entries, 8 denied.
4. Spot-check `docs/rfc/*.txt` against rfc-editor.org.
5. Nothing is pushed until the run's push/draft-PR step.
6. (Added by review.) The 1-hour-per-target fuzz runs (SPEC §5 M4 acceptance) have **not** been run. HANDOFF's claim that `make fuzz-long` passed is not supported by the timeline.
7. (Added by review.) Spot-check `testdata/corpora/canterbury/*` beyond the SHA-256 match, as noted in the P0-08 commit.

## Notes
- Performance, not a finding (P4-08/P4-09 own it):
  - At every symbol, the bit buffer holds fewer than 8 bits, so `decode_table` always falls back to the bit-serial walk on the first attempt. The 10-bit table is consulted only after a one-byte refill.
  - Fixed blocks clone both fixed tables, including a heap `Vec` of 1024 entries, on every block header.
  - `Window::push` does a `%` per byte.
- `stream.rs:311` hard-codes the one-shot growth step `32768`. PLAN P1-10 specifies it, and it is not one of the four named tunables, so I left it as a note. Consider naming it in config.rs.
- Litlen tables with a single code of length 1 are rejected as incomplete, per the PLAN decision. Some reference decoders accept this shape. If target (c) ever flags it in raw/zlib, the cause is known.
- Several task commits exceed the ~400 non-test-line principle (for example P2-04 and P3-03), mostly in oracle builder code.
- The mutation sample:
  - Killed:
    - inflate distance check (`>=`)
    - huffman incomplete-set exception
    - zlib CINFO bound
    - window overlap push
    - bitreader byte alignment
    - Adler modulus (killed only by the checksum unit test; see F1)
    - guard WebSearch rule
  - Survived: stream limit off-by-one (F3).
  - Hung, not reported: the gzip ISIZE check (F6), and one inflate `> available + 1` run where the decoder spins.

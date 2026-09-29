# Review
Round: 3

**Verdict: CHANGES REQUESTED**

Branch `build/2026-09-29` against base `c4afeb796ddf`, head `6bf067a`. The branch has 61 tasks done, 0 blocked, 0 skipped and 0 open. This round covers the 5 R2 fix tasks (`ae80bb4`, `ec1a424`, `12d00fa`, `7003e58`, `9c48aa9`). I also re-checked the whole branch against the constraints.

I checked the following myself:
- **`foundry_verify` (no file filter):** all 11 constraint rules are ok, with 0 hits and no fixture failures. This includes R2-01's new `no-panic-lint-allow-in-cleanflate-lib`. fmt, lint, corpus (quick tier, 2494 up to date), test, crate-checks and cleanroom-selftest (22/22) all pass.
- **No remaining lint suppressions:** there is no `#[allow]`, `#[expect]` or `#![allow]` of any clippy panic or indexing lint left in cleanflate library code. The only local allows are `dead_code` in `tables.rs`.
- **C12:** every commit since the round-1 review carries `[stage:] [task:]` tags and a good signature. The only untagged commits are the 6 pre-hook bootstrap commits (P0-01..P0-05 progress, plus "start implementation run"). Those are the C12 bootstrap gap that R1-08 already accepted.
- **No stray config edits:** no `.gitignore`, `.gitattributes`, CI or editor-config edits this round.
- **Diffs read:** every R2 diff against its PLAN task and SPEC section. I also read the regenerated `provenance/SUMMARY.md`, and scanned `provenance/tool-log.jsonl` (1927 lines) for allowed Bash calls that fetched URLs.

## Findings (most severe first)

### F1: SUMMARY.md's "URLs fetched" still omits every URL fetched by the fetch scripts, including all three RFC source URLs (category 5, SPEC §5 M6; R2-05 goal not met)
- **Where:** `tools/cleanroom/summarize.py:177-207` (`render_urls`) and the generated `provenance/SUMMARY.md` "URLs fetched" section.
- **What is wrong:** R2-05's stated goal was to include every URL in allowed Bash calls, "curl/wget **and fetch-script invocations**". The implementation only regex-scans the literal Bash command string. A fetch-script invocation has no URL in its command string; the URLs live inside the script and in the tracked manifests the script writes. So none of those fetches are listed:
  - The RFC vendoring run at tool-log line 78 (P0-07, 2026-09-29T14:48:57Z). It is the fetch recorded in `docs/rfc/SOURCES.json`: `https://www.rfc-editor.org/rfc/rfc1950.txt`, `rfc1951.txt` and `rfc1952.txt`.
  - The corpus fetch runs at lines 104, 387, 389, 391 and 1017. They are recorded in `testdata/corpora/SOURCES.json`.
- **Evidence:**
  - The regenerated summary lists only four URLs under `### Bash`: the Canterbury tarball and three spellings of the Silesia zip.
  - All four come from lines 106-108, which are status-code probes (`curl -s -o /dev/null -w "%{http_code}"`) that got 403s. None of them comes from the real fetches.
  - Not one rfc-editor.org URL appears anywhere in SUMMARY.md, although the RFC texts are the project's only permitted design input (CR4).
- **What would break:** the white paper cites SUMMARY.md as the record of every file and URL read. As generated, it cannot show where the RFC texts came from, and it attributes the corpora only to failed probe requests.
- **Minimal fix:**
  - In `render_urls`, detect allowed Bash calls that *invoke* one of the two fetch scripts (not ones that merely `grep`/`sed`/`shellcheck` them).
  - For each script invoked, list the URLs recorded in that script's tracked manifest under a `### Fetch scripts` heading, naming the manifest. The RFC script's manifest is `docs/rfc/SOURCES.json`; the corpus script's manifest is `testdata/corpora/SOURCES.json`. Include `fetched_by` where the manifest records it (Canterbury: `human-operator-browser`).
  - Regenerate SUMMARY.md.
- **Guard warning for the implementer (CR6):** after lockdown the guard denies any Bash command whose text contains either fetch-script filename or `CLEANROOM_ALLOW_CORPUS_FETCH` (`guard.py:363-368`), and that includes commit messages. Write the fixtures and code with Write/Edit, and keep those literal filenames out of every Bash command string, commit message bodies included.
- **Task:** R2-05 → R3-01.

## Spec issues
- **SPEC §5 M4's 70% throughput target** is still reported as NOT MET: 25.6% raw and 27.3% zlib on Silesia (`docs/PERF.md`). SPEC says "Report the actual number either way", so this is compliant as written. Whether M4 counts as complete is a human call.
- **Round 1's spec issues** (C16 vs C3 test placement, the pre-hook C12 bootstrap gap, multi-member "zero divergences") still stand. They were resolved in PLAN-compatible ways by R1-01, R1-08 and R1-07.

## Manual checks still owed (from HANDOFF.md, Round 1 and Round 2 sections)
1. Run `make fuzz-long`, or trigger `.github/workflows/fuzz-nightly.yml`, for the 1-hour-per-target signal SPEC §5 M4 requires. Only 60 s smoke runs have happened.
2. Confirm the pushed branch's GitHub Actions `ci` run is green. The CI Canterbury fetch runs from the host that returned Cloudflare 403 to every automated request during P0-08, so that step is the likely failure point.
3. Spot-check `docs/rfc/*.txt` against rfc-editor.org.
4. Re-run `make bench` on a quiet machine and sanity-check `docs/PERF.md`.
5. After the review converges and the branch merges:
   - Run `make provenance-snapshot provenance-summary build-report` and commit.
   - Then tag `v0.1.0-cleanroom` and push the tag.
   - BUILD_REPORT's "Defects caught" and "Open divergence classes" sections predate the R1 and R2 fixes.
6. Complete `provenance/human-interventions.md` with anything not yet captured, for example the round-1 re-opening of P0-08 and its skipped cascade.
7. From HANDOFF Round 2: the implementer verified R2-04's mutation by hand. This review re-ran it with `foundry_mutate` and it was killed, so this item is now closed.

## Notes
- **Mutation sample this round (all killed, none timed out):**
  - `stream.rs` R1-02 limit+1 (`max_output.saturating_add(1)`): `streaming_limit_with_large_buffer` failed. `limit_plus_one_err` passed, because R2-03's empty-region backstop turns the former hang into `OutputLimitExceeded`. R2-03's would-have-caught check is met.
  - `stream.rs` `Decoded.consumed` reverted to `in_pos`: `gzip_trailing_allow_lone_1f_one_shot` failed (26 vs 25).
  - `gzip.rs` `member_count > 0` reverted to `!headers.is_empty()`: `gzip_zero_header_cap_still_decodes` failed.
  - `oracle/ffi.rs` `1f 8b` check reduced to `1f`: `gzip_multi_member_1f_trailer_is_trailing` failed.
  - `checksum.rs` `crc32_entry` with 7 bit steps: 26 tests failed, including `crc32_table_matches_bitwise_reference`.
  - `summarize.py` dropping the allowed-only filter: `test_urls_from_bash_curl` failed.
- **R2-03's empty-write-region backstop is unreachable** through the public API while `Decompressor::decompress`'s arithmetic is correct. No test hits it directly; its value is as defence in depth, which the limit+1 mutation above demonstrates. That is fine as it stands.
- **`next_len`'s `32768` growth floor (`stream.rs:341`)** is a literal outside `config.rs`. PLAN P3/R2-03 names it explicitly and does not list it among the config tunables, so it is not a C8 finding. Move it to `config.rs` if it is ever tuned.
- **Earlier notes still apply:**
  - `tables.rs:12,49` still has a stale `#[allow(dead_code)] // used once ... (P2-02)` comment.
  - R1-01's garbled "this crate's own this crate's own" comments are still there.
  - HANDOFF.md's top half is still the superseded round-0 text.
- **Guard false positives:** the post-lockdown fetch-script check (`guard.py:363`) is a plain substring match, so it denies commit messages and read-only greps that only *mention* the scripts. The implementer logged this friction in R2-05. It is CR6-compliant behaviour, but it will keep costing time.

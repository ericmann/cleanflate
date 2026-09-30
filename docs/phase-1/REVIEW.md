# Review
Round: 4

**Verdict: APPROVED**

Branch `build/2026-09-29` against base `c4afeb796ddf`, head `8391b06`. The branch has 62 tasks done, 0 blocked, 0 skipped and 0 open. This round covers the single R3 fix task, R3-01 (`507e64a`). I also re-checked the whole branch against the constraints, and everything from rounds 1-3 still holds.

I checked the following myself:
- **`foundry_verify`, run with no file filter and again with `tools/` + `docs/DESIGN.md` files:**
  - All 11 constraint rules are ok, with 0 hits and no fixture failures.
  - fmt-check, lint, corpus (quick tier, 2494 up to date), test, crate-checks, cleanroom-selftest (22/22), tools-check (60 Python tests plus all shell script tests) and design-check all pass.
- **C12:** all 5 commits since the round-3 review carry `[stage:fix] [task:...]` tags and a good signature (`G`).
- **No stray config edits:** since `review: round 3`, only these files changed: `.foundry/state.json`, `docs/HANDOFF.md`, `docs/PROGRESS.md`, `provenance/SUMMARY.md`, `tools/cleanroom/summarize.py`, the summarize test and its fixture. There are no `.gitignore`, `.gitattributes`, CI or editor-config edits.
- **R3-01 diff checked against its PLAN task and SPEC §5 M6:**
  - `render_fetch_scripts` only considers allowed `Bash` entries.
  - It splits each command on shell separators and counts `bash|sh <script>` or `<path>/<script>` as the command word as an invocation.
  - For each invoked script it lists the URLs from that script's tracked manifest, together with the manifest path and `fetched_by`.
  - A missing manifest prints `MISSING:` rather than being skipped.
- **Regenerated `provenance/SUMMARY.md`:** its "### Fetch scripts" section now lists the three `rfc-editor.org/rfc/rfc195{0,1,2}.txt` URLs, from `docs/rfc/SOURCES.json`. It also lists the Canterbury URL (`fetched_by: human-operator-browser`) and the Silesia URL, from `testdata/corpora/SOURCES.json`. Both match the tracked manifests. Round 3's F1 is resolved.

## Findings (most severe first)

None. Categories 1-3 are clean across the whole branch, and there are no blocked or skipped tasks.

### Interpretation choices (HANDOFF Round 3)
- **R3-01 reads "invoked" narrowly:** only the command word of a shell-separated piece counts; grep, sed, shellcheck, git and similar commands that name the script are mentions. This fits the task's out-of-scope note (no general shell parsing) and SPEC's intent, since a mention does not fetch anything. It is accepted. The one known gap is that a wrapped invocation (for example `env X=1 bash <script>` or `bash -x <script>`) would be missed. The real log does not contain that shape: the regenerated summary shows both scripts.

## Spec issues
- **SPEC §5 M4's 70% throughput target** is still reported as NOT MET: 25.6% raw and 27.3% zlib on Silesia (`docs/PERF.md`). SPEC says "Report the actual number either way", so this is compliant as written. Whether M4 counts as complete is a human call.
- **Round 1's spec issues** still stand (C16 vs C3 test placement, the pre-hook C12 bootstrap gap, multi-member "zero divergences"). They were resolved in PLAN-compatible ways by R1-01, R1-08 and R1-07.

## Manual checks still owed (from HANDOFF.md, Rounds 1-3)
1. Run `make fuzz-long`, or trigger `.github/workflows/fuzz-nightly.yml`, for the 1-hour-per-target signal SPEC §5 M4 requires. Only 60 s smoke runs have happened.
2. Confirm the pushed branch's GitHub Actions `ci` run is green. The CI Canterbury fetch runs from a host that got Cloudflare 403s during P0-08, so that step is the likely failure point.
3. Spot-check `docs/rfc/*.txt` against rfc-editor.org.
4. Re-run `make bench` on a quiet machine and sanity-check `docs/PERF.md`.
5. After merge:
   - Run `make provenance-snapshot provenance-summary build-report` and commit. BUILD_REPORT's "Defects caught" and "Open divergence classes" sections predate the R1-R3 fixes.
   - Then tag `v0.1.0-cleanroom` and push the tag.
6. Complete `provenance/human-interventions.md` with anything not yet captured, for example the round-1 re-opening of P0-08 and its skipped cascade.
7. From HANDOFF Round 3: glance at the "### Fetch scripts" subsection of `provenance/SUMMARY.md` after the post-merge regeneration, to confirm it still reads sensibly.

## Notes
- **Mutation sample this round (all killed):**
  - `summarize.py`, allowed-only filter removed from `render_fetch_scripts`: `test_urls_from_fetch_scripts` failed.
  - `summarize.py`, every mention treated as an invocation: `test_urls_from_fetch_scripts` failed.
  - `summarize.py`, `fetched_by` output suppressed: `test_urls_and_fetched_by_from_allowed_corpus_fetch_script` failed.
  - `inflate.rs`, stored LEN/NLEN check disabled: `stored_nlen_mismatch_error` failed.
  - `zlib.rs`, FCHECK mod-31 check disabled: `zlib_bad_fcheck` failed.
- **`summarize.py` readability:**
  - The trailing `if first_base in MENTION_ONLY_VERBS: continue` at the end of the inner loop in `_invoked_fetch_scripts` does nothing, because nothing follows it in the loop. `MENTION_ONLY_VERBS` is therefore documentation only; the positive checks alone decide what counts as an invocation.
  - The per-script headings are rendered at the same `###` level as their parent "### Fetch scripts" heading.
  - Neither is worth a task.
- **Earlier notes still apply:**
  - The `32768` growth floor in `next_len` (`stream.rs`) is a literal outside `config.rs`.
  - There is a stale `#[allow(dead_code)]` comment in `tables.rs`.
  - R1-01's garbled "this crate's own this crate's own" comments are still there.
  - HANDOFF.md's top half is still the superseded round-0 text.
  - The guard's substring match on fetch-script names causes false positives.

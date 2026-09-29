# cleanflate build progress
Branch: build/2026-09-29
Started: 2026-09-29T14:33:40.844Z

## Tasks
- [x] P0-01 Workspace scaffold, toolchain pin and Makefile skeleton
- [x] P0-02 Crate checks script (zero deps, no unsafe token, forbidden path names)
- [x] P0-03 Provenance library and PostToolUse logger
- [x] P0-04 PreToolUse clean-room guard
- [x] P0-05 Cleanroom self-test and hook wiring
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

### P0-01 — ba42470
Workspace scaffold created: Cargo workspace (resolver=3), cleanflate (zero deps, forbid(unsafe_code), deny panics) and oracle crates, fuzz/ excluded, tools/cleanroom, docs/rfc, testdata, provenance dirs with README stubs. Makefile has real fmt-check/lint/test/ci targets and STUB targets for corpus, crate-checks, cleanroom-selftest, tools-check, design-check, fuzz-check, fuzz-smoke, fuzz-long, fuzz-divergences, proptest-10k, bomb, heavy, malformed, bench, sources-check, provenance-snapshot, provenance-summary, build-report, hooks-install, conformance-full (all needed since docs/foundry.json calls them). rust-toolchain.toml pins 1.89.0 (matches installed stable). Test: crate_builds asserts 1+1==2 (assert!(true) triggers clippy::assertions_on_constants under -D warnings). cargo metadata confirms cleanflate has 0 dependencies. make ci green.

### P0-02 — 2c549a6
tools/ci/crate-checks.sh implements checks (a) deps via cargo metadata (empty .dependencies + no build.rs), (b) unsafe-token via grep -rnw, (c) crate-attrs via literal-line/deny-list grep on lib.rs, (d) forbidden-paths via git ls-files minus crates/cleanflate|oracle grepped case-insensitively. Honors CRATE_CHECKS_ROOT. make tools-check now runs shellcheck on tools/**/*.sh, py_compile + unittest discover (guarded, dirs don't exist yet so no-ops), and every tools/ci/tests/*_test.sh and tools/git-hooks/tests/*_test.sh (also guarded). ci now includes tools-check. Fixture tests use a local path-dependency dummy crate (not crates.io) to keep cargo metadata offline/deterministic.

### P0-03 — 3829bf8
provenance.py: repo_root/log_path/context/target_of/append/derive_stage/derive_task_id/derive_model/read_hook_input, python3 stdlib only. Entry keys in ENTRY_KEYS order: ts,foundry_stage,task_id,model,tool,target,decision,reason. append() uses O_APPEND|O_CREAT + fcntl.flock, creates provenance/ if missing. log.py reads hook JSON from stdin, appends an allowed entry, always exits 0 (falls back to a logger-error reason entry, then stderr, on any exception). Stage derivation checks agent_type substring match first (planner/implementer/reviewer/summarizer, implementer consults .foundry/state.json round>=1 for fix), then FOUNDRY_STAGE env, then .foundry/implement.lock presence. Task id: FOUNDRY_TASK_ID env for implement/fix, else first "^- \[~\] (\S+)" line under "## Tasks" in docs/PROGRESS.md. Model: hook_input["model"], else CLEANROOM_MODEL, else best-effort transcript tail scan (last 256KiB, newest-first, message.model). All 11 tests pass. Added __pycache__/*.pyc to .gitignore (byproduct of the new python test suite).

### P0-04 — 26b9723
guard.py: evaluate(hook_input)->(decision,reason) plus main() wiring stdin->evaluate->log-if-denied->stderr/exit. R1 WebSearch always denied. R2 WebFetch: deny if LOCKDOWN exists, else allow only if url in allowlist.txt. R3 Read/Glob/Grep: resolve_variants (expanduser+cwd-join+normpath+realpath) checked against forbidden roots F (~/.cargo/registry|git, $CARGO_HOME/registry, target/**/build, target/doc/src), forbidden words W unless under exempt dirs E (crates/cleanflate, crates/oracle), W+ words outside root; Glob pattern/Grep glob also checked. R4 Bash: (a) git-clone/cargo-vendor always deny, gh always deny, curl/wget deny post-lockdown else require every https URL in allowlist; (b) fetch-rfcs.sh/fetch-corpora.sh/CLEANROOM_ALLOW_CORPUS_FETCH deny post-lockdown; (c) raw substring/regex check for .cargo/registry, .cargo/git, target/doc/src, target/**/build; (d) segment-split (;,&&,||,|,newline) read-family (cat/grep/find/...) check via shlex tokens, path-like vs non-path-like word checks, plus grep -r/-R without --exclude-dir=target at repo root denial. R5: any exception in evaluate() caught in main(), denies with "guard internal error: <exc>", still logs. All 32 tests in tools/cleanroom/tests/test_guard.py pass (subprocess-based logging/fail-closed tests plus direct evaluate() calls for the rest), using temp CLEANROOM_REPO_ROOT/HOME/CLEANROOM_LOG/CLEANROOM_LOCKDOWN_FILE.

### P0-05 — 0593469
.claude/settings.json created (did not exist) with PreToolUse->guard.py, PostToolUse->log.py, exactly the SPEC hooks block. tools/cleanroom/fixtures/*.json use __ROOT__/__HOME__ placeholders. selftest.sh substitutes via sed, runs 21 named PASS/FAIL cases per the task list, checks denied-log-line count matches deny-case count, checks log.py writes one allowed entry with all 8 keys in order, checks .claude/settings.json via jq, and verifies the real provenance/tool-log.jsonl is untouched. make cleanroom-selftest wired. Fixed two real guard.py bugs found by exercising it live during fixture-writing (not by a self-test case per se, caught manually first): (1) Bash segment splitting used regex split on bare "|" which broke on quoted grep alternation patterns (e.g. "zlib\|gzip") -- replaced with quote-aware split_top_level_segments(); (2) the outside-repo/target containment check required every resolved path variant (normpath AND realpath) to be inside root, which fails when the repo is reached via a symlinked working directory -- fixed by canonicalizing root with realpath and accepting a token if any variant resolves inside. Both described under Interpretation/Defects-caught in the P0-05 commit; guard.py unit tests (all 43) still pass.

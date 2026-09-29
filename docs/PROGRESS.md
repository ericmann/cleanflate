# cleanflate build progress
Branch: build/2026-09-29
Started: 2026-09-29T14:33:40.844Z

## Tasks
- [x] P0-01 Workspace scaffold, toolchain pin and Makefile skeleton
- [x] P0-02 Crate checks script (zero deps, no unsafe token, forbidden path names)
- [x] P0-03 Provenance library and PostToolUse logger
- [x] P0-04 PreToolUse clean-room guard
- [x] P0-05 Cleanroom self-test and hook wiring
- [x] P0-06 commit-msg hook adding stage and task tags
- [x] P0-07 Vendor RFC 1950/1951/1952 texts
- [!] P0-08 Download Canterbury and Silesia corpora
- [-] P0-09 CI entry point and GitHub Actions workflow
- [-] P0-10 Lockdown flag, provenance docs and log snapshot
- [-] P0-11 M0 acceptance and push
- [x] P1-01 Oracle FFI wrappers over libz-sys
- [x] P1-02 Deterministic synthetic inputs and generation matrix
- [-] P1-03 Corpus generator CLI and MANIFEST
- [x] P1-04 Adler-32 and CRC-32 in cleanflate
- [x] P1-05 Error types, options, config constants, RFC tables, DESIGN.md skeleton
- [x] P1-06 Resumable bit reader and 32 KiB window
- [x] P1-07 Canonical Huffman construction and bit-serial decoder
- [x] P1-08 Inflater state machine with block headers and stored blocks
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

### P0-06 — a33862b
tools/git-hooks/commit-msg: POSIX-ish bash, always exits 0. Skips if message already contains "[stage:". Task derivation: ^ID-N: subject -> that ID; ^plan:/^review: -> plan/review; ^progress: -> first ID-N token else "progress"; otherwise lowercase word before first colon else "unknown". Stage: plan:/review: subjects -> plan/review; subject containing "summary" -> summarize; else "fix" if .foundry/state.json round>=1 (jq if present else sed fallback) else "implement". Appends blank line + tag line at end of file. make hooks-install now runs git config core.hooksPath tools/git-hooks (was a stub); ran it for this clone. tools-check already ran tools/git-hooks/tests/*_test.sh from the P0-02 guard I added, so no Makefile test-wiring change needed here. All 8 tests pass. Verified: git log -1 --format=%B on this commit shows exactly one tag line (hook left an already-tagged message unchanged).

### P0-07 — 460c711
fetch-rfcs.sh fetched the three RFC texts via curl -fsSL from the rfc-editor.org allowlist URLs (only, retries 3x) and wrote docs/rfc/SOURCES.json ([{file,url,sha256,bytes,fetched_at}]); refuses if tools/cleanroom/LOCKDOWN exists. sources-check.sh verifies every docs/rfc/SOURCES.json entry exists with matching sha256 and each RFC text contains "Request for Comments: 195N"; also verifies testdata/corpora/SOURCES.json the same way (best-effort, only for files present locally) when that file exists. make sources-check wired, added to tools-check as a prerequisite. All 3 required test cases pass; a 4th case_corpus_tamper_fails is present per P0-08's acceptance note but self-skips (prints why) until P0-08 creates testdata/corpora/SOURCES.json. Real docs/rfc/*.txt fetched and committed; make ci green.

### P0-08 — blocked
BLOCKED: Tried: bash tools/cleanroom/fetch-corpora.sh all --record, which downloads Canterbury from the sole allowlisted URL (https://corpus.canterbury.ac.nz/resources/cantrbry.tar.gz) with 3 retries. Also tried the exact same URL directly with curl multiple times, including with full browser-like User-Agent/Accept/Accept-Language headers, both inside and outside the script. Fails: every attempt returns HTTP 403 from Cloudflare (server: cloudflare, sets a __cf_bm bot-management cookie), i.e. the corpus host is blocking this sandbox's requests at the edge, not a transient or script-level failure. Silesia, the other corpus, downloads fine (HTTP 200 on both allowlisted mirrors), and the fetch-corpora.sh script itself (usage, lockdown check, curl retry/fallback logic, tar/unzip extraction, SOURCES.json read/write, verify mode) is otherwise complete and was exercised successfully for the Silesia path. Think the fix is: this is an environment/network-policy problem (Cloudflare bot protection on corpus.canterbury.ac.nz), not something a different implementation can work around within the allowlisted URL; a human or a differently-provisioned CI runner needs to fetch Canterbury (or the SPEC/PLAN needs an approved alternate mirror URL added to the allowlist) before this task and its corpus-dependent downstream tasks (P1-02 onward) can complete for real. The fetch-corpora.sh script, its retry/verify logic and Makefile wiring are implemented and ready to rerun once Canterbury is reachable.

### P0-09 — skipped
SKIPPED: depends on P0-08

### P0-10 — skipped
SKIPPED: depends on P0-08

### P0-11 — skipped
SKIPPED: depends on P0-08

### P1-01 — 1410d90
crates/oracle now depends on libz-sys 1.1.29 (features=["static"], bundled C backend, deterministic across machines), libc 0.2.189, and cleanflate (path, unused so far). oracle::ffi provides Framing{Raw,Zlib,Gzip}, Strategy{Default,Filtered,HuffmanOnly,Rle,Fixed}, DeflateParams{level,strategy,window_bits,mem_level,framing}, deflate_bytes/deflate_zeros/inflate_bytes(->InflateOutcome{output,consumed,status})/OracleStatus{StreamEnd,DataError,NeedDict,Truncated,OutputLimit,OtherError}/crc32/adler32/zlib_version. z_stream built with every field explicit (no mem::zeroed), zalloc/zfree are extern "C" wrappers over libc::calloc/free. windowBits: raw=-wb, zlib=wb, gzip=wb+16 for deflate; inflate uses -15/15/31, gzip inflate stops at first member end (inflateInit2_ with wb=31 does this natively, verified by gzip_inflate_reports_first_member_consumed). unsafe confined to ffi.rs with a SAFETY comment per unsafe block (crate-checks/C1 only restricts crates/cleanflate). All 9 tests pass; compiled clean on first attempt using only the SPEC-listed signatures (CR5: no libz-sys source/rustdoc-src read). cargo metadata still shows cleanflate with 0 dependencies.

### P1-02 — 407bd7e
crates/oracle/src/prng.rs: SplitMix64 (state += 0x9E3779B97F4A7C15, xor-shift-multiply finalizer) + fnv1a64. inputs.rs: Tier{Synthetic,Canterbury,Silesia}, Input{id,tier,len,source}, generate_synthetic(id) covering all 16 SPEC S4 ids (empty, one-byte, zeros-{1KiB,64KiB,1MiB,16MiB}, random-{64KiB,1MiB}, text-256KiB via a 64-word list, repeat-abc-1MiB, repeat-period17-1MiB, window-d{32767,32768,32769}, run-64KiB, period258-64KiB), canterbury_inputs/silesia_inputs (enumerate testdata/corpora/{canterbury,silesia}/ sorted by name, id=<tier>-<name>, empty Vec if dir absent -- no error), find_canterbury_file. matrix.rs: CORPUS_SEED=0x636C_6561_6E66_6C61, EntrySpec, make_entry building id `<input>__<framing>_l<level>_<strategy>_w<wb>_m<mem>[_dict]` and path `<tier>/<input>/<rfc195N>-l<level>-<strategy>-w<wb>-m<mem>[-dict].bin`, groups A (levels 0-9 x 3 framings, {0,1,6,9} if input>4MiB), B (strategies, per-size branch), C (window/memlevel, only inputs<=1MiB), D (dictionary: canterbury alice29/asyoulik/cp.html x levels{1,6,9} x Zlib with lcet10.txt as dict, silently empty if lcet10.txt absent), deduplicated by entry id via HashSet. inputs_for_tier(Quick|Full, corpora_root). All 5 matrix tests pass; window test confirms exact-distance repeat; quick tier currently synthetic-only (Canterbury still blocked per P0-08) sits at the low end of 300..=6000 but within bounds. Dictionary entries are 0 right now (lcet10.txt absent) -- matrix_covers_all_parameter_values only requires dict_count>=1 when that file is present.

### P1-03 — skipped
SKIPPED: depends on P0-08

### P1-04 — d2bbfd4
crates/cleanflate/src/checksum.rs: Adler32{new,update,finish} (s1 init 1, s2 init 0, mod 65521 per byte per RFC 1950 S2.2) and adler32(); Crc32{new,update,finish} + crc32() built from a const-fn 256-entry table over the reflected polynomial 0xEDB88320 (RFC 1952 S8 sample-code style, standard invert-in/invert-out reflected CRC). RFC section citations in module rustdoc: RFC 1950 S2.2 (Adler-32 field/algorithm) + S8.2 (appendix reference algorithm); RFC 1952 S2.3.1 (CRC32 trailer field) + S8 (Appendix: Sample CRC Code). Table build uses one narrow #[allow(clippy::indexing_slicing)] (get_mut isn't const-callable on this toolchain; index is loop-bounded 0..256, exactly table's length, documented inline). Declared pub mod checksum in lib.rs; unit tests in crates/cleanflate/src/tests/checksum.rs (module registered via `mod checksum;` in tests/mod.rs, satisfying C6). All 7 cleanflate tests + 2 oracle cross-check tests (200 random buffers each, lengths 0..=10,000, SplitMix64-seeded, checked against oracle::ffi::{adler32,crc32} i.e. real C zlib, satisfying C16) pass. make lint/crate-checks/ci all green.

### P1-05 — f573839
error.rs: Error (#[non_exhaustive], 12 variants per SPEC S3.3), HeaderError, HuffmanError, ChecksumKind (all #[non_exhaustive]), ErrorKind (Error::kind(), ErrorKind::as_str()), Display + std::error::Error impls. config.rs: DEFAULT_MAX_OUTPUT=1<<30, DEFAULT_MAX_GZIP_FIELD=65_536 (re-exported from lib.rs); LITLEN_TABLE_BITS/DIST_TABLE_BITS deferred to the huffman task that needs them. options.rs: Format{Raw} (#[non_exhaustive]), TrailingData{Reject(#[default]),Allow} via #[derive(Default)], Options{max_output,trailing,dictionary,max_gzip_field} with Default sourcing config constants. tables.rs (pub(crate), #[allow(dead_code)] until Phase 2): LENGTH_BASE/LENGTH_EXTRA (29), DIST_BASE/DIST_EXTRA (30), CODE_LENGTH_ORDER (19), WINDOW_SIZE/MAX_MATCH/END_OF_BLOCK, fixed_litlen_lengths()/fixed_dist_lengths() -- all transcribed from docs/rfc/rfc1951.txt S3.2.5-S3.2.7, using .get()/iter_mut() only (C5), one narrow #[allow(clippy::indexing_slicing)] in checksum.rs's const-fn table build (pre-existing from P1-04, unrelated). docs/DESIGN.md created with D-01..D-06 and D-10 (D-06 documents the RFC's own code-284/285 length-258 overlap, caught by length_ranges_contiguous during testing and fixed by exempting that one boundary, not the table). tools/ci/design-check.sh (python3, parses "### D-" headings and requires an "RFC:" line before the next heading) + design_check_test.sh (2 cases) + Makefile design-check target wired into ci. All 17 cleanflate unit tests + 2 design-check cases pass; make ci green.

### P1-06 — 24c3a76
bitreader.rs: BitReader{buffer:u64,count:u32}, new/available/fill(input,pos,need)/peek(n<=32, zero-padded)/consume(n)/take(input,pos,n)->Option<u32>/align_to_byte. fill loads whole bytes only while count<need (never overshoots), using input.get(*pos) (no indexing). take returns None on exhaustion but leaves whatever partial fill happened (pos advanced, buffer holds loaded bytes). window.rs: Window{buf:[u8;WINDOW_SIZE],total:u64} ring buffer; push/available/get_back(distance, 1=last)/prefill(keep last 32768 via repeated push)/copy_match(distance,len,out)->usize which pushes each copied byte before reading the next, so distance<len overlaps (e.g. distance 3, len 7 on "abc" -> "abcabca") self-repeat correctly. Both modules #[allow(dead_code)] at mod declaration until Phase 2 wires them into Inflater. DESIGN.md D-07 (bit order/buffer invariant) and D-08 (window ring/overlap) added -- D-06 was already taken by P1-05's length-code boundary decision, so this task's two decisions shifted to D-07/D-08. All 13 new tests pass (30 total in the crate); two tests initially had wrong hand-computed expected values (my arithmetic, not the implementation) for align_drops_partial_byte and wraps_after_32768, corrected after seeing the real (correct) output.

### P1-07 — 0239978
huffman.rs: Huffman{counts:[u16;16],symbols:Vec<u16>}. from_lengths builds a length histogram then runs the RFC 1951 S3.2.2 recurrence as a "codes still available" left-capacity check (left=1; left=(left<<1)-count[len]; left<0 => OverSubscribed); symbols vector built ordered by (length,symbol value) by iterating len 1..=15 x symbol ascending. decode(&self, &BitReader)->Decoded{Symbol{symbol,len}|NeedMoreBits|Invalid}: for len in 1..=15, reads one new bit via peek(len)>>(len-1)&1 (MSB-first code assembly), tracks first/index via the same recurrence, matches when code-first<count. Never consumes (peek-only). tests/bitwriter.rs: test-only BitWriter{write_bits LSB-first, write_code MSB-first, align, finish} for building fixture streams, reused by later Huffman/inflate tests (module marked #[allow(dead_code)] for the currently-unused write_code). DESIGN.md D-09 added (D-06/07/08 already taken by P1-05/06). All 5 huffman tests pass, including the RFC's own ABCDEFGH worked example (F=00,A=010,...,H=1111) and 8 fixed-code spot checks (0->00110000 ... 287->11000111), on the first implementation attempt -- no debugging needed.

### P1-08 — 3ea1377
inflate.rs: Inflater{br:BitReader,window:Window,state:State,total_out:u64}. State{BlockHeader,StoredLen{bfinal},StoredCopy{remaining,bfinal},Done,Failed(Error)}. step(input,output)->Result<(consumed,produced,InflateStatus),Error> runs a loop of atomic sub-steps: BlockHeader reads 3 bits (BFINAL low bit, BTYPE next 2), btype 0->StoredLen, 1/2/10/11 all ->Err(BadBlockType) (marked "replaced by P1-09"/"P2-02" per task text) and sticky-stored via State::Failed. StoredLen aligns to byte then reads 32 bits as LEN|NLEN<<16 (little-endian 2+2 bytes), validates len==!nlen else Err(InvalidStoredLength), ->StoredCopy. StoredCopy copies min(remaining,input_left,output_left) bytes directly (byte-aligned, bypasses the bit buffer) into output and Window, returns NeedsInput/OutputFull based on which limit was hit, or transitions to Done (bfinal) / BlockHeader (more blocks) when remaining==0. Errors sticky via State::Failed checked at top of step(). InflateStatus{NeedsInput,OutputFull,Done}. is_done()/total_out() added. DESIGN.md D-11 (D-06..D-10 already used by earlier tasks). All 9 acceptance tests pass first try, including the RFC's own byte-exact "01 00 00 ff ff" empty-final-block example, 1-byte-input/1-byte-output chunking, trailing-junk-not-consumed, and sticky-error-after-failure.

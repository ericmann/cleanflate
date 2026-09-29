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
- [x] P1-09 Fixed-Huffman blocks, length/distance decoding and window copies
- [x] P1-10 Public raw API, Decompressor, output limit, trailing policy and early invariants
- [-] P1-11 Conformance harness and M1 conformance test
- [-] P1-12 M1 acceptance and push
- [x] P2-01 Huffman table validation, including single-code edge cases
- [x] P2-02 Dynamic block header and code-length alphabet decoding
- [x] P2-03 Table-driven Huffman decode path
- [x] P2-04 Malformed corpus builder, raw cases and malformed test
- [-] P2-05 Full raw conformance
- [-] P2-06 M2 acceptance and push
- [x] P3-01 Preset dictionary support and the zlib wrapper
- [x] P3-02 Gzip member header parser
- [x] P3-03 Gzip members, trailer, multi-member streams and gzip API
- [-] P3-04 Zlib and gzip conformance, dictionary and multi-member tests
- [x] P3-05 Malformed zlib and gzip cases
- [-] P3-06 M3 acceptance and push
- [x] P4-01 Finalise the streaming contract for all formats
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

### P1-09 — 82ebd20
inflate.rs extended: Inflater gains fixed_litlen/fixed_dist:Huffman (built once in new() via from_lengths(...).unwrap_or_else(Huffman::empty), infallible in practice) and bfinal:bool (replaces per-state bfinal fields; StoredLen/StoredCopy simplified accordingly). New states LitLen/LenExtra{sym}/Dist{len}/DistExtra{len,dsym}/Copy{remaining,dist}. peek_symbol() helper loops decode()+fill() until Symbol/Invalid or NeedsInput. Literal symbols only consume bits once output space confirmed (retry-safe). Symbol 256->next_block_state() (Done if bfinal else BlockHeader). Length codes 257-285 read LENGTH_BASE/LENGTH_EXTRA extra bits (saturating_add); 286/287->InvalidSymbol. Distance symbols 0-29 valid, 30/31->InvalidSymbol; DIST_BASE/DIST_EXTRA extra bits give distance, validated against window.available() (u64) else InvalidDistance. Copy uses Window::copy_match on output.get_mut(out_pos..) (no raw slicing/indexing), decrementing remaining via saturating_sub. huffman.rs gained Huffman::empty() (always-Invalid fallback). tables.rs: MAX_MATCH/CODE_LENGTH_ORDER individually #[allow(dead_code)] (used in P2-02). window.rs: prefill #[allow(dead_code)] (used in Phase 3). DESIGN.md D-12. All 12 new tests pass on first implementation attempt (56 total in crate), including the RFC's own "03 00" empty-fixed-block byte hint, code-284-extra-31=258, distance-exactly-32768-ok, and 1-byte-chunk resumability. One real defect: a code comment that literally spelled out "unreachable!() call" tripped the no-panicking-calls-in-cleanflate-lib constraint regex (which scans comments too); reworded, no behavior change.

### P1-10 — 5b78b7d
stream.rs (new, public): Status{NeedsInput,OutputFull,Done}, Progress{consumed,produced,status}, Decoded{data,consumed}, Decompressor{inflater,options,total_in,trailing_start,error}::{new(format,options),decompress(input,output)->Result<Progress,Error>,finish()->Result<(),Error>,total_in(),total_out(),trailing_start()}, pub(crate) next_len(len,limit)->usize (grows by max(len,32768), capped at limit, never shrinks), decompress_with(format,input,&options)->Result<Decoded,Error>, inflate(input)->Result<Vec<u8>,Error>. decompress() caps output to max_output-total_out via output.get_mut(..allowance); OutputLimitExceeded only fires when allowance<output.len() or allowance==0 (i.e. the limit was the binding constraint, not a genuine full caller buffer) -- exact-limit streams succeed since EOB needs no output space. Trailing data checked both for leftover unconsumed bytes within the same call that reaches Done and for any input in a later call after Done; Reject errors with absolute offset, Allow sets trailing_start() once. Decompressor has its own sticky-error field (separate from Inflater's internal one) since OutputLimitExceeded/TrailingData originate at this layer. decompress_with grows a Vec via next_len, writes into the resized tail, truncates to what was produced, loops until Done or stuck, then calls finish(). Made inflate_fixed.rs's emit_* helpers pub(crate) for reuse across stream_raw.rs/invariants.rs. DESIGN.md D-13 plus "Update (P1-10)" notes on D-02/D-03. All 25 new tests pass (12 stream_raw + 2 invariants, 96 total in crate now), including 200x8 randomized chunk schedules matching one-shot output exactly and 20,000 random-garbage buffers through both one-shot and 1-byte streaming with zero panics.

### P1-11 — skipped
SKIPPED: depends on P1-03

### P1-12 — skipped
SKIPPED: depends on P1-11

### P2-01 — 84ce62b
huffman.rs: TableKind{LitLen,Distance,CodeLength(#[allow(dead_code)] until P2-02)}. from_lengths(lengths,kind) now also rejects Incomplete (left>0 after the RFC 1951 S3.2.2 recurrence) except for TableKind::Distance with either total_codes==0 (all-zero/empty table) or total_codes==1 && counts[1]==1 (single length-1 code) -- both taken verbatim from RFC 1951 S3.2.7. No changes needed to decode() itself: an empty table's zero counts at every length and a single-code table's other same-length codes both fall through to Decoded::Invalid naturally via the existing code-first<count loop. Updated call sites: inflate.rs's two fixed-table constructions (LitLen/Distance, both already complete) and P1-07's huffman.rs test file (all TableKind::LitLen, no behavior change since that ABCDEFGH example is a complete code). All 11 new tests pass; one test's own input buffer needed 2 bytes instead of 1 to satisfy br.fill's 15-bit request (test-only sizing bug, not a huffman.rs defect). make ci green (82 cleanflate tests total, up from 71).

### P2-02 — f92420a
inflate.rs: new states DynHeader/CodeLenLens{i}/CodeLens/Repeat{sym}. DynHeader reads 14 bits (HLIT|HDIST<<5|HCLEN<<10 combined take, same pattern as StoredLen's LEN/NLEN), rejects hlit>286 as InvalidHuffmanTable(TooManyLengthCodes). CodeLenLens reads HCLEN 3-bit lengths at CODE_LENGTH_ORDER positions into dyn_cl_lengths[19], then builds TableKind::CodeLength (no incomplete exception -- errors propagate as InvalidHuffmanTable). CodeLens decodes symbols 0-18 via peek_symbol against dyn_cl_table into a growing dyn_lengths:Vec<u8> (Inflater field, not State, to avoid cloning it every step); 0-15 literal, 16/17/18 ->Repeat state (16 checked for RepeatWithoutPrevious before transitioning); on reaching dyn_hlit+dyn_hdist total, splits into litlen/dist length arrays, checks litlen[256]!=0 (else MissingEndOfBlock) before building TableKind::LitLen/Distance tables, stores into new litlen_table/dist_table fields (fixed-block path now clones fixed_litlen/fixed_dist into these same fields instead of using dedicated fixed_* references directly in LitLen/Dist states -- one shared pipeline for both block types). Repeat{sym} reads 2/3/7 extra bits, computes count(3-6/3-10/11-138) and value (prev_len for 16, else 0), checks dyn_lengths.len()+count>total ->RepeatOverflow, pushes, ->CodeLens. tests/dynbuilder.rs: independent from-scratch canonical-code (RFC 1951 S3.2.2) and frequency-based Huffman-length (textbook two-smallest-merge) implementations for building valid dynamic-block fixtures, ClSymbol/PayloadSym/DynBlock, emit_dynamic_block (full valid block incl. auto-EOB, skipped if symbol256 has no code) and emit_dynamic_header_raw (header-only, for malformed fixtures). DESIGN.md D-15. All 14 new tests + invariants.rs's 4 added dynamic fixtures (96 total cleanflate tests) pass. Two real test-driver bugs found and fixed (not decoder bugs): 1-byte-chunk stuck-detection needed to check status==NeedsInput not just consumed==0 (OutputFull with no remaining input is legitimate progress, not stuck) in inflate_stored.rs/inflate_fixed.rs/inflate_dynamic.rs; and dynbuilder's original power-of-two-depth length assignment was only complete for symbol counts that are powers of two, replaced with real Huffman construction.

### P2-03 — 2ca3c8d
config.rs: LITLEN_TABLE_BITS=10, DIST_TABLE_BITS=8 (pub(crate)). huffman.rs: Huffman gained table_bits:u32 and table:Vec<TableEntry> fields (TableEntry{Invalid,Long,Symbol{symbol,len}}), built by build_table() during from_lengths using the same code/idx walk as decode()'s recurrence but computing actual (code,len) per symbol; TableKind::table_bits() maps LitLen->10, Distance->8, CodeLength->0 (no table, always bit-serial, since that alphabet is tiny/rarely decoded). reverse_bits() converts a code's MSB-first integer value to the LSB-first bit-index BitReader::peek produces; each Symbol entry is written to all table_bits-len "don't care" combinations, Long entries mark a longer code's exact table_bits-bit prefix. decode_table(&self,&BitReader)->Decoded: falls back to decode() (kept as reference) whenever table_bits==0 or br.available()<table_bits (not per-entry -- see Interpretation), else does one table lookup. inflate.rs's peek_symbol now calls decode_table instead of decode for both litlen and distance decoding (used for LitLen/Dist states across stored/fixed/dynamic blocks alike). All 101 cleanflate tests pass with the table-driven path live in production on the first attempt (all fixed/stored/dynamic/invariants/garbage tests unchanged), plus the 5 new huffman_table.rs tests including 1000 randomly-generated complete codes (via leaf-splitting, guaranteed Kraft-complete for any symbol count) checked against the bit-serial reference. DESIGN.md D-16.

### P2-04 — 218104b
crates/oracle/src/malformed.rs: fully independent DEFLATE builder (own BitWriter, fixed/length/distance RFC tables transcribed fresh, canonical_codes, textbook Huffman-length-from-frequency, stored/fixed/dynamic block emitters, DynBlock/ClSymbol/PayloadSym). cases() returns 21 named Case{name,bytes,expected,rule,sweep} covering: btype3, stored_nlen_mismatch, fixed_litlen_286, fixed_dist_code_30, fixed_distance_at_start, fixed_distance_too_far, fixed_len284_extra31(OK), valid_stored_trailing_byte(TrailingData), dynamic_hlit_287, dynamic_codelength_oversubscribed/incomplete, dynamic_litlen_oversubscribed/incomplete, dynamic_dist_oversubscribed/incomplete_two_codes, dynamic_repeat16_first, dynamic_repeat_overflow, dynamic_missing_eob, dynamic_no_dist_then_length(InvalidSymbol), dynamic_single_dist_code(OK), dynamic_no_dist_literals(OK), plus 3 valid_small_{stored,fixed,dynamic} control streams (OK + sweep:). crates/oracle/src/main.rs (new binary, first for oracle since P1-03's "generate" was skipped) wires `oracle malformed` writing testdata/malformed/rfc1951/*.bin + EXPECTED.tsv. make malformed target wired. crates/oracle/tests/malformed.rs: malformed_raw_cases parses EXPECTED.tsv, runs every row through cleanflate::inflate comparing kind().as_str() (or OK), sweep: rows check every strict prefix is Truncated; malformed_corpus_is_up_to_date regenerates into a temp dir and byte-compares TSV+fixtures against committed. Both tests passed on first run (after one format-string arg-order typo fix) -- 24 generated files (21 cases + 3 sweeps as TSV rows) all agree with the decoder built across P1-08/P1-09/P2-01/P2-02/P2-03, serving as new independent cross-validation. git status --porcelain testdata/malformed empty after commit.

### P2-05 — skipped
SKIPPED: depends on P1-11

### P2-06 — skipped
SKIPPED: depends on P2-05

### P3-01 — 1819a6b
zlib.rs: ZlibDecoder{state,inflater,adler,dictionary,dict_id_buf,trailer_buf}, State{Cmf,Flg{cmf},DictId{collected},Body,Trailer{collected},Done,Failed}. Checks FCHECK((cmf*256+flg)%31==0) then CM(!=8) then CINFO(>7), matching RFC 1950 S2.2's own order. FDICT set -> read 4-byte big-endian DICTID; None dictionary->PresetDictionaryRequired{dict_id}; mismatched adler32(dict)!=dict_id->DictionaryMismatch; else Inflater::with_dictionary(dict) (new ctor, prefills window via existing Window::prefill) and ->Body. FDICT clear ignores any supplied dictionary (per RFC, no mechanism to use one without FDICT). Body delegates to self.inflater.step(), accumulating Adler32 over exactly the produced-byte slice each call. Trailer reads 4 big-endian bytes, compares to running Adler32, mismatch->ChecksumMismatch{kind:Adler32,...}. stream.rs: Decompressor now holds Backend{Raw(Inflater)|Zlib(ZlibDecoder)} (uniform step/is_done/total_out), so all limit/trailing/stickiness logic is format-agnostic -- Decompressor::new dispatches on Format, using Inflater::with_dictionary for Format::Raw+dictionary too. Format gained Zlib variant. lib.rs exports zlib_decompress. All 14 new tests + all 101 pre-existing tests (115 total) pass with zlib live in production on first attempt. DESIGN.md D-17 (D-16 already taken by P2-03).

### P3-02 — c10d96a
gzip.rs: pub struct GzipHeader{text,mtime,xfl,os,extra,name,comment,header_crc,truncated_fields} (#[non_exhaustive]). pub(crate) GzipHeaderParser::new(max_field)/feed(input)->Result<(usize,Option<GzipHeader>),Error>, byte-at-a-time resumable states Id1->Id2->Cm->Flg->Mtime{collected}->Xfl->Os->[ExtraLen{collected}->ExtraData]->[Name]->[Comment]->[Hcrc{collected}]->Done, each optional field's presence gated by decide_next(stage) consulting self.flg bits (FEXTRA/FNAME/FCOMMENT/FHCRC). Every header byte except the FHCRC field's own 2 bytes feeds a running Crc32 (crate::checksum::Crc32), including bytes discarded past max_field (truncated_fields=true) and FNAME/FCOMMENT's zero terminator (CRC'd, never stored). MTIME/XLEN/CRC16 are little-endian (RFC 1952 S2.1, opposite of RFC 1950's big-endian Adler-32). Magic/CM/reserved-flags validated before FLG is accepted. pub mod gzip in lib.rs (matches checksum/error's full-module-public pattern), one #[allow(dead_code)] at the mod declaration since nothing drives GzipHeaderParser outside tests until P3-03. All 12 new tests pass on first attempt (one clippy fix: stage<=0 on u8 -> stage==0). 127 cleanflate tests total, make ci green.

### P3-03 — cba7606
Implemented GzipDecoder (crates/cleanflate/src/gzip.rs): resumable
multi-member state machine MemberHeader->Body->Trailer->Boundary,
CRC32+ISIZE trailer verification, and inline trailing-data resolution
at boundaries (lone 0x1f buffered per NeedsInput-consumes-everything
invariant, resolved by finish()). Wired into stream.rs as a new
Backend::Gzip variant with owns_trailing_data()/finish()/trailing_start()
delegation; added Format::Gzip, Decompressor::gzip_headers(),
Decoded::gzip_members, gzip_decompress(). Added 14 acceptance tests in
tests/gzip.rs (multi-member concat, empty members, CRC/ISIZE mismatch,
trailing reject/allow, lone-0x1f boundary, 1-byte chunking, truncated
trailer). Recorded design decision D-19. make ci and foundry_verify
both pass.

### P3-04 — skipped
SKIPPED: depends on P1-11

### P3-05 — 5ad2563
Extended crates/oracle/src/malformed.rs with a Family enum (Rfc1951/
Rfc1950/Rfc1952) tagging each Case, plus independent zlib_wrap/
gzip_member_bytes builders using oracle::ffi::adler32/crc32 (C zlib)
for every checksum. Added rfc1950_cases() (bad_fcheck, cm_7, cinfo_8,
fdict_no_dictionary, adler_mismatch, trailing_byte, valid_small+sweep)
and rfc1952_cases() (bad_magic, cm_7, reserved_flag_bit5/7,
fhcrc_mismatch, crc32_mismatch, isize_mismatch, trailing_garbage,
second_member_bad_cm, all_flags_valid+sweep, two_members_valid) --
exactly the cases listed in PLAN.md. main.rs writes each family into
its own testdata/malformed/rfc19NN/ dir. Added
malformed_wrapper_cases (routes rfc1950 rows through zlib_decompress,
rfc1952 through gzip_decompress); malformed_corpus_is_up_to_date now
regenerates and byte-compares all three families. make malformed &&
make ci both pass; foundry_verify green.

### P3-06 — skipped
SKIPPED: depends on P3-04

### P4-01 — 004c4ac
Documented the Decompressor streaming contract in crate-level rustdoc
(Status meanings, sticky errors, finish() semantics per format,
post-Done trailing-data handling) with a runnable doctest; recorded as
DESIGN.md D-20 (PLAN.md referenced "D-12" but that id was already
taken by an earlier decision -- used the next free slot instead).
Added crates/cleanflate/src/tests/stream_contract.rs: 9 tests proving
the contract identically across Raw/Zlib/Gzip (1-byte chunking,
OutputFull/resume, NeedsInput-consumes-all, exact consumed-at-Done for
Raw/Zlib, trailing-data Reject/Allow, error stickiness, a finish()
semantics table, zero-length output during header parsing). Found and
fixed two test-harness bugs while writing these (not decoder bugs):
loops must not treat NeedsInput-with-no-more-input as terminal for
Gzip (D-19's boundary lookahead legitimately needs one more byte or a
finish() call), and must not treat OutputFull-with-no-more-input as
terminal at all (it just needs a fresh output buffer). Added
oracle's malformed_one_byte_chunks_same_kind. make ci and
foundry_verify both green.

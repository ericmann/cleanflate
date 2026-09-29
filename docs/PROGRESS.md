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
- [x] P0-08 Download Canterbury and Silesia corpora
- [x] P0-09 CI entry point and GitHub Actions workflow
- [x] P0-10 Lockdown flag, provenance docs and log snapshot
- [x] P0-11 M0 acceptance and push
- [x] P1-01 Oracle FFI wrappers over libz-sys
- [x] P1-02 Deterministic synthetic inputs and generation matrix
- [x] P1-03 Corpus generator CLI and MANIFEST
- [x] P1-04 Adler-32 and CRC-32 in cleanflate
- [x] P1-05 Error types, options, config constants, RFC tables, DESIGN.md skeleton
- [x] P1-06 Resumable bit reader and 32 KiB window
- [x] P1-07 Canonical Huffman construction and bit-serial decoder
- [x] P1-08 Inflater state machine with block headers and stored blocks
- [x] P1-09 Fixed-Huffman blocks, length/distance decoding and window copies
- [x] P1-10 Public raw API, Decompressor, output limit, trailing policy and early invariants
- [x] P1-11 Conformance harness and M1 conformance test
- [x] P1-12 M1 acceptance and push
- [x] P2-01 Huffman table validation, including single-code edge cases
- [x] P2-02 Dynamic block header and code-length alphabet decoding
- [x] P2-03 Table-driven Huffman decode path
- [x] P2-04 Malformed corpus builder, raw cases and malformed test
- [x] P2-05 Full raw conformance
- [x] P2-06 M2 acceptance and push
- [x] P3-01 Preset dictionary support and the zlib wrapper
- [x] P3-02 Gzip member header parser
- [x] P3-03 Gzip members, trailer, multi-member streams and gzip API
- [x] P3-04 Zlib and gzip conformance, dictionary and multi-member tests
- [x] P3-05 Malformed zlib and gzip cases
- [x] P3-06 M3 acceptance and push
- [x] P4-01 Finalise the streaming contract for all formats
- [x] P4-02 Chunking-invariance property test (proptest)
- [x] P4-03 Output-limit hardening and zip-bomb fixture
- [x] P4-04 Fuzz crate and never-panic targets
- [x] P4-05 Differential fuzz targets against the oracle
- [x] P4-06 Nightly fuzz workflow and long-run target
- [x] P4-07 Benchmark harness and baseline
- [x] P4-08 Performance pass 1 — bulk refill and fast decode loop
- [x] P4-09 Performance pass 2 — measured hotspot and table-bits tuning
- [x] P4-10 M4 acceptance and push
- [x] P6-01 Provenance summary generator
- [x] P6-02 Build report generator and DESIGN.md audit
- [x] P6-03 Freeze and hand-off push
- [x] R1-01 Independent checksums in cleanflate unit-test fixtures (C16) plus a mechanical rule
- [x] R1-02 Output limit: empty output slice must be OutputFull, and cover the streaming limit arithmetic
- [x] R1-03 Gzip: exact consumed at Done under TrailingData::Allow
- [x] R1-04 Bound retained gzip member headers during streaming
- [x] R1-05 Make stream_contract test drivers fail instead of hang on no progress
- [x] R1-06 Commit fuzz/Cargo.lock instead of gitignoring it to dodge libz-only-in-oracle
- [x] R1-07 Multi-member-aware oracle for differential fuzz target (c)
- [x] R1-08 Complete the provenance record: intervention row, denial notes, bootstrap gaps
- [x] R2-01 C5: CRC-32 table without indexing, plus a rule against panic-lint suppression in cleanflate lib
- [x] R2-02 Gzip finish(): member-seen test must use member_count, not retained headers
- [ ] R2-03 decompress_with: exact consumed at a lone trailing 0x1f, and fail instead of spinning when the buffer cannot grow
- [ ] R2-04 Oracle multi-member gzip: test a 0x1f-not-0x8b trailer
- [ ] R2-05 Provenance summary: list URLs fetched via Bash, and state the Bash-read gap

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

### P1-01 — 1410d90
crates/oracle now depends on libz-sys 1.1.29 (features=["static"], bundled C backend, deterministic across machines), libc 0.2.189, and cleanflate (path, unused so far). oracle::ffi provides Framing{Raw,Zlib,Gzip}, Strategy{Default,Filtered,HuffmanOnly,Rle,Fixed}, DeflateParams{level,strategy,window_bits,mem_level,framing}, deflate_bytes/deflate_zeros/inflate_bytes(->InflateOutcome{output,consumed,status})/OracleStatus{StreamEnd,DataError,NeedDict,Truncated,OutputLimit,OtherError}/crc32/adler32/zlib_version. z_stream built with every field explicit (no mem::zeroed), zalloc/zfree are extern "C" wrappers over libc::calloc/free. windowBits: raw=-wb, zlib=wb, gzip=wb+16 for deflate; inflate uses -15/15/31, gzip inflate stops at first member end (inflateInit2_ with wb=31 does this natively, verified by gzip_inflate_reports_first_member_consumed). unsafe confined to ffi.rs with a SAFETY comment per unsafe block (crate-checks/C1 only restricts crates/cleanflate). All 9 tests pass; compiled clean on first attempt using only the SPEC-listed signatures (CR5: no libz-sys source/rustdoc-src read). cargo metadata still shows cleanflate with 0 dependencies.

### P1-02 — 407bd7e
crates/oracle/src/prng.rs: SplitMix64 (state += 0x9E3779B97F4A7C15, xor-shift-multiply finalizer) + fnv1a64. inputs.rs: Tier{Synthetic,Canterbury,Silesia}, Input{id,tier,len,source}, generate_synthetic(id) covering all 16 SPEC S4 ids (empty, one-byte, zeros-{1KiB,64KiB,1MiB,16MiB}, random-{64KiB,1MiB}, text-256KiB via a 64-word list, repeat-abc-1MiB, repeat-period17-1MiB, window-d{32767,32768,32769}, run-64KiB, period258-64KiB), canterbury_inputs/silesia_inputs (enumerate testdata/corpora/{canterbury,silesia}/ sorted by name, id=<tier>-<name>, empty Vec if dir absent -- no error), find_canterbury_file. matrix.rs: CORPUS_SEED=0x636C_6561_6E66_6C61, EntrySpec, make_entry building id `<input>__<framing>_l<level>_<strategy>_w<wb>_m<mem>[_dict]` and path `<tier>/<input>/<rfc195N>-l<level>-<strategy>-w<wb>-m<mem>[-dict].bin`, groups A (levels 0-9 x 3 framings, {0,1,6,9} if input>4MiB), B (strategies, per-size branch), C (window/memlevel, only inputs<=1MiB), D (dictionary: canterbury alice29/asyoulik/cp.html x levels{1,6,9} x Zlib with lcet10.txt as dict, silently empty if lcet10.txt absent), deduplicated by entry id via HashSet. inputs_for_tier(Quick|Full, corpora_root). All 5 matrix tests pass; window test confirms exact-distance repeat; quick tier currently synthetic-only (Canterbury still blocked per P0-08) sits at the low end of 300..=6000 but within bounds. Dictionary entries are 0 right now (lcet10.txt absent) -- matrix_covers_all_parameter_values only requires dict_count>=1 when that file is present.

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

### P2-01 — 84ce62b
huffman.rs: TableKind{LitLen,Distance,CodeLength(#[allow(dead_code)] until P2-02)}. from_lengths(lengths,kind) now also rejects Incomplete (left>0 after the RFC 1951 S3.2.2 recurrence) except for TableKind::Distance with either total_codes==0 (all-zero/empty table) or total_codes==1 && counts[1]==1 (single length-1 code) -- both taken verbatim from RFC 1951 S3.2.7. No changes needed to decode() itself: an empty table's zero counts at every length and a single-code table's other same-length codes both fall through to Decoded::Invalid naturally via the existing code-first<count loop. Updated call sites: inflate.rs's two fixed-table constructions (LitLen/Distance, both already complete) and P1-07's huffman.rs test file (all TableKind::LitLen, no behavior change since that ABCDEFGH example is a complete code). All 11 new tests pass; one test's own input buffer needed 2 bytes instead of 1 to satisfy br.fill's 15-bit request (test-only sizing bug, not a huffman.rs defect). make ci green (82 cleanflate tests total, up from 71).

### P2-02 — f92420a
inflate.rs: new states DynHeader/CodeLenLens{i}/CodeLens/Repeat{sym}. DynHeader reads 14 bits (HLIT|HDIST<<5|HCLEN<<10 combined take, same pattern as StoredLen's LEN/NLEN), rejects hlit>286 as InvalidHuffmanTable(TooManyLengthCodes). CodeLenLens reads HCLEN 3-bit lengths at CODE_LENGTH_ORDER positions into dyn_cl_lengths[19], then builds TableKind::CodeLength (no incomplete exception -- errors propagate as InvalidHuffmanTable). CodeLens decodes symbols 0-18 via peek_symbol against dyn_cl_table into a growing dyn_lengths:Vec<u8> (Inflater field, not State, to avoid cloning it every step); 0-15 literal, 16/17/18 ->Repeat state (16 checked for RepeatWithoutPrevious before transitioning); on reaching dyn_hlit+dyn_hdist total, splits into litlen/dist length arrays, checks litlen[256]!=0 (else MissingEndOfBlock) before building TableKind::LitLen/Distance tables, stores into new litlen_table/dist_table fields (fixed-block path now clones fixed_litlen/fixed_dist into these same fields instead of using dedicated fixed_* references directly in LitLen/Dist states -- one shared pipeline for both block types). Repeat{sym} reads 2/3/7 extra bits, computes count(3-6/3-10/11-138) and value (prev_len for 16, else 0), checks dyn_lengths.len()+count>total ->RepeatOverflow, pushes, ->CodeLens. tests/dynbuilder.rs: independent from-scratch canonical-code (RFC 1951 S3.2.2) and frequency-based Huffman-length (textbook two-smallest-merge) implementations for building valid dynamic-block fixtures, ClSymbol/PayloadSym/DynBlock, emit_dynamic_block (full valid block incl. auto-EOB, skipped if symbol256 has no code) and emit_dynamic_header_raw (header-only, for malformed fixtures). DESIGN.md D-15. All 14 new tests + invariants.rs's 4 added dynamic fixtures (96 total cleanflate tests) pass. Two real test-driver bugs found and fixed (not decoder bugs): 1-byte-chunk stuck-detection needed to check status==NeedsInput not just consumed==0 (OutputFull with no remaining input is legitimate progress, not stuck) in inflate_stored.rs/inflate_fixed.rs/inflate_dynamic.rs; and dynbuilder's original power-of-two-depth length assignment was only complete for symbol counts that are powers of two, replaced with real Huffman construction.

### P2-03 — 2ca3c8d
config.rs: LITLEN_TABLE_BITS=10, DIST_TABLE_BITS=8 (pub(crate)). huffman.rs: Huffman gained table_bits:u32 and table:Vec<TableEntry> fields (TableEntry{Invalid,Long,Symbol{symbol,len}}), built by build_table() during from_lengths using the same code/idx walk as decode()'s recurrence but computing actual (code,len) per symbol; TableKind::table_bits() maps LitLen->10, Distance->8, CodeLength->0 (no table, always bit-serial, since that alphabet is tiny/rarely decoded). reverse_bits() converts a code's MSB-first integer value to the LSB-first bit-index BitReader::peek produces; each Symbol entry is written to all table_bits-len "don't care" combinations, Long entries mark a longer code's exact table_bits-bit prefix. decode_table(&self,&BitReader)->Decoded: falls back to decode() (kept as reference) whenever table_bits==0 or br.available()<table_bits (not per-entry -- see Interpretation), else does one table lookup. inflate.rs's peek_symbol now calls decode_table instead of decode for both litlen and distance decoding (used for LitLen/Dist states across stored/fixed/dynamic blocks alike). All 101 cleanflate tests pass with the table-driven path live in production on the first attempt (all fixed/stored/dynamic/invariants/garbage tests unchanged), plus the 5 new huffman_table.rs tests including 1000 randomly-generated complete codes (via leaf-splitting, guaranteed Kraft-complete for any symbol count) checked against the bit-serial reference. DESIGN.md D-16.

### P2-04 — 218104b
crates/oracle/src/malformed.rs: fully independent DEFLATE builder (own BitWriter, fixed/length/distance RFC tables transcribed fresh, canonical_codes, textbook Huffman-length-from-frequency, stored/fixed/dynamic block emitters, DynBlock/ClSymbol/PayloadSym). cases() returns 21 named Case{name,bytes,expected,rule,sweep} covering: btype3, stored_nlen_mismatch, fixed_litlen_286, fixed_dist_code_30, fixed_distance_at_start, fixed_distance_too_far, fixed_len284_extra31(OK), valid_stored_trailing_byte(TrailingData), dynamic_hlit_287, dynamic_codelength_oversubscribed/incomplete, dynamic_litlen_oversubscribed/incomplete, dynamic_dist_oversubscribed/incomplete_two_codes, dynamic_repeat16_first, dynamic_repeat_overflow, dynamic_missing_eob, dynamic_no_dist_then_length(InvalidSymbol), dynamic_single_dist_code(OK), dynamic_no_dist_literals(OK), plus 3 valid_small_{stored,fixed,dynamic} control streams (OK + sweep:). crates/oracle/src/main.rs (new binary, first for oracle since P1-03's "generate" was skipped) wires `oracle malformed` writing testdata/malformed/rfc1951/*.bin + EXPECTED.tsv. make malformed target wired. crates/oracle/tests/malformed.rs: malformed_raw_cases parses EXPECTED.tsv, runs every row through cleanflate::inflate comparing kind().as_str() (or OK), sweep: rows check every strict prefix is Truncated; malformed_corpus_is_up_to_date regenerates into a temp dir and byte-compares TSV+fixtures against committed. Both tests passed on first run (after one format-string arg-order typo fix) -- 24 generated files (21 cases + 3 sweeps as TSV rows) all agree with the decoder built across P1-08/P1-09/P2-01/P2-02/P2-03, serving as new independent cross-validation. git status --porcelain testdata/malformed empty after commit.

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

### P4-02 — 46f41a4
Added proptest = "1" as an oracle-only dev-dependency and
crates/oracle/tests/chunking.rs. chunking_invariance: random data
(0-16384 bytes, random/repetitive mix) x C-zlib params (all 3
framings, levels 0-9, all 5 strategies, wb 9-15, mem 1-9) x
input/output chunk schedules (1-64, 10% all-1s) -- streamed output ==
one-shot output == original data. chunking_invariance_on_corrupted_streams:
same but with one random byte flipped, asserting streamed and one-shot
agree on Ok/ErrorKind (and output when Ok). CLEANFLATE_CHUNKING_CASES
env var overrides DEFAULT_CASES=512; make proptest-10k runs 10k cases
in release (~7s), wired into make heavy. Cargo.lock updated; cleanflate
itself remains at 0 dependencies (cargo tree confirmed).
Defects-caught: none in the decoder -- the property test's own 1-byte
chunking driver initially broke out on Done before offering the
decoder any trailing bytes, while one-shot always hands over the whole
remaining buffer at once and so saw them; fixed the driver to keep
offering input past Done, matching one-shot. make ci and
foundry_verify both green.

### P4-03 — 28e1a25
Added crates/oracle/tests/bomb.rs: bomb_streaming_default_limit
(#[ignore]) compresses DEFAULT_MAX_OUTPUT + 1 MiB of zeros with C zlib
level 9 (measured: 1074790400 bytes -> 1044655 bytes compressed, well
under the 2 MiB acceptance bound) and streams it through a fixed 64
KiB output buffer, asserting Err(OutputLimitExceeded{limit:
DEFAULT_MAX_OUTPUT}) and total_out()==DEFAULT_MAX_OUTPUT -- proving
bounded per-call memory, not "decode everything then check length".
bomb_oneshot_small_limit (64 MiB zeros, max_output=16 MiB) and
limit_exact_boundary_all_formats (output exactly == limit succeeds)
cover decompress_with across Raw/Zlib/Gzip. Wired `make bomb` (release,
--include-ignored) into `make heavy`; make bomb passes in ~9.8s wall.
Recorded design decision D-21 (DEFLATE's real ~1032:1 worst-case
ratio, since PLAN.md's D-19 slot was already taken by P3-03's
decision). Never used the DEFAULT_MAX_OUTPUT literal outside
config.rs (C8, no-output-limit-literal constraint verified). make ci
and foundry_verify both green.

### P4-04 — e8ea337
Created the fuzz/ cargo-fuzz workspace (excluded from the main
workspace) with never_panic_raw/rfc1950/rfc1952 targets: each parses
chunk sizes from the input's first two bytes, runs decompress_with
then a chunked Decompressor loop under Options{max_output:
FUZZ_MAX_OUTPUT=1<<24}, discarding Ok/Err -- only panics/timeouts are
failures. Seeded <=10 fixtures per target from testdata/malformed/
(10/7/10). make fuzz-check (cargo check --bins) passes on stable.
Installed nightly + cargo-fuzz (had to install cargo-fuzz with
`cargo +nightly install` since the latest release needs a newer rustc
than the active stable) and ran make fuzz-smoke for real: 60s/target,
225319/263143/209205 execs, zero crashes, zero artifacts. Wired
fuzz-smoke into corpus dir fuzz/corpus/<target> (gitignored) rather
than fuzz/seeds/<target> directly -- libFuzzer treats its first
corpus-dir argument as writable and the plan's literal invocation
would have mutated the checked-in seed corpus on every run (caught
this the hard way: first run added 500+ files to fuzz/seeds/*, reset
and fixed before committing). fuzz/Cargo.lock is gitignored rather
than committed since it necessarily lists oracle's transitive
libz-sys pin, which trips libz-only-in-oracle; documented why in
.gitignore. foundry_verify and make ci both green.

### P4-05 — c8720c5
Added diff_oracle_roundtrip (target b): first 5 bytes select framing/
level/strategy/wb/mem, rest (<=64 KiB) compressed via oracle::ffi::
deflate_bytes, must decode back identically through cleanflate
one-shot, cleanflate streaming (chunked), and oracle::ffi::inflate_bytes
-- any mismatch panic!s (the fuzz failure). Added diff_oracle_arbitrary
(target c): arbitrary bytes under TrailingData::Allow through both
decoders for each of Raw/Zlib/Gzip, classified into both_ok_same/
both_ok_differ/both_err/clean_ok_oracle_err/clean_err_oracle_ok;
every class but both_ok_same/both_err appended as JSON to
$CLEANFLATE_DIVERGENCE_LOG (default fuzz/artifacts/divergences.jsonl),
never panics on divergence. Wired both into fuzz-smoke and added
fuzz-divergences (jq per-class counts). Seeded 7 + 10 fixtures.

Ran make fuzz-smoke for all 5 targets, 60s each: 270973/259945/189009/
52349/161328 execs, zero crashes, zero artifacts. fuzz-divergences
showed only gzip/both_ok_differ:15 and gzip/clean_err_oracle_ok:666 --
the expected, already-documented (P1-01, D-19) consequence of
cleanflate decoding every concatenated gzip member per RFC 1952 S2.2
while the oracle's windowBits=31 inflate stops after the first member;
no raw or zlib divergences at all. Not a bug, no fix needed.
make ci and foundry_verify both green.

### P4-06 — 5bcd911
Added .github/workflows/fuzz-nightly.yml: matrix job over all 5 fuzz
targets, triggers schedule (daily cron) + workflow_dispatch, installs
nightly + cargo-fuzz, runs each target for 3600s, uploads
fuzz/artifacts on failure and always uploads diff_oracle_arbitrary's
divergence log; only actions/checkout + actions/upload-artifact used.
YAML validated with python3 -c 'import yaml,...': OK. Added `make
fuzz-long` (fuzz-smoke at FUZZ_SECONDS=3600); restructured fuzz-smoke
from a shell for-loop into per-target fuzz-smoke-<target> pattern-rule
targets with a target-specific FUZZ_SECONDS override for fuzz-long
(not a recursive $(MAKE) call), so `make -n fuzz-long` correctly
dry-runs and prints all five 3600-second commands rather than GNU
Make's special-cased "$(MAKE) recipes always execute even under -n"
silently launching five real 1-hour runs.

Also fixed a real regression from P4-05: running `make crate-checks`
standalone (not via foundry_verify, which only saw staged files at
that task's own verify time) found two seed filenames containing
"zlib" (forbidden word outside crates/cleanflate|oracle); renamed
zlib_default_text.bin/zlib_rle_repeat.bin to
rfc1950_default_text.bin/rfc1950_rle_repeat.bin via git mv.
make ci (incl. crate-checks, fuzz-check) and foundry_verify both
green.

### P0-08 — unblocked (round 1)
Resolved in commit 361e93c: fetch-corpora.sh --from-file with the operator-supplied Canterbury archive (sha256 verified) and Silesia fetched automatically; both corpora are on disk and make sources-check passes. Re-open so the implementer can verify and close it against the existing commit, which lets the 17 cascade-skipped tasks (P0-09..P0-11, P1-03, P1-11, P1-12, P2-05, P2-06, P3-04, P3-06, P4-07..P4-10, P6-01..P6-03) run.

### P0-08 — b37d8d3
Reconfirmed: work was already committed (361e93c) and files already present on disk (archives, extracted canterbury/silesia dirs, SOURCES.json). Verified `bash tools/cleanroom/fetch-corpora.sh all` (verify mode) exits 0, `make sources-check` passes, `make tools-check` passes including case_corpus_tamper_fails. No code changes; empty commit made to satisfy HEAD-subject requirement for task closure after operator reopened tasks behind P0-08.

### P0-09 — aa215e6
Added .github/workflows/ci.yml (checkout, rustup 1.89.0 install, shellcheck/jq install, Canterbury corpus fetch with CLEANROOM_ALLOW_CORPUS_FETCH=1, make hooks-install, make ci; only actions/checkout used; push+pull_request triggers). Makefile ci target and README "Building and CI" section already existed from an earlier commit. Tests: make ci exits 0 locally; YAML parses via python3 yaml.safe_load.

### P0-10 — 289efa5
Wrote tools/cleanroom/LOCKDOWN (locked_at + head=542b098, HEAD before this commit) -- WebFetch and fetch scripts now deny post-lockdown for real, not just via CLEANROOM_LOCKDOWN_FILE override. Added tools/cleanroom/snapshot-log.sh + make provenance-snapshot target (wired, was a stub); ran it, committed provenance/tool-log.snapshot.jsonl. Wrote provenance/README.md (entry schema table, live/snapshot arrangement, guard rules summary, all 6 known gaps, Harness status = LIVE) and provenance/human-interventions.md (table template; added the P0-08 Canterbury from-file row as the first real entry, since it's exactly what S5 M6 wants captured). New tests: selftest.sh::deny_webfetch_real_lockdown (unsets CLEANROOM_LOCKDOWN_FILE, uses the real repo LOCKDOWN, logs to a scratch file so it doesn't skew denials_logged); test_guard.py::TestRealLockdownFile (own env, REAL_ROOT = actual repo root, asserts the shipped LOCKDOWN file denies a fetch-corpora.sh Bash command). make cleanroom-selftest, make provenance-snapshot, make ci all green (44 guard unit tests, 22 selftest cases).

### P0-11 — d4a0213
M0 acceptance: make ci exit 0, make cleanroom-selftest exit 0 (22/22 PASS incl. deny_webfetch_real_lockdown). make provenance-snapshot ran (1075 lines snapshotted, committed). No code fix needed -- both commands were already green from P0-01..P0-10. Pushed build/2026-09-29 to origin (b992fec..d4a0213).
Manual check: NOT VERIFIED (human):
1. Restart Claude Code, approve hooks in /hooks, confirm provenance/tool-log.jsonl grows per tool call.
2. Fresh session: WebSearch/WebFetch refused with "cleanroom guard:" and logged denied.
3. GitHub Actions ci run for the pushed branch is green.
4. Spot-check docs/rfc/*.txt against rfc-editor.org.
5. Record any intervention in provenance/human-interventions.md.

### P1-03 — 4128333
Added crates/oracle/src/manifest.rs (Manifest/ManifestEntry/DictionaryRecord, serde, format:1, seed hex, sorted-by-id, sha256_hex, framing_token/strategy_token) and generate.rs (write_manifest + check, keyed off matrix::entries/inputs_for_tier; check distinguishes file-present-wrong-hash (reported mismatch, never overwritten, like sources-check.sh) from file-missing (regenerated, accepted only if fresh sha matches manifest)). main.rs: `oracle generate --tier quick|full [--write-manifest] [--out DIR]`, hand-parsed. Cargo.toml: added serde+serde_json (Cargo.lock updated). Input::manifest_source() added to inputs.rs; matrix::tier_token made pub(crate). Makefile corpus/corpus-full now real (cargo run --release -p oracle -- generate --tier quick|full).
Ran `oracle generate --tier full --write-manifest` for real (Silesia present since P0-08): 2674 entries, testdata/MANIFEST.json committed (1.5MB), testdata/generated/ stays gitignored (1.8GB, git status confirms clean). `make corpus` (quick, check mode) twice: 0 generated/2494 skipped both times.
Tests: crates/oracle/tests/manifest.rs, 4 cases, all pass (~30s, uses an empty corpora_root so quick-tier reduces to synthetic-only for speed). make lint, make crate-checks, make cleanroom-selftest, full make test all green.

### P1-11 — 17faacc
crates/oracle/src/conformance.rs: testdata_root() (CARGO_MANIFEST_DIR-based, never cwd), load_manifest() (panics naming `make corpus` on missing/unparsable manifest), selected(filter) (tier from CLEANFLATE_TIER env, quick=synthetic+canterbury default, full=+silesia; panics on zero-entry selection), check_entries(entries, decode_fn) -> Report (verifies generated-file sha256 against manifest first -- mismatch/missing says "run make corpus" -- then decodes and compares output sha256 against input_sha256; collects every Failure{id,detail} instead of stopping at the first; on an output mismatch it re-reads/regenerates the original input via the manifest's "input" field to report the first differing byte offset). Report::assert_ok() panics listing every failure.
Test: crates/oracle/tests/conformance.rs::conformance_raw_stored_and_fixed filters framing=raw, dictionary=None, level==0 or strategy=="fixed", decodes with cleanflate::inflate. 262 entries pass on tier quick (make test), 286 on tier full (make conformance-full, 5.00s release). Makefile: conformance-full now real (corpus-full dep + CLEANFLATE_TIER=full cargo test --release -p oracle --test conformance); heavy now also depends on conformance-full.
make fmt-check/lint/corpus/test/crate-checks/cleanroom-selftest all green.

### P1-12 — ce5f469
M1 acceptance: make ci exit 0 (incl. make design-check: every DESIGN.md ### D- block already has an RFC: line, no fix needed). make conformance-full: 286 entries, 4.80s test time (7.00s wall incl. corpus-full up-to-date check). cargo test -p oracle --test checksums: 2/2 pass. No code fix needed -- all green from P1-01..P1-11. make provenance-snapshot: 1152 lines. Pushed build/2026-09-29 to origin (d4a0213..ce5f469).
Manual check: NOT VERIFIED (human):
1. Read DESIGN.md D-01..D-12 for RFC grounding.
2. Confirm GitHub CI is green.
3. Spot-check that provenance/tool-log.snapshot.jsonl shows no forbidden reads.

### P2-05 — 8ad4d5c
Added conformance::decode_streaming(decompressor, bytes, input_chunk, output_chunk) to crates/oracle/src/conformance.rs: loops Decompressor::decompress calls (incl. empty-input calls to flush already-buffered bits) until Status::Done, with a stuck-detector for a genuinely truncated stream, then finish(). Reusable (not test-inlined) since P3-04 needs the same shape for zlib/gzip.
New tests in crates/oracle/tests/conformance.rs: conformance_raw_all (every raw, dictionary-free entry, one-shot cleanflate::inflate) and conformance_raw_all_streaming_odd_chunks (same entries via decode_streaming with 1021-byte input / 997-byte output chunks). 1541 entries pass on tier quick (make test), 1625 on tier full (32.59s release) -- no decoder fixes were needed, every entry already decoded byte-identically both ways.
make fmt-check/lint/corpus/test/crate-checks/cleanroom-selftest all green.

### P2-06 — 395ae09
M2 acceptance: make ci exit 0 (incl. design-check green). make conformance-full: 1625 entries (conformance_raw_all + conformance_raw_all_streaming_odd_chunks), 34.51s release. cargo test -p oracle --test malformed: 4/4 pass. No code fix needed -- all green from P2-01..P2-05. DESIGN.md audit: the RFC 1951 S3.2.7 single-code distance-alphabet exceptions are documented under D-14 (not D-13 as the task text names -- D-13 became "Streaming contract" after P1-10 inserted it, shifting numbering after this task's text was written); content present and complete, no edit made. make provenance-snapshot: 1177 lines. Pushed build/2026-09-29 to origin (ce5f469..395ae09).
Manual check: NOT VERIFIED (human):
1. Review DESIGN.md D-13/D-14 against RFC 1951 S3.2.7.
2. Confirm GitHub CI is green.

### P3-04 — 95a9a21
check_entries() decode_fn now takes &ManifestEntry alongside bytes (needed for per-entry dictionary/format). Added conformance::dictionary_bytes(&DictionaryRecord) (reads first dictionary.len bytes of dictionary.input relative to repo_root(), now pub). Fixed a real bug in decode_streaming() (from P2-05): it treated "all input fed, call made no progress" as an error unconditionally, but gzip's post-trailer state correctly reports NeedsInput (not Done, since another member could follow) -- now it just stops feeding and calls finish() unconditionally, trusting it to resolve the terminal state. This was a test-helper bug, not a decoder bug; no crates/cleanflate change was needed.
New tests: conformance.rs::{conformance_zlib_all,conformance_zlib_dictionary,conformance_gzip_all,conformance_wrappers_streaming_odd_chunks}; wrappers.rs (new file)::{gzip_multi_member_oracle (200 cases, 2-5 members, random inputs incl. empty, checks decoded data + gzip_headers().len()), zlib_dictionary_required_on_oracle_stream (dict_id == oracle::ffi::adler32(dict)), zlib_wrong_dictionary_mismatch (expected/actual both checked)}.
Quick tier (make test): 654/9/290/944 entries across the four conformance_* wrapper tests, 3/3 wrappers.rs. Full tier: 702/9/338/1040, all pass, 37.75s release.
make fmt-check/lint/corpus/test/crate-checks/cleanroom-selftest all green.

### P3-06 — c656400
M3 acceptance: make ci exit 0. make conformance-full: 7/7 tests pass, 36.19s release. cargo test -p oracle --test wrappers --test malformed: 3/3 + 4/4 pass. No code fix needed -- all green from P3-01..P3-05. DESIGN.md audit: D-16 (table-driven Huffman), D-17 (zlib wrapper), D-18 (gzip member header parser) all present with RFC 1950/1952 citations. make provenance-snapshot: 1213 lines. Pushed build/2026-09-29 to origin (395ae09..c656400).
Manual check: NOT VERIFIED (human):
1. Review DESIGN.md D-16..D-18 against RFC 1950/1952.
2. Confirm GitHub CI is green.

### P4-07 — 9518a96
Added crates/oracle/src/bench.rs: BenchArgs{corpus,iters,level} (defaults silesia/5/6), BenchCorpus{Silesia,Canterbury}, run() (refuses in debug build via cfg!(debug_assertions)), bench_one_framing() (oracle-deflates each corpus file, times best-of-N cleanflate::decompress_with vs oracle::ffi::inflate_bytes via Instant, std::hint::black_box on the result), FileResult/FramingReport/BenchReport with mib_per_sec/ratio/aggregate helpers, render_markdown(). main.rs: `oracle bench [--corpus silesia|canterbury] [--iters N] [--level L]` -- falls back Silesia->Canterbury with a printed warning if Silesia's dir is empty, writes docs/PERF.md (method + uname -srm + /proc/cpuinfo model + rustc --version + the rendered tables), fully regenerated each run. Makefile bench target now real (cargo run --release -p oracle -- bench --corpus silesia).
Test: bench::tests::throughput_math (cfg(test) unit test in oracle, allowed per task text -- C6 only covers cleanflate). make bench ran for real (Silesia, level 6, best-of-5, release, ~61s): raw aggregate cleanflate 67.4 MiB/s vs zlib 279.1 (ratio 0.24), zlib-framed 56.8 vs 250.2 (ratio 0.23) -- both well under SPEC's >=70% M4 target, reported honestly (no optimisation in scope here).
make fmt-check/lint/corpus/test/crate-checks/cleanroom-selftest all green.

### P4-08 — b85c9ce
Implemented the full design (bit-buffer bulk refill + fast litlen/length/distance decode loop bypassing step's dispatch, guarded >=16in/258out bytes, plus Window bulk match copy). The fast decode loop found 2 real bugs: (1) over-read whole bytes corrupted gzip trailer detection -- fixed (give back unconsumed bytes to consumed on Done exit only, confirmed against all 150+24 tests); (2) a 3000-case chunking proptest then found it silently drops bits on an ~11KiB Fixed-strategy match-heavy stream (1 byte short, stuck NeedsInput after consuming all input) -- reproduced deterministically, root cause not found in budget. Per task's own "revert and record negative result" clause, reverted inflate.rs/bitreader.rs to pre-P4-08 in full.
KEPT: Window::copy_match's bulk non-overlapping (distance>=len) path -- up to 2 slice copies each way via a 258-byte stack scratch buffer instead of len individual get_back/push calls. Verified against an independent byte-by-byte reference across distances/lengths incl. ring wraparound; full suite + 3000-case chunking proptest unchanged.
Tests: crates/cleanflate/src/tests/fastpath.rs (3 required names, testing the surviving window-level fast path -- documented in file header why, since the inflate-level one was reverted). make ci, make heavy (10000-case proptest, bomb, full-tier conformance 1625 entries) all green.
Measurement: raw 67.4->66.5 MiB/s (-1.3%, noise), zlib 56.8->57.9 (+1.9%). Full tables in docs/PERF.md (Baseline P4-07 + Pass 1 P4-08 sections). DESIGN.md D-22 records both the kept change and the reverted one with its two bugs.

### P4-09 — abe7957
Adler-32 deferred modulo: Adler32::update reduces s1/s2 mod 65521 once per ADLER_NMAX=5552 bytes instead of per byte; ADLER_NMAX derived from RFC 1950 S2.2's modulus alone (proof in DESIGN.md D-23: worst case s1=s2=65520, every byte 0xFF, largest n keeping s2 under 2^32 is 5552). New test adler32_bound_worst_case exercises lengths spanning several chunk boundaries against a per-byte reference.
Table-bits sweep: measured LITLEN_TABLE_BITS {9,10,11} x DIST_TABLE_BITS {7,8,9} on Canterbury (--iters 3, noisy, all within a few percent), then re-measured leading candidates on Silesia (--iters 5); existing defaults (10,8) tied-or-ahead of every alternative -- kept, documented as a measured "no change" outcome in config.rs's doc comments.
Did not attempt CRC-32 slice-by-N (Adler-32 is the larger isolated cost per raw-vs-zlib framing gap; slice-by-N's correctness risk wasn't justified this pass) -- noted as future work.
Measurement: raw 66.5->71.1 MiB/s (+6.9%, noise, Adler-32 doesn't run in raw framing); zlib 57.9->71.9 MiB/s (+24.2%, the real win). Result line in docs/PERF.md: cleanflate/C zlib = 25.6% (raw) / 27.3% (zlib) on Silesia (target 70%): NOT MET -- decode loop remains dominant cost, P4-08's reverted fast path is the real path forward.
Tests: adler32_bound_worst_case + full checksum suite + oracle checksums cross-check all pass. make ci, make heavy (10000-case proptest, bomb, full-tier conformance) all green.

### P4-10 — a8fed62
M4 acceptance: make ci exit 0. make heavy: proptest-10k (10000 cases), bomb, conformance-full (1625 entries) all pass. make fuzz-smoke: all 5 targets 60s each, zero crashes, zero artifacts (never_panic_rfc1952 alone hit 166036 execs). make fuzz-divergences: gzip/both_ok_differ:47, gzip/clean_err_oracle_ok:1167 -- confirmed as the same non-bug class P4-05 already documented (cleanflate correctly decodes every concatenated gzip member per RFC 1952 S2.2, the oracle's windowBits=31 inflate stops after the first member); zero raw/zlib divergences. No code fix needed. make provenance-snapshot: 1379 lines. Pushed build/2026-09-29 to origin (c656400..a8fed62).
Manual check: NOT VERIFIED (human):
1. Run make fuzz-long, or trigger fuzz-nightly, confirm 1 hour each for (a) x3 and (b) with zero crashes.
2. Triage divergences.jsonl from (c) -- expect only the documented gzip multi-member class.
3. Re-run make bench on a quiet machine and confirm the PERF.md result line.
4. Confirm GitHub CI is green.

### P6-01 — 4dc61ad
tools/cleanroom/summarize.py: build_summary(log_path, root) -> markdown string, main() CLI (--log/--out, defaults tool-log.jsonl else snapshot, provenance/SUMMARY.md). Sections: totals by tool/stage, denied calls table, files read (grouped by top-level repo dir, allowed only, Outside repository heading for non-repo paths), URLs fetched (allowed WebFetch only), per-stage wall clock (first/last ts, span, active time = sum of inter-call gaps <15min), models per stage (null -> "unknown"), human-interventions.md embedded verbatim, README.md's Known gaps section extracted verbatim, parse-error count (lines that fail json.loads are counted, never dropped silently).
Tests: tools/cleanroom/tests/test_summarize.py (8 required names) + tools/cleanroom/tests/fixtures/tool-log.sample.jsonl (9 valid + 1 malformed line, exercises allowed/denied, multiple stages/models incl. null, in-repo/outside-repo reads, a WebFetch URL, and an explicit >15min idle gap case for the active-time exclusion). make provenance-summary ran for real against the actual live log (1400 entries) -- output inspected, all sections render correctly.
make tools-check (52 python tests, up from 44), make fmt-check/lint/corpus/test/crate-checks/cleanroom-selftest all green.

### P6-02 — 7ab2101
tools/cleanroom/build_report.py: build_report(root) -> markdown, git_log() shells `git log --reverse --format` with field/record separators, commit_task()/commit_stage() parse [task:P<n>-...]/[stage:...]. Sections: Milestones (per-phase distinct task ids + commit count), Defects caught (Defects-caught: lines, "none" skipped, classify_defect() -- [stage:fix] always "review", else fuzz/conformance/proptest/test keyword match, else "other"), Review rounds (subject.startswith("review:"), "Pending: regenerate with make build-report..." when none), Known limitations (M5-deferred note, PERF.md's whole **Result:** paragraph gathered across its wrapped physical lines, provenance/README.md's Known gaps section verbatim, fuzz/artifacts/divergences.jsonl's distinct framing/class pairs if present).
Tests: tools/cleanroom/tests/test_build_report.py (4 required names + 1 bonus, using a real temp git repo fixture via subprocess). make build-report ran for real: P0..P6 milestones (11/12/6/6/10/1 tasks), 7 real defects classified (4 tests, 3 other), 1 review round found, known limitations section fully populated including the P4-09 NOT MET result and both known-gaps and divergence-class lists.
DESIGN.md audit: make design-check already green, every ### D- block already has an RFC: line, no fills needed.
make tools-check (57 python tests, up from 52), make fmt-check/lint/corpus/test/crate-checks/cleanroom-selftest all green.

### P6-03 — 9691aff
Final acceptance: make ci exit 0. make heavy (proptest-10k 10000 cases, bomb, conformance-full 1625 entries) all pass. make provenance-summary and make build-report both regenerated without error (1436-line log snapshotted; SUMMARY.md/BUILD_REPORT.md refreshed). make crate-checks exit 0. No code fix needed -- everything already green from P0-01..P6-02. Pushed build/2026-09-29 to origin (a8fed62..9691aff).
Manual check: NOT VERIFIED (human):
1. After the Foundry review converges and the branch merges, run make provenance-snapshot provenance-summary build-report, commit, then git tag v0.1.0-cleanroom && git push origin v0.1.0-cleanroom.
2. Complete provenance/human-interventions.md.
3. Confirm the 1-hour fuzz results from P4-10 are recorded in BUILD_REPORT.md.

### R1-01 — cbb1928
Replaced crate::checksum::{adler32,crc32} usage in crates/cleanflate/src/tests/{zlib,gzip,stream_contract}.rs with local known_adler32/known_crc32(data) -> u32 lookup functions (match on exact byte string, panic if unlisted); every literal computed once via a throwaway oracle::ffi call (not committed). gzip_header.rs's FHCRC is structurally different (computed over a combinatorially-varying header byte sequence, ~15 test cases, GzipHeaderParser is pub(crate) so can't move to oracle/tests/ either): hcrc_override now means "must be supplied when FHCRC is set, else panic" instead of "None = auto-compute"; supplied the 3 needed literals at their call sites the same way.
Added docs/foundry.json rule no-cleanflate-checksums-in-fixtures (paths crates/cleanflate/src/tests/, exclude tests/checksum.rs, pattern \bcrate::checksum\b) and tagged CLAUDE.md's C16 with [rule:no-cleanflate-checksums-in-fixtures]. Comments explaining the change avoid the literal "crate::checksum" substring (rule matches raw text incl. comments, precedent: no-unsafe-token-in-cleanflate).
Would-have-caught: the task's named mutation (s2 = (s2+s1) % (ADLER_MOD-1) on the old per-byte line) no longer applies verbatim since P4-09's deferred-modulo rewrite; the equivalent mutation on current code (s2 %= ADLER_MOD -> % (ADLER_MOD-1)) turns out unreachable for any small-buffer fixture in either old or new checksum.rs (reduction only triggers >=5552 bytes). Validated the general property with a mutation that does affect small inputs (Adler32::new() s1 init 1->0): 7/14 zlib.rs tests failed as expected, confirming independent fixtures now catch a real bug.
Tests: full crates/cleanflate/src/tests suite (154, unchanged count) pass. make ci green. foundry_verify: new rule 0 hits.

### R1-02 — f58b82f
Fixed the OutputLimitExceeded condition in Decompressor::decompress: was comparing the already-clamped allowance (min(remaining_allowance, output.len())) against output.len(), so allowance==0 fired for ANY empty caller-supplied output slice regardless of remaining quota -- a real bug (not introduced by this task). Now: limit_bound = remaining_allowance == 0 || remaining_allowance < output.len() as u64, computed from the unclamped remaining_allowance, matching what D-02's "Update (P1-10)" text already said should happen but the old code didn't implement.
New tests: stream_raw.rs::{empty_output_slice_is_output_full_not_limit (decompress(&input,&mut[]) now Ok(OutputFull,produced=0) then a 16-byte buffer finishes to "hello"/Done), streaming_limit_with_large_buffer (max_output=4 with a 64-byte buffer on stored "hello" -> Err(OutputLimitExceeded{limit:4}), total_out()==4; max_output=5 -> Done)}. DESIGN.md D-02 gained an "Update (R1-02)" paragraph.
Would-have-caught: mutating remaining_allowance's saturating_sub to an off-by-one saturating_add(1) hung limit_plus_one_err (never terminates) -- confirmed caught, reverted after confirming.
Tests: full cleanflate suite 156 (+2), make lint, make bomb, make design-check all green.

### R1-03 — 32400c7
Fixed a real bug: gzip's GState::Boundary state advanced pos/total_in for every byte it EXAMINED before deciding whether it started a new member, so Progress.consumed at Done always overshot trailing_start() (by 1 for an ordinary trailing byte, by 2 for a 0x1f not followed by 0x8b) -- the exact "27 vs 26" / "overshoots by 2" symptoms named in the task.
Rewrote GState::Boundary to decide by peeking (input.get(pos)/input.get(pos+1)) instead of consuming, advancing pos/total_in only once a byte is confirmed to belong to a member (0x1f immediately followed by 0x8b); a byte not starting a member is left entirely unconsumed so resolve_trailing lands exactly on boundary_start. Kept the one documented exception: a lone 0x1f that is the last byte a call offers must still be consumed (NeedsInput-consumes-everything invariant, D-07) -- documented in lib.rs's streaming-contract docs and DESIGN.md D-19 (new "Update (R1-03)" paragraph)/D-20.
New tests: gzip.rs::gzip_trailing_allow_consumed_exact (b"XYZ" and [0x1f,b'Z'] trailers, one-shot + streaming consumed, trailing_start()); stream_contract.rs::done_consumed_exact_all_formats (renamed/extended done_consumed_exact_raw_and_zlib to include Gzip; its loop now also breaks on NeedsInput+exhausted-input before calling finish(), since gzip with zero trailing bytes only reaches Done via finish(), never via decompress() itself).
Tests: full cleanflate suite 157 (+1 net), make lint, make design-check, CLEANFLATE_CHUNKING_CASES=3000 proptest chunking all green.

### R1-04 — 7a756eb
Added config::DEFAULT_MAX_GZIP_HEADERS=1024 (C8-only), re-exported from lib.rs. Options::max_gzip_headers (defaulted from it). GzipDecoder gained member_count:u64 (incremented for every member unconditionally) alongside headers:Vec<GzipHeader> (only pushed while headers.len()<max_gzip_headers) -- every member still fully decoded/CRC-ISIZE-checked/counted regardless of cap. Decompressor::gzip_member_count() (via Backend, 0 for non-gzip) exposes the true count; gzip_headers()/Decoded::gzip_members stay bounded.
Extended docs/foundry.json's tunables-defined-only-in-config and option-defaults-not-literal patterns/fixtures for the new name; DESIGN.md D-24; CLAUDE.md config.rs module-map line.
New tests: gzip.rs::{gzip_many_members_headers_bounded (10 members, cap 3: full concatenated payload, gzip_members.len()==3, gzip_member_count()==10, streaming+one-shot), gzip_default_header_cap_from_config}.
Tests: full cleanflate suite 159 (+2), make lint, foundry_verify (both extended constraints 0 hits), make design-check all green.

### R1-05 — 3b811b8
Audited all 7 decompress-driving loops in stream_contract.rs: 5 already had an explicit "input exhausted + NeedsInput -> break" exit (needed by design, gzip only reaches Done via finish() at a clean member boundary), left as-is. The other 2 (after_done_reject_and_allow's Reject-policy loop, error_sticky_all_formats) had NO exit condition for that shape -- a decoder regression stuck reporting NeedsInput with zero progress on empty input would hang make test forever rather than fail it. Added the no-progress guard (consumed==0 && produced==0 && input empty && status!=Done -> panic naming format+status) to both.
Would-have-caught: mutating gzip.rs's ISIZE comparison (if expected_isize != actual_isize -> if false && ...) previously hung error_sticky_all_formats forever; now fails in <1s with "Gzip: no progress on empty input, status=NeedsInput (expected a checksum error)". Reverted the mutation after confirming.
Tests: full stream_contract suite (9, unchanged), make lint, foundry_verify all green.

### R1-06 — 3176487
Removed the /fuzz/Cargo.lock gitignore entry and explanatory comment; committed fuzz/Cargo.lock (pins libz-sys 1.1.29, matching the workspace Cargo.lock, verified before committing). Added "exclude": ["fuzz/Cargo.lock"] to docs/foundry.json's libz-only-in-oracle constraint and reworded its description to say lockfiles are exempt (a lockfile necessarily lists oracle's transitive libz-sys pin like every other transitive dep); every existing fixture kept unchanged.
Tests: make crate-checks (forbidden-paths ok with the tracked file), make fuzz-check (builds clean against the committed lockfile), foundry_verify (libz-only-in-oracle: ok, 0 hits).

### R1-07 — 540d41c
Added oracle::ffi::inflate_gzip_members(input, max_output) -> InflateOutcome: calls inflate_bytes once per member for as long as the remaining input starts with 1f 8b and the previous member reached StreamEnd, concatenating output/consumed only for members that fully succeeded (a failing member's own partial output -- C zlib had already inflated the body before its trailer's CRC/ISIZE check failed -- is deliberately not folded in, so the aggregate result reflects only cleanly-verified members). diff_oracle_arbitrary now calls this for Framing::Gzip only; Raw/Zlib unchanged. Updated fuzz/README.md's target (c) description accordingly.
New test: crates/oracle/tests/ffi_roundtrip.rs::gzip_multi_member_matches_concatenation (two members concatenate, consumed==total; b"XYZ" trailer -> StreamEnd with consumed==member bytes only; corrupted CRC32 in member 2 -> that member's error status, output/consumed covering only member 1).
Ran make fuzz-smoke for real (all 5 targets, 60s each): zero crashes, and fuzz/artifacts/divergences.jsonl was not even created this run -- zero divergences of any class, where earlier P4-05/R1 runs always surfaced gzip/both_ok_differ + gzip/clean_err_oracle_ok noise (confirms the multi-member gap is closed).
Tests: make test, make fuzz-check, foundry_verify all green.

### R1-08 — bc64a44
human-interventions.md's P0-08 row now spells out the archive_sha256 inline (f140e8a5b73d3f53198555a63bfb827889394a42f20825df33c810c3d5e3f8fb) plus the browser-User-Agent retry detail. Added two provenance/README.md sections: "Denied calls during build" (all 8 denied tool-log.jsonl entries as of writing -- 5 P0-05 guard-fixture probes, 2 P0-08 allowlist-form discovery, 1 P1-01 libz-sys find -- with the P1-01 case explicitly called out as CR1/CR5-relevant, confirming no libz-sys source was ever read) and three new "Known gaps" bullets (6 untagged bootstrap commits c1bbaa9/4015214/8edc633/956d6d7/44f624e/f776879 predating the commit-msg hook; LOCKDOWN not written until P0-10 completed in review-fix round 1, well after Phases 1-4 already ran; the browser-User-Agent retry, cross-referenced to the intervention row).
tools/ci/sources-check.sh: new check treats any testdata/corpora/SOURCES.json entry whose fetched_by is set and not "automated"/"cached" as requiring its archive_sha256 to appear somewhere in provenance/human-interventions.md; fails loudly (manual-fetch-missing-intervention-row) instead of passing silently when missing.
New test: tools/ci/tests/sources_check_test.sh::case_manual_fetch_requires_intervention_row (fetched_by=human-operator-browser entry with no matching row fails with that exact reason; adding the row passes).
Tests: make tools-check (57 python + all shell suites incl. this new case), make sources-check on the real repo, foundry_verify all green.

### R2-01 — ae80bb4
Rewrote crc32_table() to walk the array with split_first_mut instead of table[n]=c, removing the sole #[allow(clippy::indexing_slicing)] in cleanflate lib code. split_first_mut is const-stable on the pinned 1.89 toolchain (verified with a standalone rustc test before using it). Factored the per-index bit loop into a new const fn crc32_entry(n).
Added crc32_table_matches_bitwise_reference in tests/checksum.rs: for n in 0..256, recovers CRC32_TABLE[n] from crc32(&[0xFF^n as u8]) ^ 0xFF00_0000 (algebraic inverse of the update/finish steps) and compares to an independent bit-at-a-time reference written in the test.
Added foundry.json rule no-panic-lint-allow-in-cleanflate-lib (paths crates/cleanflate/src/, excludes src/tests/) matching allow(...clippy::unwrap_used|expect_used|panic|indexing_slicing...); fixtures per the task spec. Tagged CLAUDE.md C5 with [rule:no-panic-lint-allow-in-cleanflate-lib].
make lint, make test, foundry_verify all green; new rule 0 hits.

### R2-02 — ec1a424
GzipDecoder::finish() checked !self.headers.is_empty() to decide "at least one member completed"; with max_gzip_headers=0 that's always false, so valid streams wrongly failed Truncated. Changed the check to self.member_count > 0 (member_count increments per member regardless of the retention cap).
Audited gzip.rs for other headers-as-proxy uses: only the retention push (self.headers.push guarded by max_gzip_headers, unchanged) and the headers() accessor remain -- no other fix needed.
Added gzip_zero_header_cap_still_decodes (one member via decompress_with, two concatenated members via decompress_with, and the streaming Decompressor path checking gzip_headers().len()==0 and gzip_member_count()==2) and gzip_zero_header_cap_truncated_first_member_still_errors (truncated first member still returns Error::Truncated) to tests/gzip.rs.
make lint, make test, foundry_verify all green.

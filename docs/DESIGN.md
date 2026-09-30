# cleanflate design decisions

This document records every non-trivial design decision made while
building `cleanflate`, as evidence that the design flowed from the RFCs
and SPEC rather than from a prior implementation (SPEC S3.4).

## How to read this document

Each decision is one `### D-NN: <title>` block with three lines:

- `RFC:` the RFC section (or sections) the decision implements, or `n/a
  (API)` for decisions that are pure API surface with no RFC counterpart.
- `Decision:` what was chosen.
- `Rationale:` why.

`tools/ci/design-check.sh` (`make design-check`) mechanically verifies
every `### D-` heading is followed by an `RFC:` line before the next
heading.

Performance decisions carry a `[perf]` tag right after the id
(`### D-NN: [perf] <title>`). Per SPEC section 2.1, each such block also
needs a `Measurement:` line and the exact phrase "general technique; not
derived from any specific implementation"; `make design-check` fails a
`[perf]` block missing either.

## Decisions

### D-01: Error model

RFC: n/a (API)
Decision: A single `#[non_exhaustive] pub enum Error` with one variant per
SPEC S3.3 error category (bad header, preset-dictionary/dictionary
mismatch, bad block type, invalid stored-block length, invalid Huffman
table, invalid symbol, invalid distance, checksum mismatch, truncated
input, trailing data, output limit exceeded), plus a parallel
`ErrorKind` enum (`Error::kind()`, `ErrorKind::as_str()`) for
stringly-comparable but still typed matching (used by the malformed
corpus's `EXPECTED.tsv`).
Rationale: SPEC S3.3 requires "a typed error enum with distinct variants
... No stringly-typed errors." Splitting `Error` (which carries payload
data useful for debugging) from `ErrorKind` (a stable, payload-free tag)
lets test fixtures name an expected error without depending on exact
payload values.

### D-02: Output limit default

RFC: n/a (API)
Decision: `DEFAULT_MAX_OUTPUT = 1 << 30` (1 GiB), defined once in
`config.rs` and referenced everywhere else by name.
Rationale: SPEC S3.3 requires a documented default output limit as a
decompression-bomb guard. 1 GiB is larger than any conformance corpus
member (the largest Silesia file is about 51 MB) so no legitimate
corpus decode fails, while still bounding memory well below what a
1:1000+ DEFLATE bomb could otherwise produce.
Update (P1-10): the limit is enforced by handing the inflater at most
`max_output - total_out` bytes of the caller's output buffer per call. A
stream that produces exactly `max_output` bytes succeeds (the final
end-of-block symbol needs no output space); `OutputLimitExceeded` is only
returned when the inflater reports `OutputFull` *and* that allowance
(not the caller's raw buffer) was the binding constraint.
Update (R1-02): P1-10's own implementation of "that allowance was the
binding constraint" was wrong for a zero-length caller output slice: it
compared the already-clamped `allowance = min(remaining, output.len())`
against `output.len()`, so `allowance == 0` fired the error whenever the
caller passed an empty slice, even with a large remaining allowance --
indistinguishable from the allowance genuinely being exhausted. Fixed by
comparing `remaining_allowance` (before clamping) directly against
`output.len()`: the limit is the binding constraint iff
`remaining_allowance == 0` or `remaining_allowance < output.len()`,
which correctly lets `decompress(&input, &mut [])` return `Ok(Progress {
status: OutputFull, produced: 0, .. })` (resumable -- the same contract
`zero_length_output_slice_makes_header_progress` already established for
header-only progress, D-20) rather than `Err(OutputLimitExceeded)` when
the limit itself was never actually reached.

### D-03: Trailing-data policy

RFC: n/a (API)
Decision: `TrailingData::{Reject, Allow}`, defaulting to `Reject`.
Under `Allow`, the API reports how many input bytes were consumed so
the caller can inspect what follows.
Rationale: SPEC S3.3: "Trailing-data behavior is explicit and
configurable (error vs. return remaining bytes)." Rejecting by default
matches the "correctness first" priority (SPEC S1): silently accepting
garbage after a stream is a more common bug source than a legitimate
use case, so the lenient behavior is opt-in.
Update (P1-10): trailing bytes are detected both within the call that
reaches `Done` (leftover, unconsumed bytes in that same `input` slice)
and in any later call made after `Done`; both cases report the same
absolute input offset via `Error::TrailingData{offset}` or
`Decompressor::trailing_start()`.

### D-04: Single resumable decoder core

RFC: n/a (API)
Decision: The raw DEFLATE decoder is one resumable state machine
(`Inflater`) from Phase 1 onward. The one-shot APIs (`inflate`,
`zlib_decompress`, `gzip_decompress`) drive this same state machine to
completion in one call; the streaming `Decompressor` drives it
incrementally.
Rationale: SPEC S3.3 requires a streaming API that "must produce
identical output regardless of how input and output are chunked."
Retrofitting resumability onto a slice-based one-shot decoder later
would be a rewrite; building one code path from the start makes
chunking invariance structural rather than a property to test for and
hope holds.

### D-05: RFC 1951 constant tables' source

RFC: RFC 1951 S3.2.5, S3.2.6, S3.2.7
Decision: `tables.rs`'s length-code base/extra table, distance-code
base/extra table, code-length transmission order, and fixed Huffman
code lengths are transcribed directly from the tables printed in
`docs/rfc/rfc1951.txt`, with each constant's rustdoc citing the
specific RFC table it came from.
Rationale: These are the specification's own numbers; transcribing them
directly (and testing them against the RFC's own printed examples, e.g.
code 265 -> base 11, extra 1 bit) is the most direct way to keep the
implementation traceable to the RFC text rather than to a reimplementation
of it.

### D-06: Length code 284/285 boundary

RFC: RFC 1951 S3.2.5
Decision: Code 284 (base 227, 5 extra bits) is kept exactly as the RFC's
table states, even though its nominal range 227..=258 (32 values)
overlaps code 285's fixed value of 258. Extra-bit value 31 on code 284
therefore also decodes to length 258, the same value code 285 encodes
directly.
Rationale: The RFC's own printed table gives code 284 as "5 227-257" in
one place and effectively extends to 258 arithmetically
(227 + (2^5 - 1) = 258) when all 5 extra bits are set; RFC 1951 does not
special-case this, so the arithmetic decode is accepted as specified
rather than patched to avoid the overlap.

### D-07: Bit order and the resumable reader's buffer invariant

RFC: RFC 1951 S3.1.1
Decision: `BitReader` buffers bits LSB-first in a `u64`: each loaded byte
is OR'd in shifted left by the current bit count, and reads take the low
`n` bits. `fill` loads whole bytes only while fewer than `need` bits are
buffered, so decode steps consume exactly the bytes the stream format
requires -- after any completed step the buffer holds at most 7 bits
(whatever remainder is left over from the most recently loaded byte).
Rationale: RFC 1951 S3.1.1: "Data elements are packed into bytes...
starting with the least-significant bit of the byte." Buffering in a
`u64` (up to 8 bytes) rather than byte-at-a-time lets one `fill` call
satisfy a multi-byte read (e.g. a 32-bit stored-block length) without
looping per bit, while `fill`'s "only while count < need" rule keeps the
resumable contract exact: a caller that stops feeding input mid-symbol
gets `None` back with the reader's byte position advanced by exactly the
whole bytes it was able to load, never more.

### D-08: Window ring buffer and overlap semantics

RFC: RFC 1951 S2, S3.2.5
Decision: `Window` is a fixed 32,768-byte ring plus a `total: u64`
counter of all bytes ever pushed. `copy_match` writes the requested match
byte by byte, pushing each copied byte into the ring before reading the
next one, so a distance shorter than the match length (e.g. distance 3,
length 7) correctly repeats the pattern that copy is itself producing.
Rationale: RFC 1951 S2 draws distances from `(1..32,768)`, and S3.2.5's
length/distance pairs are explicit about a match being able to copy from
data the match itself is still writing (the classic `distance < length`
overlap, e.g. run-length patterns). Interleaving the push with the copy
(rather than copying from a snapshot of the window) is what makes that
overlap correct without special-casing it.

### D-09: Canonical Huffman construction and bit-serial decode

RFC: RFC 1951 S3.2.2, S3.1.1
Decision: `Huffman::from_lengths` counts codes per length and validates
only over-subscription in this task (RFC 1951 S3.2.2 steps 1-2's
recurrence, run as a "codes still available at this length" check);
incomplete-set validation is deferred to P2-01. `Huffman::decode` walks
lengths 1..=15, reading one new stream bit per length and folding it in
as `code = (code << 1) | bit` (MSB-first, per RFC 1951 S3.1.1), tracking
`first` (the first code value assigned at the current length, via the
same recurrence used to build the table) and `index` (how many shorter
symbols precede this length in the length-then-symbol-ordered `symbols`
list). A length matches as soon as `code - first < count` for that
length's code count. `decode` only peeks; the caller consumes.
Rationale: This is the direct generalization of RFC 1951 S3.2.2's own
canonical-code construction algorithm (its `next_code[bits]` recurrence)
into an incremental decode: because codes of a given length are
lexicographically consecutive starting at `first_code[len]`, checking
membership in `[first_code[len], first_code[len] + count)` at each length
is exactly the RFC's own encoding rule read backwards. Building the code
MSB-first one bit at a time (rather than peeking many bits and reversing
them) keeps the decoder naturally resumable: `NeedMoreBits` can be
returned the instant the buffer runs out, with no bits consumed either
way, satisfying the streaming API's chunking-invariance requirement
(SPEC S3.3) before that API exists.

### D-10: Checksums

RFC: RFC 1950 S2.2, RFC 1950 S8.2, RFC 1952 S2.3.1, RFC 1952 S8
Decision: Adler-32 and CRC-32 are implemented from the RFC 1950/1952
algorithm descriptions in `checksum.rs`: Adler-32 as two mod-65521 running
sums (s1 initialized to 1, s2 to 0), CRC-32 as a reflected-polynomial
(0xEDB88320) table built by a `const fn`, both exposed as incremental
(`Adler32`/`Crc32`) and one-shot (`adler32`/`crc32`) APIs. Cross-checked
against the C zlib oracle (`crates/oracle/tests/checksums.rs`) on 200
random buffers each, lengths 0..=10,000.
Rationale: SPEC S3.2 requires checksums "implemented in-crate, derived
from the RFCs' definitions." RFC 1950 S2.2 defines the Adler-32 running
sums and modulus directly; RFC 1950 S8.2 gives the reference update
algorithm. RFC 1952 S2.3.1 defines the CRC32 trailer field and S8 gives
sample CRC code establishing the reflected-polynomial table construction.
Cross-checking against the independent C zlib oracle (never read as
source, only linked via its public API per CR5) is the correctness signal
that these two from-scratch implementations agree with a real-world
zlib on arbitrary data, not just the RFC's own worked examples.

### D-11: The Inflater step contract

RFC: RFC 1951 S3.2.3, S3.2.4
Decision: `Inflater::step(input, output) -> Result<(consumed, produced,
InflateStatus), Error>` runs a `loop` over small, atomic sub-steps
(read block header; align and read LEN/NLEN; copy stored bytes). Each
sub-step either completes and advances the state, or returns immediately
with `NeedsInput` (bits/bytes ran out; whatever whole bytes were loaded
into the bit buffer are still counted as consumed) or `OutputFull` (a
byte was ready but `output` had no room). Errors are stored in the state
(`State::Failed`) and returned as a clone on every subsequent call, so a
caller that keeps calling `step` after an error keeps getting the same
answer rather than undefined behavior from a half-updated state machine.
Rationale: SPEC S3.3 requires the streaming API to "produce identical
output regardless of how input and output are chunked", including
1-byte chunks. Modelling the decoder as small atomic sub-steps rather
than one big function that assumes all its bits/bytes are available is
what makes arbitrary chunking safe: a sub-step either fully commits (bits
consumed, state advanced) or fully backs out (returns `NeedsInput`/
`OutputFull` having only advanced the byte position by whole bytes
actually loaded, never a partial bit-level commitment). RFC 1951 S3.2.3's
"read block header ... otherwise ..." loop and S3.2.4's byte-align-then-
LEN/NLEN sequence map directly onto the `BlockHeader`/`StoredLen`/
`StoredCopy` states.

### D-12: Fixed-Huffman symbol pipeline

RFC: RFC 1951 S3.2.5, S3.2.6
Decision: A fixed-Huffman block is decoded as a small chain of atomic
states -- `LitLen` (peek a literal/length symbol) -> `LenExtra` (read that
length code's extra bits) -> `Dist` (peek a distance symbol) ->
`DistExtra` (read its extra bits, then validate `distance <=
window.available()`) -> `Copy` (write matched bytes through
`Window::copy_match`, one output-buffer's worth at a time) -> back to
`LitLen`. A literal symbol only commits (consumes its Huffman code's
bits, writes the byte) once `output` is confirmed to have room; if not,
the call returns `OutputFull` having consumed nothing; the next call
re-decodes the same still-buffered bits for free, since `Huffman::decode`
is a pure peek. Length code 284 with all-ones extra bits (31) is accepted
as length 258 with no special case (DESIGN.md D-06 already covers why the
table allows this). All base+extra and distance arithmetic uses
`saturating_add`/`saturating_sub` (CLAUDE.md C15).
Rationale: RFC 1951 S3.2.5's decoding algorithm is itself exactly this
five-step loop (decode litlen; if a length, decode distance; copy). The
"don't consume until room is confirmed" discipline is what keeps the
whole pipeline resumable at arbitrary chunk boundaries (SPEC S3.3): every
state transition is only ever taken once its precondition (enough bits,
or enough output room) is already satisfied, so no state is ever "half
committed" across a `step` call boundary.

### D-13: Streaming contract, output limit and one-shot growth

RFC: n/a (API)
Decision: `Decompressor::decompress` wraps `Inflater::step` with three
additional, orthogonal concerns: (1) the output-limit check described in
the D-02 update; (2) trailing-data detection per the D-03 update; (3) its
own error stickiness (`self.error`), separate from `Inflater`'s internal
stickiness, because `OutputLimitExceeded` and `TrailingData` are
generated at this layer, not by the inflater. The one-shot
`decompress_with` drives a `Decompressor` with a `Vec<u8>` grown by
`pub(crate) fn next_len(len, limit) -> usize` (`max(len, 32768)` growth,
capped at `limit`, never shrinking), writing into the freshly-resized
tail and truncating to what was actually produced each round.
Rationale: SPEC S3.3's streaming rules ("with `NeedsInput`, all input was
consumed"; "input may be left unconsumed only with `OutputFull` or
`Done`") are already guaranteed by `Inflater::step` (P1-08's D-11), so
`Decompressor` only has to preserve them while layering its own concerns
on top, never contradict them. Capping the one-shot buffer's growth at
`options.max_output` means a hostile input's forced buffer growth is
bounded by the same limit that bounds decompressed output, so the
"decompression bomb guard" (D-02) protects one-shot callers too, not just
streaming callers who supply their own fixed buffer.

### D-14: Huffman table validation, incomplete-set exceptions

RFC: RFC 1951 S3.2.2, S3.2.7
Decision: `Huffman::from_lengths` now takes a `TableKind` (`LitLen`,
`Distance`, `CodeLength`) and rejects an incomplete code (fewer codes
present than the length counts allow, tracked via the same running "left"
capacity used for the over-subscription check) as `HuffmanError::Incomplete`,
with exactly two exceptions, both `Distance`-only, both taken verbatim
from RFC 1951 S3.2.7: a single code of length 1 ("there is a single code
length of one, with one unused code"), and no codes at all ("there are no
distance codes used at all"). A single literal/length code alone is
*not* exempted and is rejected as incomplete, since the RFC's exception
text names only the distance alphabet. Decoding the unused code of a
single-code table, or decoding from an empty table, falls out of the
existing `decode` loop as `Decoded::Invalid` with no special-casing
needed: an empty table has zero counts at every length, and a
single-code table's other codes at that length simply never match any
assigned range.
Rationale: RFC 1951 leaves general incomplete-code handling unspecified
but explicitly carves out these two distance-alphabet cases as legal,
naming the literal/length case by omission (it is a complete code in
every valid DEFLATE stream). Rejecting incompleteness by default,
narrowly excepting only what the RFC text names, keeps the decoder
strict everywhere the spec is silent and lenient only where it is
explicit.

### D-15: Dynamic-Huffman block decoding

RFC: RFC 1951 S3.2.7
Decision: A dynamic block is decoded as a chain of atomic states --
`DynHeader` (read HLIT/HDIST/HCLEN as one 14-bit field, reject HLIT >
286) -> `CodeLenLens` (read HCLEN 3-bit lengths in `CODE_LENGTH_ORDER`,
build the code-length table with `TableKind::CodeLength`) -> `CodeLens`
(decode literal/length + distance code lengths as one combined sequence
of `HLIT + HDIST` values, handling repeat codes 16/17/18 via a `Repeat`
state) -> table construction (`TableKind::LitLen`/`TableKind::Distance`,
rejecting a zero length for symbol 256 as `MissingEndOfBlock` before
attempting table construction) -> the existing `LitLen`/`Dist`/`Copy`
pipeline from P1-09, now driven by per-block `litlen_table`/`dist_table`
fields instead of always the fixed tables. Code 16 with no prior decoded
length is `RepeatWithoutPrevious`; any repeat whose count would push the
sequence past `HLIT + HDIST` is `RepeatOverflow`; repeats are allowed to
run from the literal/length lengths into the distance lengths, since RFC
1951 treats them as "a single sequence of HLIT + HDIST + 258 values" (we
track only the `HLIT + HDIST` code-length portion; the `+258` refers to
the code-length alphabet's own transmitted count elsewhere in the RFC's
accounting, not an addend here). All extra-bit and length/distance
arithmetic reuses the existing `saturating_add`/`saturating_sub`
discipline (C15).
Rationale: This is RFC 1951 S3.2.7's own two-phase structure (first
transmit the meta-alphabet, then use it to transmit the real alphabets'
lengths) mapped directly onto atomic, resumable states, the same
discipline P1-08/P1-09 established for stored and fixed blocks. Reusing
`litlen_table`/`dist_table` as ordinary fields (rather than duplicating
the `LitLen`/`Dist`/`Copy` states per block type) means fixed and dynamic
blocks share one decode pipeline once their tables are built, so a bug
fix or property proven for one applies to both.

### D-16: Table-driven Huffman fast-decode path

RFC: RFC 1951 S3.2.2, S3.1.1
Decision: Each `Huffman` table carries a `1 << table_bits`-entry lookup
table (`LITLEN_TABLE_BITS = 10`, `DIST_TABLE_BITS = 8`, both in
`config.rs`; the code-length alphabet gets `table_bits = 0`, i.e. no
table, since it is tiny and decoded only a handful of times per block).
Every table index whose low `len` bits equal a symbol's canonical code,
bit-reversed (since the table is indexed by `BitReader::peek`'s
LSB-first bit order, while codes are constructed MSB-first per RFC 1951
S3.1.1), gets a `Symbol` entry, written redundantly across every
combination of the remaining `table_bits - len` "don't care" bits; a
code longer than `table_bits` gets a single `Long` marker at its
`table_bits`-bit prefix. `decode_table` only ever consults the table
when `br.available() >= table_bits`; otherwise it defers to the
bit-serial `decode` (kept as the reference implementation) exactly as it
also does for a `Long` entry.
Rationale: The "peek `bits`, unavailable bits read as 0" framing works
for a `Symbol` entry once `available() >= len` (the entry does not
depend on padding beyond its own length, by construction), but it is
*not* safe for deciding between `Long`, `Invalid` and a different
`Symbol` at a shorter prefix when `available() < table_bits`: the fake
zero-padded bits can select a different table index than the true future
bits would, misidentifying the entry's kind, not just its length. Gating
the whole table lookup behind `available() >= table_bits` sidesteps that
case entirely rather than reasoning about which partial-availability
states are safe to trust, at the cost of an early fallback to the slower
path only in the (performance-irrelevant, correctness-first) regime near
a chunk boundary -- P4-08/P4-09 may revisit this once performance is
being tuned deliberately.

### D-17: zlib wrapper (header, preset dictionary, trailer)

RFC: RFC 1950 S2.2, S2.3
Decision: `ZlibDecoder` is a small resumable state machine (`Cmf` ->
`Flg` -> optionally `DictId` -> `Body` -> `Trailer` -> `Done`) around a
plain `Inflater`, sharing the same `step(input, output) ->
(consumed, produced, InflateStatus)` shape so `Decompressor` can drive it
through a `Backend` enum without any format-specific logic in its own
limit/trailing/stickiness code. Header fields are checked in the order
FCHECK, CM, CINFO, exactly as RFC 1950 S2.2 lists them. CINFO is
validated (`<= 7`) but never used to bound match distances -- the window
is always the full 32 KiB regardless, since RFC 1950 defines CINFO as an
encoder hint ("the base-2 logarithm of the LZ77 window size... used by
the compressor") with no decoder-side enforcement duty stated. FDICT
clear makes any supplied `Options::dictionary` a no-op (RFC 1950 gives no
mechanism to apply a dictionary without FDICT/DICTID). The Adler-32
trailer is accumulated incrementally over exactly the bytes `Inflater`
produces during `Body` (never the dictionary, matching "excluding any
dictionary data"). `Inflater::with_dictionary` reuses `Window::prefill`
(now exercised for the first time) for both the zlib-with-FDICT path and
a bare `Format::Raw` decompression given `Options::dictionary` directly.
Rationale: Treating the wrapper as "one more backend behind the same
`step` shape" rather than special-casing zlib throughout `Decompressor`
keeps the output-limit and trailing-data logic (D-02/D-03 updates,
D-13) correct for zlib for free: neither depends on which backend
produced the bytes, only on the produced-byte count and the leftover
input after `Done`.

### D-18: Gzip member header parser

RFC: RFC 1952 S2.1, S2.3.1
Decision: `GzipHeaderParser` is a byte-at-a-time resumable state machine
(`Id1` -> `Id2` -> `Cm` -> `Flg` -> `Mtime` -> `Xfl` -> `Os` ->
optionally `ExtraLen`/`ExtraData` -> optionally `Name` -> optionally
`Comment` -> optionally `Hcrc` -> `Done`), independent of any particular
alignment (gzip header fields are always byte-, never bit-, oriented per
RFC 1952 S2.1). Every header byte except the FHCRC field's own two bytes
feeds a running CRC-32 (`crate::checksum::Crc32`), matching "the two
least significant bytes of the CRC32 for all bytes of the gzip header up
to and not including the CRC16" (S2.3.1) literally, including bytes that
end up discarded because a field exceeded `max_gzip_field` -- the CRC
covers what was transmitted, not what was retained. FNAME/FCOMMENT are
read byte-by-byte until a zero terminator (itself CRC'd but never
stored); FEXTRA reads a 2-byte little-endian XLEN then exactly that many
opaque bytes. All multi-byte fields (MTIME, XLEN, the FHCRC's CRC16) are
little-endian per RFC 1952 S2.1 ("stored with the least-significant byte
first"), the opposite convention from RFC 1950's big-endian Adler-32.
Rationale: RFC 1952's own decode algorithm is exactly this sequence of
fixed and length-prefixed/terminated fields; modelling each field
transition as its own atomic state (rather than, say, buffering the
whole header before parsing) keeps this parser resumable under
arbitrary chunking with no separate code path for the "enough bytes
arrived in one call" case, consistent with every other resumable state
machine in this crate (D-07, D-11, D-15, D-17).

### D-19: Gzip members, trailer, and multi-member boundary

RFC: RFC 1952 S2.2, S2.3, S2.3.1
Decision: `GzipDecoder` chains `MemberHeader(GzipHeaderParser)` ->
`Body` -> `Trailer` -> `Boundary` -> (`MemberHeader` again, or `Done`)
per member, with the running `Inflater`/CRC-32/byte-count for the
in-progress member kept as plain `GzipDecoder` fields rather than inside
the state enum (an enum variant embedding a whole `Inflater` -- which
carries the 32 KiB window -- would make every other, tiny, variant pay
for its size; `ZlibDecoder`, D-17, already does the same). The CRC-32
and ISIZE trailer fields are read as raw little-endian bytes (S2.3,
matching D-18's endianness note) and compared only once both are fully
buffered, so a truncated trailer is reported as `Error::Truncated`
rather than a checksum error. At a member boundary, RFC 1952 S2.2 says
members are "simply concatenated, with no additional information before,
between, or after them", which means the *only* way to know whether the
stream continues is to look for the next member's own `1f 8b` magic;
anything else at a boundary is trailing data under SPEC S3.3's
trailing-data policy. Because that decision requires consuming
disambiguating bytes before it can be made, `GzipDecoder` resolves its
own trailing-data policy internally (via `TrailingData` passed into its
constructor) rather than letting `Decompressor`'s generic post-`Done`
leftover check handle it as for Raw/Zlib; `Backend::owns_trailing_data`
tells `Decompressor` to skip its own leftover check for the `Gzip`
variant, and `Backend::trailing_start`/`GzipDecoder::trailing_start`
expose the offset so both paths report through the same
`Decompressor::trailing_start()` accessor. A lone `0x1f` at true
end-of-input (matching the resumable "a `NeedsInput` call must consume
everything offered" invariant used throughout this crate, e.g. D-07)
is buffered in `boundary_pending` rather than left unconsumed, since a
second byte might still arrive; `GzipDecoder::finish` (called from
`Decompressor::finish`, via the new `Backend::finish` delegation) applies
the trailing-data policy to that still-ambiguous byte, or confirms a
clean stop when the boundary is unambiguous and at least one member has
completed. `GzipHeaderParser::new_primed` skips re-CRCing/re-checking
the `1f 8b` bytes the boundary logic already consumed and verified
before committing to a new member.
Update (R1-03): the original `Boundary` implementation consumed each
byte it examined (`pos`/`total_in` advanced) before deciding whether it
started a new member, so `resolve_trailing`'s caller-visible `consumed`
always overshot `trailing_start()` by however many bytes had been
examined (1 for an ordinary non-`0x1f` trailing byte, 2 for a `0x1f` not
followed by `0x8b`). Fixed by deciding at each byte via `input.get(pos)`/
`input.get(pos + 1)` peeks and only advancing `pos`/`total_in` once a
byte is confirmed to belong to a member (`0x1f` immediately followed by
`0x8b`); a byte found not to start a member is left unconsumed entirely,
so `resolve_trailing` always lands exactly on `boundary_start`. The one
narrow exception (documented in `lib.rs`'s streaming-contract docs)
applies only to streaming `Decompressor` callers: a lone `0x1f` that is
the very last byte a call offers must be consumed then (the
`NeedsInput`-consumes-everything invariant, D-07, leaves no choice
before a second byte can arrive to disambiguate it), so in that
specific case a streaming caller's running `consumed` total can end up
one byte ahead of `trailing_start()`. `decompress_with` (R2-03) sees
the whole input in one shot and has no such excuse, so it reports
`Decoded::consumed` as `trailing_start()` (falling back to its own
accumulated `in_pos` only when `trailing_start()` is unset) rather than
the raw bytes-consumed total, undoing that one-byte overshoot for the
one-shot API.
Rationale: Keeping the "is this trailing data" decision inside the
format that alone has enough information to make it (only gzip's own
concatenation rule creates the header-magic ambiguity; zlib and raw
streams have no equivalent "maybe another one follows" question) avoids
threading gzip-specific lookahead logic into `Decompressor`'s otherwise
format-agnostic core, matching the "backend owns its own protocol rules,
`Decompressor` owns cross-cutting policy" split already established by
D-17.

### D-20: Streaming contract finalized across Raw/Zlib/Gzip

RFC: n/a (API) -- SPEC S3.3
Decision: `Decompressor::decompress`'s three-way `Status` and
`Decompressor::finish` behavior (documented in full in the crate's
top-level rustdoc, "Streaming contract" section) is now explicit and
uniform across all three formats: `NeedsInput` always means all of
`input` was consumed; `OutputFull` always means either the caller's own
output slice filled up (resume with a fresh buffer) or the configured
output-limit allowance was exhausted, which is surfaced as
`Error::OutputLimitExceeded` rather than a `Status`, never both; `Done`
always means `consumed` stops exactly at the stream's logical end,
regardless of format-specific trailing-data mechanics (D-19's gzip
member-boundary lookahead decides by peeking, not consuming, so it
resolves before `Done` is returned without overshooting -- see D-19's
R1-03 update for the one lone-`0x1f`-at-end-of-call exception, which is
streaming-only: `decompress_with` corrects for it, per R2-03).
Errors are sticky (checked first, before touching the backend, on every
call). `finish()` is delegated per backend (`Backend::finish`): Raw/Zlib
require `is_done()`; Gzip additionally resolves an outstanding
lone-`0x1f`-at-EOF ambiguity via the configured `TrailingData` policy
(D-19). A `decompress` call made after `Done` with non-empty input is
handled once, uniformly, by `handle_post_done_input`, independent of
which backend produced `Done`.
Rationale: The format-specific decoders (`Inflater`, `ZlibDecoder`,
`GzipDecoder`) already agree on a `step(input, output) -> (consumed,
produced, InflateStatus)` shape (D-17); this decision is about pinning
down, and testing, that `Decompressor` never leaks a format-specific
wrinkle into the public contract callers write chunking loops against --
the same 1-byte-chunk loop already used throughout this crate's own test
suite (D-07 and friends) must work unmodified for any `Format`.

### D-21: Output-limit hardening and the zip-bomb fixture's real ratio

RFC: n/a (API) -- SPEC S3.3 (Limits), S5 M4
Decision: `Decompressor`'s output-limit check (D-02/D-03, exercised
throughout `Decompressor::decompress`) is proven under an actual
decompression-bomb-shaped input in `crates/oracle/tests/bomb.rs`, always
via `cleanflate::DEFAULT_MAX_OUTPUT` rather than a repeated `1 << 30`
literal (C8; enforced by the `no-output-limit-literal` constraint).
`bomb_streaming_default_limit` compresses `DEFAULT_MAX_OUTPUT + 1 MiB`
zero bytes with C zlib at level 9 and streams the result through a fixed
64 KiB output buffer, proving the limit is enforced with bounded
per-call memory rather than by first materializing the (would-be
multi-gigabyte) output and checking its length afterward.
`bomb_oneshot_small_limit` and `limit_exact_boundary_all_formats` cover
the one-shot path (`decompress_with`) and the inclusive boundary
(output exactly equal to the limit succeeds) across Raw, Zlib and Gzip.
DEFLATE's actual worst-case compression ratio is bounded by its own
format: the longest match (RFC 1951 S3.2.5, length code 258) costs at
minimum one length/literal Huffman code (as little as 7-9 bits under
the fixed table, D-yet-earlier fixed-code-length decisions) plus one
distance code, so in the best real case roughly 258 bytes of output
cost on the order of 2 bytes of compressed input -- about 1032:1, not
the 1000:1-from-1-KiB figure sometimes quoted informally (which would
require decompressing 1 KiB of *compressed* input into 1 MiB of output
using fewer bits per match than DEFLATE's own alphabet can encode).
The `bomb_streaming_default_limit` fixture's measured ratio (roughly
1074790400 zero bytes compressing to about 1.04 MB) matches this
figure.
Rationale: A hardening test that only ever compresses a few kilobytes
would not actually prove the limit stops a large decompression before
it exhausts memory; using `oracle::ffi`'s real C zlib compressor (never
`cleanflate` itself, consistent with C16) to build a fixture whose
compressed size is small but whose *claimed* decompressed size is
larger than the configured limit is the only way to exercise the limit
check under a realistic adversarial shape, while keeping the test suite
itself fast (the fixture's own compression is a few hundred
milliseconds of C code, not a multi-gigabyte allocation).

### D-22: Window bulk match copy; fast decode loop attempted and reverted

RFC: RFC 1951 S2, S3.2.5 (n/a beyond that for the performance-pass
mechanics themselves)
Decision: `Window::copy_match` now special-cases the non-overlapping
case (`distance >= len`, i.e. every source byte is already final
history from before this match started): it copies via up to two
contiguous slice copies each way (splitting only at the ring buffer's
own wraparound point), through a small `[u8; 258]` stack scratch buffer
(never a heap allocation, since RFC 1951 S3.2.5 bounds a match to 258
bytes) rather than `len` individual `get_back`/`push` calls. The
overlapping case (`distance < len`, RFC 1951 S2's classic
distance-shorter-than-length repeat) is unchanged: byte-by-byte,
pushing each copied byte into the ring before reading the next, exactly
as D-08 describes, since a bulk copy cannot see a byte the match itself
is still producing. A second, more ambitious change -- a fast decode
loop in `Inflater::step`, entered only while `state == LitLen` and at
least 16 input bytes/258 output bytes remained, decoding several
literal/length+distance symbols directly with a proactively
bulk-refilled bit buffer instead of returning to the outer state
dispatch after each one -- was implemented, found to corrupt gzip's
per-member trailer boundary detection (fixed: giving back whole
unconsumed buffered bytes to `consumed` on a `Done` exit restores the
resumable-reader invariant D-07 describes), then found to silently drop
bits on a heavily-matched ~11 KiB stream (a `CLEANFLATE_CHUNKING_CASES=3000`
proptest case), and reverted in full rather than shipped with an
unresolved correctness bug.
Rationale: PLAN.md P4-08 explicitly allows reverting an unsuccessful
performance change and recording the negative result rather than
shipping it; a decompressor silently dropping bytes is worse than a
missed throughput target (SPEC S1: safety and correctness rank above
performance). The window bulk-copy path is provably safe (checked
against an independent byte-by-byte reference across distances/lengths
including ring wraparound, plus the full existing suite and 3000
chunking-invariance proptest cases, all unchanged) even though it is
not, by itself, a measurable win on the Silesia corpus at level 6 (see
docs/PERF.md's Pass 1 table): most matches there are short and/or
short-distance, the overlap-eligible case this change does not help.

### D-23: Adler-32 deferred-modulo performance pass

RFC: RFC 1950 S2.2
Decision: `Adler32::update` processes `data` in chunks of `ADLER_NMAX`
(5552) bytes, accumulating `s1`/`s2` with plain `u32` addition inside a
chunk and reducing both mod `ADLER_MOD` (65521) only once per chunk,
instead of after every byte. `ADLER_NMAX` is derived, not borrowed from
any implementation: with `s1`/`s2` both at their maximum possible
already-reduced value (`ADLER_MOD - 1 = 65520`) and every subsequent
byte the worst case `0xFF` (the maximum possible contribution), `s1`
after `n` bytes is `65520 + 255n`, and `s2` (the running sum of `s1`
after each byte) is `65520 + n*65520 + 255*n*(n+1)/2`; the largest `n`
keeping that under `2^32` is 5552 (`s2 = 4,294,690,200`), since 5553
already exceeds it (`s2 = 4,296,171,735`). `adler32_bound_worst_case`
(`crates/cleanflate/src/tests/checksum.rs`) exercises exactly this
worst case (a run of `0xFF`) at lengths spanning several chunk
boundaries (`ADLER_NMAX - 1`, `ADLER_NMAX`, `ADLER_NMAX + 1`, and
beyond), checked against a direct per-byte reference computation.
Rationale: `(a + b) % m == ((a % m) + b) % m` for repeated addition, so
deferring the reduction to once per chunk is numerically identical to
reducing every byte, but replaces up to 5552 divisions with one; SPEC
S5 M4's performance pass calls this out by name as a candidate
(alongside CRC-32 multi-table, not attempted in this pass -- see
docs/PERF.md's Pass 2 for the measured effect and why slice-by-N CRC-32
was left as future work) precisely because it changes nothing
observable (same modulus, same per-byte contribution, just less often
reduced) while removing the dominant per-byte cost.

### D-24: Bounded retained gzip member headers

RFC: RFC 1952 S2.2
Decision: `GzipDecoder` gains `max_gzip_headers` (from the new
`Options::max_gzip_headers`, defaulting to
`config::DEFAULT_MAX_GZIP_HEADERS = 1024`) and a `member_count: u64`
field. Every member header is still parsed and validated (including its
FHCRC) exactly as before; `self.headers.push(header)` only happens while
`self.headers.len() < self.max_gzip_headers`, while `member_count` is
incremented unconditionally for every member. `Decompressor::gzip_headers`/
`Decoded::gzip_members` therefore report at most `max_gzip_headers`
entries; the new `Decompressor::gzip_member_count()` reports the true
total regardless of the cap.
Rationale: RFC 1952 S2.2 puts no limit on how many members a gzip stream
concatenates ("simply concatenated, with no additional information
before, between, or after them"), and each `GzipHeader` can itself carry
retained FNAME/FCOMMENT/FEXTRA bytes (already bounded per-field by
`max_gzip_field`, P3-03/D-18) plus a timestamp and flags -- a hostile
stream of many tiny members would otherwise grow `Vec<GzipHeader>`
without bound even though `max_gzip_field` already bounds each entry's
own size, violating SPEC S1's "bounded memory" regardless of the
per-field cap. Capping the count while still decoding, checksumming and
counting every member (rather than stopping early) preserves the
existing guarantee that every byte of every member's compressed payload
is verified; only the header bookkeeping is bounded.

### D-25: decompress_with reports the corrected consumed offset, and refuses to spin when its buffer cannot grow

RFC: n/a (API) -- SPEC S3.3
Decision: `decompress_with` (R2-03) now reports `Decoded::consumed` as
`Decompressor::trailing_start()` whenever that is set, falling back to
its own accumulated `in_pos` only when it is not. This undoes the one
streaming-only overshoot D-19/D-20 document (a lone trailing `0x1f`
consumed at true end-of-call before a second byte can disambiguate it):
since `decompress_with` sees the whole input in one call, it has no
excuse to report that extra byte as consumed the way a streaming caller
does. Separately, its growth loop now tracks whether the `write_region`
it offered `decompress` was empty *before* the call, and if the result
is `Status::OutputFull` with that empty region, returns
`Error::OutputLimitExceeded` directly instead of `continue`-ing. This is
a backstop independent of `Decompressor::decompress`'s own limit-bound
arithmetic: once `buf.len()` reaches `options.max_output` (`next_len`'s
cap), every further call would otherwise offer an empty write region,
get `OutputFull` back, and `continue` forever if that inner arithmetic
ever disagreed about whether the limit was truly binding.
Rationale: A one-shot API that reports its own resumable-only quirks to
the caller is a worse contract than the streaming one it wraps; folding
the correction in here keeps `Decoded::consumed` meaning exactly what it
says without asking every one-shot caller to know about D-19's
exception. The `write_region`-was-empty backstop turns a possible
silent hang (SPEC S1's "no CPU exhaustion" requirement, checked here by
construction rather than only by the `remaining_allowance` calculation
in `Decompressor::decompress`) into a defined error the moment the
buffer genuinely cannot grow further, regardless of which of the two
independent checks would have caught it.

### D-26: Decode-path switch and a uniform invalid-code rule

RFC: RFC 1951 S3.2.2, S3.2.7
Decision: Every decoder layer carries a `DecodePath { Fast, Reference }`.
`Reference` decodes Huffman symbols bit-serially and copies matches byte
by byte; `Fast` uses the table lookup and the bulk copy. Only the
`test-hooks` feature exposes a way to request `Reference`, so a default
build always runs `Fast`. Both decoders share one invalid-code rule: a
code is `Invalid` as soon as at least one buffered bit shows that it
matches no code and no longer code can extend it. Per RFC 1951 S3.2.7
this only happens for the two incomplete distance tables it allows (no
codes, or one code of length 1), and both are decided after 1 bit. The
bit-serial walk returns `Invalid` after failing at length `len` when no
code longer than `len` exists; the table path returns `Invalid` for an
`Invalid` entry and delegates to the bit-serial walk while fewer than
`table_bits` bits are buffered.
Rationale: The differential test needs a path that is simple enough to
trust. Before this rule the table path could answer `Invalid` while the
bit-serial path answered `NeedMoreBits` for the same buffer, so the two
could disagree about the error for a stream that is both truncated and
invalid. The rule can only turn `Truncated` into `InvalidSymbol` for such
a stream; whether a stream is accepted or rejected is unchanged.

### D-27: [perf] Bulk bit refill with an exact give-back invariant

RFC: RFC 1951 S3.1.1, S3.2.4
Decision: On the Fast path the bit reader tops its buffer up a word at a
time (`refill_bulk`: one 64-bit little-endian read when at least 8 input
bytes remain, single bytes otherwise, never more than 63 buffered bits)
whenever fewer than 15 bits (the longest RFC 1951 code) are buffered, and
then decodes. This supersedes D-07's "at most 7 bits between steps" with
an invariant stated at the exits of `Inflater::step`:
(a) a `NeedsInput` exit has consumed all of `input`, and the buffer holds
only bits of the pending sub-step, fewer than it requires;
(b) an `OutputFull` or `Done` exit goes through one shared exit helper
that gives back (`give_back`) every whole buffered byte above the bits of
a decoded-but-unconsumed symbol. That symbol is the pending literal when
`LitLen` stops for lack of output room, and its code length is the number
of bits kept; otherwise at most 7 bits stay. The input position moves
back by the number of bytes returned;
(c) after LEN and NLEN of a stored block the reader gives back every
whole byte, so the stored payload is read straight from `input` (a
defensive drain remains for any byte that would still be buffered).
Consequence: after every `step` return the Fast reader holds exactly the
bits the exact-fill Reference reader holds, so the per-call `consumed`
count is identical on both paths, and a zlib or gzip trailer that follows
`Done` starts at the right byte. Bytes given back were always loaded in
the same call, because bits carried in from an earlier call are consumed
by the first sub-step. Error exits are unchanged: they are sticky and
report no `consumed`.
Rationale: The buffer was refilled one byte at a time and held at most 7
bits between steps, so the table lookup, which needs 10 bits, was skipped
for about half of all symbols (46% of literal/length and 48% of distance
symbols took the bit-serial walk, one refill per 1.08 bytes). Loading a
word at a time removes that, but a reader that holds bytes it does not
yet need must hand them back at every point where the caller regains
control. The phase-1 attempt at a faster loop failed exactly there: it
over-read, corrupted a gzip trailer, and left a stream that was one byte
short stuck at `NeedsInput`. Stating the invariant at the exits, giving
back through one helper instead of at each return, and comparing per-call
`consumed`, `produced` and status against the Reference path (the
differential traces and `tests/bitreader_bulk.rs`) make that class of bug
visible at the call where it happens.
Provenance: general technique; not derived from any specific implementation.
Measurement: docs/perf/P1-01.json against docs/perf/P1-01-before.json:
Silesia level 6 one-shot raw 51.1 -> 79.3 MiB/s (WIN), zlib 49.7 -> 75.5,
gzip 45.3 -> 57.8; streaming raw 50.6 -> 78.2.

### D-28: [perf] Two-level lookup tables for every symbol

RFC: RFC 1951 S3.2.2, S3.1.1, S3.2.7
Decision: The Fast path decodes every literal/length, distance and
code-length symbol through lookup tables; the bit-serial walk (`decode`)
stays as the Reference path and as the specification the tables are
checked against. A table has a first level of `1 << table_bits` entries
indexed by the next `table_bits` stream bits (codes are stored reversed
because the stream is read LSB-first), and, for every first-level prefix
that some longer code shares, a second-level table appended in the same
`Vec`, indexed by the following `max_len - table_bits` bits, where
`max_len` is the longest code under that prefix. Short codes fill every
index that agrees with them on their own bits; a second-level table is
filled the same redundant way. Entries are packed into one `u32`: an
`Invalid` marker, a `Symbol` (symbol and full code length), or a `Sub`
(offset and width). The code-length alphabet gets a single 7-bit level:
RFC 1951 S3.2.7 sends its lengths as 3-bit fields, so its codes are at
most 7 bits. The widths of the other first levels stay in `config.rs`.
Lookup, with `a` buffered bits and unavailable bits reading as zero:
index the first level; follow a `Sub` into its table with the next bits;
a `Symbol` of length at most `a` is the answer; a `Symbol` longer than
`a` means `NeedMoreBits`; an `Invalid` entry with `a >= 1` is `Invalid`
(the uniform rule of D-26). This supersedes D-16's gate that used the
table only when at least `table_bits` bits were buffered.
Why the lookup equals `decode` for every buffer state: a prefix code is
prefix-free, so the real bits, if they contain a whole code, contain
exactly one. If a code of length `len <= a` matches the real bits, the
zero padding cannot change which entry the lookup reaches, so it returns
that code. If no code of length at most `a` matches, then any entry the
padded index reaches is longer than `a` (a shorter match would have been
found), so the lookup says `NeedMoreBits`, as the serial walk does when it
runs out of bits. The `Invalid` cases exist only in the two incomplete
distance tables of S3.2.7, where an index bit already proves no code
matches. `tests/huffman_twolevel.rs` checks the equality for fixed,
exception and random complete tables (with 15-bit codes) at every buffer
size and for widths 7 to 12.
Rationale: After bulk refill (D-27) the buffer usually holds enough bits,
but a code longer than the first level and a nearly empty buffer both
still sent the decoder down the serial walk, one bit per step. Making
the lookup total removes that walk from the Fast path and gives the
later fast loop a single lookup per symbol. The extra memory is bounded:
at most `1 << (15 - table_bits)` entries per first-level prefix.
Provenance: general technique; not derived from any specific implementation.
Measurement: docs/perf/P1-02.json against docs/perf/P1-02-before.json:
NEUTRAL on Silesia (level 6 one-shot raw 78.9 -> 81.9 MiB/s, +3.8%, inside
twice the noise), WINs on several Canterbury aggregates; kept as the
prerequisite of the fast loop.

### D-29: [perf] Fast decode loop with margins

RFC: RFC 1951 S3.2.5, S3.2.6
Decision: On the Fast path, when the state is "decode a literal/length
symbol" and both margins hold, `Inflater::step` hands control to a loop
(`fast_loop.rs`) that decodes literals, matches and the end-of-block
symbol without cloning the state or returning to the state machine for
each symbol. The margins are tunables in `config.rs`: at least
`FAST_LOOP_MIN_INPUT` input bytes and `FAST_LOOP_MIN_OUTPUT` output bytes
must remain at the start of every iteration. Each iteration tops the bit
buffer up to at least 48 bits when it holds fewer, does a literal/length
lookup, and for a length symbol reads the extra bits, does a distance
lookup, reads the distance's extra bits, checks the distance against the
history, and copies the whole match. The loop stops, and the state machine
continues, when a margin no longer holds; at the end-of-block symbol (the
loop sets the next-block state); and on any error, with the same `Error`
value the state machine reports (invalid symbol for an invalid lookup,
distance codes 30 and 31, and codes 286 and 287; invalid distance with the
same `distance` and `available`). After a margin stop, one state-machine
dispatch runs before the loop is tried again, so every pass makes
progress.
Why no step inside an iteration can run out of bits or room: an iteration
needs at most 15 + 5 + 15 + 13 = 48 bits (the longest literal/length code,
the extra length bits, the longest distance code, the extra distance
bits). With at least 8 input bytes left, one word refill leaves at least
56 bits buffered, so the lookups always see a whole code and can never
report that more bits are needed. The longest match is 258 bytes (RFC 1951
S3.2.5), and an iteration starts only with at least that much output room,
so a match is always copied whole and never has to stop halfway. The
input margin is larger than 8 so the loop stays away from the end of the
input, where the ordinary rules of D-27 apply.
Exits follow the invariant of D-27 unchanged: the loop only consumes
whole symbols, keeps the buffer and the positions in locals, writes them
back on every exit, and leaves any whole buffered bytes for the shared
exit of `step` to give back, so per-call `consumed`, `produced` and status
equal the Reference path's. The phase-1 attempt at such a loop (D-22) was
reverted for a correctness bug in exactly this exit handling; the
differential traces, the boundary tests at margin minus one, at the margin
and above, and the fixtures make the class of bug visible where it
happens.
Rationale: After bulk refill and total table lookup, most of the time left
was the outer dispatch and a state clone per symbol (about 31% of samples
at the M0 profile). Nothing about RFC 1951 needs a state change between
two symbols of one block, so a tight loop that only leaves for the reasons
above removes that overhead without changing what is decoded.
Provenance: general technique; not derived from any specific implementation.
Measurement: docs/perf/P2-01.json against docs/perf/P2-01-before.json:
Silesia level 6 one-shot raw 83.1 -> 132.2 MiB/s, zlib 80.2 -> 131.0, gzip
66.1 -> 92.6; streaming raw 83.8 -> 135.5.

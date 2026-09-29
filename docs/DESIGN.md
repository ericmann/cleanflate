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
member-boundary lookahead already resolves before `Done` is returned).
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

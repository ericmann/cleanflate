//! The only place tunable numbers get values (CLAUDE.md C8). Everything
//! else in the crate refers to these constants by name.

/// Default maximum decompressed output size (SPEC S3.3: "configurable
/// maximum output size (decompression-bomb guard) ... default
/// documented"). 1 GiB: larger than any conformance corpus member, small
/// enough to stop a zip-bomb-style input quickly.
pub const DEFAULT_MAX_OUTPUT: u64 = 1 << 30;

/// Default cap on how many bytes of each gzip FNAME/FCOMMENT/FEXTRA field
/// are retained in memory (the rest is still parsed/consumed, just not
/// stored), bounding memory use on a hostile header (SPEC S1: "bounded
/// memory").
pub const DEFAULT_MAX_GZIP_FIELD: usize = 65_536;

/// Default cap on how many parsed [`crate::GzipHeader`] values a
/// streaming decode retains (RFC 1952 S2.2 puts no limit on how many
/// members a gzip stream concatenates; SPEC S1 requires bounded memory
/// regardless). Every member past this cap is still fully decoded,
/// checksummed and counted -- only the header itself stops being kept
/// around after this many.
pub const DEFAULT_MAX_GZIP_HEADERS: usize = 1_024;

/// Bits of lookahead of the first level of the literal/length lookup
/// table (SPEC S5 M2: "a table-driven fast decode path"). Codes longer
/// than this are resolved through a second-level table, never through a
/// bit-serial walk (D-28). P4-09 swept
/// `{9,10,11}` against every `DIST_TABLE_BITS` in `{7,8,9}` (Canterbury,
/// then a head-to-head of the leading candidates on Silesia at more
/// iterations for a cleaner read); this value's throughput was tied
/// with or ahead of every other candidate within measurement noise, so
/// it was kept rather than swapped for a difference too small to trust
/// (see docs/PERF.md's Pass 2 numbers).
pub(crate) const LITLEN_TABLE_BITS: u32 = 10;

/// Bits of lookahead the distance table-driven decode path indexes on.
/// Measured alongside `LITLEN_TABLE_BITS` in the same P4-09 sweep; kept
/// for the same reason.
pub(crate) const DIST_TABLE_BITS: u32 = 8;

/// Input bytes that must remain for one iteration of the fast decode loop
/// to start (SPEC S6 M2). One iteration consumes at most 48 bits (15 for
/// the literal/length code, 5 extra length bits, 15 for the distance code,
/// 13 extra distance bits), and the loop refills to at least 48 bits with a
/// single word read. That read needs at least 8 bytes, so this must be at
/// least 8; the margin above that keeps the loop away from the end of the
/// input, where the state machine takes over. Initial value; re-swept in
/// P4-03.
pub(crate) const FAST_LOOP_MIN_INPUT: usize = 16;

/// Output bytes that must remain for one iteration of the fast decode loop
/// to start (SPEC S6 M2). The longest match is 258 bytes (RFC 1951 S3.2.5),
/// so this must be at least that: an iteration never has to stop halfway
/// through a match for lack of room. Initial value; re-swept in P4-03.
pub(crate) const FAST_LOOP_MIN_OUTPUT: usize = 258;

/// Smallest growth step of the one-shot decompressor's output buffer, in
/// bytes: each growth adds `max(len, ONESHOT_GROWTH_FLOOR)` (SPEC S6 M3).
/// Moved here from a literal in `stream.rs`; re-swept in P4-03.
pub(crate) const ONESHOT_GROWTH_FLOOR: usize = 32_768;

/// Size hint for the first output allocation of a one-shot decode of raw or
/// zlib data: this many output bytes per input byte (SPEC S6 M3). Only a
/// hint: growth continues from it and the output never depends on it.
/// Initial value; re-swept in P4-03.
pub(crate) const ONESHOT_INITIAL_RATIO: usize = 4;

/// Largest first allocation, in bytes, a size hint may cause (SPEC S6 M3),
/// so a hostile hint (a gzip ISIZE of 4 GiB, say) cannot force a large
/// up-front allocation: memory stays proportional to the output actually
/// produced beyond this fixed cap. Initial value; re-swept in P4-03.
pub(crate) const ONESHOT_MAX_PREALLOC: usize = 64 << 20;

/// Bytes the CRC-32 update consumes per step, and so the number of lookup
/// tables it uses (SPEC S6 M4; DESIGN.md D-32 derives the tables). Must be
/// at least 4: the running register is folded into the first four bytes of
/// each chunk. Initial value; re-swept in P4-03.
pub(crate) const CRC32_SLICES: usize = 8;

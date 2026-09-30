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

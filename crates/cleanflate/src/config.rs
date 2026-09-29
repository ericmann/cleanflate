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

/// Bits of lookahead the literal/length table-driven decode path
/// indexes on (SPEC S5 M2: "a table-driven fast decode path"). Codes
/// longer than this fall back to the bit-serial walk. Tuned properly in
/// P4-09; this is a reasonable starting value.
pub(crate) const LITLEN_TABLE_BITS: u32 = 10;

/// Bits of lookahead the distance table-driven decode path indexes on.
/// Tuned properly in P4-09; this is a reasonable starting value.
pub(crate) const DIST_TABLE_BITS: u32 = 8;

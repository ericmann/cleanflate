//! The public `Format`/`Options`/`TrailingData` types (SPEC S3.3).

use crate::config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_GZIP_HEADERS, DEFAULT_MAX_OUTPUT};

/// Which framing to decode. `#[non_exhaustive]` keeps future additions
/// non-breaking.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Raw RFC 1951 DEFLATE, no wrapper.
    Raw,
    /// RFC 1950 zlib wrapper.
    Zlib,
    /// RFC 1952 gzip wrapper (one or more concatenated members).
    Gzip,
}

/// What to do with bytes remaining after the logical end of a stream
/// (SPEC S3.3: "Trailing-data behavior is explicit and configurable").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingData {
    /// Fail with [`crate::Error::TrailingData`] if any input remains.
    #[default]
    Reject,
    /// Succeed, reporting how much input was consumed; the caller can
    /// inspect the rest.
    Allow,
}

/// Decompression options shared by every entry point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Maximum number of bytes to produce before failing with
    /// [`crate::Error::OutputLimitExceeded`].
    pub max_output: u64,
    /// Policy for bytes remaining after the logical end of the stream.
    pub trailing: TrailingData,
    /// Preset dictionary, if the stream requires one (zlib FDICT).
    pub dictionary: Option<Vec<u8>>,
    /// Maximum bytes retained per gzip FNAME/FCOMMENT/FEXTRA field.
    pub max_gzip_field: usize,
    /// Maximum number of parsed gzip member headers retained by a
    /// streaming decode (every member past this cap is still decoded and
    /// counted, just not kept in [`crate::Decoded::gzip_members`] /
    /// [`crate::Decompressor::gzip_headers`]).
    pub max_gzip_headers: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_output: DEFAULT_MAX_OUTPUT,
            trailing: TrailingData::default(),
            dictionary: None,
            max_gzip_field: DEFAULT_MAX_GZIP_FIELD,
            max_gzip_headers: DEFAULT_MAX_GZIP_HEADERS,
        }
    }
}

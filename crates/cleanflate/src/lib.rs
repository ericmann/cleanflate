#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
#![warn(missing_docs)]

//! `cleanflate` is a clean-room, zero-dependency, safe-Rust implementation of
//! DEFLATE decompression (RFC 1951) with zlib (RFC 1950) and gzip (RFC 1952)
//! wrappers, built from the RFC texts alone.
//!
//! # Streaming contract
//!
//! [`Decompressor::decompress`] can be fed arbitrarily chunked input and
//! output buffers -- down to one byte at a time on either side -- and
//! always produces identical output regardless of chunking (SPEC S3.3).
//! Every call returns a [`Progress`] whose [`Status`] means exactly one
//! of:
//!
//! - [`Status::NeedsInput`]: all of `input` was consumed; call again with
//!   more.
//! - [`Status::OutputFull`]: the caller's `output` slice filled up before
//!   the stream finished (or, for [`decompress_with`]'s internal growth,
//!   [`Options::max_output`] was reached, which surfaces as
//!   [`Error::OutputLimitExceeded`] instead of `OutputFull`); call again
//!   with a fresh output buffer.
//! - [`Status::Done`]: the stream is fully decoded; `consumed` stops
//!   exactly at its logical end (any input after that is not counted as
//!   consumed by this call). [`Format::Gzip`] has one narrow exception,
//!   and it applies only to streaming [`Decompressor`] callers, never to
//!   [`decompress_with`]: deciding whether a trailing `0x1f` starts
//!   another member requires a second byte, and if that lone `0x1f` is
//!   the very last byte a call offers, it must be consumed then (the
//!   `NeedsInput` rule above leaves no choice) even though it turns out,
//!   once the next byte arrives, to have been trailing data all along;
//!   in that one case [`Decompressor::trailing_start`] can be one byte
//!   behind the total `consumed` a caller has accumulated across calls.
//!   [`decompress_with`] sees the whole input at once, so it has no such
//!   excuse: its `Decoded::consumed` always equals
//!   [`Decompressor::trailing_start`] whenever that is set. Every other
//!   boundary byte is only ever peeked before the decision, never
//!   counted as consumed until it is confirmed to belong to a member.
//!
//! Errors are sticky: once `decompress` or [`Decompressor::finish`]
//! returns `Err`, every later call on that `Decompressor` returns a clone
//! of the same error. Output written into `output` during a call that
//! returns `Err` is unspecified and must not be relied upon.
//!
//! [`Decompressor::finish`] confirms the stream reached a valid stopping
//! point: for [`Format::Raw`] and [`Format::Zlib`], `Ok(())` iff
//! [`Status::Done`] was already reached, else [`Error::Truncated`]; for
//! [`Format::Gzip`], see [`GzipHeader`] and the gzip module docs for the
//! member-boundary rules `finish` additionally resolves.
//!
//! Calling `decompress` again after `Done` (extra bytes past the end of
//! the logical stream) is governed by [`Options::trailing`]:
//! [`TrailingData::Reject`] fails with [`Error::TrailingData`];
//! [`TrailingData::Allow`] succeeds with `Ok(Progress { consumed: 0,
//! produced: 0, status: Status::Done })`, and
//! [`Decompressor::trailing_start`] reports where the trailing bytes
//! began.
//!
//! [`Decompressor::reset`] returns a decompressor to its freshly-constructed
//! state while keeping its allocations, for decoding many streams in a
//! row.
//!
//! ```
//! use cleanflate::{Decompressor, Format, Options, Status};
//!
//! # fn main() -> Result<(), cleanflate::Error> {
//! // "abc" as a stored (uncompressed) raw DEFLATE block.
//! let compressed = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
//!
//! let mut d = Decompressor::new(Format::Raw, Options::default());
//! let mut out = [0u8; 16];
//! let mut produced = 0usize;
//! let mut input = &compressed[..];
//! loop {
//!     let progress = d.decompress(input, &mut out[produced..])?;
//!     input = &input[progress.consumed..];
//!     produced += progress.produced;
//!     if progress.status == Status::Done {
//!         break;
//!     }
//! }
//! d.finish()?;
//! if &out[..produced] != b"abc" {
//!     return Err(cleanflate::Error::Truncated);
//! }
//! # Ok(())
//! # }
//! ```

#[macro_use]
mod stats;

mod bitreader;
pub mod checksum;
mod config;
mod decode_path;
pub mod error;
mod fast_loop;
pub mod gzip;
mod huffman;
mod inflate;
mod options;
mod sink;
mod stream;
mod tables;
#[cfg(feature = "test-hooks")]
pub mod test_hooks;
mod window;
mod zlib;

pub use config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_GZIP_HEADERS, DEFAULT_MAX_OUTPUT};
pub use error::{ChecksumKind, Error, ErrorKind, HeaderError, HuffmanError};
pub use gzip::GzipHeader;
pub use options::{Format, Options, TrailingData};
#[cfg(feature = "stats")]
pub use stats::Stats;
#[cfg(feature = "stats")]
pub use stream::decompress_with_stats;
pub use stream::{
    decompress_with, gzip_decompress, inflate, zlib_decompress, Decoded, Decompressor, Progress,
    Status,
};

#[cfg(test)]
mod tests;

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

mod bitreader;
pub mod checksum;
mod config;
pub mod error;
pub mod gzip;
mod huffman;
mod inflate;
mod options;
mod stream;
mod tables;
mod window;
mod zlib;

pub use config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_OUTPUT};
pub use error::{ChecksumKind, Error, ErrorKind, HeaderError, HuffmanError};
pub use gzip::GzipHeader;
pub use options::{Format, Options, TrailingData};
pub use stream::{
    decompress_with, gzip_decompress, inflate, zlib_decompress, Decoded, Decompressor, Progress,
    Status,
};

#[cfg(test)]
mod tests;

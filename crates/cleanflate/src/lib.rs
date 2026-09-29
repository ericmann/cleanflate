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
mod huffman;
#[allow(dead_code)] // driven by the public API, arriving in P1-10
mod inflate;
mod options;
mod tables;
mod window;

pub use config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_OUTPUT};
pub use error::{ChecksumKind, Error, ErrorKind, HeaderError, HuffmanError};
pub use options::{Format, Options, TrailingData};

#[cfg(test)]
mod tests;

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

#[allow(dead_code)] // fully consumed starting in Phase 2 (inflate.rs)
mod bitreader;
pub mod checksum;
mod config;
pub mod error;
#[allow(dead_code)] // consumed starting in Phase 2 (inflate.rs)
mod huffman;
mod options;
#[allow(dead_code)] // consumed starting in Phase 2 (inflate.rs)
mod tables;
#[allow(dead_code)] // consumed starting in Phase 2 (inflate.rs)
mod window;

pub use config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_OUTPUT};
pub use error::{ChecksumKind, Error, ErrorKind, HeaderError, HuffmanError};
pub use options::{Format, Options, TrailingData};

#[cfg(test)]
mod tests;

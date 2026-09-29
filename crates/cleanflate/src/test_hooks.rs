//! Hooks for tests and differential fuzzing only. Not part of the stable
//! API: compiled only with the `test-hooks` feature.

pub use crate::decode_path::DecodePath;

use crate::error::Error;
use crate::options::{Format, Options};
use crate::stream::{Decoded, Decompressor};

/// Like [`crate::decompress_with`], but decoding with `path`.
pub fn decompress_with_path(
    format: Format,
    input: &[u8],
    options: &Options,
    path: DecodePath,
) -> Result<Decoded, Error> {
    crate::stream::decompress_with_path(format, input, options, path)
}

/// Like [`Decompressor::new`], but decoding with `path`.
pub fn decompressor_with_path(format: Format, options: Options, path: DecodePath) -> Decompressor {
    Decompressor::new_with_path(format, options, path)
}

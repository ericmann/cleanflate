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

/// Checksum variants for the microbenchmark and its tests. Every function
/// takes and returns the finished checksum value (the value `finish()`
/// returns): start from `0` for CRC-32 and `1` for Adler-32.
pub mod checksum {
    use crate::checksum::{
        adler32_update_bytewise, adler32_update_value, crc32_update_bytewise, crc32_update_sliced,
    };

    const INVERT: u32 = 0xFFFF_FFFF;

    fn crc_with(crc: u32, f: impl FnOnce(u32) -> u32) -> u32 {
        f(crc ^ INVERT) ^ INVERT
    }

    /// CRC-32, one byte per step.
    pub fn crc32_bytewise(crc: u32, data: &[u8]) -> u32 {
        crc_with(crc, |r| crc32_update_bytewise(r, data))
    }

    /// CRC-32, four bytes per step.
    pub fn crc32_sliced4(crc: u32, data: &[u8]) -> u32 {
        crc_with(crc, |r| crc32_update_sliced::<4>(r, data))
    }

    /// CRC-32, eight bytes per step.
    pub fn crc32_sliced8(crc: u32, data: &[u8]) -> u32 {
        crc_with(crc, |r| crc32_update_sliced::<8>(r, data))
    }

    /// CRC-32, sixteen bytes per step.
    pub fn crc32_sliced16(crc: u32, data: &[u8]) -> u32 {
        crc_with(crc, |r| crc32_update_sliced::<16>(r, data))
    }

    /// The production CRC-32 update.
    pub fn crc32_production(crc: u32, data: &[u8]) -> u32 {
        let mut c = crate::checksum::Crc32::from_value(crc);
        c.update(data);
        c.finish()
    }

    /// Adler-32, modulo after every byte.
    pub fn adler32_bytewise(adler: u32, data: &[u8]) -> u32 {
        adler32_update_bytewise(adler, data)
    }

    /// The production Adler-32 update.
    pub fn adler32_production(adler: u32, data: &[u8]) -> u32 {
        adler32_update_value(adler, data)
    }
}

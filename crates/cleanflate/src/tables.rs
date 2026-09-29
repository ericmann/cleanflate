//! RFC 1951 S3.2.5-S3.2.7 constant tables: length/distance code bases and
//! extra-bit counts, the code-length alphabet transmission order, and the
//! fixed Huffman code lengths. Every value below is transcribed directly
//! from `docs/rfc/rfc1951.txt`.

/// End of the 32 KiB sliding window (RFC 1951 S2, "distance is drawn from
/// (1..32,768)").
pub(crate) const WINDOW_SIZE: usize = 32768;

/// Maximum length of a length/distance match (RFC 1951 S3.2.5: "length is
/// drawn from (3..258)").
pub(crate) const MAX_MATCH: usize = 258;

/// The literal/length alphabet's end-of-block symbol (RFC 1951 S3.2.5:
/// "the value 256 indicates end-of-block").
pub(crate) const END_OF_BLOCK: u16 = 256;

/// Base length for length codes 257..=285 (RFC 1951 S3.2.5 table), indexed
/// by `code - 257`.
pub(crate) const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];

/// Extra-bit count for length codes 257..=285 (RFC 1951 S3.2.5 table),
/// indexed by `code - 257`.
pub(crate) const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Base distance for distance codes 0..=29 (RFC 1951 S3.2.5 table),
/// indexed by `code`.
pub(crate) const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// Extra-bit count for distance codes 0..=29 (RFC 1951 S3.2.5 table),
/// indexed by `code`.
pub(crate) const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Order in which HCLEN code-length-alphabet lengths are transmitted
/// (RFC 1951 S3.2.7: "in the order: 16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11,
/// 4, 12, 3, 13, 2, 14, 1, 15").
pub(crate) const CODE_LENGTH_ORDER: [u8; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// The fixed literal/length Huffman code lengths (RFC 1951 S3.2.6):
/// 0-143 -> 8 bits, 144-255 -> 9 bits, 256-279 -> 7 bits, 280-287 -> 8
/// bits.
pub(crate) fn fixed_litlen_lengths() -> [u8; 288] {
    let mut lengths = [0u8; 288];
    for (i, slot) in lengths.iter_mut().enumerate() {
        *slot = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            280..=287 => 8,
            _ => 0,
        };
    }
    lengths
}

/// The fixed distance Huffman code lengths (RFC 1951 S3.2.6): all 32
/// codes have length 5; codes 30-31 "will never actually occur in the
/// compressed data" but still participate in code construction.
pub(crate) fn fixed_dist_lengths() -> [u8; 32] {
    [5u8; 32]
}

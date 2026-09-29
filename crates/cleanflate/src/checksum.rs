//! Adler-32 (RFC 1950) and CRC-32 (RFC 1952) checksums, implemented from
//! the RFC definitions.
//!
//! RFC 1950 S2.2: "Adler-32 is composed of two sums accumulated per byte:
//! s1 is the sum of all bytes, s2 is the sum of all s1 values. Both sums
//! are done modulo 65521. s1 is initialized to 1, s2 to zero." RFC 1950
//! S8.2 gives the reference update algorithm this module follows.
//!
//! RFC 1952 S2.3.1 defines the CRC32 trailer field as "a Cyclic
//! Redundancy Check value of the uncompressed data computed according to
//! CRC-32 algorithm used in the ISO 3309 standard". RFC 1952 S8
//! ("Appendix: Sample CRC Code") gives the reflected-polynomial table
//! construction this module follows.

/// The Adler-32 modulus (RFC 1950 S2.2): "Both sums are done modulo
/// 65521", the largest prime smaller than 2^16.
const ADLER_MOD: u32 = 65521;

/// Incremental Adler-32 (RFC 1950 S2.2, S8.2).
#[derive(Debug, Clone)]
pub struct Adler32 {
    s1: u32,
    s2: u32,
}

impl Adler32 {
    /// A fresh checksum: "s1 is initialized to 1, s2 to zero" (RFC 1950
    /// S2.2).
    pub fn new() -> Self {
        Adler32 { s1: 1, s2: 0 }
    }

    /// Fold `data` into the running checksum.
    pub fn update(&mut self, data: &[u8]) {
        let mut s1 = self.s1;
        let mut s2 = self.s2;
        for &byte in data {
            s1 = (s1 + byte as u32) % ADLER_MOD;
            s2 = (s2 + s1) % ADLER_MOD;
        }
        self.s1 = s1;
        self.s2 = s2;
    }

    /// The current checksum value: "stored as s2*65536 + s1 in
    /// most-significant-byte first (network) order" (RFC 1950 S2.2) --
    /// the byte order is a wire-format concern for the caller; this
    /// returns the numeric value s2*65536 + s1.
    pub fn finish(&self) -> u32 {
        (self.s2 << 16) | self.s1
    }
}

impl Default for Adler32 {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot Adler-32 of `data` (RFC 1950 S2.2).
pub fn adler32(data: &[u8]) -> u32 {
    let mut a = Adler32::new();
    a.update(data);
    a.finish()
}

/// Reflected CRC-32 polynomial (RFC 1952 S8): 0xEDB88320.
const CRC32_POLY: u32 = 0xEDB8_8320;

// `clippy::indexing_slicing` is allowed only on this single assignment:
// `slice::get_mut` is not callable in a `const fn` on this toolchain (see
// the compiler's E0015), and `n` is provably in `0..256`, the exact
// length of `table`, by the enclosing `while n < 256` loop, so this
// indexing can never panic.
#[allow(clippy::indexing_slicing)]
const fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            if c & 1 != 0 {
                c = CRC32_POLY ^ (c >> 1);
            } else {
                c >>= 1;
            }
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

const CRC32_TABLE: [u32; 256] = crc32_table();

/// Incremental CRC-32 (RFC 1952 S2.3.1, S8).
#[derive(Debug, Clone)]
pub struct Crc32 {
    /// Stored inverted, per the standard reflected-CRC implementation
    /// technique described alongside the RFC 1952 S8 sample code.
    crc: u32,
}

impl Crc32 {
    /// A fresh checksum.
    pub fn new() -> Self {
        Crc32 { crc: 0xFFFF_FFFF }
    }

    /// Fold `data` into the running checksum.
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.crc;
        for &byte in data {
            let index = ((crc ^ byte as u32) & 0xFF) as usize;
            let table_entry = CRC32_TABLE.get(index).copied().unwrap_or(0);
            crc = table_entry ^ (crc >> 8);
        }
        self.crc = crc;
    }

    /// The current checksum value.
    pub fn finish(&self) -> u32 {
        self.crc ^ 0xFFFF_FFFF
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot CRC-32 of `data` (RFC 1952 S2.3.1, S8).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = Crc32::new();
    c.update(data);
    c.finish()
}

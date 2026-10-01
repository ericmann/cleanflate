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

/// The largest number of bytes `update` can fold into `s1`/`s2` (each
/// already reduced mod `ADLER_MOD`, so each is `<= ADLER_MOD - 1 =
/// 65520`) before it must reduce again to guarantee neither sum
/// overflows a `u32` (SPEC S5 M4 performance pass; DESIGN.md D-23 proves
/// this bound from RFC 1950 S2.2's modulus alone, deriving it rather
/// than taking it from any implementation).
///
/// Worst case, every byte is 0xFF (adds the maximum 255 to `s1` each
/// step) and both sums start at their maximum, `ADLER_MOD - 1 = 65520`.
/// After `n` such bytes: `s1_n = 65520 + 255n`, and since `s2` is the
/// running sum of `s1` after each byte, `s2_n = 65520 + n*65520 +
/// 255*n*(n+1)/2`. The largest `n` keeping `s2_n < 2^32` is 5552 (5553
/// already overflows): `s2_5552 = 4,294,690,200 < 4,294,967,296`, while
/// `s2_5553 = 4,296,171,735 > 2^32`.
pub(crate) const ADLER_NMAX: usize = 5552;

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

    /// Fold `data` into the running checksum. Reduces `s1`/`s2` mod
    /// `ADLER_MOD` only once per `ADLER_NMAX`-sized chunk (see its doc)
    /// instead of once per byte, since the two are numerically identical
    /// (`(a + b) % m == ((a % m) + b) % m` for the plain-addition
    /// accumulation this does chunk-internally) but the deferred version
    /// does far fewer divisions.
    pub fn update(&mut self, data: &[u8]) {
        let (s1, s2) = adler_fold(self.s1, self.s2, data);
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

/// Fold `data` into the pair `(s1, s2)`, each already reduced mod
/// `ADLER_MOD`; the result is reduced the same way.
fn adler_fold(mut s1: u32, mut s2: u32, data: &[u8]) -> (u32, u32) {
    for chunk in data.chunks(ADLER_NMAX) {
        for &byte in chunk {
            s1 += byte as u32;
            s2 += s1;
        }
        s1 %= ADLER_MOD;
        s2 %= ADLER_MOD;
    }
    (s1, s2)
}

/// Split a finished Adler-32 value back into `(s1, s2)`.
#[cfg(any(test, feature = "test-hooks"))]
const fn adler_split(adler: u32) -> (u32, u32) {
    (adler & 0xFFFF, adler >> 16)
}

#[cfg(feature = "test-hooks")]
/// The production update over a finished value: fold `data` into `adler`
/// (start from 1 for an empty checksum). Same arithmetic as
/// [`Adler32::update`]; used by the benchmark hooks.
pub(crate) fn adler32_update_value(adler: u32, data: &[u8]) -> u32 {
    let (s1, s2) = adler_split(adler);
    let (s1, s2) = adler_fold(s1, s2, data);
    (s2 << 16) | s1
}

#[cfg(any(test, feature = "test-hooks"))]
/// The reference: fold `data` into the finished value `adler`, reducing
/// modulo `ADLER_MOD` after every byte (RFC 1950 S8.2 as written).
pub(crate) fn adler32_update_bytewise(adler: u32, data: &[u8]) -> u32 {
    let (mut s1, mut s2) = adler_split(adler);
    for &byte in data {
        s1 = (s1 + byte as u32) % ADLER_MOD;
        s2 = (s2 + s1) % ADLER_MOD;
    }
    (s2 << 16) | s1
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

use crate::config::CRC32_SLICES;

/// The widest slicing variant the tables support. Table `k` does not depend
/// on the slice count (it is the register after a byte and `k` zero bytes),
/// so one set serves every width up to this one; the production width is
/// `CRC32_SLICES`. A derived capacity, not a tunable.
pub(crate) const CRC32_MAX_SLICES: usize = 16;

/// Reflected CRC-32 polynomial (RFC 1952 S8): 0xEDB88320.
const CRC32_POLY: u32 = 0xEDB8_8320;

/// The reflected-CRC table entry for byte value `n` (RFC 1952 S8): eight
/// bitwise steps of "if the low bit is set, XOR the shifted polynomial in".
const fn crc32_entry(n: u32) -> u32 {
    let mut c = n;
    let mut k = 0;
    while k < 8 {
        if c & 1 != 0 {
            c = CRC32_POLY ^ (c >> 1);
        } else {
            c >>= 1;
        }
        k += 1;
    }
    c
}

/// One step of the register over a zero byte: XORing 0 into the low byte
/// leaves it alone, so this is just the eight shifts of `crc32_entry`.
const fn crc32_zero_byte_step(mut c: u32) -> u32 {
    let mut k = 0;
    while k < 8 {
        if c & 1 != 0 {
            c = CRC32_POLY ^ (c >> 1);
        } else {
            c >>= 1;
        }
        k += 1;
    }
    c
}

/// Table entry `b` of slice table `k`: the register after processing the
/// byte `b` followed by `k` zero bytes (DESIGN.md D-32).
const fn crc32_slice_entry(b: u32, k: usize) -> u32 {
    let mut c = crc32_entry(b);
    let mut i = 0;
    while i < k {
        c = crc32_zero_byte_step(c);
        i += 1;
    }
    c
}

// Walks the tables and their rows with `split_first_mut` instead of
// `table[n] = ...`: each step peels the first remaining element off the
// *tail*, so there is never a `[]` index to get wrong, and the borrow
// checker (not a runtime bounds check) is what proves this terminates and
// never panics. `split_first_mut` is const-stable on the pinned 1.89
// toolchain.
const fn crc32_tables() -> [[u32; 256]; CRC32_MAX_SLICES] {
    let mut tables = [[0u32; 256]; CRC32_MAX_SLICES];
    let mut rest: &mut [[u32; 256]] = &mut tables;
    let mut k = 0usize;
    while let Some((row, tail)) = rest.split_first_mut() {
        let mut cells: &mut [u32] = row;
        let mut b = 0u32;
        while let Some((cell, cells_tail)) = cells.split_first_mut() {
            *cell = crc32_slice_entry(b, k);
            b += 1;
            cells = cells_tail;
        }
        k += 1;
        rest = tail;
    }
    tables
}

/// The slice tables (`CRC32_MAX_SLICES` of them); table 0 is the classic
/// byte-at-a-time table.
pub(crate) static CRC32_TABLES: [[u32; 256]; CRC32_MAX_SLICES] = crc32_tables();

/// Fold `data` into the (inverted) register one byte at a time with table
/// 0. Used for the tail of `update` and as the test reference.
pub(crate) fn crc32_update_bytewise(mut crc: u32, data: &[u8]) -> u32 {
    let table = CRC32_TABLES.first();
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        let entry = table.and_then(|t| t.get(index)).copied().unwrap_or(0);
        crc = entry ^ (crc >> 8);
    }
    crc
}

/// Fold one `N`-byte chunk into the register: the register is XORed into
/// the chunk's first four bytes, and byte `i` then looks up table `N - 1 -
/// i` (DESIGN.md D-32). `tables` are the first `N` slice tables.
fn crc32_fold_chunk<const N: usize>(tables: &[[u32; 256]; N], crc: u32, chunk: &[u8; N]) -> u32 {
    let mut bytes = *chunk;
    for (b, c) in bytes.iter_mut().zip(crc.to_le_bytes()) {
        *b ^= c;
    }
    let mut acc = 0u32;
    for (table, &b) in tables.iter().rev().zip(bytes.iter()) {
        acc ^= table.get(b as usize).copied().unwrap_or(0);
    }
    acc
}

/// Fold `data` into the (inverted) register `N` bytes at a time, then the
/// tail bytewise. `N` below 4 cannot fold the register into the chunk and
/// `N` above `CRC32_MAX_SLICES` has no tables, so both fall back to the
/// bytewise path (same result, slower).
pub(crate) fn crc32_update_sliced<const N: usize>(crc: u32, data: &[u8]) -> u32 {
    if N < 4 {
        return crc32_update_bytewise(crc, data);
    }
    let Some(tables) = CRC32_TABLES.first_chunk::<N>() else {
        return crc32_update_bytewise(crc, data);
    };
    let (chunks, tail) = data.as_chunks::<N>();
    let mut crc = crc;
    for chunk in chunks {
        crc = crc32_fold_chunk(tables, crc, chunk);
    }
    crc32_update_bytewise(crc, tail)
}

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

    /// A checksum resumed from a finished value (`0` for an empty one).
    #[cfg(feature = "test-hooks")]
    pub(crate) fn from_value(crc: u32) -> Self {
        Crc32 {
            crc: crc ^ 0xFFFF_FFFF,
        }
    }

    /// Fold `data` into the running checksum.
    pub fn update(&mut self, data: &[u8]) {
        self.crc = crc32_update_sliced::<CRC32_SLICES>(self.crc, data);
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

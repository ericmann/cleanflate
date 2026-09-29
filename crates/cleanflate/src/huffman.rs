//! Canonical Huffman code construction and bit-serial decoding
//! (RFC 1951 S3.2.2, S3.1.1).

use crate::bitreader::BitReader;
use crate::config::{DIST_TABLE_BITS, LITLEN_TABLE_BITS};
use crate::error::HuffmanError;

/// One entry of the table-driven fast-decode lookup (SPEC S5 M2).
#[derive(Debug, Clone, Copy)]
enum TableEntry {
    /// No code has this prefix (only possible for a table `TableKind`
    /// allows to stay incomplete).
    Invalid,
    /// A code longer than the table's lookahead shares this prefix; fall
    /// back to the bit-serial walk.
    Long,
    /// A code of length `len` (`<=` the table's bit width) matches.
    Symbol { symbol: u16, len: u32 },
}

/// A canonical Huffman decode table: how many codes exist at each length
/// (`counts[len]`, `len` 1..=15), and which symbols they decode to,
/// ordered by (length, symbol value) as RFC 1951 S3.2.2's canonical rule
/// requires ("All codes of a given bit length have lexicographically
/// consecutive values, in the same order as the symbols they
/// represent"). Also carries a table-driven fast-decode lookup for codes
/// up to `table_bits` long (DESIGN.md D-16).
#[derive(Debug, Clone)]
pub(crate) struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
    table_bits: u32,
    table: Vec<TableEntry>,
}

/// Which alphabet a table is for: this selects the incomplete-set
/// exception (RFC 1951 S3.2.7 applies only to the distance alphabet) and
/// how many bits of fast-decode lookahead to build (the code-length
/// alphabet is small and decoded only a handful of times per block, so
/// it gets no fast table and always uses the bit-serial walk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableKind {
    /// The literal/length alphabet (0..=287).
    LitLen,
    /// The distance alphabet (0..=31).
    Distance,
    /// The code-length alphabet (0..=18) used to transmit dynamic tables.
    CodeLength,
}

impl TableKind {
    fn table_bits(self) -> u32 {
        match self {
            TableKind::LitLen => LITLEN_TABLE_BITS,
            TableKind::Distance => DIST_TABLE_BITS,
            TableKind::CodeLength => 0,
        }
    }
}

/// Reverse the low `nbits` bits of `value` (the rest must be zero).
/// Converts between a Huffman code's MSB-first integer value and the
/// LSB-first bit-index a `BitReader::peek` naturally produces (RFC 1951
/// S3.1.1: "the first bit of the code in the relative LSB position").
fn reverse_bits(value: u32, nbits: u32) -> u32 {
    let mut v = value;
    let mut r = 0u32;
    for _ in 0..nbits {
        r = (r << 1) | (v & 1);
        v >>= 1;
    }
    r
}

/// The result of one bit-serial decode attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decoded {
    /// A symbol was decoded; the caller must consume `len` bits.
    Symbol {
        /// The decoded symbol.
        symbol: u16,
        /// How many bits long its code is.
        len: u32,
    },
    /// Not enough bits are buffered to know the answer yet.
    NeedMoreBits,
    /// 15 bits were available and none of them formed a valid code.
    Invalid,
}

impl Huffman {
    /// A table with no codes at all: every `decode` call returns
    /// `Invalid`. Used only as a panic-free fallback in the impossible
    /// case that building one of the crate's own constant fixed tables
    /// (RFC 1951 S3.2.6) somehow failed.
    pub(crate) fn empty() -> Huffman {
        Huffman {
            counts: [0u16; 16],
            symbols: Vec::new(),
            table_bits: 0,
            table: Vec::new(),
        }
    }

    /// Build a canonical Huffman table from per-symbol code lengths
    /// (RFC 1951 S3.2.2 steps 1-2: count codes per length, then derive
    /// each length's first code value). Lengths are 0-15; a length of 0
    /// means the symbol has no code.
    ///
    /// Rejects over-subscribed tables (more codes claimed than a length
    /// allows) and incomplete tables (fewer codes than the length counts
    /// allow), except for `TableKind::Distance`, which additionally
    /// accepts the two cases RFC 1951 S3.2.7 carves out: exactly one code
    /// of length 1 ("If only one distance code is used, it is encoded
    /// using one bit, not zero bits ... there is a single code length of
    /// one, with one unused code"), and no codes at all ("One distance
    /// code of zero bits means that there are no distance codes used at
    /// all").
    pub(crate) fn from_lengths(lengths: &[u8], kind: TableKind) -> Result<Huffman, HuffmanError> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            let len = len as usize;
            if len > 15 {
                return Err(HuffmanError::OverSubscribed);
            }
            if let Some(c) = counts.get_mut(len) {
                *c += 1;
            }
        }

        // RFC 1951 S3.2.2 step 2's recurrence, extended with a running
        // "codes still available at this length" check: at length `len`
        // there are `left` unused code slots before this length's counts
        // are assigned; if consuming them would go negative, more codes
        // were claimed than fit. Whatever is left over after length 15 is
        // how many codes the table is short of being complete.
        let mut left: i32 = 1;
        for len in 1..=15usize {
            let count = *counts.get(len).unwrap_or(&0) as i32;
            left <<= 1;
            left -= count;
            if left < 0 {
                return Err(HuffmanError::OverSubscribed);
            }
        }

        if left > 0 {
            let total_codes: u32 = counts.iter().skip(1).map(|&c| c as u32).sum();
            let all_zero = total_codes == 0;
            let single_code_len1 = total_codes == 1 && *counts.get(1).unwrap_or(&0) == 1;
            let exception = kind == TableKind::Distance && (all_zero || single_code_len1);
            if !exception {
                return Err(HuffmanError::Incomplete);
            }
        }

        let mut symbols: Vec<u16> = Vec::with_capacity(lengths.len());
        for len in 1..=15usize {
            for (sym, &l) in lengths.iter().enumerate() {
                if l as usize == len {
                    symbols.push(sym as u16);
                }
            }
        }

        let table_bits = kind.table_bits();
        let table = build_table(&counts, &symbols, table_bits);

        Ok(Huffman {
            counts,
            symbols,
            table_bits,
            table,
        })
    }

    /// Decode the next symbol from `br` without consuming any bits.
    /// Walks lengths 1..=15, building the code MSB-first from successive
    /// stream bits (RFC 1951 S3.1.1: Huffman codes are packed MSB-first),
    /// and stops at the first length where the accumulated code falls
    /// inside that length's assigned range. Returns `NeedMoreBits` if
    /// `br` runs out of buffered bits first, or `Invalid` if all 15
    /// lengths were tried with no match.
    pub(crate) fn decode(&self, br: &BitReader) -> Decoded {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: usize = 0;

        for len in 1..=15u32 {
            if br.available() < len {
                return Decoded::NeedMoreBits;
            }
            let peeked = br.peek(len);
            let bit = (peeked >> (len - 1)) & 1;
            code = (code << 1) | bit as i32;

            let count = *self.counts.get(len as usize).unwrap_or(&0) as i32;
            if code - first < count {
                let sym_index = index + (code - first) as usize;
                let symbol = *self.symbols.get(sym_index).unwrap_or(&0);
                return Decoded::Symbol { symbol, len };
            }
            index += count as usize;
            first = (first + count) << 1;

            // Uniform invalid-code rule (RFC 1951 S3.2.2, S3.2.7): once no
            // code of any longer length exists, this prefix can never
            // become a valid code, so the answer is already known.
            let longer_exists = self.counts.iter().skip(len as usize + 1).any(|&c| c != 0);
            if !longer_exists {
                return Decoded::Invalid;
            }
        }

        Decoded::Invalid
    }

    /// Decode the next symbol using the table-driven fast path, falling
    /// back to the bit-serial walk (`decode`) for codes longer than this
    /// table's lookahead, or whenever fewer than `table_bits` bits are
    /// buffered (DESIGN.md D-16 explains why that fallback keeps this
    /// path correct rather than merely fast). `decode` remains the
    /// reference implementation the table is checked against, and both
    /// share the uniform invalid-code rule: an `Invalid` table entry is
    /// `Invalid`, and the bit-serial fallback for a short buffer reports
    /// `Invalid` after one bit whenever no longer code exists.
    pub(crate) fn decode_table(&self, br: &BitReader) -> Decoded {
        if self.table_bits == 0 || br.available() < self.table_bits {
            return self.decode(br);
        }
        let idx = br.peek(self.table_bits) as usize;
        match self.table.get(idx) {
            Some(TableEntry::Symbol { symbol, len }) => Decoded::Symbol {
                symbol: *symbol,
                len: *len,
            },
            Some(TableEntry::Long) => self.decode(br),
            Some(TableEntry::Invalid) | None => Decoded::Invalid,
        }
    }
}

/// Build a `bits`-wide fast-decode table for a canonical code described
/// by `counts`/`symbols`: every table index whose low `len` bits equal a
/// symbol's code (reversed, since the table is indexed LSB-first) gets a
/// `Symbol` entry, for every combination of the remaining `bits - len`
/// "don't care" high bits; codes longer than `bits` get a `Long` marker
/// at their `bits`-bit prefix. `bits == 0` (the code-length alphabet)
/// yields an empty table, forcing the caller to `decode` always.
fn build_table(counts: &[u16; 16], symbols: &[u16], bits: u32) -> Vec<TableEntry> {
    if bits == 0 {
        return Vec::new();
    }
    let size = 1usize << bits;
    let mut table = vec![TableEntry::Invalid; size];

    let mut code: u32 = 0;
    let mut idx = 0usize;
    for len in 1..=15u32 {
        let count = *counts.get(len as usize).unwrap_or(&0) as u32;
        for _ in 0..count {
            let symbol = *symbols.get(idx).unwrap_or(&0);
            if len <= bits {
                let reversed = reverse_bits(code, len) as usize;
                let step = 1usize << len;
                let mut i = reversed;
                while i < size {
                    if let Some(slot) = table.get_mut(i) {
                        *slot = TableEntry::Symbol { symbol, len };
                    }
                    i += step;
                }
            } else {
                let prefix = code >> (len - bits);
                let reversed = reverse_bits(prefix, bits) as usize;
                if let Some(slot) = table.get_mut(reversed) {
                    *slot = TableEntry::Long;
                }
            }
            code += 1;
            idx += 1;
        }
        code <<= 1;
    }

    table
}

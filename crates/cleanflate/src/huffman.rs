//! Canonical Huffman code construction and bit-serial decoding
//! (RFC 1951 S3.2.2, S3.1.1).

use crate::bitreader::BitReader;
use crate::error::HuffmanError;

/// A canonical Huffman decode table: how many codes exist at each length
/// (`counts[len]`, `len` 1..=15), and which symbols they decode to,
/// ordered by (length, symbol value) as RFC 1951 S3.2.2's canonical rule
/// requires ("All codes of a given bit length have lexicographically
/// consecutive values, in the same order as the symbols they
/// represent").
#[derive(Debug, Clone)]
pub(crate) struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
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
    /// Build a canonical Huffman table from per-symbol code lengths
    /// (RFC 1951 S3.2.2 steps 1-2: count codes per length, then derive
    /// each length's first code value). Lengths are 0-15; a length of 0
    /// means the symbol has no code.
    ///
    /// This only rejects over-subscription (more codes claimed than a
    /// length allows). Incomplete-set validation arrives in P2-01.
    pub(crate) fn from_lengths(lengths: &[u8]) -> Result<Huffman, HuffmanError> {
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
        // were claimed than fit.
        let mut left: i32 = 1;
        for len in 1..=15usize {
            let count = *counts.get(len).unwrap_or(&0) as i32;
            left <<= 1;
            left -= count;
            if left < 0 {
                return Err(HuffmanError::OverSubscribed);
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

        Ok(Huffman { counts, symbols })
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
        }

        Decoded::Invalid
    }
}

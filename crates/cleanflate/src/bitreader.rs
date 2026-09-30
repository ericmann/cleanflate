//! LSB-first resumable bit reader (RFC 1951 S3.1.1).
//!
//! RFC 1951 S3.1.1: "Data elements are packed into bytes in order of
//! increasing bit number within the byte, i.e., starting with the
//! least-significant bit of the byte... Huffman codes are packed starting
//! with the most-significant bit of the code." This reader implements the
//! byte-packing rule; MSB-first Huffman code assembly is the Huffman
//! decoder's job (it reads bits with this reader one at a time and shifts
//! them into a code value from the low end).

/// A resumable, LSB-first bit buffer. The exact-fill path holds at most 7
/// bits between steps; the bulk-refill path may hold more within a call and
/// returns whole unneeded bytes to the input at exit (DESIGN.md D-27).
#[derive(Debug, Clone, Default)]
pub(crate) struct BitReader {
    buffer: u64,
    count: u32,
    /// Fills that loaded at least one byte.
    #[cfg(feature = "stats")]
    pub(crate) refills: u64,
    /// Bytes loaded by those fills.
    #[cfg(feature = "stats")]
    pub(crate) refill_bytes: u64,
    /// Bytes returned unread to the input by `give_back`.
    #[cfg(feature = "stats")]
    pub(crate) given_back_bytes: u64,
}

impl BitReader {
    /// A reader with no buffered bits.
    pub(crate) fn new() -> Self {
        BitReader::default()
    }

    /// Number of bits currently buffered.
    pub(crate) fn available(&self) -> u32 {
        self.count
    }

    /// Load whole bytes from `input[*pos..]` into the buffer, advancing
    /// `*pos`, until at least `need` bits are buffered or `input` is
    /// exhausted. Never loads a byte once `need` is already satisfied.
    /// Returns whether `need` bits are now available.
    pub(crate) fn fill(&mut self, input: &[u8], pos: &mut usize, need: u32) -> bool {
        #[cfg(feature = "stats")]
        let start = *pos;
        while self.count < need {
            let Some(&byte) = input.get(*pos) else {
                break;
            };
            // count is always < 64 here (need <= 32, and this loop stops
            // once count >= need), so the shift is always in range.
            self.buffer |= (byte as u64) << self.count;
            self.count += 8;
            *pos += 1;
        }
        #[cfg(feature = "stats")]
        {
            let loaded = pos.saturating_sub(start);
            if loaded > 0 {
                stat!(self.refills, 1);
                stat!(self.refill_bytes, loaded);
            }
        }
        self.count >= need
    }

    /// Top the buffer up a word at a time (Fast path only). With at least
    /// 8 input bytes left, one little-endian 64-bit read supplies as many
    /// whole bytes as fit above the buffered bits; with fewer, single bytes
    /// are loaded while at most 55 bits are buffered. The count never
    /// exceeds 63, so no shift reaches 64.
    pub(crate) fn refill_bulk(&mut self, input: &[u8], pos: &mut usize) {
        #[cfg(feature = "stats")]
        let start = *pos;
        let end = pos.saturating_add(8);
        let word = input
            .get(*pos..end)
            .and_then(|s| <[u8; 8]>::try_from(s).ok())
            .map(u64::from_le_bytes);
        match word {
            Some(word) if self.count <= 63 => {
                let k = (63 - self.count) / 8;
                let bits = 8 * k;
                let mask = if bits >= 64 {
                    u64::MAX
                } else {
                    (1u64 << bits) - 1
                };
                self.buffer |= (word & mask) << self.count;
                self.count += bits;
                *pos += k as usize;
            }
            _ => {
                while self.count <= 55 {
                    let Some(&byte) = input.get(*pos) else {
                        break;
                    };
                    self.buffer |= (byte as u64) << self.count;
                    self.count += 8;
                    *pos += 1;
                }
            }
        }
        #[cfg(feature = "stats")]
        {
            let loaded = pos.saturating_sub(start);
            if loaded > 0 {
                stat!(self.refills, 1);
                stat!(self.refill_bytes, loaded);
            }
        }
    }

    /// Return the whole bytes buffered above the low `keep_bits` bits to
    /// the input: `k` = whole bytes above `keep_bits` (0 when fewer than
    /// `keep_bits + 8` bits are buffered). The top `8k` bits are dropped
    /// and `k` is returned; the caller moves its input position back by
    /// `k`. Bytes sit in the buffer in load order, so the top bytes are
    /// the last ones loaded.
    pub(crate) fn give_back(&mut self, keep_bits: u32) -> usize {
        let Some(extra) = self.count.checked_sub(keep_bits) else {
            return 0;
        };
        let k = extra / 8;
        if k == 0 {
            return 0;
        }
        self.count -= 8 * k;
        let mask = if self.count >= 64 {
            u64::MAX
        } else {
            (1u64 << self.count) - 1
        };
        self.buffer &= mask;
        stat!(self.given_back_bytes, k);
        k as usize
    }

    /// Peek at the low `n` bits (n <= 32) without consuming them.
    /// Unavailable high bits (beyond what has been buffered) read as 0.
    pub(crate) fn peek(&self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let n = n.min(32);
        if n == 32 {
            return (self.buffer & 0xFFFF_FFFF) as u32;
        }
        let mask = (1u64 << n) - 1;
        (self.buffer & mask) as u32
    }

    /// Drop the low `n` bits from the buffer.
    pub(crate) fn consume(&mut self, n: u32) {
        let n = n.min(self.count);
        self.buffer >>= n;
        self.count -= n;
    }

    /// Fill, peek and consume `n` bits in one step. Returns `None` if
    /// `input` was exhausted before `n` bits were available; whatever
    /// bytes `fill` did manage to load remain buffered (state advances by
    /// the bytes consumed from `input`, but no bits are peeked/consumed
    /// from the buffer).
    pub(crate) fn take(&mut self, input: &[u8], pos: &mut usize, n: u32) -> Option<u32> {
        if !self.fill(input, pos, n) {
            return None;
        }
        let value = self.peek(n);
        self.consume(n);
        Some(value)
    }

    /// Drop any partial byte, per RFC 1951 S3.2.4 (stored blocks begin on
    /// a byte boundary).
    pub(crate) fn align_to_byte(&mut self) {
        let drop = self.count % 8;
        self.buffer >>= drop;
        self.count -= drop;
    }
}

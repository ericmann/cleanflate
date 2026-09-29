//! LSB-first resumable bit reader (RFC 1951 S3.1.1).
//!
//! RFC 1951 S3.1.1: "Data elements are packed into bytes in order of
//! increasing bit number within the byte, i.e., starting with the
//! least-significant bit of the byte... Huffman codes are packed starting
//! with the most-significant bit of the code." This reader implements the
//! byte-packing rule; MSB-first Huffman code assembly is the Huffman
//! decoder's job (it reads bits with this reader one at a time and shifts
//! them into a code value from the low end).

/// A resumable, LSB-first bit buffer. Holds at most 7 bits between
/// completed decode steps (see DESIGN.md D-07): the buffer never carries
/// a fractional byte's worth of already-consumed-by-a-symbol bits across
/// calls beyond what a single in-progress `take`/`fill` leaves behind.
#[derive(Debug, Clone, Default)]
pub(crate) struct BitReader {
    buffer: u64,
    count: u32,
}

impl BitReader {
    /// A reader with no buffered bits.
    pub(crate) fn new() -> Self {
        BitReader {
            buffer: 0,
            count: 0,
        }
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
        self.count >= need
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

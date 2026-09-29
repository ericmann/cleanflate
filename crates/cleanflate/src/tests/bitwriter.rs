//! Test-only bit writer, the inverse of `BitReader`, used to build fixture
//! byte streams for Huffman/inflate tests (RFC 1951 S3.1.1 packing rules).

#[derive(Debug, Default)]
pub(crate) struct BitWriter {
    buffer: u64,
    count: u32,
    bytes: Vec<u8>,
}

impl BitWriter {
    pub(crate) fn new() -> Self {
        BitWriter::default()
    }

    /// Append the low `n` bits of `value`, LSB-first (RFC 1951 S3.1.1:
    /// "Data elements other than Huffman codes are packed starting with
    /// the least-significant bit of the data element").
    pub(crate) fn write_bits(&mut self, value: u32, n: u32) {
        for i in 0..n {
            let bit = (value >> i) & 1;
            self.buffer |= (bit as u64) << self.count;
            self.count += 1;
            if self.count == 8 {
                self.bytes.push((self.buffer & 0xFF) as u8);
                self.buffer >>= 8;
                self.count = 0;
            }
        }
    }

    /// Append a Huffman code, MSB-first (RFC 1951 S3.1.1: "Huffman codes
    /// are packed starting with the most-significant bit of the code").
    pub(crate) fn write_code(&mut self, code: u32, len: u32) {
        for i in (0..len).rev() {
            let bit = (code >> i) & 1;
            self.write_bits(bit, 1);
        }
    }

    /// Pad the current partial byte with zero bits.
    pub(crate) fn align(&mut self) {
        if self.count > 0 {
            self.bytes.push((self.buffer & 0xFF) as u8);
            self.buffer = 0;
            self.count = 0;
        }
    }

    /// Flush any partial byte and return the accumulated bytes.
    pub(crate) fn finish(mut self) -> Vec<u8> {
        self.align();
        self.bytes
    }
}

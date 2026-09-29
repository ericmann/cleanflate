use crate::bitreader::BitReader;

#[test]
fn reads_lsb_first() {
    let input = [0b1010_1100u8];
    let mut r = BitReader::new();
    let mut pos = 0;
    assert_eq!(r.take(&input, &mut pos, 3), Some(0b100));
    assert_eq!(r.take(&input, &mut pos, 5), Some(0b10101));
}

#[test]
fn value_spanning_bytes() {
    // Low 4 bits of byte0 then low 4 bits of byte1, taken as one 12-bit
    // value, must equal byte0's low nibble in the low 4 bits and byte1's
    // low 8 bits above that (LSB-first byte packing).
    let input = [0b1111_0011u8, 0b0000_1010u8];
    let mut r = BitReader::new();
    let mut pos = 0;
    let v = r.take(&input, &mut pos, 12).expect("enough input");
    // Bytes are packed LSB-first (RFC 1951 S3.1.1): byte0 supplies the
    // low 8 bits, byte1's low 4 bits supply bits 8..12.
    let expected = ((input[0] as u32) | ((input[1] as u32) << 8)) & 0xFFF;
    assert_eq!(v, expected);
}

#[test]
fn fill_loads_only_needed_bytes() {
    let input = [1u8, 2, 3, 4, 5];
    let mut r = BitReader::new();
    let mut pos = 0;
    assert!(r.fill(&input, &mut pos, 9));
    // 9 bits needs 2 bytes.
    assert_eq!(pos, 2);
    assert_eq!(r.available(), 16);
}

#[test]
fn take_short_returns_none() {
    let input = [0xFFu8];
    let mut r = BitReader::new();
    let mut pos = 0;
    assert_eq!(r.take(&input, &mut pos, 32), None);
    // The one available byte was still loaded.
    assert_eq!(pos, 1);
    assert_eq!(r.available(), 8);
}

#[test]
fn align_drops_partial_byte() {
    let input = [0xFFu8, 0xAAu8];
    let mut r = BitReader::new();
    let mut pos = 0;
    assert_eq!(r.take(&input, &mut pos, 3), Some(0b111));
    // Only the first byte needed to be loaded to satisfy 3 bits.
    assert_eq!(r.available(), 5);
    r.align_to_byte();
    assert_eq!(r.available(), 0);
    // The next full read starts at the next byte boundary.
    assert_eq!(r.take(&input, &mut pos, 8), Some(0xAA));
}

#[test]
fn peek_pads_with_zero() {
    let input = [0b0000_0001u8];
    let mut r = BitReader::new();
    let mut pos = 0;
    assert!(r.fill(&input, &mut pos, 8));
    assert_eq!(r.peek(16), 1);
}

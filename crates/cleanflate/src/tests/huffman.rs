use crate::bitreader::BitReader;
use crate::huffman::{Decoded, Huffman};
use crate::tables::fixed_litlen_lengths;
use crate::tests::bitwriter::BitWriter;

/// Push the bits of `s` (a string of '0'/'1', first char = first
/// transmitted bit) in stream order.
fn push_bit_string(w: &mut BitWriter, s: &str) {
    for c in s.chars() {
        w.write_bits(u32::from(c == '1'), 1);
    }
}

fn decode_one(table: &Huffman, input: &[u8], pos: &mut usize, br: &mut BitReader) -> Decoded {
    // Keep filling until decode stops asking for more (or the input is
    // exhausted, at which point NeedMoreBits is the honest answer).
    loop {
        match table.decode(br) {
            Decoded::NeedMoreBits => {
                if !br.fill(input, pos, br.available() + 1) {
                    return Decoded::NeedMoreBits;
                }
            }
            other => return other,
        }
    }
}

#[test]
fn rfc_example_abcdefgh() {
    // A..H = symbols 0..7; lengths per RFC 1951 S3.2.2's worked example.
    let lengths = [3u8, 3, 3, 3, 3, 2, 4, 4];
    let table =
        Huffman::from_lengths(&lengths, crate::huffman::TableKind::LitLen).expect("valid lengths");

    let cases: [(u16, &str); 8] = [
        (5, "00"),   // F
        (0, "010"),  // A
        (1, "011"),  // B
        (2, "100"),  // C
        (3, "101"),  // D
        (4, "110"),  // E
        (6, "1110"), // G
        (7, "1111"), // H
    ];

    for (symbol, code_str) in cases {
        let mut w = BitWriter::new();
        push_bit_string(&mut w, code_str);
        let bytes = w.finish();

        let mut br = BitReader::new();
        let mut pos = 0;
        let decoded = decode_one(&table, &bytes, &mut pos, &mut br);
        match decoded {
            Decoded::Symbol { symbol: got, len } => {
                assert_eq!(got, symbol, "code {code_str}");
                assert_eq!(len as usize, code_str.len(), "code {code_str}");
            }
            other => panic!("code {code_str}: expected Symbol, got {other:?}"),
        }
    }
}

#[test]
fn fixed_litlen_codes_rfc_3_2_6() {
    let lengths = fixed_litlen_lengths();
    let table = Huffman::from_lengths(&lengths, crate::huffman::TableKind::LitLen)
        .expect("fixed lengths are valid");

    let cases: [(u16, &str); 8] = [
        (0, "00110000"),
        (143, "10111111"),
        (144, "110010000"),
        (255, "111111111"),
        (256, "0000000"),
        (279, "0010111"),
        (280, "11000000"),
        (287, "11000111"),
    ];

    for (symbol, code_str) in cases {
        let mut w = BitWriter::new();
        push_bit_string(&mut w, code_str);
        let bytes = w.finish();

        let mut br = BitReader::new();
        let mut pos = 0;
        let decoded = decode_one(&table, &bytes, &mut pos, &mut br);
        match decoded {
            Decoded::Symbol { symbol: got, len } => {
                assert_eq!(got, symbol, "code {code_str}");
                assert_eq!(len as usize, code_str.len(), "code {code_str}");
            }
            other => panic!("code {code_str}: expected Symbol, got {other:?}"),
        }
    }
}

#[test]
fn oversubscribed_rejected() {
    // Three symbols all claiming length 1: only 2 length-1 codes exist.
    let lengths = [1u8, 1, 1];
    let result = Huffman::from_lengths(&lengths, crate::huffman::TableKind::LitLen);
    assert!(matches!(
        result,
        Err(crate::error::HuffmanError::OverSubscribed)
    ));
}

#[test]
fn need_more_bits_when_short() {
    let lengths = [3u8, 3, 3, 3, 3, 2, 4, 4];
    let table =
        Huffman::from_lengths(&lengths, crate::huffman::TableKind::LitLen).expect("valid lengths");

    // "A" is code 010 (3 bits); supply zero bits.
    let br = BitReader::new();
    assert_eq!(table.decode(&br), Decoded::NeedMoreBits);
}

#[test]
fn decode_does_not_consume() {
    let lengths = [3u8, 3, 3, 3, 3, 2, 4, 4];
    let table =
        Huffman::from_lengths(&lengths, crate::huffman::TableKind::LitLen).expect("valid lengths");

    let mut w = BitWriter::new();
    push_bit_string(&mut w, "010"); // A
    let bytes = w.finish();

    let mut br = BitReader::new();
    let mut pos = 0;
    assert!(br.fill(&bytes, &mut pos, 3));
    let before = br.available();

    let decoded = table.decode(&br);
    assert_eq!(decoded, Decoded::Symbol { symbol: 0, len: 3 });
    // decode() must not have consumed anything.
    assert_eq!(br.available(), before);
}

use crate::bitreader::BitReader;
use crate::huffman::{Decoded, Huffman, TableKind};
use crate::tables::{fixed_dist_lengths, fixed_litlen_lengths};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::canonical_codes;

/// Test-local xorshift64 generator.
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn range1(&mut self, max: usize) -> usize {
        if max <= 1 {
            return 0;
        }
        (self.next_u64() as usize) % max
    }
}

/// Build a valid complete prefix code for `n` symbols (0..n) by
/// repeatedly splitting a random leaf, up to `max_depth`. This always
/// yields a complete code (Kraft's inequality holds with equality),
/// since splitting a leaf into two children preserves the invariant.
fn random_complete_lengths(rng: &mut XorShift, n: usize, max_depth: u8) -> Vec<u8> {
    let mut leaves: Vec<u8> = vec![0u8];
    while leaves.len() < n {
        let splittable: Vec<usize> = leaves
            .iter()
            .enumerate()
            .filter(|&(_, &d)| d < max_depth)
            .map(|(i, _)| i)
            .collect();
        if splittable.is_empty() {
            break;
        }
        let pick = splittable
            .get(rng.range1(splittable.len()))
            .copied()
            .expect("non-empty");
        let d = *leaves.get(pick).expect("in range");
        if let Some(slot) = leaves.get_mut(pick) {
            *slot = d + 1;
        }
        leaves.push(d + 1);
    }
    leaves
}

/// For every symbol with a code in `lengths`, feed its codeword plus
/// random padding through both `decode_table` and `decode` (the
/// bit-serial reference) and assert they agree.
fn assert_table_matches_bitserial(lengths: &[u8], kind: TableKind, rng: &mut XorShift) {
    let table = Huffman::from_lengths(lengths, kind).expect("valid table");
    let codes = canonical_codes(lengths);

    for (sym, code_opt) in codes.iter().enumerate() {
        let Some((code, len)) = code_opt else {
            continue;
        };
        let mut w = BitWriter::new();
        w.write_code(*code, *len);
        let pad_bits: u32 = 16;
        w.write_bits((rng.next_u64() & 0xFFFF) as u32, pad_bits);
        let bytes = w.finish();

        let mut br = BitReader::new();
        let mut pos = 0usize;
        assert!(br.fill(&bytes, &mut pos, (*len + pad_bits).min(48)));

        let table_result = table.decode_table(&br);
        let bitserial_result = table.decode(&br);
        assert_eq!(
            table_result, bitserial_result,
            "symbol {sym} (len {len}): table and bit-serial disagree"
        );
        match table_result {
            Decoded::Symbol {
                symbol,
                len: got_len,
            } => {
                assert_eq!(symbol, sym as u16, "wrong symbol for code of len {len}");
                assert_eq!(got_len, *len);
            }
            other => panic!("symbol {sym}: expected Symbol, got {other:?}"),
        }
    }
}

#[test]
fn table_equals_bitserial_fixed_tables() {
    let mut rng = XorShift::new(1);
    assert_table_matches_bitserial(&fixed_litlen_lengths(), TableKind::LitLen, &mut rng);
    assert_table_matches_bitserial(&fixed_dist_lengths(), TableKind::Distance, &mut rng);
}

#[test]
fn table_equals_bitserial_random_complete_codes() {
    let mut rng = XorShift::new(0x1234_5678_9ABC_DEF0);
    for trial in 0..1000u32 {
        let n = 2 + (rng.range1(60));
        let lengths = random_complete_lengths(&mut rng, n, 15);
        let kind = if trial % 2 == 0 {
            TableKind::LitLen
        } else {
            TableKind::Distance
        };
        // Distance tables built here are always complete-multi-code (n>=2
        // leaves after at least one split), so no exception path is
        // needed; construction must succeed for both kinds.
        assert_table_matches_bitserial(&lengths, kind, &mut rng);
    }
}

#[test]
fn table_handles_codes_longer_than_table_bits() {
    // Force some codes past LITLEN_TABLE_BITS (10) by using many symbols
    // at max depth 15: a code count large enough that splitting to reach
    // depth 15 is required.
    let mut rng = XorShift::new(42);
    let lengths = random_complete_lengths(&mut rng, 40, 15);
    assert!(
        lengths.iter().any(|&l| l > 10),
        "fixture should include at least one code longer than the table width"
    );
    assert_table_matches_bitserial(&lengths, TableKind::LitLen, &mut rng);
}

#[test]
fn table_partial_bits_need_more() {
    // RFC 1951 S3.2.2's ABCDEFGH example. With zero bytes of input at
    // all (so `fill` cannot load anything and `available()` stays 0,
    // well under both the table's lookahead and any real code's
    // length), both the table-driven and bit-serial paths must report
    // NeedMoreBits rather than guess from padding.
    let lengths = [3u8, 3, 3, 3, 3, 2, 4, 4];
    let table = Huffman::from_lengths(&lengths, TableKind::LitLen).expect("valid lengths");

    let empty: [u8; 0] = [];
    let mut br = BitReader::new();
    let mut pos = 0usize;
    assert!(!br.fill(&empty, &mut pos, 2));
    assert_eq!(br.available(), 0);

    assert_eq!(table.decode_table(&br), Decoded::NeedMoreBits);
    assert_eq!(table.decode(&br), Decoded::NeedMoreBits);
}

#[test]
fn table_single_distance_code_unused_is_invalid() {
    let lengths = [1u8]; // single distance code, length 1
    let table = Huffman::from_lengths(&lengths, TableKind::Distance).expect("valid exception");

    let mut w = BitWriter::new();
    w.write_bits(0xFFFF, 16); // the unused code (bit value 1) plus padding
    let bytes = w.finish();

    let mut br = BitReader::new();
    let mut pos = 0usize;
    assert!(br.fill(&bytes, &mut pos, 16));

    assert_eq!(table.decode_table(&br), Decoded::Invalid);
    assert_eq!(table.decode(&br), Decoded::Invalid);
}

/// A reader holding exactly `available` (0..=15) bits, whose contents
/// are the top bits of `pattern`.
fn reader_with(pattern: u32, available: u32) -> BitReader {
    let bytes = pattern.to_le_bytes();
    let mut br = BitReader::new();
    let mut pos = 0usize;
    assert!(br.fill(&bytes, &mut pos, 32));
    br.consume(32 - available);
    assert_eq!(br.available(), available);
    br
}

#[test]
fn table_decode_equals_serial_for_all_buffer_states() {
    let mut rng = XorShift::new(0xDEC0_DE5E);
    let mut tables: Vec<(Huffman, &str)> = vec![
        (
            Huffman::from_lengths(&fixed_litlen_lengths(), TableKind::LitLen).expect("fixed"),
            "fixed litlen",
        ),
        (
            Huffman::from_lengths(&fixed_dist_lengths(), TableKind::Distance).expect("fixed"),
            "fixed dist",
        ),
        (
            Huffman::from_lengths(&[1u8], TableKind::Distance).expect("single"),
            "single code",
        ),
        (
            Huffman::from_lengths(&[0u8], TableKind::Distance).expect("empty"),
            "no codes",
        ),
    ];
    for trial in 0..60usize {
        let n = 2 + rng.range1(120);
        let lengths = random_complete_lengths(&mut rng, n, 15);
        let kind = if trial % 2 == 0 {
            TableKind::LitLen
        } else {
            TableKind::Distance
        };
        tables.push((
            Huffman::from_lengths(&lengths, kind).expect("complete"),
            "random",
        ));
    }
    assert!(tables.len() >= 54);
    for (table, name) in &tables {
        for available in 0..=15u32 {
            for _ in 0..64 {
                let br = reader_with(rng.next_u64() as u32, available);
                assert_eq!(
                    table.decode_table(&br),
                    table.decode(&br),
                    "{name}: available={available}"
                );
            }
        }
    }
}

#[test]
fn invalid_code_detected_after_one_bit() {
    let single = Huffman::from_lengths(&[1u8], TableKind::Distance).expect("single");
    let none = Huffman::from_lengths(&[0u8], TableKind::Distance).expect("empty");

    // Single code of length 1 is code 0; bit 1 is the unused code.
    let one = reader_with(0xFFFF_FFFF, 1);
    assert_eq!(single.decode(&one), Decoded::Invalid);
    assert_eq!(single.decode_table(&one), Decoded::Invalid);
    let zero = reader_with(0, 1);
    assert_eq!(single.decode(&zero), Decoded::Symbol { symbol: 0, len: 1 });
    assert_eq!(single.decode_table(&zero), single.decode(&zero));

    for pattern in [0u32, 0xFFFF_FFFF] {
        let br = reader_with(pattern, 1);
        assert_eq!(none.decode(&br), Decoded::Invalid);
        assert_eq!(none.decode_table(&br), Decoded::Invalid);
    }
    let empty = reader_with(0, 0);
    assert_eq!(none.decode(&empty), Decoded::NeedMoreBits);
    assert_eq!(none.decode_table(&empty), Decoded::NeedMoreBits);
}

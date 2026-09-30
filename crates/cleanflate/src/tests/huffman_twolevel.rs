use crate::bitreader::BitReader;
use crate::huffman::{Decoded, Huffman, TableKind};
use crate::tables::{fixed_dist_lengths, fixed_litlen_lengths};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::canonical_codes;

/// Test-local xorshift64 generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
}

/// A random complete prefix code for `n` symbols by repeatedly splitting a
/// leaf up to `max_depth`. With probability 1/2 the deepest leaf is split,
/// which produces long codes; otherwise a random one, which balances.
fn complete_lengths(rng: &mut Rng, n: usize, max_depth: u8) -> Vec<u8> {
    let mut leaves: Vec<u8> = vec![0];
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
        let pick = if rng.below(2) == 0 {
            *splittable
                .iter()
                .max_by_key(|&&i| leaves[i])
                .expect("non-empty")
        } else {
            splittable[rng.below(splittable.len())]
        };
        let d = leaves[pick];
        leaves[pick] = d + 1;
        leaves.push(d + 1);
    }
    leaves
}

/// A reader holding exactly `available` (0..=15) bits, taken from the top
/// bits of `pattern`.
fn reader_with(pattern: u32, available: u32) -> BitReader {
    let bytes = pattern.to_le_bytes();
    let mut br = BitReader::new();
    let mut pos = 0usize;
    assert!(br.fill(&bytes, &mut pos, 32));
    br.consume(32 - available);
    assert_eq!(br.available(), available);
    br
}

fn assert_lookup_equals_serial(table: &Huffman, name: &str, rng: &mut Rng) {
    for available in 0..=15u32 {
        for _ in 0..64 {
            let br = reader_with(rng.next() as u32, available);
            assert_eq!(
                table.decode_table(&br),
                table.decode(&br),
                "{name}: available={available}"
            );
        }
    }
}

#[test]
fn lookup_equals_serial_for_all_buffer_states() {
    let mut rng = Rng(0xC0DE_D00D_1234_5677);
    let mut tables: Vec<(Huffman, String)> = vec![
        (
            Huffman::from_lengths(&fixed_litlen_lengths(), TableKind::LitLen).expect("fixed"),
            "fixed litlen".into(),
        ),
        (
            Huffman::from_lengths(&fixed_dist_lengths(), TableKind::Distance).expect("fixed"),
            "fixed dist".into(),
        ),
        (
            Huffman::from_lengths(&[1u8], TableKind::Distance).expect("single"),
            "single code".into(),
        ),
        (
            Huffman::from_lengths(&[0u8], TableKind::Distance).expect("none"),
            "no codes".into(),
        ),
    ];
    let mut long_sets = 0;
    for trial in 0..220usize {
        let n = 2 + rng.below(if trial % 4 == 0 { 300 } else { 40 });
        let lengths = complete_lengths(&mut rng, n, 15);
        if lengths.contains(&15) {
            long_sets += 1;
        }
        let kind = if trial % 2 == 0 {
            TableKind::LitLen
        } else {
            TableKind::Distance
        };
        tables.push((
            Huffman::from_lengths(&lengths, kind).expect("complete"),
            format!("random #{trial}"),
        ));
    }
    assert!(
        long_sets >= 20,
        "fixture must include 15-bit codes ({long_sets})"
    );
    assert!(tables.len() >= 200);
    for (table, name) in &tables {
        assert_lookup_equals_serial(table, name, &mut rng);
    }
}

#[test]
fn subtables_built_for_long_codes() {
    // Lengths 1,2,3,...,14,15,15: a complete code with a 15-bit tail.
    let mut lengths: Vec<u8> = (1..=15u8).collect();
    lengths.push(15);
    let long = Huffman::from_lengths_with_bits(&lengths, TableKind::LitLen, 10).expect("valid");
    assert!(
        long.table_entries() > 1 << 10,
        "codes longer than 10 bits need second-level tables"
    );

    // Every code of at most 4 bits: the first level alone is enough.
    let short =
        Huffman::from_lengths_with_bits(&[2u8, 2, 2, 2], TableKind::LitLen, 10).expect("valid");
    assert_eq!(short.table_entries(), 1 << 10);

    // Each symbol decodes through its subtable and consumes its full length.
    let codes = canonical_codes(&lengths);
    for (sym, code) in codes.iter().enumerate() {
        let Some((value, len)) = code else { continue };
        let mut w = BitWriter::new();
        w.write_code(*value, *len);
        w.write_bits(0x5A5A, 16);
        w.write_bits(0xA5A5, 16);
        let bytes = w.finish();
        let mut br = BitReader::new();
        let mut pos = 0usize;
        assert!(br.fill(&bytes, &mut pos, 32));
        assert_eq!(
            long.decode_table(&br),
            Decoded::Symbol {
                symbol: sym as u16,
                len: *len
            },
            "symbol {sym}"
        );
    }
}

#[test]
fn codelen_table_decodes_all_symbols() {
    // The code-length alphabet has 19 symbols; give them a complete code
    // of at most 7 bits.
    let mut rng = Rng(42);
    for _ in 0..50 {
        let lengths = complete_lengths(&mut rng, 19, 7);
        let table = Huffman::from_lengths(&lengths, TableKind::CodeLength).expect("valid");
        assert_eq!(table.table_entries(), 1 << 7, "a single 7-bit level");
        let codes = canonical_codes(&lengths);
        for (sym, code) in codes.iter().enumerate() {
            let Some((value, len)) = code else { continue };
            let mut w = BitWriter::new();
            w.write_code(*value, *len);
            w.write_bits(rng.next() as u32 & 0xFFFF, 16);
            w.write_bits(0xA5A5, 16);
            let bytes = w.finish();
            let mut br = BitReader::new();
            let mut pos = 0usize;
            assert!(br.fill(&bytes, &mut pos, 32));
            assert_eq!(
                table.decode_table(&br),
                Decoded::Symbol {
                    symbol: sym as u16,
                    len: *len
                }
            );
        }
        assert_lookup_equals_serial(&table, "code-length", &mut rng);
    }
}

#[test]
fn table_widths_sweepable() {
    let mut rng = Rng(0xABCD_EF01);
    for bits in 7..=12u32 {
        for trial in 0..30usize {
            let n = 2 + rng.below(if trial % 3 == 0 { 250 } else { 30 });
            let lengths = complete_lengths(&mut rng, n, 15);
            let table = Huffman::from_lengths_with_bits(&lengths, TableKind::LitLen, bits)
                .expect("complete");
            assert_lookup_equals_serial(&table, &format!("bits {bits} trial {trial}"), &mut rng);
        }
        let single = Huffman::from_lengths_with_bits(&[1u8], TableKind::Distance, bits).expect("x");
        assert_lookup_equals_serial(&single, "single", &mut rng);
    }
}

use crate::bitreader::BitReader;
use crate::error::HuffmanError;
use crate::huffman::{Decoded, Huffman, TableKind};
use crate::tables::{fixed_dist_lengths, fixed_litlen_lengths};

fn assert_incomplete(lengths: &[u8], kind: TableKind) {
    let result = Huffman::from_lengths(lengths, kind);
    assert!(
        matches!(result, Err(HuffmanError::Incomplete)),
        "expected Incomplete, got {result:?}"
    );
}

fn assert_ok(lengths: &[u8], kind: TableKind) -> Huffman {
    Huffman::from_lengths(lengths, kind).unwrap_or_else(|e| panic!("expected Ok, got {e:?}"))
}

#[test]
fn oversubscribed_litlen_rejected() {
    // Three symbols all claiming length 1: only 2 length-1 codes exist.
    let result = Huffman::from_lengths(&[1u8, 1, 1], TableKind::LitLen);
    assert!(matches!(result, Err(HuffmanError::OverSubscribed)));
}

#[test]
fn incomplete_litlen_rejected() {
    // Two symbols of length 2 leaves two length-2 codes unused (4 exist).
    assert_incomplete(&[2u8, 2], TableKind::LitLen);
}

#[test]
fn incomplete_codelength_rejected() {
    // Same shape, but for the code-length alphabet: no exception applies.
    assert_incomplete(&[2u8, 2], TableKind::CodeLength);
}

#[test]
fn single_litlen_code_rejected() {
    // One symbol of length 1: incomplete for LitLen (the exception is
    // Distance-only).
    assert_incomplete(&[1u8], TableKind::LitLen);
}

#[test]
fn distance_single_code_len1_accepted() {
    // RFC 1951 S3.2.7: "If only one distance code is used, it is encoded
    // using one bit ... a single code length of one, with one unused
    // code."
    let table = assert_ok(&[1u8], TableKind::Distance);
    // The one code (bit 0) decodes to symbol 0.
    let mut br = BitReader::new();
    let mut pos = 0usize;
    let input = [0b0000_0000u8];
    assert!(br.fill(&input, &mut pos, 1));
    assert_eq!(table.decode(&br), Decoded::Symbol { symbol: 0, len: 1 });
}

#[test]
fn distance_single_code_len2_rejected() {
    // A single code, but of length 2, is not the RFC's exception (which
    // requires length exactly 1).
    assert_incomplete(&[2u8], TableKind::Distance);
}

#[test]
fn distance_all_zero_accepted() {
    // RFC 1951 S3.2.7: "One distance code of zero bits means that there
    // are no distance codes used at all (the data is all literals)."
    let table = assert_ok(&[0u8, 0, 0], TableKind::Distance);
    let br = BitReader::new();
    assert_eq!(table.decode(&br), Decoded::NeedMoreBits);
}

#[test]
fn distance_two_codes_incomplete_rejected() {
    // Two length-1 codes would be a *complete* code (not an exception
    // case) but there are only 2 slots at length 1 and using both is
    // fine -- the point of this test is that going the other way, an
    // *incomplete* two-symbol distance table is still rejected (the
    // exception is only for exactly one code or zero codes).
    assert_incomplete(&[1u8, 3], TableKind::Distance);
}

#[test]
fn decode_unused_code_of_single_code_table_invalid() {
    let table = assert_ok(&[1u8], TableKind::Distance);
    let mut br = BitReader::new();
    let mut pos = 0usize;
    let input = [0b0000_0001u8, 0x00u8]; // the unused code: bit value 1
    assert!(br.fill(&input, &mut pos, 15));
    assert_eq!(table.decode(&br), Decoded::Invalid);
}

#[test]
fn decode_from_empty_table_invalid() {
    let table = assert_ok(&[], TableKind::Distance);
    let mut br = BitReader::new();
    let mut pos = 0usize;
    let input = [0xFFu8, 0xFFu8];
    assert!(br.fill(&input, &mut pos, 15));
    assert_eq!(table.decode(&br), Decoded::Invalid);
}

#[test]
fn fixed_tables_complete() {
    assert_ok(&fixed_litlen_lengths(), TableKind::LitLen);
    assert_ok(&fixed_dist_lengths(), TableKind::Distance);
}

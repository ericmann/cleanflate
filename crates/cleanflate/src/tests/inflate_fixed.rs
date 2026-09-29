use crate::inflate::{InflateStatus, Inflater};
use crate::tables::{DIST_BASE, DIST_EXTRA, LENGTH_BASE, LENGTH_EXTRA};
use crate::tests::bitwriter::BitWriter;

/// The fixed literal/length Huffman code for `symbol` (RFC 1951 S3.2.6):
/// (code, bit length), code given as an integer meant for `write_code`
/// (MSB-first).
fn fixed_litlen_code(symbol: u16) -> (u32, u32) {
    match symbol {
        0..=143 => (0x30 + symbol as u32, 8),
        144..=255 => (0x190 + (symbol - 144) as u32, 9),
        256..=279 => (symbol as u32 - 256, 7),
        280..=287 => (0xC0 + (symbol - 280) as u32, 8),
        _ => panic!("symbol out of range"),
    }
}

fn length_symbol_and_extra(length: u16) -> (u16, u32, u32) {
    for (i, &base) in LENGTH_BASE.iter().enumerate() {
        let extra = *LENGTH_EXTRA.get(i).expect("in range") as u32;
        let max = base + ((1u32 << extra) - 1) as u16;
        if length >= base && length <= max {
            return (257 + i as u16, extra, (length - base) as u32);
        }
    }
    panic!("length {length} out of range");
}

fn distance_symbol_and_extra(dist: u32) -> (u16, u32, u32) {
    for (i, &base) in DIST_BASE.iter().enumerate() {
        let extra = *DIST_EXTRA.get(i).expect("in range") as u32;
        let max = base as u32 + (1u32 << extra) - 1;
        if dist >= base as u32 && dist <= max {
            return (i as u16, extra, dist - base as u32);
        }
    }
    panic!("distance {dist} out of range");
}

pub(crate) fn emit_block_header(w: &mut BitWriter, bfinal: bool, btype: u32) {
    let value = (bfinal as u32) | (btype << 1);
    w.write_bits(value, 3);
}

pub(crate) fn emit_literal(w: &mut BitWriter, symbol: u16) {
    let (code, bits) = fixed_litlen_code(symbol);
    w.write_code(code, bits);
}

pub(crate) fn emit_end_of_block(w: &mut BitWriter) {
    emit_literal(w, 256);
}

pub(crate) fn emit_match(w: &mut BitWriter, length: u16, dist: u32) {
    let (lsym, lextra_bits, lextra_val) = length_symbol_and_extra(length);
    emit_literal(w, lsym);
    w.write_bits(lextra_val, lextra_bits);
    let (dsym, dextra_bits, dextra_val) = distance_symbol_and_extra(dist);
    w.write_code(dsym as u32, 5); // fixed distance codes are always 5 bits
    w.write_bits(dextra_val, dextra_bits);
}

/// Drive `inflater` to completion, feeding all of `input` at once but
/// through a small fixed-size output buffer (so OutputFull is exercised
/// too).
fn run(inflater: &mut Inflater, input: &[u8]) -> Result<(Vec<u8>, usize), crate::error::Error> {
    let mut out = Vec::new();
    let mut consumed_total = 0usize;
    let mut chunk = [0u8; 8];
    loop {
        let remaining_input = input.get(consumed_total..).unwrap_or(&[]);
        let (consumed, produced, status) = inflater.step(remaining_input, &mut chunk)?;
        consumed_total += consumed;
        out.extend_from_slice(chunk.get(..produced).unwrap_or(&[]));
        match status {
            InflateStatus::Done => return Ok((out, consumed_total)),
            InflateStatus::OutputFull => continue,
            InflateStatus::NeedsInput => {
                if consumed == 0 || remaining_input.is_empty() {
                    return Ok((out, consumed_total));
                }
            }
        }
    }
}

fn overlap_expected(seed: &[u8], dist: usize, len: usize) -> Vec<u8> {
    let mut v = seed.to_vec();
    for _ in 0..len {
        let idx = v.len() - dist;
        let b = *v.get(idx).expect("in range");
        v.push(b);
    }
    v
}

#[test]
fn fixed_literals_abc() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    emit_literal(&mut w, b'b' as u16);
    emit_literal(&mut w, b'c' as u16);
    emit_end_of_block(&mut w);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, b"abc");
    assert!(inflater.is_done());
}

#[test]
fn fixed_empty_block() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_end_of_block(&mut w);
    let bytes = w.finish();
    assert_eq!(bytes, [0x03u8, 0x00u8]);

    let mut inflater = Inflater::new();
    let (out, consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, Vec::<u8>::new());
    assert_eq!(consumed, 2);
    assert!(inflater.is_done());
}

#[test]
fn fixed_overlapping_match_distance_1() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'z' as u16);
    emit_match(&mut w, 5, 1);
    emit_end_of_block(&mut w);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, b"zzzzzz");
}

#[test]
fn fixed_match_distance_3_len_10() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    emit_literal(&mut w, b'b' as u16);
    emit_literal(&mut w, b'c' as u16);
    emit_match(&mut w, 10, 3);
    emit_end_of_block(&mut w);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, overlap_expected(b"abc", 3, 10));
}

#[test]
fn fixed_length_284_extra_31_is_258() {
    // length 258 via code 284 (base 227, extra 5, value 31): 227+31=258.
    let (sym, extra_bits, extra_val) = length_symbol_and_extra(258);
    assert_eq!(sym, 284);
    assert_eq!(extra_bits, 5);
    assert_eq!(extra_val, 31);

    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    emit_match(&mut w, 258, 1);
    emit_end_of_block(&mut w);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    let mut expected = vec![b'a'; 259];
    expected[0] = b'a';
    assert_eq!(out, expected);
    assert_eq!(out.len(), 259);
}

#[test]
fn fixed_symbol_286_invalid() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, 286);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("symbol 286 is invalid");
    assert_eq!(err, crate::error::Error::InvalidSymbol);
}

#[test]
fn fixed_distance_code_30_invalid() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    let (lsym, lextra_bits, lextra_val) = length_symbol_and_extra(3);
    emit_literal(&mut w, lsym);
    w.write_bits(lextra_val, lextra_bits);
    w.write_code(30, 5); // distance code 30: never valid
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("distance code 30 is invalid");
    assert_eq!(err, crate::error::Error::InvalidSymbol);
}

#[test]
fn distance_before_stream_start_invalid() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_match(&mut w, 3, 1); // no prior output at all
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("distance before stream start");
    assert_eq!(
        err,
        crate::error::Error::InvalidDistance {
            distance: 1,
            available: 0,
        }
    );
}

#[test]
fn max_distance_32768_ok() {
    // A stored block of exactly WINDOW_SIZE bytes, BFINAL=0.
    let pattern: Vec<u8> = (0..32768u32).map(|i| (i % 256) as u8).collect();
    let len: u16 = 32768u16.wrapping_sub(0); // 32768 doesn't fit u16 (max 65535 OK actually)
    let mut input = vec![0x00u8]; // BFINAL=0, BTYPE=00
    let len_field: u16 = pattern.len() as u16; // 32768 fits in u16 (<=65535)
    input.extend_from_slice(&len_field.to_le_bytes());
    input.extend_from_slice(&(!len_field).to_le_bytes());
    input.extend_from_slice(&pattern);
    let _ = len;

    // Then a fixed block: a match with distance exactly WINDOW_SIZE
    // (32768), referring to the very first byte of the stored block.
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_match(&mut w, 3, 32768);
    emit_end_of_block(&mut w);
    input.extend_from_slice(&w.finish());

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &input).expect("distance 32768 is valid");
    assert_eq!(out.len(), pattern.len() + 3);
    // The 3 matched bytes equal the first 3 bytes of the pattern.
    assert_eq!(
        out.get(pattern.len()..),
        Some(pattern.get(0..3).unwrap_or(&[]))
    );
}

#[test]
fn distance_one_past_available_invalid() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in b"abcd" {
        emit_literal(&mut w, b as u16);
    }
    emit_match(&mut w, 3, 5); // only 4 bytes available, distance 5
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("distance 5 after 4 bytes");
    assert_eq!(
        err,
        crate::error::Error::InvalidDistance {
            distance: 5,
            available: 4,
        }
    );
}

#[test]
fn fixed_one_byte_chunks_same_output() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    emit_literal(&mut w, b'b' as u16);
    emit_literal(&mut w, b'c' as u16);
    emit_match(&mut w, 10, 3);
    emit_end_of_block(&mut w);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let one_in = bytes.get(pos..(pos + 1).min(bytes.len())).unwrap_or(&[]);
        let mut one_out = [0u8; 1];
        let (consumed, produced, status) = inflater.step(one_in, &mut one_out).expect("no error");
        pos += consumed;
        out.extend_from_slice(one_out.get(..produced).unwrap_or(&[]));
        if status == InflateStatus::Done {
            break;
        }
        if consumed == 0 && one_in.is_empty() {
            panic!("ran out of input before Done");
        }
    }
    assert_eq!(out, overlap_expected(b"abc", 3, 10));
}

#[test]
fn stored_then_fixed_multi_block() {
    let mut input = vec![0x00u8]; // BFINAL=0, BTYPE=00
    let len: u16 = 2;
    input.extend_from_slice(&len.to_le_bytes());
    input.extend_from_slice(&(!len).to_le_bytes());
    input.extend_from_slice(b"ab");

    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'c' as u16);
    emit_end_of_block(&mut w);
    input.extend_from_slice(&w.finish());

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &input).expect("no error");
    assert_eq!(out, b"abc");
}

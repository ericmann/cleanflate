use crate::error::{Error, HuffmanError};
use crate::inflate::{InflateStatus, Inflater};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{
    canonical_codes, emit_dynamic_block, emit_dynamic_header_raw, huffman_lengths_from_freqs_u16,
    literal_cl_program, ClSymbol, DynBlock, PayloadSym,
};
use crate::tests::inflate_fixed::{emit_block_header, length_symbol_and_extra};

/// A valid, complete litlen length table: symbol 256 (end-of-block) plus
/// whatever extra symbols the caller lists, built with equal-frequency
/// canonical Huffman construction so it is always complete regardless of
/// how many distinct symbols are used (unlike a naive power-of-two-depth
/// assignment, which is only complete when the count happens to be a
/// power of two).
fn small_litlen_lengths(used_symbols: &[u16], hlit: usize) -> Vec<u8> {
    let mut symbols: Vec<u16> = used_symbols.to_vec();
    if !symbols.contains(&256) {
        symbols.push(256);
    }
    symbols.sort_unstable();
    symbols.dedup();
    let freqs: Vec<(u16, u32)> = symbols.iter().map(|&s| (s, 1u32)).collect();
    let lens = huffman_lengths_from_freqs_u16(&freqs);

    let mut lengths = vec![0u8; hlit];
    for (sym, len) in lens {
        if let Some(slot) = lengths.get_mut(sym as usize) {
            *slot = len;
        }
    }
    lengths
}

/// A single-entry distance table (length 1): the RFC 1951 S3.2.7
/// single-distance-code exception, decoding distance-symbol 0 (base 1).
fn single_distance_code() -> Vec<u8> {
    vec![1u8]
}

/// No distance codes at all (RFC 1951 S3.2.7's other exception).
fn no_distance_codes() -> Vec<u8> {
    vec![0u8]
}

fn combined(litlen_lengths: &[u8], dist_lengths: &[u8]) -> Vec<u8> {
    let mut v = litlen_lengths.to_vec();
    v.extend_from_slice(dist_lengths);
    v
}

fn run(inflater: &mut Inflater, input: &[u8]) -> Result<(Vec<u8>, usize), Error> {
    let mut out = Vec::new();
    let mut consumed_total = 0usize;
    let mut chunk = [0u8; 32];
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

#[test]
fn dynamic_literals_only() {
    let hlit = 257;
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16, b'b' as u16, b'c' as u16], hlit);
    let dist_lengths = no_distance_codes();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Literal(b'b'),
        PayloadSym::Literal(b'c'),
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, b"abc");
}

#[test]
fn dynamic_with_matches() {
    let hlit = 260;
    let (lsym, _, _) = length_symbol_and_extra(4);
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16, lsym], hlit);
    let dist_lengths = single_distance_code();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Match { len: 4, dist: 1 },
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, b"aaaaa");
}

#[test]
fn dynamic_repeat_codes_16_17_18() {
    // Symbols 0..=7 all length 4 (8 * 2^-4 = 0.5) and symbol 256 length 1
    // (1 * 2^-1 = 0.5): a complete code (sum exactly 1), hand-picked so
    // symbols 0..=7 legitimately share one length and can be transmitted
    // via a single literal length plus repeat-16 codes.
    let hlit = 257;
    let mut litlen_lengths = vec![0u8; hlit];
    for s in 0..8usize {
        if let Some(slot) = litlen_lengths.get_mut(s) {
            *slot = 4;
        }
    }
    if let Some(slot) = litlen_lengths.get_mut(256) {
        *slot = 1;
    }
    let dist_lengths = no_distance_codes();

    // Sequence position 0: symbol 0's length (4), literal.
    let mut program = vec![ClSymbol::Lit(4)];
    // positions 1..=7: repeat the previous length (4) seven times, via
    // two repeat-16 runs (4 + 3 = 7).
    program.push(ClSymbol::Rep16(4 - 3));
    program.push(ClSymbol::Rep16(3 - 3));
    // positions 8..=255: 248 zeros via repeat-18 runs (max 138 each).
    let mut remaining = 256 - 8;
    while remaining > 0 {
        let take = remaining.min(138);
        program.push(ClSymbol::Rep18((take - 11) as u8));
        remaining -= take;
    }
    // position 256: symbol 256's length (1), literal.
    program.push(ClSymbol::Lit(1));
    // the single distance-table entry: length 0 (no distance codes).
    program.push(ClSymbol::Lit(0));

    let block = DynBlock {
        bfinal: true,
        litlen_lengths,
        dist_lengths,
        cl_program: program,
    };
    let payload = [PayloadSym::Literal(0), PayloadSym::Literal(3)];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, vec![0u8, 3u8]);
}

#[test]
fn dynamic_repeat_crosses_boundary_ok() {
    // HLIT=260 (symbols 0..=259), HDIST=3. Only symbols 0 and 256 carry
    // real codes (both length 1, a complete 2-leaf code); everything
    // else is zero. The final zero run (positions 257..=262, 6 entries)
    // spans the litlen table's trailing padding (257..=259) and the
    // entire distance table (260..=262) in a single repeat-17 code,
    // crossing the HLIT/HDIST boundary mid-run (RFC 1951 S3.2.7: "the
    // code length repeat codes can cross from HLIT+257 to the HDIST+1
    // code lengths").
    let hlit = 260;
    let hdist = 3;
    let litlen_lengths = small_litlen_lengths(&[0u16, 256], hlit);
    let dist_lengths = vec![0u8; hdist];

    let mut program = vec![ClSymbol::Lit(1)]; // position 0: symbol 0
    let mut remaining = 255; // positions 1..=255: zeros
    while remaining > 0 {
        let take = remaining.min(138);
        program.push(ClSymbol::Rep18((take - 11) as u8));
        remaining -= take;
    }
    program.push(ClSymbol::Lit(1)); // position 256: symbol 256
                                    // positions 257..=262 (6 entries): one repeat-17 run, crossing
                                    // from litlen padding (257..=259) into the distance table
                                    // (260..=262).
    program.push(ClSymbol::Rep17(6 - 3));

    let block = DynBlock {
        bfinal: true,
        litlen_lengths,
        dist_lengths,
        cl_program: program,
    };
    let payload = [PayloadSym::Literal(0)];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no error");
    assert_eq!(out, vec![0u8]);
}

#[test]
fn dynamic_hlit_287_rejected() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b10);
    let hlit_field: u32 = 30; // hlit = 287
    w.write_bits(hlit_field, 14);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("hlit 287 rejected");
    assert_eq!(
        err,
        Error::InvalidHuffmanTable(HuffmanError::TooManyLengthCodes)
    );
}

#[test]
fn dynamic_hdist_32_accepted() {
    let hlit = 257;
    let hdist = 32;
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16], hlit);
    let dist_lengths = vec![5u8; hdist]; // complete: 32 * 2^-5 = 1

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [PayloadSym::Literal(b'a')];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("hdist 32 accepted");
    assert_eq!(out, b"a");
}

#[test]
fn dynamic_repeat16_first_rejected() {
    let mut cl_lengths = [0u8; 19];
    *cl_lengths.get_mut(16).expect("in range") = 1;
    *cl_lengths.get_mut(0).expect("in range") = 1;

    let mut w = BitWriter::new();
    emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
    let codes = canonical_codes(&cl_lengths);
    let (code, len) = codes
        .get(16)
        .copied()
        .flatten()
        .expect("symbol 16 has a code");
    w.write_code(code, len);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("repeat 16 with no previous length");
    assert_eq!(
        err,
        Error::InvalidHuffmanTable(HuffmanError::RepeatWithoutPrevious)
    );
}

#[test]
fn dynamic_repeat_overflow_rejected() {
    // HLIT=257, HDIST=1 (258 lengths needed total). Transmit 200 zeros,
    // then a repeat-18 of 138 -- 200 + 138 > 258.
    let mut cl_lengths = [0u8; 19];
    *cl_lengths.get_mut(0).expect("in range") = 1;
    *cl_lengths.get_mut(18).expect("in range") = 1;

    let mut w = BitWriter::new();
    emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
    let codes = canonical_codes(&cl_lengths);
    let (zero_code, zero_len) = codes
        .first()
        .copied()
        .flatten()
        .expect("symbol 0 has a code");
    let (rep_code, rep_len) = codes
        .get(18)
        .copied()
        .flatten()
        .expect("symbol 18 has a code");
    for _ in 0..200 {
        w.write_code(zero_code, zero_len);
    }
    w.write_code(rep_code, rep_len);
    w.write_bits(127, 7); // extra=127 -> repeat count 138
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("repeat overflow");
    assert_eq!(
        err,
        Error::InvalidHuffmanTable(HuffmanError::RepeatOverflow)
    );
}

#[test]
fn dynamic_missing_eob_rejected() {
    let hlit = 257;
    let mut litlen_lengths = small_litlen_lengths(&[b'a' as u16, b'b' as u16], hlit);
    if let Some(slot) = litlen_lengths.get_mut(256) {
        *slot = 0;
    }
    let dist_lengths = no_distance_codes();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [PayloadSym::Literal(b'a')];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("missing end-of-block code");
    assert_eq!(
        err,
        Error::InvalidHuffmanTable(HuffmanError::MissingEndOfBlock)
    );
}

#[test]
fn dynamic_single_distance_code_ok() {
    let hlit = 260;
    let (lsym, _, _) = length_symbol_and_extra(3);
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16, lsym], hlit);
    let dist_lengths = single_distance_code();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Match { len: 3, dist: 1 },
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("single distance code ok");
    assert_eq!(out, b"aaaa");
}

#[test]
fn dynamic_no_distance_codes_literals_ok() {
    let hlit = 257;
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16, b'b' as u16], hlit);
    let dist_lengths = no_distance_codes();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [PayloadSym::Literal(b'a'), PayloadSym::Literal(b'b')];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let (out, _consumed) = run(&mut inflater, &bytes).expect("no distance codes, literals ok");
    assert_eq!(out, b"ab");
}

#[test]
fn dynamic_no_distance_codes_then_length_invalid_symbol() {
    let hlit = 260;
    let (lsym, _, _) = length_symbol_and_extra(3);
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16, lsym], hlit);
    let dist_lengths = no_distance_codes();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Match { len: 3, dist: 1 },
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("length code with empty distance table");
    assert_eq!(err, Error::InvalidSymbol);
}

#[test]
fn dynamic_oversubscribed_codelength_rejected() {
    let mut cl_lengths = [0u8; 19];
    *cl_lengths.get_mut(0).expect("in range") = 1;
    *cl_lengths.get_mut(1).expect("in range") = 1;
    *cl_lengths.get_mut(2).expect("in range") = 1;
    *cl_lengths.get_mut(3).expect("in range") = 1;

    let mut w = BitWriter::new();
    emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
    let bytes = w.finish();

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &bytes).expect_err("over-subscribed code-length table");
    assert_eq!(
        err,
        Error::InvalidHuffmanTable(HuffmanError::OverSubscribed)
    );
}

#[test]
fn dynamic_one_byte_chunks_same_output() {
    let hlit = 260;
    let (lsym, _, _) = length_symbol_and_extra(4);
    let litlen_lengths = small_litlen_lengths(&[b'a' as u16, lsym], hlit);
    let dist_lengths = single_distance_code();

    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined(&litlen_lengths, &dist_lengths)),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Match { len: 4, dist: 1 },
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
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
        if status == InflateStatus::NeedsInput && consumed == 0 && one_in.is_empty() {
            panic!("ran out of input before Done");
        }
    }
    assert_eq!(out, b"aaaaa");
}

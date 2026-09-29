use crate::error::Error;
use crate::inflate::{InflateStatus, Inflater};

/// Drive `inflater` to completion (Done, NeedsInput with no more input to
/// give, or an error), feeding all of `input` and collecting output into
/// a growing `Vec<u8>`. Returns (output, total consumed, final status or
/// error).
fn run(inflater: &mut Inflater, input: &[u8]) -> Result<(Vec<u8>, usize, InflateStatus), Error> {
    let mut out = Vec::new();
    let mut consumed_total = 0usize;
    let mut chunk = [0u8; 64];

    loop {
        let remaining_input = input.get(consumed_total..).unwrap_or(&[]);
        let (consumed, produced, status) = inflater.step(remaining_input, &mut chunk)?;
        consumed_total += consumed;
        out.extend_from_slice(chunk.get(..produced).unwrap_or(&[]));

        match status {
            InflateStatus::Done => return Ok((out, consumed_total, InflateStatus::Done)),
            InflateStatus::OutputFull => continue,
            InflateStatus::NeedsInput => {
                if consumed == 0 {
                    return Ok((out, consumed_total, InflateStatus::NeedsInput));
                }
                // Otherwise loop again: more input may follow (this
                // helper is also used with the full buffer already
                // available, so consumed==0 with no progress means truly
                // out of input).
                if remaining_input.is_empty() {
                    return Ok((out, consumed_total, InflateStatus::NeedsInput));
                }
            }
        }
    }
}

#[test]
fn stored_empty_final_block() {
    let input = [0x01u8, 0x00, 0x00, 0xFF, 0xFF];
    let mut inflater = Inflater::new();
    let (out, consumed, status) = run(&mut inflater, &input).expect("no error");
    assert_eq!(out, Vec::<u8>::new());
    assert_eq!(status, InflateStatus::Done);
    assert_eq!(consumed, 5);
    assert!(inflater.is_done());
    assert_eq!(inflater.total_out(), 0);
}

#[test]
fn stored_hello() {
    // BFINAL=1, BTYPE=00, then LEN=5 NLEN=!5, then "hello".
    let len: u16 = 5;
    let nlen = !len;
    let mut input = vec![0x01u8];
    input.extend_from_slice(&len.to_le_bytes());
    input.extend_from_slice(&nlen.to_le_bytes());
    input.extend_from_slice(b"hello");

    let mut inflater = Inflater::new();
    let (out, consumed, status) = run(&mut inflater, &input).expect("no error");
    assert_eq!(out, b"hello");
    assert_eq!(status, InflateStatus::Done);
    assert_eq!(consumed, input.len());
    assert_eq!(inflater.total_out(), 5);
}

#[test]
fn stored_two_blocks_second_final() {
    let mut input = vec![0x00u8]; // BFINAL=0, BTYPE=00
    let len1: u16 = 2;
    input.extend_from_slice(&len1.to_le_bytes());
    input.extend_from_slice(&(!len1).to_le_bytes());
    input.extend_from_slice(b"ab");

    input.push(0x01); // BFINAL=1, BTYPE=00
    let len2: u16 = 3;
    input.extend_from_slice(&len2.to_le_bytes());
    input.extend_from_slice(&(!len2).to_le_bytes());
    input.extend_from_slice(b"cde");

    let mut inflater = Inflater::new();
    let (out, consumed, status) = run(&mut inflater, &input).expect("no error");
    assert_eq!(out, b"abcde");
    assert_eq!(status, InflateStatus::Done);
    assert_eq!(consumed, input.len());
}

#[test]
fn stored_nlen_mismatch_error() {
    let mut input = vec![0x01u8];
    input.extend_from_slice(&5u16.to_le_bytes());
    input.extend_from_slice(&5u16.to_le_bytes()); // should be !5, not 5
    input.extend_from_slice(b"hello");

    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &input).expect_err("nlen mismatch");
    assert_eq!(err, Error::InvalidStoredLength { len: 5, nlen: 5 });
}

#[test]
fn btype3_is_bad_block_type() {
    // BFINAL=1, BTYPE=11 -> low 3 bits = 1 | (0b11 << 1) = 0b111 = 0x07.
    let input = [0x07u8];
    let mut inflater = Inflater::new();
    let err = run(&mut inflater, &input).expect_err("reserved block type");
    assert_eq!(err, Error::BadBlockType);
}

#[test]
fn stored_one_byte_input_and_output_chunks_same_output() {
    let len: u16 = 4;
    let mut input = vec![0x01u8];
    input.extend_from_slice(&len.to_le_bytes());
    input.extend_from_slice(&(!len).to_le_bytes());
    input.extend_from_slice(b"data");

    let mut inflater = Inflater::new();
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let one_byte_in = input.get(pos..(pos + 1).min(input.len())).unwrap_or(&[]);
        let mut one_byte_out = [0u8; 1];
        let (consumed, produced, status) = inflater
            .step(one_byte_in, &mut one_byte_out)
            .expect("no error");
        pos += consumed;
        out.extend_from_slice(one_byte_out.get(..produced).unwrap_or(&[]));
        if status == InflateStatus::Done {
            break;
        }
        if consumed == 0 && one_byte_in.is_empty() {
            panic!("ran out of input before Done");
        }
    }
    assert_eq!(out, b"data");
    assert_eq!(pos, input.len());
}

#[test]
fn truncated_input_returns_needs_input() {
    let input = [0x01u8, 0x05, 0x00]; // header + partial LEN, no NLEN
    let mut inflater = Inflater::new();
    let mut out = [0u8; 16];
    let (consumed, produced, status) = inflater.step(&input, &mut out).expect("no error");
    assert_eq!(status, InflateStatus::NeedsInput);
    assert_eq!(consumed, input.len());
    assert_eq!(produced, 0);
}

#[test]
fn consumed_exact_with_trailing_bytes() {
    let mut input = vec![0x01u8];
    input.extend_from_slice(&3u16.to_le_bytes());
    input.extend_from_slice(&(!3u16).to_le_bytes());
    input.extend_from_slice(b"xyz");
    let junk = b"JUNKJUNK";
    let stream_len = input.len();
    input.extend_from_slice(junk);

    let mut inflater = Inflater::new();
    let (out, consumed, status) = run(&mut inflater, &input).expect("no error");
    assert_eq!(out, b"xyz");
    assert_eq!(status, InflateStatus::Done);
    assert_eq!(consumed, stream_len);
}

#[test]
fn error_is_sticky() {
    let input = [0x07u8]; // BTYPE 11
    let mut inflater = Inflater::new();
    let mut out = [0u8; 16];
    let first = inflater.step(&input, &mut out);
    assert!(first.is_err());

    // Calling again, even with different (valid-looking) input, returns
    // the same error.
    let valid_looking = [0x01u8, 0x00, 0x00, 0xFF, 0xFF];
    let second = inflater.step(&valid_looking, &mut out);
    assert_eq!(second, first);
}

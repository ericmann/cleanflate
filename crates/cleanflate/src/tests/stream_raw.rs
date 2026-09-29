use crate::error::Error;
use crate::options::{Format, Options, TrailingData};
use crate::stream::{decompress_with, inflate, next_len, Decompressor, Status};
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal, emit_match};

fn stored_stream(bfinal: bool, data: &[u8]) -> Vec<u8> {
    let mut out = vec![(bfinal as u8)];
    let len = data.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn fixed_literals_stream(bytes: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in bytes {
        emit_literal(&mut w, b as u16);
    }
    emit_end_of_block(&mut w);
    w.finish()
}

#[test]
fn inflate_stored_hello() {
    let stream = stored_stream(true, b"hello");
    let out = inflate(&stream).expect("no error");
    assert_eq!(out, b"hello");
}

#[test]
fn inflate_fixed_abc() {
    let stream = fixed_literals_stream(b"abc");
    let out = inflate(&stream).expect("no error");
    assert_eq!(out, b"abc");
}

#[test]
fn limit_exact_ok() {
    let stream = stored_stream(true, b"hello");
    let options = Options {
        max_output: 5,
        ..Options::default()
    };
    let decoded = decompress_with(Format::Raw, &stream, &options).expect("exact limit ok");
    assert_eq!(decoded.data, b"hello");
}

#[test]
fn limit_plus_one_err() {
    let stream = stored_stream(true, b"hello");
    let options = Options {
        max_output: 4,
        ..Options::default()
    };
    let err = decompress_with(Format::Raw, &stream, &options).expect_err("limit exceeded");
    assert_eq!(err, Error::OutputLimitExceeded { limit: 4 });
}

#[test]
fn next_len_never_exceeds_limit() {
    let limit: u64 = 100_000;
    let mut len = 0usize;
    for _ in 0..50 {
        let n = next_len(len, limit);
        assert!(
            n as u64 <= limit,
            "next_len({len}, {limit}) = {n} exceeds limit"
        );
        assert!(n >= len, "next_len must never shrink: {len} -> {n}");
        if n == len {
            break;
        }
        len = n;
    }
    assert_eq!(
        len as u64, limit,
        "growth should eventually reach the limit"
    );
}

#[test]
fn trailing_reject_default() {
    let mut stream = stored_stream(true, b"hi");
    stream.extend_from_slice(b"JUNK");
    let err = inflate(&stream).expect_err("trailing data rejected by default");
    assert!(matches!(err, Error::TrailingData { .. }));
}

#[test]
fn trailing_allow_reports_offset() {
    let mut stream = stored_stream(true, b"hi");
    let stream_len = stream.len();
    stream.extend_from_slice(b"JUNK");

    let options = Options {
        trailing: TrailingData::Allow,
        ..Options::default()
    };
    let mut d = Decompressor::new(Format::Raw, options);
    let mut out = [0u8; 64];
    let mut pos = 0usize;
    loop {
        let progress = d
            .decompress(stream.get(pos..).unwrap_or(&[]), &mut out)
            .expect("no error");
        pos += progress.consumed;
        if progress.status == Status::Done {
            break;
        }
    }
    d.finish().expect("finished cleanly");
    assert_eq!(d.trailing_start(), Some(stream_len as u64));
}

#[test]
fn trailing_after_done_in_later_call_rejected() {
    let stream = stored_stream(true, b"hi");
    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = [0u8; 64];
    let progress = d.decompress(&stream, &mut out).expect("no error");
    assert_eq!(progress.status, Status::Done);

    let err = d
        .decompress(b"JUNK", &mut out)
        .expect_err("trailing in later call");
    assert!(matches!(err, Error::TrailingData { .. }));
}

#[test]
fn empty_input_truncated() {
    let err = inflate(&[]).expect_err("empty input is truncated");
    assert_eq!(err, Error::Truncated);
}

#[test]
fn finish_before_done_truncated() {
    let stream = stored_stream(true, b"hello");
    let mut d = Decompressor::new(Format::Raw, Options::default());
    // Feed only part of the stream, never reaching Done.
    let mut out = [0u8; 64];
    let partial = stream.get(..2).unwrap_or(&[]);
    let _ = d.decompress(partial, &mut out);
    let err = d.finish().expect_err("not done yet");
    assert_eq!(err, Error::Truncated);
}

#[test]
fn error_is_sticky() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b11); // reserved BTYPE
    let stream = w.finish();

    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = [0u8; 16];
    let first = d.decompress(&stream, &mut out);
    assert!(first.is_err());

    let second = d.decompress(&stream, &mut out);
    assert_eq!(second, first);
}

#[test]
fn needs_input_consumes_everything() {
    let stream = stored_stream(true, b"hello");
    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = [0u8; 64];
    // Feed only the header byte plus half of LEN: not enough for a full
    // sub-step, so NeedsInput with everything consumed.
    let partial = stream.get(..2).unwrap_or(&[]);
    let progress = d.decompress(partial, &mut out).expect("no error");
    assert_eq!(progress.status, Status::NeedsInput);
    assert_eq!(progress.consumed, partial.len());
}

#[test]
fn fixed_match_via_public_api() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    emit_literal(&mut w, b'b' as u16);
    emit_literal(&mut w, b'c' as u16);
    emit_match(&mut w, 5, 3);
    emit_end_of_block(&mut w);
    let stream = w.finish();

    let out = inflate(&stream).expect("no error");
    assert_eq!(out, b"abcabcab");
}

#[test]
fn empty_output_slice_is_output_full_not_limit() {
    // R1-02: a zero-length `output` slice must be distinguishable from a
    // zero-length *max_output allowance* -- the former is just the
    // caller asking for no bytes this call (resumable `OutputFull`), not
    // the decompression-bomb guard actually having nothing left to give.
    let stream = stored_stream(true, b"hello");
    let mut d = Decompressor::new(Format::Raw, Options::default());

    let progress = d
        .decompress(&stream, &mut [])
        .expect("no error, not OutputLimitExceeded");
    assert_eq!(progress.status, Status::OutputFull);
    assert_eq!(progress.produced, 0);

    let mut out = [0u8; 16];
    let progress = d
        .decompress(&stream[progress.consumed..], &mut out)
        .expect("no error");
    assert_eq!(progress.status, Status::Done);
    assert_eq!(out.get(..progress.produced).unwrap_or(&[]), b"hello");
}

#[test]
fn streaming_limit_with_large_buffer() {
    // R1-02: the limit must fire based on the remaining `max_output`
    // allowance, not on whether the caller's buffer happens to be
    // larger than what's left -- a 64-byte buffer against a 4-byte
    // allowance still hits the limit, exactly at total_out() == limit.
    let stream = stored_stream(true, b"hello");

    let options = Options {
        max_output: 4,
        ..Options::default()
    };
    let mut d = Decompressor::new(Format::Raw, options);
    let mut out = [0u8; 64];
    let err = d.decompress(&stream, &mut out).expect_err("limit exceeded");
    assert_eq!(err, Error::OutputLimitExceeded { limit: 4 });
    assert_eq!(d.total_out(), 4);

    let options = Options {
        max_output: 5,
        ..Options::default()
    };
    let mut d = Decompressor::new(Format::Raw, options);
    let mut out = [0u8; 64];
    let progress = d.decompress(&stream, &mut out).expect("exact limit ok");
    assert_eq!(progress.status, Status::Done);
    assert_eq!(out.get(..progress.produced).unwrap_or(&[]), b"hello");
}

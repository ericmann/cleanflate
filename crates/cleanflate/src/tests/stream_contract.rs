//! Cross-format acceptance tests for the public streaming contract
//! (SPEC S3.3, DESIGN.md D-20): `Decompressor::decompress`'s `Status`
//! meanings, `finish()` semantics, trailing-data handling after `Done`,
//! and error stickiness, proven identical across `Format::Raw`,
//! `Format::Zlib` and `Format::Gzip`.

use crate::error::Error;
use crate::options::{Format, Options, TrailingData};
use crate::stream::{decompress_with, Decompressor, Status};
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal};

fn raw_stream(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in data {
        emit_literal(&mut w, b as u16);
    }
    emit_end_of_block(&mut w);
    w.finish()
}

/// C16: never call this crate's own this crate's own checksum module's adler32/crc32 in
/// a test fixture -- that would let a real checksum bug hide (a broken
/// encoder would satisfy a broken decoder's expectation). Every value
/// here was computed once with `oracle::ffi::{adler32,crc32}` over the
/// exact bytes named.
fn known_adler32(data: &[u8]) -> u32 {
    match data {
        b"the quick brown fox" => 0x478E_0734,
        b"abcdefghij" => 0x1586_03F8,
        b"hi" => 0x013B_00D2,
        b"exact" => 0x063B_0216,
        b"hello" => 0x062C_0215,
        b"finish-me" => 0x1204_0381,
        b"x" => 0x0079_0079,
        b"one-shot vs streaming" => 0x5804_0821,
        other => panic!(
            "known_adler32: no precomputed value for {other:?} -- add one \
             (compute with oracle::ffi::adler32 once, never this crate's own checksum module)"
        ),
    }
}

fn known_crc32(data: &[u8]) -> u32 {
    match data {
        b"the quick brown fox" => 0x91C1_02CA,
        b"abcdefghij" => 0x3981_703A,
        b"hi" => 0xD893_2AAC,
        b"exact" => 0x69D6_AB05,
        b"hello" => 0x3610_A686,
        b"finish-me" => 0x6F58_2E47,
        b"x" => 0x8CDC_1683,
        b"one-shot vs streaming" => 0xABF8_90B8,
        other => panic!(
            "known_crc32: no precomputed value for {other:?} -- add one \
             (compute with oracle::ffi::crc32 once, never this crate's own checksum module)"
        ),
    }
}

fn zlib_stream(data: &[u8]) -> Vec<u8> {
    let body = raw_stream(data);
    let mut out = vec![0x78u8, 0x9c]; // CM=8/CINFO=7, a valid FCHECK, no FDICT
    out.extend_from_slice(&body);
    out.extend_from_slice(&known_adler32(data).to_be_bytes());
    out
}

fn gzip_stream(data: &[u8]) -> Vec<u8> {
    let body = raw_stream(data);
    let mut out = vec![0x1fu8, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    out.extend_from_slice(&body);
    out.extend_from_slice(&known_crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

const FORMATS: [Format; 3] = [Format::Raw, Format::Zlib, Format::Gzip];

fn stream_for(format: Format, data: &[u8]) -> Vec<u8> {
    match format {
        Format::Raw => raw_stream(data),
        Format::Zlib => zlib_stream(data),
        Format::Gzip => gzip_stream(data),
    }
}

#[test]
fn one_byte_in_one_byte_out_all_formats() {
    let data = b"the quick brown fox";
    for &format in &FORMATS {
        let bytes = stream_for(format, data);
        let mut d = Decompressor::new(format, Options::default());
        let mut out = Vec::new();
        let mut pos = 0usize;
        loop {
            let one_in = bytes.get(pos..(pos + 1).min(bytes.len())).unwrap_or(&[]);
            let mut one_out = [0u8; 1];
            let progress = d.decompress(one_in, &mut one_out).unwrap_or_else(|e| {
                panic!("{format:?}: decompress failed: {e:?}");
            });
            pos += progress.consumed;
            out.extend_from_slice(one_out.get(..progress.produced).unwrap_or(&[]));
            if progress.status == Status::Done {
                break;
            }
            // Gzip may need one more byte to disambiguate a member
            // boundary (D-19) before it can report Done; once there is
            // truly no more input, stop and let finish() decide.
            if progress.status == Status::NeedsInput && one_in.is_empty() {
                break;
            }
        }
        d.finish()
            .unwrap_or_else(|e| panic!("{format:?}: finish failed: {e:?}"));
        assert_eq!(
            out, data,
            "{format:?}: output mismatch under 1-byte chunking"
        );
    }
}

#[test]
fn output_full_then_resume() {
    let data = b"abcdefghij";
    for &format in &FORMATS {
        let bytes = stream_for(format, data);
        let mut d = Decompressor::new(format, Options::default());
        let mut out = vec![0u8; data.len()];
        let mut produced_total = 0usize;
        let mut pos = 0usize;
        let mut saw_output_full = false;
        loop {
            let end = (produced_total + 3).min(out.len());
            let write_region = out.get_mut(produced_total..end).unwrap_or(&mut []);
            let input_slice = bytes.get(pos..).unwrap_or(&[]);
            let input_was_empty = input_slice.is_empty();
            let progress = d
                .decompress(input_slice, write_region)
                .unwrap_or_else(|e| panic!("{format:?}: decompress failed: {e:?}"));
            pos += progress.consumed;
            produced_total += progress.produced;
            match progress.status {
                Status::OutputFull => saw_output_full = true,
                Status::Done => break,
                Status::NeedsInput => {
                    // Gzip may need one more byte to disambiguate a
                    // member boundary (D-19); once there truly is no
                    // more input to offer, stop and let finish() decide.
                    if input_was_empty {
                        break;
                    }
                }
            }
        }
        d.finish()
            .unwrap_or_else(|e| panic!("{format:?}: finish failed: {e:?}"));
        assert!(
            saw_output_full,
            "{format:?}: expected at least one OutputFull"
        );
        assert_eq!(&out[..produced_total], data, "{format:?}: output mismatch");
    }
}

#[test]
fn needs_input_consumes_all_formats() {
    for &format in &FORMATS {
        let bytes = stream_for(format, b"hi");
        let mut d = Decompressor::new(format, Options::default());
        let mut out = [0u8; 64];
        // Feed a small prefix that cannot possibly complete the stream.
        let prefix_len = 1.min(bytes.len());
        let prefix = bytes.get(..prefix_len).unwrap_or(&[]);
        let progress = d
            .decompress(prefix, &mut out)
            .unwrap_or_else(|e| panic!("{format:?}: decompress failed: {e:?}"));
        assert_eq!(
            progress.status,
            Status::NeedsInput,
            "{format:?}: expected NeedsInput from a 1-byte prefix"
        );
        assert_eq!(
            progress.consumed,
            prefix.len(),
            "{format:?}: NeedsInput must consume everything offered"
        );
    }
}

#[test]
fn done_consumed_exact_raw_and_zlib() {
    // Gzip's own member-boundary lookahead (D-19) makes its exact
    // `consumed` count at `Done` a format-specific story tested in
    // tests/gzip.rs; Raw and Zlib have no such ambiguity; `consumed`
    // must land exactly at the end of the stream with no trailing bytes.
    for &format in &[Format::Raw, Format::Zlib] {
        let data = b"exact";
        let bytes = stream_for(format, data);
        let mut d = Decompressor::new(format, Options::default());
        let mut out = [0u8; 64];
        let mut pos = 0usize;
        loop {
            let progress = d
                .decompress(bytes.get(pos..).unwrap_or(&[]), &mut out)
                .unwrap_or_else(|e| panic!("{format:?}: decompress failed: {e:?}"));
            pos += progress.consumed;
            if progress.status == Status::Done {
                break;
            }
        }
        d.finish()
            .unwrap_or_else(|e| panic!("{format:?}: finish failed: {e:?}"));
        assert_eq!(
            pos,
            bytes.len(),
            "{format:?}: total consumed must equal the stream's exact length"
        );
    }
}

#[test]
fn after_done_reject_and_allow() {
    for &format in &FORMATS {
        let data = b"hi";
        let mut bytes = stream_for(format, data);
        let stream_len = bytes.len();
        bytes.extend_from_slice(&[0, 0, 0, 0]);

        // Reject (default): trailing data fails.
        {
            let mut d = Decompressor::new(format, Options::default());
            let mut out = [0u8; 64];
            let mut pos = 0usize;
            let mut result = Ok(());
            loop {
                match d.decompress(bytes.get(pos..).unwrap_or(&[]), &mut out) {
                    Ok(progress) => {
                        pos += progress.consumed;
                        if progress.status == Status::Done {
                            break;
                        }
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            if result.is_ok() {
                result = d.finish();
            }
            assert!(
                matches!(result, Err(Error::TrailingData { .. })),
                "{format:?}: expected TrailingData under the default Reject policy, got {result:?}"
            );
        }

        // Allow: succeeds, reports the offset.
        {
            let options = Options {
                trailing: TrailingData::Allow,
                ..Options::default()
            };
            let mut d = Decompressor::new(format, options);
            let mut out = [0u8; 64];
            let mut pos = 0usize;
            loop {
                let progress = d
                    .decompress(bytes.get(pos..).unwrap_or(&[]), &mut out)
                    .unwrap_or_else(|e| panic!("{format:?}: decompress failed: {e:?}"));
                pos += progress.consumed;
                if progress.status == Status::Done {
                    break;
                }
                if progress.status == Status::NeedsInput && pos >= bytes.len() {
                    break;
                }
            }
            d.finish()
                .unwrap_or_else(|e| panic!("{format:?}: finish failed: {e:?}"));
            assert_eq!(
                d.trailing_start(),
                Some(stream_len as u64),
                "{format:?}: trailing_start should report where the real stream ended"
            );
        }
    }
}

#[test]
fn error_sticky_all_formats() {
    for &format in &FORMATS {
        let mut bytes = stream_for(format, b"hello");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF; // corrupt the trailer checksum

        let mut d = Decompressor::new(format, Options::default());
        let mut out = [0u8; 64];
        let mut pos = 0usize;
        let first_err = loop {
            match d.decompress(bytes.get(pos..).unwrap_or(&[]), &mut out) {
                Ok(progress) => {
                    pos += progress.consumed;
                    if progress.status == Status::Done {
                        panic!("{format:?}: expected a checksum error, got Done");
                    }
                }
                Err(e) => break e,
            }
        };

        // Every later call returns a clone of the same error.
        let again = d
            .decompress(&[], &mut out)
            .expect_err("error must stay sticky");
        assert_eq!(again, first_err, "{format:?}: sticky error mismatch");
        let finish_err = d
            .finish()
            .expect_err("finish must also see the sticky error");
        assert_eq!(
            finish_err, first_err,
            "{format:?}: finish sticky error mismatch"
        );
    }
}

#[test]
fn finish_semantics_table() {
    for &format in &FORMATS {
        let bytes = stream_for(format, b"finish-me");

        // Truncated mid-stream: finish() must fail with Truncated.
        {
            let truncated = bytes.get(..bytes.len() / 2).unwrap_or(&[]);
            let mut d = Decompressor::new(format, Options::default());
            let mut out = [0u8; 64];
            let _ = d.decompress(truncated, &mut out);
            let err = d.finish().expect_err("finish before Done must fail");
            assert_eq!(err, Error::Truncated, "{format:?}: expected Truncated");
        }

        // Complete stream: finish() must succeed once Done is reached.
        {
            let mut d = Decompressor::new(format, Options::default());
            let mut out = [0u8; 64];
            let mut pos = 0usize;
            loop {
                let input_slice = bytes.get(pos..).unwrap_or(&[]);
                let input_was_empty = input_slice.is_empty();
                let progress = d
                    .decompress(input_slice, &mut out)
                    .unwrap_or_else(|e| panic!("{format:?}: decompress failed: {e:?}"));
                pos += progress.consumed;
                if progress.status == Status::Done {
                    break;
                }
                if progress.status == Status::NeedsInput && input_was_empty {
                    break;
                }
            }
            d.finish()
                .unwrap_or_else(|e| panic!("{format:?}: finish should succeed: {e:?}"));
        }
    }
}

#[test]
fn zero_length_output_slice_makes_header_progress() {
    // Zlib and Gzip both have header bytes that precede any output;
    // feeding input with a zero-length output slice must still let the
    // decoder consume those header bytes (nothing needs to be written
    // yet).
    for &format in &[Format::Zlib, Format::Gzip] {
        let bytes = stream_for(format, b"x");
        let mut d = Decompressor::new(format, Options::default());
        let header_len = if format == Format::Zlib { 2 } else { 10 };
        let progress = d
            .decompress(bytes.get(..header_len).unwrap_or(&[]), &mut [])
            .unwrap_or_else(|e| panic!("{format:?}: decompress failed: {e:?}"));
        assert_eq!(
            progress.consumed, header_len,
            "{format:?}: header bytes should be consumed even with a zero-length output slice"
        );
        assert_eq!(progress.produced, 0);
    }
}

#[test]
fn one_shot_matches_streaming_all_formats() {
    let data = b"one-shot vs streaming";
    for &format in &FORMATS {
        let bytes = stream_for(format, data);
        let one_shot = decompress_with(format, &bytes, &Options::default())
            .unwrap_or_else(|e| panic!("{format:?}: one-shot failed: {e:?}"));
        assert_eq!(one_shot.data, data, "{format:?}: one-shot mismatch");
    }
}

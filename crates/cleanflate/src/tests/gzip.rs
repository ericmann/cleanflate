use crate::error::{ChecksumKind, Error};
use crate::options::{Format, Options, TrailingData};
use crate::stream::{decompress_with, gzip_decompress, Decompressor, Status};
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal};

fn stored_body(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01u8]; // BFINAL=1, BTYPE=00
    let len = data.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn fixed_literals_body(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in data {
        emit_literal(&mut w, b as u16);
    }
    emit_end_of_block(&mut w);
    w.finish()
}

/// C16: never call this crate's own this crate's own checksum module's crc32 in a test
/// fixture -- that would let a real CRC-32 bug hide (a broken encoder
/// would satisfy a broken decoder's expectation). Every value here was
/// computed once with `oracle::ffi::crc32` over the exact bytes named.
fn known_crc32(data: &[u8]) -> u32 {
    match data {
        b"" => 0x0000_0000,
        b"hello" => 0x3610_A686,
        b"foo" => 0x8C73_6521,
        b"bar" => 0x76FF_8CAA,
        b"hi" => 0xD893_2AAC,
        b"a" => 0xE8B7_BE43,
        b"b" => 0x71BE_EFF9,
        b"ab" => 0x9E83_486D,
        b"cd" => 0x45D6_8FDA,
        other => panic!(
            "known_crc32: no precomputed value for {other:?} -- add one \
             (compute with oracle::ffi::crc32 once, never this crate's own checksum module)"
        ),
    }
}

/// Wrap raw DEFLATE `body` (already decoding to `data`) as one minimal
/// gzip member (RFC 1952 S2.3): ID1/ID2/CM/FLG=0/MTIME=0/XFL=0/OS=255,
/// body, then CRC32+ISIZE (both little-endian).
fn gzip_member(body: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1fu8, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    out.extend_from_slice(body);
    out.extend_from_slice(&known_crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

#[test]
fn gzip_minimal_empty_member() {
    let bytes = gzip_member(&stored_body(b""), b"");
    let out = gzip_decompress(&bytes).expect("no error");
    assert_eq!(out, Vec::<u8>::new());
}

#[test]
fn gzip_hello() {
    let bytes = gzip_member(&fixed_literals_body(b"hello"), b"hello");
    let out = gzip_decompress(&bytes).expect("no error");
    assert_eq!(out, b"hello");
}

#[test]
fn gzip_two_members_concatenate() {
    let mut bytes = gzip_member(&fixed_literals_body(b"foo"), b"foo");
    bytes.extend_from_slice(&gzip_member(&fixed_literals_body(b"bar"), b"bar"));
    let out = gzip_decompress(&bytes).expect("no error");
    assert_eq!(out, b"foobar");
}

#[test]
fn gzip_empty_member_between() {
    let mut bytes = gzip_member(&fixed_literals_body(b"foo"), b"foo");
    bytes.extend_from_slice(&gzip_member(&stored_body(b""), b""));
    bytes.extend_from_slice(&gzip_member(&fixed_literals_body(b"bar"), b"bar"));
    let out = gzip_decompress(&bytes).expect("no error");
    assert_eq!(out, b"foobar");
}

#[test]
fn gzip_crc_mismatch() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    let len = bytes.len();
    bytes[len - 8] ^= 0xFF; // corrupt a CRC32 byte
    let err = gzip_decompress(&bytes).expect_err("crc mismatch");
    assert!(matches!(
        err,
        Error::ChecksumMismatch {
            kind: ChecksumKind::Crc32,
            ..
        }
    ));
}

#[test]
fn gzip_isize_mismatch() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    let len = bytes.len();
    bytes[len - 1] ^= 0xFF; // corrupt an ISIZE byte
    let err = gzip_decompress(&bytes).expect_err("isize mismatch");
    assert!(matches!(
        err,
        Error::ChecksumMismatch {
            kind: ChecksumKind::Isize,
            ..
        }
    ));
}

#[test]
fn gzip_trailing_zeros_rejected_by_default() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    let err = gzip_decompress(&bytes).expect_err("trailing data rejected by default");
    assert!(matches!(err, Error::TrailingData { .. }));
}

#[test]
fn gzip_trailing_allowed_reports_offset() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    let stream_len = bytes.len();
    bytes.extend_from_slice(&[0, 0, 0, 0]);

    let options = Options {
        trailing: TrailingData::Allow,
        ..Options::default()
    };
    let mut d = Decompressor::new(Format::Gzip, options);
    let mut out = [0u8; 64];
    let mut pos = 0usize;
    loop {
        let progress = d
            .decompress(bytes.get(pos..).unwrap_or(&[]), &mut out)
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
fn gzip_second_member_bad_header() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    bytes.extend_from_slice(&[0x1f, 0x8b, 7, 0, 0, 0, 0, 0, 0, 255]); // CM=7, not 8
    let err = gzip_decompress(&bytes).expect_err("bad second member header");
    assert!(matches!(err, Error::BadHeader(_)));
}

#[test]
fn gzip_lone_1f_trailing_reject() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    bytes.push(0x1f);
    let err = gzip_decompress(&bytes).expect_err("lone 0x1f rejected by default");
    assert!(matches!(err, Error::TrailingData { .. }));
}

#[test]
fn gzip_lone_1f_trailing_allow() {
    let mut bytes = gzip_member(&stored_body(b"hi"), b"hi");
    let stream_len = bytes.len();
    bytes.push(0x1f);

    let options = Options {
        trailing: TrailingData::Allow,
        ..Options::default()
    };
    let decoded = decompress_with(Format::Gzip, &bytes, &options).expect("allowed");
    assert_eq!(decoded.data, b"hi");

    // Re-derive via the streaming API to check the reported offset too.
    let mut d = Decompressor::new(Format::Gzip, options);
    let mut out = [0u8; 64];
    let mut pos = 0usize;
    loop {
        let progress = d
            .decompress(bytes.get(pos..).unwrap_or(&[]), &mut out)
            .expect("no error");
        pos += progress.consumed;
        if progress.status == Status::Done {
            break;
        }
        if progress.status == Status::NeedsInput && pos >= bytes.len() {
            break;
        }
    }
    d.finish().expect("finished cleanly");
    assert_eq!(d.trailing_start(), Some(stream_len as u64));
}

#[test]
fn gzip_one_byte_chunks_multi_member() {
    let mut bytes = gzip_member(&fixed_literals_body(b"ab"), b"ab");
    bytes.extend_from_slice(&gzip_member(&fixed_literals_body(b"cd"), b"cd"));

    let mut d = Decompressor::new(Format::Gzip, Options::default());
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let one_in = bytes.get(pos..(pos + 1).min(bytes.len())).unwrap_or(&[]);
        let mut one_out = [0u8; 1];
        let progress = d.decompress(one_in, &mut one_out).expect("no error");
        pos += progress.consumed;
        out.extend_from_slice(one_out.get(..progress.produced).unwrap_or(&[]));
        if progress.status == Status::Done {
            break;
        }
        // gzip cannot tell "clean end of stream" from "a new member is
        // about to start" until it either sees the next member's magic
        // bytes or `finish()` confirms no more input is coming; once the
        // one-byte-chunk supply is exhausted, stop feeding and finish().
        if progress.status == Status::NeedsInput && one_in.is_empty() {
            break;
        }
    }
    d.finish().expect("finished cleanly");
    assert_eq!(out, b"abcd");
}

#[test]
fn gzip_headers_exposed() {
    let mut bytes = gzip_member(&stored_body(b"a"), b"a");
    bytes.extend_from_slice(&gzip_member(&stored_body(b"b"), b"b"));
    let decoded = decompress_with(Format::Gzip, &bytes, &Options::default()).expect("no error");
    assert_eq!(decoded.gzip_members.len(), 2);
    assert_eq!(decoded.gzip_members[0].os, 255);
    assert_eq!(decoded.gzip_members[1].os, 255);
}

#[test]
fn gzip_trailing_allow_consumed_exact() {
    // R1-03: consumed at Done, and trailing_start(), must both land
    // exactly at the end of the last member -- not overshoot into bytes
    // that were only peeked while deciding whether a new member follows.
    for trailer in [&b"XYZ"[..], &[0x1f, b'Z'][..]] {
        let member = gzip_member(&stored_body(b"hi"), b"hi");
        let member_len = member.len();
        let mut bytes = member.clone();
        bytes.extend_from_slice(trailer);

        let options = Options {
            trailing: TrailingData::Allow,
            ..Options::default()
        };
        let decoded = decompress_with(Format::Gzip, &bytes, &options).expect("allowed");
        assert_eq!(
            decoded.consumed, member_len,
            "one-shot: trailer={trailer:?}"
        );

        let mut d = Decompressor::new(Format::Gzip, options);
        let mut out = [0u8; 64];
        let progress = d.decompress(&bytes, &mut out).expect("no error");
        assert_eq!(
            progress.consumed, member_len,
            "streaming: trailer={trailer:?}"
        );
        assert_eq!(progress.status, Status::Done, "trailer={trailer:?}");
        assert_eq!(
            d.trailing_start(),
            Some(member_len as u64),
            "trailing_start: trailer={trailer:?}"
        );
    }
}

#[test]
fn gzip_truncated_in_trailer() {
    let bytes = gzip_member(&stored_body(b"hi"), b"hi");
    // Cut off 2 of the 8 trailer bytes.
    let truncated = bytes.get(..bytes.len() - 2).unwrap_or(&[]);
    let err = gzip_decompress(truncated).expect_err("truncated trailer");
    assert_eq!(err, Error::Truncated);
}

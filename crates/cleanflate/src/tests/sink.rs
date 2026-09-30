use crate::config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_GZIP_HEADERS};
use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::gzip::GzipDecoder;
use crate::options::TrailingData;
use crate::sink::{OutputSink, SliceSink};
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal, emit_match};

#[test]
fn slice_sink_push_extend_room() {
    let mut buf = [0u8; 6];
    let mut s = SliceSink::new(&mut buf);
    assert_eq!((s.written(), s.room()), (0, 6));
    assert!(s.push(1));
    assert_eq!(s.extend_from_slice(&[2, 3]), 2);
    assert_eq!((s.written(), s.room()), (3, 3));
    // A longer slice writes only what fits.
    assert_eq!(s.extend_from_slice(&[4, 5, 6, 7, 8]), 3);
    assert_eq!((s.written(), s.room()), (6, 0));
    assert!(!s.push(9), "no room: nothing written");
    assert_eq!(s.extend_from_slice(&[1]), 0);
    assert_eq!(s.written_since(0), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(buf, [1, 2, 3, 4, 5, 6]);
}

#[test]
fn slice_sink_copy_within_back_preconditions() {
    let mut buf = [0u8; 10];
    let mut s = SliceSink::new(&mut buf);
    s.extend_from_slice(&[1, 2, 3]);
    assert_eq!(s.copy_within_back(0, 1), 0, "dist 0");
    assert_eq!(s.copy_within_back(4, 1), 0, "dist beyond written");
    assert_eq!(s.copy_within_back(2, 3), 0, "n beyond dist (overlap)");
    assert_eq!(s.written(), 3, "failed copies write nothing");
    assert_eq!(s.copy_within_back(3, 3), 3);
    assert_eq!(s.written_since(0), &[1, 2, 3, 1, 2, 3]);
    // Room limits give a short count.
    assert_eq!(s.copy_within_back(5, 5), 4);
    assert_eq!(s.written(), 10);
    assert_eq!(s.copy_within_back(1, 1), 0, "no room");
}

#[test]
fn slice_sink_get_back_and_written_since() {
    let mut buf = [0u8; 8];
    let mut s = SliceSink::new(&mut buf);
    assert_eq!(s.get_back(1), None);
    s.extend_from_slice(&[10, 20, 30, 40]);
    assert_eq!(s.get_back(0), None);
    assert_eq!(s.get_back(1), Some(40));
    assert_eq!(s.get_back(4), Some(10));
    assert_eq!(s.get_back(5), None);
    assert_eq!(s.written_since(0), &[10, 20, 30, 40]);
    assert_eq!(s.written_since(2), &[30, 40]);
    assert!(s.written_since(4).is_empty());
    assert!(s.written_since(9).is_empty(), "start past the end is empty");
}

/// One fixed-Huffman member body: literals, then an optional match.
fn body(literals: &[u8], match_: Option<(u16, u32)>) -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in literals {
        emit_literal(&mut w, b as u16);
    }
    if let Some((len, dist)) = match_ {
        emit_match(&mut w, len, dist);
    }
    emit_end_of_block(&mut w);
    w.finish()
}

/// C16: every CRC below is a literal computed once with `oracle::ffi::crc32`.
#[test]
fn gzip_second_member_history_isolated_in_one_call() {
    let header = [0x1fu8, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    // Member 1: "abc", CRC-32 0x352441C2, ISIZE 3.
    let mut input = header.to_vec();
    input.extend_from_slice(&body(b"abc", None));
    input.extend_from_slice(&0x3524_41C2u32.to_le_bytes());
    input.extend_from_slice(&3u32.to_le_bytes());
    // Member 2: literal 'x', then a match at distance 2. Only one byte of
    // its own output exists, so the distance must be rejected even though
    // member 1's bytes sit right behind it in the same output.
    input.extend_from_slice(&header);
    input.extend_from_slice(&body(b"x", Some((3, 2))));
    input.extend_from_slice(&[0u8; 8]);

    for path in [DecodePath::Fast, DecodePath::Reference] {
        let mut d = GzipDecoder::new(
            TrailingData::Reject,
            DEFAULT_MAX_GZIP_FIELD,
            DEFAULT_MAX_GZIP_HEADERS,
            path,
        );
        let mut out = [0u8; 64];
        let err = d
            .step(&input, &mut out)
            .expect_err("distance reaches into member 1");
        assert_eq!(
            err,
            Error::InvalidDistance {
                distance: 2,
                available: 1
            },
            "{path:?}"
        );
    }
}

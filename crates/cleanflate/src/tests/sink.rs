use crate::config::{DEFAULT_MAX_GZIP_FIELD, DEFAULT_MAX_GZIP_HEADERS};
use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::gzip::GzipDecoder;
use crate::options::TrailingData;
use crate::options::{Format, Options};
use crate::sink::{OutputSink, SliceSink, VecSink};
use crate::stream::{decompress_core_vec, decompress_with_path, grow_output, next_len};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{emit_dynamic_block, literal_cl_program, DynBlock, PayloadSym};
use crate::tests::inflate_fixed::length_symbol_and_extra;
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

#[test]
fn vec_sink_push_extend_copy_room() {
    let mut v: Vec<u8> = Vec::with_capacity(8);
    v.push(9); // earlier contents are not part of this call's output
    let mut s = VecSink::new(&mut v, 5);
    assert_eq!((s.written(), s.room()), (0, 5));
    assert!(s.push(1));
    assert_eq!(s.extend_from_slice(&[2, 3]), 2);
    assert_eq!(s.copy_within_back(3, 3), 2, "room limits the copy");
    assert_eq!((s.written(), s.room()), (5, 0));
    assert!(!s.push(7));
    assert_eq!(s.get_back(1), Some(2));
    assert_eq!(s.get_back(5), Some(1));
    assert_eq!(s.get_back(6), None, "reaches before this call's output");
    assert_eq!(s.written_since(2), &[3, 1, 2]);
    assert_eq!(s.copy_within_back(0, 1), 0);
    assert_eq!(v, [9, 1, 2, 3, 1, 2]);
    assert!(v.capacity() >= 6);
}

#[test]
fn vec_sink_growth_follows_next_len_and_cap() {
    for limit in [0u64, 1, 10_000, 70_000, 1 << 20] {
        let first = next_len(0, limit);
        let mut v: Vec<u8> = Vec::with_capacity(first);
        let mut caps = vec![v.capacity()];
        while grow_output(&mut v, limit) {
            caps.push(v.capacity());
            assert!(caps.len() < 64, "growth must terminate");
        }
        for pair in caps.windows(2) {
            assert_eq!(pair[1], next_len(pair[0], limit), "limit {limit}");
        }
        assert!(
            v.capacity() as u64 <= limit.max(first as u64),
            "capacity {} past limit {limit}",
            v.capacity()
        );
    }
}

fn stored_body(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01u8];
    let len = data.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn vec_decode(
    format: Format,
    input: &[u8],
    options: &Options,
    path: DecodePath,
) -> Result<crate::stream::Decoded, Error> {
    decompress_core_vec(format, input, options, path).map(|c| c.decoded)
}

#[test]
fn vec_sink_exact_limit_ok_one_over_errors() {
    let n = 100usize;
    let data: Vec<u8> = (0..n as u32).map(|i| (i * 7) as u8).collect();
    let input = stored_body(&data);
    for path in [DecodePath::Fast, DecodePath::Reference] {
        let exact = Options {
            max_output: n as u64,
            ..Options::default()
        };
        let under = Options {
            max_output: n as u64 - 1,
            ..Options::default()
        };
        let slice_ok = decompress_with_path(Format::Raw, &input, &exact, path).expect("slice ok");
        let vec_ok = vec_decode(Format::Raw, &input, &exact, path).expect("vec ok");
        assert_eq!(slice_ok.data, data);
        assert_eq!(vec_ok.data, data);
        let want = Error::OutputLimitExceeded {
            limit: n as u64 - 1,
        };
        assert_eq!(
            decompress_with_path(Format::Raw, &input, &under, path).expect_err("slice over"),
            want
        );
        assert_eq!(
            vec_decode(Format::Raw, &input, &under, path).expect_err("vec over"),
            want
        );
    }
}

fn fixed_with_match() -> (Vec<u8>, Vec<u8>) {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in b"abcab" {
        emit_literal(&mut w, b as u16);
    }
    emit_match(&mut w, 20, 3);
    emit_match(&mut w, 258, 1);
    emit_end_of_block(&mut w);
    let mut expected = b"abcab".to_vec();
    for (len, dist) in [(20usize, 3usize), (258, 1)] {
        for _ in 0..len {
            let b = expected[expected.len() - dist];
            expected.push(b);
        }
    }
    (w.finish(), expected)
}

fn dynamic_with_match() -> (Vec<u8>, Vec<u8>) {
    let (lsym, _, _) = length_symbol_and_extra(4);
    let mut litlen = vec![0u8; 260];
    litlen[b'a' as usize] = 2;
    litlen[256] = 2;
    litlen[lsym as usize] = 1;
    let dist = vec![1u8];
    let mut all = litlen.clone();
    all.extend_from_slice(&dist);
    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&all),
        litlen_lengths: litlen,
        dist_lengths: dist,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Match { len: 4, dist: 1 },
    ];
    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    (w.finish(), b"aaaaa".to_vec())
}

#[test]
fn vec_sink_matches_slice_sink_hand_built() {
    let header = [0x1fu8, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    // C16: CRC-32 literals computed once with oracle::ffi::crc32.
    let mut two_members = header.to_vec();
    two_members.extend_from_slice(&stored_body(b"abc"));
    two_members.extend_from_slice(&0x3524_41C2u32.to_le_bytes());
    two_members.extend_from_slice(&3u32.to_le_bytes());
    two_members.extend_from_slice(&header);
    two_members.extend_from_slice(&stored_body(b"foo"));
    two_members.extend_from_slice(&0x8C73_6521u32.to_le_bytes());
    two_members.extend_from_slice(&3u32.to_le_bytes());

    let (fixed, fixed_out) = fixed_with_match();
    let (dynamic, dynamic_out) = dynamic_with_match();
    let stored = stored_body(b"hello stored");
    // Dictionary: a match reaching back into the preset window.
    let mut dict_body = BitWriter::new();
    emit_block_header(&mut dict_body, true, 0b01);
    emit_match(&mut dict_body, 3, 3);
    emit_end_of_block(&mut dict_body);
    let dict_body = dict_body.finish();
    let dict_options = Options {
        dictionary: Some(b"xabc".to_vec()),
        ..Options::default()
    };

    type Case = (&'static str, Format, Vec<u8>, Options, Vec<u8>);
    let cases: Vec<Case> = vec![
        ("fixed", Format::Raw, fixed, Options::default(), fixed_out),
        (
            "dynamic",
            Format::Raw,
            dynamic,
            Options::default(),
            dynamic_out,
        ),
        (
            "stored",
            Format::Raw,
            stored,
            Options::default(),
            b"hello stored".to_vec(),
        ),
        (
            "dictionary",
            Format::Raw,
            dict_body,
            dict_options,
            b"abc".to_vec(),
        ),
        (
            "two members",
            Format::Gzip,
            two_members,
            Options::default(),
            b"abcfoo".to_vec(),
        ),
    ];
    for (name, format, input, options, expected) in cases {
        for path in [DecodePath::Fast, DecodePath::Reference] {
            let slice = decompress_with_path(format, &input, &options, path).expect(name);
            let vec = vec_decode(format, &input, &options, path).expect(name);
            assert_eq!(slice.data, expected, "{name} slice {path:?}");
            assert_eq!(vec.data, expected, "{name} vec {path:?}");
            assert_eq!(vec.consumed, slice.consumed, "{name} consumed");
            assert_eq!(vec.gzip_members, slice.gzip_members, "{name} headers");
        }
    }
    // An error is the same on both sinks.
    let bad = [0x07u8];
    assert_eq!(
        decompress_with_path(Format::Raw, &bad, &Options::default(), DecodePath::Fast)
            .map(|d| d.data),
        vec_decode(Format::Raw, &bad, &Options::default(), DecodePath::Fast).map(|d| d.data)
    );
    assert_eq!(
        vec_decode(Format::Raw, &[], &Options::default(), DecodePath::Fast).map(|d| d.data),
        Err(Error::Truncated)
    );
}

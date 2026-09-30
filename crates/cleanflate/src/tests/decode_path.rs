use crate::config::DEFAULT_MAX_GZIP_FIELD;
use crate::config::DEFAULT_MAX_GZIP_HEADERS;
use crate::decode_path::DecodePath;
use crate::gzip::GzipDecoder;
use crate::inflate::InflateStatus;
use crate::options::{Format, Options, TrailingData};
use crate::stream::decompress_with_path;
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{emit_dynamic_block, literal_cl_program, DynBlock, PayloadSym};
use crate::tests::inflate_fixed::{
    emit_block_header, emit_end_of_block, emit_literal, emit_match, length_symbol_and_extra,
};
use crate::window::Window;

#[test]
fn default_path_is_fast() {
    assert_eq!(DecodePath::default(), DecodePath::Fast);
}

fn stored_stream() -> (Vec<u8>, Vec<u8>) {
    (
        vec![0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'],
        b"abc".to_vec(),
    )
}

fn fixed_stream() -> (Vec<u8>, Vec<u8>) {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in b"abcab" {
        emit_literal(&mut w, b as u16);
    }
    emit_match(&mut w, 20, 3);
    emit_match(&mut w, 258, 1);
    emit_end_of_block(&mut w);
    let mut expected = b"abcab".to_vec();
    for len in [20usize, 258] {
        let dist = if len == 20 { 3 } else { 1 };
        for _ in 0..len {
            let b = expected[expected.len() - dist];
            expected.push(b);
        }
    }
    (w.finish(), expected)
}

fn dynamic_stream() -> (Vec<u8>, Vec<u8>) {
    let (lsym, _, _) = length_symbol_and_extra(4);
    let mut litlen = vec![0u8; 260];
    let a = b'a' as usize;
    litlen[a] = 2;
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
fn reference_path_same_output_fixed_dynamic_stored() {
    for (bytes, expected) in [stored_stream(), fixed_stream(), dynamic_stream()] {
        let options = Options::default();
        for path in [DecodePath::Fast, DecodePath::Reference] {
            let got = decompress_with_path(Format::Raw, &bytes, &options, path).expect("decodes");
            assert_eq!(got.data, expected, "path {path:?}");
            assert_eq!(got.consumed, bytes.len());
        }
    }
}

#[test]
fn gzip_second_member_uses_same_path() {
    let member: Vec<u8> = {
        let mut v = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03];
        v.extend_from_slice(&[0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c']);
        // CRC-32 of "abc" and ISIZE 3, little endian.
        v.extend_from_slice(&[0xc2, 0x41, 0x24, 0x35, 0x03, 0x00, 0x00, 0x00]);
        v
    };
    let mut input = member.clone();
    input.extend_from_slice(member.get(..10).expect("header"));

    let mut d = GzipDecoder::new(
        TrailingData::Reject,
        DEFAULT_MAX_GZIP_FIELD,
        DEFAULT_MAX_GZIP_HEADERS,
        DecodePath::Reference,
    );
    assert_eq!(d.inflater_path(), DecodePath::Reference);
    let mut out = [0u8; 16];
    let (consumed, produced, status) = d.step(&input, &mut out).expect("steps");
    assert_eq!(consumed, input.len());
    assert_eq!(out.get(..produced), Some(&b"abc"[..]));
    assert_eq!(status, InflateStatus::NeedsInput);
    assert_eq!(
        d.member_count(),
        2,
        "second member header must have been parsed"
    );
    assert_eq!(d.inflater_path(), DecodePath::Reference);
}

fn seeded_bytes(total: usize) -> Vec<u8> {
    let mut x: u32 = 0x9E37_79B9;
    (0..total)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x >> 11) as u8
        })
        .collect()
}

#[test]
fn reference_byte_copy_matches_bulk_copy() {
    // Windows whose ring has wrapped, with the source of a match crossing
    // the ring's end for some distances, and outputs that already hold
    // bytes (`cur`) so matches also reach into the current call's output.
    for total in [65_536 - 100, 65_536 + 100] {
        let mut window = Window::new();
        window.append(&seeded_bytes(total));
        for cur in [0usize, 50, 300] {
            let prefix = seeded_bytes(cur + 7);
            for distance in 1..=300usize {
                for len in 3..=258usize {
                    let mut fast_out = vec![0u8; cur + 258];
                    let mut ref_out = vec![0u8; cur + 258];
                    fast_out[..cur].copy_from_slice(&prefix[..cur]);
                    ref_out[..cur].copy_from_slice(&prefix[..cur]);
                    let nf = window.copy_match(DecodePath::Fast, &mut fast_out, cur, distance, len);
                    let nr =
                        window.copy_match(DecodePath::Reference, &mut ref_out, cur, distance, len);
                    assert_eq!(nf, nr, "cur={cur} d={distance} len={len}");
                    assert_eq!(nf, len);
                    assert_eq!(fast_out, ref_out, "cur={cur} d={distance} len={len}");
                }
            }
        }
    }
}

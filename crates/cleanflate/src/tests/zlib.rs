use crate::error::{ChecksumKind, Error, HeaderError};
use crate::options::{Format, Options, TrailingData};
use crate::stream::{decompress_with, zlib_decompress, Decompressor, Status};
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal, emit_match};

/// CMF for CM=8 (deflate), CINFO=7 (32 KiB window): 0x78.
fn cmf_default() -> u8 {
    0x78
}

/// Find an FLG byte (FLEVEL=0, given FDICT) making `(cmf*256+flg) % 31 == 0`
/// (RFC 1950 S2.2).
fn flg_for(cmf: u8, fdict: bool) -> u8 {
    for fcheck in 0u8..32 {
        let flg = fcheck | ((fdict as u8) << 5);
        if (cmf as u32 * 256 + flg as u32) % 31 == 0 {
            return flg;
        }
    }
    panic!("no valid FCHECK found");
}

/// C16: never call this crate's own this crate's own checksum module's adler32 in a test
/// fixture -- that would let a real Adler-32 bug hide (a broken encoder
/// would satisfy a broken decoder's expectation). Every value here was
/// computed once with `oracle::ffi::adler32` over the exact bytes named.
fn known_adler32(data: &[u8]) -> u32 {
    match data {
        b"hello" => 0x062C_0215,
        b"hi" => 0x013B_00D2,
        b"a" => 0x0062_0062,
        b"the quick brown fox" => 0x478E_0734,
        b"a completely different dictionary" => 0xD693_0CED,
        b"the quick brown fox jumps" => 0x7A06_0983,
        b"jumps" => 0x0685_0230,
        b"abc" => 0x024D_0127,
        b"abcdefgh" => 0x0E00_0325,
        other => panic!(
            "known_adler32: no precomputed value for {other:?} -- add one \
             (compute with oracle::ffi::adler32 once, never this crate's own checksum module)"
        ),
    }
}

/// Wrap a raw DEFLATE `body` (already a valid RFC 1951 stream decoding to
/// `data`) in a zlib header/trailer (RFC 1950 S2.2), optionally with a
/// preset dictionary.
fn zlib_wrap(body: &[u8], data: &[u8], dict: Option<&[u8]>) -> Vec<u8> {
    let cmf = cmf_default();
    let flg = flg_for(cmf, dict.is_some());
    let mut out = vec![cmf, flg];
    if let Some(d) = dict {
        out.extend_from_slice(&known_adler32(d).to_be_bytes());
    }
    out.extend_from_slice(body);
    out.extend_from_slice(&known_adler32(data).to_be_bytes());
    out
}

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

#[test]
fn zlib_minimal_empty() {
    let bytes = [0x78u8, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01];
    let out = zlib_decompress(&bytes).expect("no error");
    assert_eq!(out, Vec::<u8>::new());
}

#[test]
fn zlib_stored_hello() {
    let body = stored_body(b"hello");
    let bytes = zlib_wrap(&body, b"hello", None);
    let out = zlib_decompress(&bytes).expect("no error");
    assert_eq!(out, b"hello");
}

#[test]
fn zlib_bad_fcheck() {
    let mut bytes = zlib_wrap(&stored_body(b"hi"), b"hi", None);
    bytes[1] ^= 0x01; // flip a FCHECK bit, breaking the mod-31 check
    let err = zlib_decompress(&bytes).expect_err("bad fcheck");
    assert_eq!(err, Error::BadHeader(HeaderError::ZlibFcheck));
}

#[test]
fn zlib_cm_7_rejected() {
    // CMF with CM=7 (not 8) in the low nibble, CINFO=7: 0x77. Then find
    // a matching FCHECK.
    let cmf = 0x77u8;
    let flg = flg_for(cmf, false);
    let bytes = vec![cmf, flg];
    let err = zlib_decompress(&bytes).expect_err("cm != 8");
    assert_eq!(err, Error::BadHeader(HeaderError::ZlibCompressionMethod(7)));
}

#[test]
fn zlib_cinfo_8_rejected() {
    // CINFO=8 (> 7) in the high nibble, CM=8: 0x88.
    let cmf = 0x88u8;
    let flg = flg_for(cmf, false);
    let bytes = vec![cmf, flg];
    let err = zlib_decompress(&bytes).expect_err("cinfo > 7");
    assert_eq!(err, Error::BadHeader(HeaderError::ZlibWindowSize(8)));
}

#[test]
fn zlib_fdict_without_dictionary() {
    let dict = b"the quick brown fox";
    let body = fixed_literals_body(b"a");
    let bytes = zlib_wrap(&body, b"a", Some(dict));
    let err = zlib_decompress(&bytes).expect_err("dictionary required");
    assert_eq!(
        err,
        Error::PresetDictionaryRequired {
            dict_id: known_adler32(dict)
        }
    );
}

#[test]
fn zlib_fdict_wrong_dictionary() {
    let dict = b"the quick brown fox";
    let wrong_dict = b"a completely different dictionary";
    let body = fixed_literals_body(b"a");
    let bytes = zlib_wrap(&body, b"a", Some(dict));

    let options = Options {
        dictionary: Some(wrong_dict.to_vec()),
        ..Options::default()
    };
    let err = decompress_with(Format::Zlib, &bytes, &options).expect_err("wrong dictionary");
    assert_eq!(
        err,
        Error::DictionaryMismatch {
            expected: known_adler32(dict),
            actual: known_adler32(wrong_dict),
        }
    );
}

#[test]
fn zlib_fdict_with_dictionary_copies_from_dictionary() {
    let dict = b"the quick brown fox jumps";
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_match(&mut w, 5, 5); // copy "brown" (last 5 bytes of dict, i.e. distance 5 = "fox j"? just verify by decode)
    emit_end_of_block(&mut w);
    let body = w.finish();

    // The expected output is whatever the last 5 bytes of the dictionary
    // are, copied via distance 5 -- compute it the same way the decoder
    // will, from the dictionary's own tail.
    let expected: Vec<u8> = dict[dict.len() - 5..].to_vec();

    let bytes = zlib_wrap(&body, &expected, Some(dict));
    let options = Options {
        dictionary: Some(dict.to_vec()),
        ..Options::default()
    };
    let decoded = decompress_with(Format::Zlib, &bytes, &options).expect("no error");
    assert_eq!(decoded.data, expected);
}

#[test]
fn raw_with_dictionary_prefill() {
    let dict = b"abcdefgh";
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_match(&mut w, 3, 3); // copy 3 bytes from distance 3 back in dict
    emit_end_of_block(&mut w);
    let bytes = w.finish();

    let expected: Vec<u8> = dict[dict.len() - 3..].to_vec();

    let options = Options {
        dictionary: Some(dict.to_vec()),
        ..Options::default()
    };
    let decoded = decompress_with(Format::Raw, &bytes, &options).expect("no error");
    assert_eq!(decoded.data, expected);
}

#[test]
fn zlib_adler_mismatch() {
    let body = stored_body(b"hello");
    let mut bytes = zlib_wrap(&body, b"hello", None);
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF; // corrupt the trailer
    let err = zlib_decompress(&bytes).expect_err("adler mismatch");
    assert!(matches!(
        err,
        Error::ChecksumMismatch {
            kind: ChecksumKind::Adler32,
            ..
        }
    ));
}

#[test]
fn zlib_trailing_reject() {
    let body = stored_body(b"hi");
    let mut bytes = zlib_wrap(&body, b"hi", None);
    bytes.push(b'X');
    let err = zlib_decompress(&bytes).expect_err("trailing data rejected by default");
    assert!(matches!(err, Error::TrailingData { .. }));
}

#[test]
fn zlib_trailing_allow_offset() {
    let body = stored_body(b"hi");
    let mut bytes = zlib_wrap(&body, b"hi", None);
    let stream_len = bytes.len();
    bytes.extend_from_slice(b"JUNK");

    let options = Options {
        trailing: TrailingData::Allow,
        ..Options::default()
    };
    let mut d = Decompressor::new(Format::Zlib, options);
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
fn zlib_truncated_in_trailer() {
    let body = stored_body(b"hi");
    let bytes = zlib_wrap(&body, b"hi", None);
    // Cut off 2 of the 4 trailer bytes.
    let truncated = bytes.get(..bytes.len() - 2).unwrap_or(&[]);
    let err = zlib_decompress(truncated).expect_err("truncated trailer");
    assert_eq!(err, Error::Truncated);
}

#[test]
fn zlib_one_byte_chunks_same_output() {
    let body = fixed_literals_body(b"abc");
    let bytes = zlib_wrap(&body, b"abc", None);

    let mut d = Decompressor::new(Format::Zlib, Options::default());
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
        if progress.status == Status::NeedsInput && progress.consumed == 0 && one_in.is_empty() {
            panic!("ran out of input before Done");
        }
    }
    d.finish().expect("finished cleanly");
    assert_eq!(out, b"abc");
}

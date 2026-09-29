use crate::checksum::crc32;
use crate::error::{ChecksumKind, Error, HeaderError};
use crate::gzip::GzipHeaderParser;

const FTEXT: u8 = 0x01;
const FHCRC: u8 = 0x02;
const FEXTRA: u8 = 0x04;
const FNAME: u8 = 0x08;
const FCOMMENT: u8 = 0x10;

#[allow(clippy::too_many_arguments)]
fn build_header_bytes(
    flg: u8,
    mtime: u32,
    xfl: u8,
    os: u8,
    extra: Option<&[u8]>,
    name: Option<&[u8]>,
    comment: Option<&[u8]>,
    hcrc_override: Option<u16>,
) -> Vec<u8> {
    let mut bytes = vec![0x1fu8, 0x8b, 8, flg];
    bytes.extend_from_slice(&mtime.to_le_bytes());
    bytes.push(xfl);
    bytes.push(os);
    if flg & FEXTRA != 0 {
        let e = extra.expect("extra required when FEXTRA set");
        bytes.extend_from_slice(&(e.len() as u16).to_le_bytes());
        bytes.extend_from_slice(e);
    }
    if flg & FNAME != 0 {
        let n = name.expect("name required when FNAME set");
        bytes.extend_from_slice(n);
        bytes.push(0);
    }
    if flg & FCOMMENT != 0 {
        let c = comment.expect("comment required when FCOMMENT set");
        bytes.extend_from_slice(c);
        bytes.push(0);
    }
    if flg & FHCRC != 0 {
        let hcrc = match hcrc_override {
            Some(v) => v,
            None => (crc32(&bytes) & 0xFFFF) as u16,
        };
        bytes.extend_from_slice(&hcrc.to_le_bytes());
    }
    bytes
}

#[test]
fn minimal_header() {
    let bytes = build_header_bytes(0, 0, 0, 255, None, None, None, None);
    let mut parser = GzipHeaderParser::new(65536);
    let (consumed, header) = parser.feed(&bytes).expect("no error");
    assert_eq!(consumed, bytes.len());
    let h = header.expect("header complete");
    assert!(!h.text);
    assert_eq!(h.mtime, 0);
    assert_eq!(h.xfl, 0);
    assert_eq!(h.os, 255);
    assert_eq!(h.extra, None);
    assert_eq!(h.name, None);
    assert_eq!(h.comment, None);
    assert!(!h.header_crc);
    assert!(!h.truncated_fields);
}

#[test]
fn all_flags_header() {
    let flg = FTEXT | FHCRC | FEXTRA | FNAME | FCOMMENT;
    let bytes = build_header_bytes(
        flg,
        123456,
        2,
        3,
        Some(b"EXTRA-DATA"),
        Some(b"file.txt"),
        Some(b"a comment"),
        None,
    );
    let mut parser = GzipHeaderParser::new(65536);
    let (consumed, header) = parser.feed(&bytes).expect("no error");
    assert_eq!(consumed, bytes.len());
    let h = header.expect("header complete");
    assert!(h.text);
    assert_eq!(h.mtime, 123456);
    assert_eq!(h.xfl, 2);
    assert_eq!(h.os, 3);
    assert_eq!(h.extra, Some(b"EXTRA-DATA".to_vec()));
    assert_eq!(h.name, Some(b"file.txt".to_vec()));
    assert_eq!(h.comment, Some(b"a comment".to_vec()));
    assert!(h.header_crc);
    assert!(!h.truncated_fields);
}

#[test]
fn fextra_parsed() {
    let bytes = build_header_bytes(FEXTRA, 0, 0, 0, Some(b"subfield-bytes"), None, None, None);
    let mut parser = GzipHeaderParser::new(65536);
    let (_consumed, header) = parser.feed(&bytes).expect("no error");
    let h = header.expect("header complete");
    assert_eq!(h.extra, Some(b"subfield-bytes".to_vec()));
    assert_eq!(h.name, None);
    assert_eq!(h.comment, None);
}

#[test]
fn fname_and_comment_parsed() {
    let flg = FNAME | FCOMMENT;
    let bytes = build_header_bytes(
        flg,
        0,
        0,
        0,
        None,
        Some(b"myfile.dat"),
        Some(b"hello"),
        None,
    );
    let mut parser = GzipHeaderParser::new(65536);
    let (_consumed, header) = parser.feed(&bytes).expect("no error");
    let h = header.expect("header complete");
    assert_eq!(h.name, Some(b"myfile.dat".to_vec()));
    assert_eq!(h.comment, Some(b"hello".to_vec()));
}

#[test]
fn fhcrc_ok() {
    let bytes = build_header_bytes(FHCRC, 0, 0, 0, None, None, None, None);
    let mut parser = GzipHeaderParser::new(65536);
    let (_consumed, header) = parser.feed(&bytes).expect("no error");
    assert!(header.expect("header complete").header_crc);
}

#[test]
fn fhcrc_mismatch() {
    let bytes = build_header_bytes(FHCRC, 0, 0, 0, None, None, None, Some(0xFFFF));
    let mut parser = GzipHeaderParser::new(65536);
    let err = parser.feed(&bytes).expect_err("crc mismatch");
    assert!(matches!(
        err,
        Error::ChecksumMismatch {
            kind: ChecksumKind::GzipHeaderCrc,
            ..
        }
    ));
}

#[test]
fn bad_magic() {
    let mut bytes = build_header_bytes(0, 0, 0, 0, None, None, None, None);
    bytes[0] = 0x00;
    let mut parser = GzipHeaderParser::new(65536);
    let err = parser.feed(&bytes).expect_err("bad magic");
    assert_eq!(err, Error::BadHeader(HeaderError::GzipMagic));
}

#[test]
fn bad_cm() {
    let mut bytes = build_header_bytes(0, 0, 0, 0, None, None, None, None);
    bytes[2] = 7;
    let mut parser = GzipHeaderParser::new(65536);
    let err = parser.feed(&bytes).expect_err("bad cm");
    assert_eq!(err, Error::BadHeader(HeaderError::GzipCompressionMethod(7)));
}

#[test]
fn reserved_flag_bits_rejected() {
    let bytes = build_header_bytes(0x20, 0, 0, 0, None, None, None, None);
    let mut parser = GzipHeaderParser::new(65536);
    let err = parser.feed(&bytes).expect_err("reserved flag bit set");
    assert_eq!(err, Error::BadHeader(HeaderError::GzipReservedFlags(0x20)));
}

#[test]
fn field_longer_than_limit_truncated_not_rejected() {
    let long_name = b"this-name-is-longer-than-the-limit";
    let bytes = build_header_bytes(FNAME, 0, 0, 0, None, Some(long_name), None, None);
    let mut parser = GzipHeaderParser::new(4);
    let (_consumed, header) = parser.feed(&bytes).expect("no error, just truncated");
    let h = header.expect("header complete");
    assert_eq!(h.name, Some(long_name[..4].to_vec()));
    assert!(h.truncated_fields);
}

#[test]
fn byte_by_byte_equals_all_at_once() {
    let flg = FTEXT | FHCRC | FEXTRA | FNAME | FCOMMENT;
    let bytes = build_header_bytes(
        flg,
        99,
        4,
        3,
        Some(b"xtra"),
        Some(b"n.txt"),
        Some(b"cmt"),
        None,
    );

    let mut whole = GzipHeaderParser::new(65536);
    let (_c, whole_header) = whole.feed(&bytes).expect("no error");

    let mut byte_by_byte = GzipHeaderParser::new(65536);
    let mut result = None;
    let mut pos = 0usize;
    while pos < bytes.len() {
        let chunk = bytes.get(pos..pos + 1).unwrap_or(&[]);
        let (consumed, header) = byte_by_byte.feed(chunk).expect("no error");
        pos += consumed;
        if let Some(h) = header {
            result = Some(h);
            break;
        }
    }

    assert_eq!(result, whole_header);
}

#[test]
fn truncated_header_needs_more() {
    let bytes = build_header_bytes(0, 0, 0, 0, None, None, None, None);
    let partial = bytes.get(..3).unwrap_or(&[]);
    let mut parser = GzipHeaderParser::new(65536);
    let (consumed, header) = parser.feed(partial).expect("no error, just incomplete");
    assert_eq!(consumed, partial.len());
    assert_eq!(header, None);
}

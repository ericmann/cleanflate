use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::options::{Format, Options, TrailingData};
use crate::stream::{Decompressor, Status};

fn stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01u8];
    out.extend_from_slice(&(data.len() as u16).to_le_bytes());
    out.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// One gzip member holding "hi"; C16: the CRC-32 is a literal computed once
/// with `oracle::ffi::crc32`.
fn gzip_hi() -> Vec<u8> {
    let mut v = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03];
    v.extend_from_slice(&stored(b"hi"));
    v.extend_from_slice(&0xD893_2AACu32.to_le_bytes());
    v.extend_from_slice(&2u32.to_le_bytes());
    v
}

fn decode_all(d: &mut Decompressor, input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; 64];
    let p = d.decompress(input, &mut out)?;
    // A gzip stream stays at a member boundary until `finish` resolves it.
    assert!(matches!(p.status, Status::Done | Status::NeedsInput));
    out.truncate(p.produced);
    d.finish()?;
    Ok(out)
}

#[test]
fn reset_preserves_path() {
    for format in [Format::Raw, Format::Zlib, Format::Gzip] {
        let mut d = Decompressor::new_with_path(format, Options::default(), DecodePath::Reference);
        assert_eq!(d.inflater_path(), DecodePath::Reference);
        d.reset();
        assert_eq!(d.inflater_path(), DecodePath::Reference, "{format:?}");
        let mut f = Decompressor::new_with_path(format, Options::default(), DecodePath::Fast);
        f.reset();
        assert_eq!(f.inflater_path(), DecodePath::Fast, "{format:?}");
    }
}

#[test]
fn reset_clears_sticky_error() {
    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = [0u8; 16];
    let first = d
        .decompress(&[0x07], &mut out)
        .expect_err("BTYPE 3 is reserved");
    assert_eq!(first, Error::BadBlockType);
    assert_eq!(d.decompress(&[0x01], &mut out).expect_err("sticky"), first);
    assert_eq!(d.finish().expect_err("sticky"), first);

    d.reset();
    assert_eq!(
        decode_all(&mut d, &stored(b"abc")).expect("decodes"),
        b"abc"
    );
    assert_eq!((d.total_in(), d.total_out()), (8, 3));
}

#[test]
fn reset_clears_trailing_and_headers() {
    let options = Options {
        trailing: TrailingData::Allow,
        ..Options::default()
    };
    let mut d = Decompressor::new(Format::Gzip, options);
    let mut input = gzip_hi();
    let member_len = input.len();
    input.extend_from_slice(b"XYZ");
    let mut out = [0u8; 16];
    let p = d.decompress(&input, &mut out).expect("first member");
    assert_eq!(&out[..p.produced], b"hi");
    // The boundary decision is resolved by the next call or finish.
    d.decompress(&input[p.consumed..], &mut out)
        .expect("trailing allowed");
    d.finish().expect("finishes");
    assert_eq!(d.trailing_start(), Some(member_len as u64));
    assert_eq!(d.gzip_headers().len(), 1);
    assert_eq!(d.gzip_member_count(), 1);
    assert!(d.total_in() > 0 && d.total_out() == 2);

    d.reset();
    assert_eq!(d.trailing_start(), None);
    assert!(d.gzip_headers().is_empty());
    assert_eq!(d.gzip_member_count(), 0);
    assert_eq!((d.total_in(), d.total_out()), (0, 0));
    // And it decodes a fresh stream like a new decompressor.
    let mut fresh = Decompressor::new(
        Format::Gzip,
        Options {
            trailing: TrailingData::Allow,
            ..Options::default()
        },
    );
    let a = decode_all(&mut d, &gzip_hi()).expect("after reset");
    let b = decode_all(&mut fresh, &gzip_hi()).expect("fresh");
    assert_eq!(a, b);
    assert_eq!(d.gzip_member_count(), fresh.gzip_member_count());
}

#[cfg(feature = "stats")]
#[test]
fn reset_stats_zeroed() {
    for (format, input) in [(Format::Raw, stored(b"abc")), (Format::Gzip, gzip_hi())] {
        let mut d = Decompressor::new(format, Options::default());
        decode_all(&mut d, &input).expect("decodes");
        assert!(d.stats().output_bytes > 0, "{format:?}");
        d.reset();
        assert_eq!(
            d.stats(),
            Decompressor::new(format, Options::default()).stats()
        );
    }
}

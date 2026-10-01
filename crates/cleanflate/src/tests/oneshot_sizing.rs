use crate::config::{
    MAX_DEFLATE_EXPANSION, ONESHOT_GROWTH_FLOOR, ONESHOT_INITIAL_RATIO, ONESHOT_MAX_PREALLOC,
};
use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::options::{Format, Options, TrailingData};
use crate::stream::{
    decompress_core, decompress_core_vec, decompress_with, initial_len, next_len, Decoded,
};

#[test]
fn initial_len_gzip_uses_isize() {
    let mut input = vec![0u8; 100];
    let n = input.len();
    input[n - 4..].copy_from_slice(&1000u32.to_le_bytes());
    assert_eq!(initial_len(Format::Gzip, &input, u64::MAX), 1000);
    // Exactly 18 bytes is the smallest gzip member that has an ISIZE.
    let mut eighteen = vec![0u8; 18];
    eighteen[14..].copy_from_slice(&77u32.to_le_bytes());
    assert_eq!(initial_len(Format::Gzip, &eighteen, u64::MAX), 77);
    // Only gzip reads an ISIZE: the same bytes as raw use the ratio.
    assert_eq!(
        initial_len(Format::Raw, &input, u64::MAX),
        100 * ONESHOT_INITIAL_RATIO
    );
}

#[test]
fn initial_len_capped_by_limit_and_prealloc() {
    // 1 MiB of input could decode to ~1 GiB, so the expansion bound does
    // not bind here and the preallocation cap does.
    let mut input = vec![0u8; 1 << 20];
    let n = input.len();
    input[n - 4..].copy_from_slice(&(200u32 << 20).to_le_bytes());
    // A 200 MiB ISIZE is capped by the preallocation cap...
    assert_eq!(
        initial_len(Format::Gzip, &input, u64::MAX),
        ONESHOT_MAX_PREALLOC
    );
    // ...and by a smaller output limit.
    assert_eq!(initial_len(Format::Gzip, &input, 500), 500);
    assert_eq!(initial_len(Format::Gzip, &input, 0), 0);
    // The ratio hint is capped the same way.
    let big = vec![0u8; 1 << 20];
    assert_eq!(initial_len(Format::Zlib, &big, 1000), 1000);
    assert!(initial_len(Format::Zlib, &big, u64::MAX) <= ONESHOT_MAX_PREALLOC);
}

#[test]
fn initial_len_raw_zlib_ratio() {
    for format in [Format::Raw, Format::Zlib] {
        for len in [1usize, 10, 1000, 4096] {
            let input = vec![0u8; len];
            assert_eq!(
                initial_len(format, &input, u64::MAX),
                len * ONESHOT_INITIAL_RATIO,
                "{format:?} {len}"
            );
        }
    }
}

#[test]
fn initial_len_tiny_or_hostile_inputs() {
    for format in [Format::Raw, Format::Zlib, Format::Gzip] {
        assert_eq!(initial_len(format, &[], u64::MAX), 0);
        // Fewer than 18 bytes: no gzip ISIZE, ratio hint.
        let short = [0xFFu8; 17];
        assert_eq!(
            initial_len(format, &short, u64::MAX),
            17 * ONESHOT_INITIAL_RATIO
        );
    }
    // A hostile ISIZE of u32::MAX cannot force a large allocation: the
    // hint is bounded by what 40 input bytes could ever decode to.
    let mut hostile = vec![0u8; 40];
    let n = hostile.len();
    hostile[n - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
    let got = initial_len(Format::Gzip, &hostile, u64::MAX);
    assert_eq!(got, 40 * MAX_DEFLATE_EXPANSION);
    assert_eq!(initial_len(Format::Gzip, &hostile, 10), 10);
    // And such an input still fails cleanly rather than allocating.
    assert!(decompress_with(Format::Gzip, &hostile, &Options::default()).is_err());
}

#[test]
fn initial_len_bounded_by_deflate_expansion() {
    // The smallest input that carries an ISIZE (18 bytes) claiming 4 GiB
    // gets at most 18 x 1032 bytes up front, not ONESHOT_MAX_PREALLOC.
    let mut eighteen = vec![0u8; 18];
    eighteen[14..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(MAX_DEFLATE_EXPANSION, 1032);
    assert_eq!(initial_len(Format::Gzip, &eighteen, u64::MAX), 18 * 1032);
    assert_ne!(
        initial_len(Format::Gzip, &eighteen, u64::MAX),
        ONESHOT_MAX_PREALLOC
    );
    // An honest ISIZE below the bound is used as-is.
    eighteen[14..].copy_from_slice(&500u32.to_le_bytes());
    assert_eq!(initial_len(Format::Gzip, &eighteen, u64::MAX), 500);
    // The bound is inclusive: a claim of exactly 18 x 1032 is kept.
    eighteen[14..].copy_from_slice(&(18u32 * 1032).to_le_bytes());
    assert_eq!(initial_len(Format::Gzip, &eighteen, u64::MAX), 18 * 1032);
}

#[test]
fn next_len_uses_config_floor() {
    assert_eq!(next_len(0, u64::MAX), ONESHOT_GROWTH_FLOOR);
    assert_eq!(next_len(1, u64::MAX), 1 + ONESHOT_GROWTH_FLOOR);
    assert_eq!(
        next_len(ONESHOT_GROWTH_FLOOR, u64::MAX),
        2 * ONESHOT_GROWTH_FLOOR
    );
    assert_eq!(
        next_len(10 * ONESHOT_GROWTH_FLOOR, u64::MAX),
        20 * ONESHOT_GROWTH_FLOOR
    );
    // Never past the limit, never below the current length.
    assert_eq!(next_len(0, 100), 100);
    assert_eq!(next_len(500, 100), 500);
}

/// A gzip member holding one stored block of `len` copies of `byte`, with
/// the CRC-32 given as a literal (fixtures never take checksums from
/// cleanflate, C16).
fn member(byte: u8, len: usize, crc: u32) -> Vec<u8> {
    let mut v = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03];
    v.push(1); // BFINAL, stored
    v.extend_from_slice(&(len as u16).to_le_bytes());
    v.extend_from_slice(&(!(len as u16)).to_le_bytes());
    v.extend(std::iter::repeat_n(byte, len));
    v.extend_from_slice(&crc.to_le_bytes());
    v.extend_from_slice(&(len as u32).to_le_bytes());
    v
}

#[test]
fn gzip_multi_member_hint_too_small_still_correct() {
    // CRC-32 of 40000 x 'a' and of 5000 x 'b' (computed independently).
    let mut input = member(b'a', 40_000, 0xf3dd_b8f7);
    input.extend_from_slice(&member(b'b', 5_000, 0xd091_08fc));
    // The hint is the last member's ISIZE (5000), far below the output.
    assert_eq!(initial_len(Format::Gzip, &input, u64::MAX), 5_000);
    let decoded = decompress_with(Format::Gzip, &input, &Options::default()).expect("decodes");
    assert_eq!(decoded.data.len(), 45_000);
    assert!(decoded.data[..40_000].iter().all(|&b| b == b'a'));
    assert!(decoded.data[40_000..].iter().all(|&b| b == b'b'));
    assert_eq!(decoded.consumed, input.len());

    // A small output limit is still enforced, whatever the hint says.
    let limited = Options {
        max_output: 100,
        ..Options::default()
    };
    assert_eq!(
        decompress_with(Format::Gzip, &input, &limited),
        Err(Error::OutputLimitExceeded { limit: 100 })
    );
}

fn stored_raw(data: &[u8]) -> Vec<u8> {
    let mut v = vec![0x01u8];
    v.extend_from_slice(&(data.len() as u16).to_le_bytes());
    v.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
    v.extend_from_slice(data);
    v
}

fn both(
    format: Format,
    input: &[u8],
    options: &Options,
    path: DecodePath,
) -> (Result<Decoded, Error>, Result<Decoded, Error>) {
    (
        decompress_core(format, input, options, path).map(|c| c.decoded),
        decompress_core_vec(format, input, options, path).map(|c| c.decoded),
    )
}

#[test]
fn oneshot_vec_and_slice_drivers_agree() {
    let mut fixed = crate::tests::bitwriter::BitWriter::new();
    crate::tests::inflate_fixed::emit_block_header(&mut fixed, true, 0b01);
    for &b in b"abcab" {
        crate::tests::inflate_fixed::emit_literal(&mut fixed, b as u16);
    }
    crate::tests::inflate_fixed::emit_match(&mut fixed, 30, 3);
    crate::tests::inflate_fixed::emit_end_of_block(&mut fixed);
    let fixed = fixed.finish();

    let mut two = member(b'a', 40_000, 0xf3dd_b8f7);
    two.extend_from_slice(&member(b'b', 5_000, 0xd091_08fc));
    let cases = [
        (Format::Raw, stored_raw(&[7u8; 300])),
        (Format::Raw, fixed),
        (Format::Gzip, two),
        (Format::Raw, vec![0x07u8]),
        (Format::Raw, stored_raw(&[1u8; 50])[..20].to_vec()),
    ];
    for (format, input) in &cases {
        for path in [DecodePath::Fast, DecodePath::Reference] {
            let (slice, vec) = both(*format, input, &Options::default(), path);
            assert_eq!(slice, vec, "{format:?} {path:?}");
        }
    }
}

#[test]
fn oneshot_limit_semantics_unchanged() {
    let data = vec![0x5Au8; 1000];
    let input = stored_raw(&data);
    let exact = Options {
        max_output: 1000,
        ..Options::default()
    };
    let under = Options {
        max_output: 999,
        ..Options::default()
    };
    assert_eq!(
        decompress_with(Format::Raw, &input, &exact)
            .expect("exact")
            .data,
        data
    );
    assert_eq!(
        decompress_with(Format::Raw, &input, &under),
        Err(Error::OutputLimitExceeded { limit: 999 })
    );
    assert_eq!(
        decompress_with(Format::Raw, b"", &exact),
        Err(Error::Truncated)
    );

    // Trailing data after a gzip member: the error offset and the allowed
    // consumed count are both the member's end, for a long and a lone byte.
    let mut gz = super_member();
    let stream_len = gz.len();
    for trailer in [&b"XYZ"[..], &[0x1f][..], &[0x1f, b'Z'][..]] {
        gz.truncate(stream_len);
        gz.extend_from_slice(trailer);
        assert_eq!(
            decompress_with(Format::Gzip, &gz, &Options::default()),
            Err(Error::TrailingData {
                offset: stream_len as u64
            }),
            "trailer {trailer:?}"
        );
        let allow = Options {
            trailing: TrailingData::Allow,
            ..Options::default()
        };
        let decoded = decompress_with(Format::Gzip, &gz, &allow).expect("allowed");
        assert_eq!(decoded.data, b"hi");
        assert_eq!(decoded.consumed, stream_len, "trailer {trailer:?}");
        // The slice driver reports exactly the same.
        let (slice, vec) = both(Format::Gzip, &gz, &allow, DecodePath::Fast);
        assert_eq!(slice, vec);
    }
}

/// One gzip member holding "hi" (CRC-32 0xD8932AAC, a literal computed
/// with `oracle::ffi::crc32`, C16).
fn super_member() -> Vec<u8> {
    let mut v = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03];
    v.extend_from_slice(&stored_raw(b"hi"));
    v.extend_from_slice(&0xD893_2AACu32.to_le_bytes());
    v.extend_from_slice(&2u32.to_le_bytes());
    v
}

#[test]
fn oneshot_capacity_bounded_by_limit() {
    let data = vec![0x33u8; 5000];
    let input = stored_raw(&data);
    for limit in [5000u64, 5001, 6000, 70_000, 1 << 20] {
        let options = Options {
            max_output: limit,
            ..Options::default()
        };
        let decoded = decompress_with(Format::Raw, &input, &options).expect("decodes");
        assert_eq!(decoded.data.len(), 5000);
        assert!(
            decoded.data.capacity() as u64 <= limit,
            "capacity {} past limit {limit}",
            decoded.data.capacity()
        );
    }
}

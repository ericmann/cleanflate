//! M3 wrapper-specific tests (SPEC S5 M3): gzip multi-member streams and
//! zlib preset-dictionary error paths, built with the C zlib oracle so
//! they're independent of cleanflate's own encoder-less test fixtures.

use cleanflate::{decompress_with, gzip_decompress, zlib_decompress, Error, Format, Options};
use oracle::ffi::{adler32, deflate_bytes, DeflateParams, Framing, Strategy};
use oracle::prng::SplitMix64;

fn strategy_for(n: u64) -> Strategy {
    match n % 5 {
        0 => Strategy::Default,
        1 => Strategy::Filtered,
        2 => Strategy::HuffmanOnly,
        3 => Strategy::Rle,
        _ => Strategy::Fixed,
    }
}

/// 200 cases concatenating 2-5 oracle-built gzip members of random
/// inputs (varied levels/strategies, including empty members). The
/// concatenation must decode to the concatenated inputs, and
/// `gzip_headers().len()` must equal the member count.
#[test]
fn gzip_multi_member_oracle() {
    for case in 0u64..200 {
        let mut rng = SplitMix64::new(0x6f72_6163_6c65_5f77 ^ case);
        let member_count = 2 + (rng.next_u64() % 4) as usize; // 2..=5

        let mut concatenated_input = Vec::new();
        let mut concatenated_stream = Vec::new();

        for _ in 0..member_count {
            let len = if rng.next_u64() % 7 == 0 {
                0
            } else {
                (rng.next_u64() % 4096) as usize
            };
            let input = rng.bytes(len);
            let level = (rng.next_u64() % 10) as u8;
            let strategy = strategy_for(rng.next_u64());
            let params = DeflateParams {
                level,
                strategy,
                window_bits: 15,
                mem_level: 8,
                framing: Framing::Gzip,
            };
            let member = deflate_bytes(&input, &params, None).unwrap_or_else(|e| {
                panic!("case {case}: deflate failed (level={level}, strategy={strategy:?}): {e:?}")
            });
            concatenated_input.extend_from_slice(&input);
            concatenated_stream.extend_from_slice(&member);
        }

        let decoded = decompress_with(Format::Gzip, &concatenated_stream, &Options::default())
            .unwrap_or_else(|e| panic!("case {case}: decode failed: {e:?}"));

        assert_eq!(
            decoded.data, concatenated_input,
            "case {case}: decoded output does not match concatenated inputs"
        );
        assert_eq!(
            decoded.gzip_members.len(),
            member_count,
            "case {case}: gzip_headers().len() should equal the member count"
        );

        // gzip_decompress() (the convenience one-shot) must agree too.
        let via_convenience = gzip_decompress(&concatenated_stream)
            .unwrap_or_else(|e| panic!("case {case}: gzip_decompress failed: {e:?}"));
        assert_eq!(
            via_convenience, concatenated_input,
            "case {case}: gzip_decompress mismatch"
        );
    }
}

/// RFC 1950 S2.2: a zlib stream built with a preset dictionary, decoded
/// without one, must fail with `PresetDictionaryRequired { dict_id }`
/// where `dict_id` is the dictionary's Adler-32 (computed by C zlib).
#[test]
fn zlib_dictionary_required_on_oracle_stream() {
    let dictionary = SplitMix64::new(0xd1c7).bytes(4096);
    let input = SplitMix64::new(0x1a17).bytes(8192);
    let params = DeflateParams {
        level: 6,
        strategy: Strategy::Default,
        window_bits: 15,
        mem_level: 8,
        framing: Framing::Zlib,
    };
    let compressed =
        deflate_bytes(&input, &params, Some(&dictionary)).expect("oracle deflate with dictionary");

    let err = zlib_decompress(&compressed).expect_err("dictionary required but not supplied");
    match err {
        Error::PresetDictionaryRequired { dict_id } => {
            assert_eq!(dict_id, adler32(&dictionary));
        }
        other => panic!("expected PresetDictionaryRequired, got {other:?}"),
    }
}

/// A zlib stream built with one dictionary, decoded with a different
/// one, must fail with `DictionaryMismatch { expected, actual }` naming
/// both dictionaries' Adler-32 values.
#[test]
fn zlib_wrong_dictionary_mismatch() {
    let dictionary = SplitMix64::new(0xd1c7).bytes(4096);
    let wrong_dictionary = SplitMix64::new(0xbad0).bytes(4096);
    assert_ne!(dictionary, wrong_dictionary);

    let input = SplitMix64::new(0x1a17).bytes(8192);
    let params = DeflateParams {
        level: 6,
        strategy: Strategy::Default,
        window_bits: 15,
        mem_level: 8,
        framing: Framing::Zlib,
    };
    let compressed =
        deflate_bytes(&input, &params, Some(&dictionary)).expect("oracle deflate with dictionary");

    let options = Options {
        dictionary: Some(wrong_dictionary.clone()),
        ..Options::default()
    };
    let err = decompress_with(Format::Zlib, &compressed, &options)
        .expect_err("wrong dictionary must be rejected");
    match err {
        Error::DictionaryMismatch { expected, actual } => {
            assert_eq!(expected, adler32(&dictionary));
            assert_eq!(actual, adler32(&wrong_dictionary));
        }
        other => panic!("expected DictionaryMismatch, got {other:?}"),
    }
}

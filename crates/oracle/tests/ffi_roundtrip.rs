use oracle::ffi::{
    adler32, crc32, deflate_bytes, deflate_zeros, inflate_bytes, inflate_gzip_members,
    zlib_version, DeflateParams, Framing, OracleStatus, Strategy,
};

// Small local xorshift generator for pseudo-random test fixtures. Not the
// project's official PRNG (that arrives in P1-02) -- just enough entropy
// to exercise the oracle here.
fn xorshift_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn text_like(len: usize) -> Vec<u8> {
    let phrase = b"the quick brown fox jumps over the lazy dog. ";
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        out.extend_from_slice(phrase);
    }
    out.truncate(len);
    out
}

fn default_params(level: u8, framing: Framing) -> DeflateParams {
    DeflateParams {
        level,
        strategy: Strategy::Default,
        window_bits: 15,
        mem_level: 8,
        framing,
    }
}

#[test]
fn roundtrip_all_framings_and_levels() {
    let inputs: Vec<Vec<u8>> = vec![xorshift_bytes(0x1234_5678, 10 * 1024), text_like(10 * 1024)];
    for input in &inputs {
        for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
            for level in 0..=9u8 {
                let params = default_params(level, framing);
                let compressed =
                    deflate_bytes(input, &params, None).expect("deflate should succeed");
                let outcome = inflate_bytes(&compressed, framing, None, 1 << 20);
                assert_eq!(
                    outcome.status,
                    OracleStatus::StreamEnd,
                    "framing={framing:?} level={level}"
                );
                assert_eq!(outcome.output, *input, "framing={framing:?} level={level}");
            }
        }
    }
}

#[test]
fn roundtrip_all_strategies_windowbits_memlevels() {
    let input = text_like(20 * 1024);
    let strategies = [
        Strategy::Default,
        Strategy::Filtered,
        Strategy::HuffmanOnly,
        Strategy::Rle,
        Strategy::Fixed,
    ];
    let window_bits = [9u8, 12, 15];
    let mem_levels = [1u8, 5, 9];

    for &strategy in &strategies {
        for &wb in &window_bits {
            for &ml in &mem_levels {
                let params = DeflateParams {
                    level: 6,
                    strategy,
                    window_bits: wb,
                    mem_level: ml,
                    framing: Framing::Raw,
                };
                let compressed =
                    deflate_bytes(&input, &params, None).expect("deflate should succeed");
                let outcome = inflate_bytes(&compressed, Framing::Raw, None, 1 << 20);
                assert_eq!(
                    outcome.status,
                    OracleStatus::StreamEnd,
                    "strategy={strategy:?} wb={wb} ml={ml}"
                );
                assert_eq!(
                    outcome.output, input,
                    "strategy={strategy:?} wb={wb} ml={ml}"
                );
            }
        }
    }
}

#[test]
fn zlib_dictionary_roundtrip() {
    let dictionary = text_like(4096);
    let input = text_like(8192);
    let params = default_params(6, Framing::Zlib);

    let compressed =
        deflate_bytes(&input, &params, Some(&dictionary)).expect("deflate should succeed");

    let with_dict = inflate_bytes(&compressed, Framing::Zlib, Some(&dictionary), 1 << 20);
    assert_eq!(with_dict.status, OracleStatus::StreamEnd);
    assert_eq!(with_dict.output, input);

    let without_dict = inflate_bytes(&compressed, Framing::Zlib, None, 1 << 20);
    assert_eq!(without_dict.status, OracleStatus::NeedDict);
}

#[test]
fn inflate_garbage_is_error_not_panic() {
    let garbage = xorshift_bytes(0xDEAD_BEEF, 512);
    for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
        let outcome = inflate_bytes(&garbage, framing, None, 1 << 20);
        assert!(
            matches!(
                outcome.status,
                OracleStatus::DataError | OracleStatus::OtherError(_) | OracleStatus::Truncated
            ),
            "framing={framing:?} got {:?}",
            outcome.status
        );
    }
}

#[test]
fn crc32_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn adler32_known_value() {
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
}

#[test]
fn deflate_zeros_matches_deflate_bytes() {
    let params = default_params(6, Framing::Raw);
    let total: u64 = 1024 * 1024;
    let via_zeros = deflate_zeros(total, &params).expect("deflate_zeros should succeed");
    let zeros = vec![0u8; total as usize];
    let via_bytes = deflate_bytes(&zeros, &params, None).expect("deflate_bytes should succeed");
    assert_eq!(via_zeros, via_bytes);
}

#[test]
fn gzip_inflate_reports_first_member_consumed() {
    let params = default_params(6, Framing::Gzip);
    let member1 = deflate_bytes(b"hello", &params, None).expect("deflate should succeed");
    let member2 = deflate_bytes(b"world", &params, None).expect("deflate should succeed");

    let mut both = member1.clone();
    both.extend_from_slice(&member2);

    let outcome = inflate_bytes(&both, Framing::Gzip, None, 1 << 20);
    assert_eq!(outcome.status, OracleStatus::StreamEnd);
    assert_eq!(outcome.output, b"hello");
    assert_eq!(outcome.consumed, member1.len());
}

#[test]
fn gzip_multi_member_matches_concatenation() {
    let params = default_params(6, Framing::Gzip);
    let member1 = deflate_bytes(b"hello", &params, None).expect("deflate should succeed");
    let member2 = deflate_bytes(b"world", &params, None).expect("deflate should succeed");

    let mut both = member1.clone();
    both.extend_from_slice(&member2);

    let outcome = inflate_gzip_members(&both, 1 << 20);
    assert_eq!(outcome.status, OracleStatus::StreamEnd);
    assert_eq!(outcome.output, b"helloworld");
    assert_eq!(outcome.consumed, both.len());

    // Trailing bytes that don't start a new member: stop cleanly with
    // consumed covering only the two real members.
    let mut with_trailing = both.clone();
    with_trailing.extend_from_slice(b"XYZ");
    let outcome = inflate_gzip_members(&with_trailing, 1 << 20);
    assert_eq!(outcome.status, OracleStatus::StreamEnd);
    assert_eq!(outcome.output, b"helloworld");
    assert_eq!(outcome.consumed, both.len());

    // A second member with a corrupted CRC32 (RFC 1952 S2.3.1's trailer,
    // the first 4 of the 8 trailer bytes) reports that member's error
    // status; the first member's output is still returned.
    let mut corrupt = member1.clone();
    let mut bad_member2 = member2.clone();
    let crc_byte = bad_member2.len() - 8;
    if let Some(byte) = bad_member2.get_mut(crc_byte) {
        *byte ^= 0xFF;
    }
    corrupt.extend_from_slice(&bad_member2);
    let outcome = inflate_gzip_members(&corrupt, 1 << 20);
    assert_ne!(outcome.status, OracleStatus::StreamEnd);
    assert_eq!(outcome.output, b"hello");
    assert_eq!(outcome.consumed, member1.len());
}

#[test]
fn gzip_multi_member_1f_trailer_is_trailing() {
    // R2-04: a lone `0x1f` not followed by `0x8b` must not be mistaken
    // for the start of a new member -- covers `inflate_gzip_members`'s
    // `starts_new_member` check (both bytes required, not just the
    // first), so a regression that treats any `0x1f` as a new member
    // (which would reintroduce spurious fuzz target (c) gzip
    // divergences) fails this test.
    let params = default_params(6, Framing::Gzip);
    let member1 = deflate_bytes(b"hello", &params, None).expect("deflate should succeed");
    let member2 = deflate_bytes(b"world", &params, None).expect("deflate should succeed");

    let mut both = member1;
    both.extend_from_slice(&member2);

    let mut with_1f_trailer = both.clone();
    with_1f_trailer.extend_from_slice(&[0x1f, b'Z']);

    let outcome = inflate_gzip_members(&with_1f_trailer, 1 << 20);
    assert_eq!(outcome.status, OracleStatus::StreamEnd);
    assert_eq!(outcome.output, b"helloworld");
    assert_eq!(outcome.consumed, both.len());
}

#[test]
fn zlib_version_is_nonempty() {
    assert!(!zlib_version().is_empty());
}

fn mixed_data(len: usize) -> Vec<u8> {
    let mut v = text_like(len / 2);
    v.extend(xorshift_bytes(0xfeed_beef, len - len / 2));
    v
}

#[test]
fn compile_flags_stable() {
    let a = oracle::ffi::zlib_compile_flags();
    let b = oracle::ffi::zlib_compile_flags();
    assert_eq!(a, b);
    assert!(!format!("{a:#x}").is_empty());
}

#[test]
fn streaming_equals_oneshot_all_framings() {
    let input = mixed_data(256 * 1024);
    for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
        for &level in &[1u8, 6, 9] {
            let compressed = deflate_bytes(&input, &default_params(level, framing), None)
                .expect("deflate should succeed");
            let oneshot = inflate_bytes(&compressed, framing, None, 1 << 22);
            assert_eq!(oneshot.status, OracleStatus::StreamEnd);
            for &(ic, oc) in &[(1usize, 1usize), (7, 13), (65536, 65536)] {
                let mut out = vec![0u8; input.len()];
                let n = oracle::ffi::inflate_streaming_into(&compressed, framing, ic, &mut out, oc)
                    .unwrap_or_else(|e| panic!("{framing:?} l{level} ({ic},{oc}): {e:?}"));
                assert_eq!(n, input.len());
                assert_eq!(out, input);
                assert_eq!(out, oneshot.output);
            }
        }
    }
}

#[test]
fn streaming_truncated_is_error() {
    let input = mixed_data(4096);
    let c = deflate_bytes(&input, &default_params(6, Framing::Zlib), None).unwrap();
    let cut = &c[..c.len() / 2];
    let mut out = vec![0u8; 8192];
    let r = oracle::ffi::inflate_streaming_into(cut, Framing::Zlib, 64, &mut out, 64);
    assert_eq!(r, Err(OracleStatus::Truncated));
}

#[test]
fn streaming_output_too_small_is_error() {
    let input = mixed_data(4096);
    let c = deflate_bytes(&input, &default_params(6, Framing::Raw), None).unwrap();
    let mut out = vec![0u8; 100];
    let r = oracle::ffi::inflate_streaming_into(&c, Framing::Raw, 64, &mut out, 64);
    assert_eq!(r, Err(OracleStatus::OutputLimit));
}

#[test]
fn streaming_zero_chunk_is_error() {
    let c = deflate_bytes(b"abc", &default_params(6, Framing::Raw), None).unwrap();
    let mut out = vec![0u8; 16];
    assert!(oracle::ffi::inflate_streaming_into(&c, Framing::Raw, 0, &mut out, 4).is_err());
    assert!(oracle::ffi::inflate_streaming_into(&c, Framing::Raw, 4, &mut out, 0).is_err());
}

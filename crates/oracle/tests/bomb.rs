//! Output-limit hardening against decompression-bomb-shaped input (SPEC
//! S3.3 Limits, S5 M4). The default limit is never spelled out as a
//! literal here -- always `cleanflate::DEFAULT_MAX_OUTPUT` (C8) -- and
//! every fixture is all-zero input compressed by C zlib, so its ratio
//! is bounded by DEFLATE's own worst case (DESIGN.md D-21: about
//! 1032:1, far short of the SPEC's illustrative 1000:1-on-1-KiB
//! example, which is mathematically impossible for any real DEFLATE
//! stream).

use cleanflate::{decompress_with, Decompressor, Error, Format, Options, Status};
use oracle::ffi::{deflate_zeros, DeflateParams, Framing, Strategy};

fn to_cleanflate_format(framing: Framing) -> Format {
    match framing {
        Framing::Raw => Format::Raw,
        Framing::Zlib => Format::Zlib,
        Framing::Gzip => Format::Gzip,
    }
}

fn zeros_params(framing: Framing) -> DeflateParams {
    DeflateParams {
        level: 9,
        strategy: Strategy::Default,
        window_bits: 15,
        mem_level: 8,
        framing,
    }
}

/// A ~1 GiB-plus-a-bit all-zero raw DEFLATE stream, streamed through a
/// fixed 64 KiB output buffer, must fail fast at exactly
/// `DEFAULT_MAX_OUTPUT` without ever needing to hold the whole (would-be
/// multi-gigabyte) decompressed output in memory. Ignored by default
/// (heavy: `make bomb`) since it still has to produce a full
/// `DEFAULT_MAX_OUTPUT` bytes of real decoded output before it can even
/// observe the limit.
#[test]
#[ignore]
fn bomb_streaming_default_limit() {
    let total = cleanflate::DEFAULT_MAX_OUTPUT + (1u64 << 20);
    let params = zeros_params(Framing::Raw);
    let compressed = deflate_zeros(total, &params).expect("compress the bomb fixture");
    assert!(
        compressed.len() < 2 * 1024 * 1024,
        "compressed bomb fixture should be tiny (DEFLATE's worst-case ratio is ~1032:1); got {} bytes",
        compressed.len()
    );
    eprintln!(
        "bomb_streaming_default_limit: {total} zero bytes compressed to {} bytes",
        compressed.len()
    );

    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = vec![0u8; 64 * 1024];
    let mut pos = 0usize;
    let outcome = loop {
        let input_slice = compressed.get(pos..).unwrap_or(&[]);
        match d.decompress(input_slice, &mut out) {
            Ok(progress) => {
                pos += progress.consumed;
                if progress.status == Status::Done {
                    break Ok(());
                }
            }
            Err(e) => break Err(e),
        }
    };

    match outcome {
        Err(Error::OutputLimitExceeded { limit }) => {
            assert_eq!(limit, cleanflate::DEFAULT_MAX_OUTPUT);
        }
        other => panic!("expected Err(OutputLimitExceeded), got {other:?}"),
    }
    assert_eq!(d.total_out(), cleanflate::DEFAULT_MAX_OUTPUT);
}

/// A smaller (64 MiB) all-zero stream against a much smaller configured
/// limit (16 MiB), one-shot, across all three formats.
#[test]
fn bomb_oneshot_small_limit() {
    let total = 64u64 * 1024 * 1024;
    let small_limit = 16u64 * 1024 * 1024;

    for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
        let params = zeros_params(framing);
        let compressed = deflate_zeros(total, &params).expect("compress");
        let format = to_cleanflate_format(framing);
        let options = Options {
            max_output: small_limit,
            ..Options::default()
        };

        let err = decompress_with(format, &compressed, &options)
            .expect_err("small limit must be exceeded");
        assert_eq!(
            err,
            Error::OutputLimitExceeded { limit: small_limit },
            "{framing:?}: expected OutputLimitExceeded"
        );
    }
}

/// An output exactly equal to the configured limit must succeed (the
/// limit is inclusive), across all three formats.
#[test]
fn limit_exact_boundary_all_formats() {
    let exact: u64 = 200_000;

    for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
        let params = zeros_params(framing);
        let compressed = deflate_zeros(exact, &params).expect("compress");
        let format = to_cleanflate_format(framing);
        let options = Options {
            max_output: exact,
            ..Options::default()
        };

        let decoded = decompress_with(format, &compressed, &options)
            .unwrap_or_else(|e| panic!("{framing:?}: exact-boundary output should succeed: {e:?}"));
        assert_eq!(
            decoded.data.len() as u64,
            exact,
            "{framing:?}: wrong output length"
        );
        assert!(
            decoded.data.iter().all(|&b| b == 0),
            "{framing:?}: expected all-zero output"
        );
    }
}

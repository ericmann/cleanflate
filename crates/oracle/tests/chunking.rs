//! Chunking-invariance property test (SPEC S5 M4): for streams C zlib
//! compresses (any framing/level/strategy/windowBits/memLevel), feeding
//! `cleanflate`'s streaming `Decompressor` arbitrary input/output chunk
//! sizes -- including all-1-byte schedules -- must produce output
//! identical to the one-shot decoder, which must equal the original
//! data. A second property does the same for streams with one corrupted
//! byte: streamed and one-shot must agree on `Ok`/`ErrorKind`, and on
//! the output when both are `Ok`.

use proptest::prelude::*;

use cleanflate::{decompress_with, Decompressor, Error, Format, Options, Status};
use oracle::ffi::{deflate_bytes, DeflateParams, Framing, Strategy as ZlibStrategy};

/// Case count, overridable for the heavy `make proptest-10k` target;
/// falls back to a small default for ordinary `make test` runs.
const DEFAULT_CASES: u32 = 512;

fn case_count() -> u32 {
    std::env::var("CLEANFLATE_CHUNKING_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_CASES)
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: case_count(),
        ..ProptestConfig::default()
    }
}

fn to_cleanflate_format(framing: Framing) -> Format {
    match framing {
        Framing::Raw => Format::Raw,
        Framing::Zlib => Format::Zlib,
        Framing::Gzip => Format::Gzip,
    }
}

fn framing_strategy() -> impl Strategy<Value = Framing> {
    prop_oneof![Just(Framing::Raw), Just(Framing::Zlib), Just(Framing::Gzip),]
}

fn zlib_strategy_strategy() -> impl Strategy<Value = ZlibStrategy> {
    prop_oneof![
        Just(ZlibStrategy::Default),
        Just(ZlibStrategy::Filtered),
        Just(ZlibStrategy::HuffmanOnly),
        Just(ZlibStrategy::Rle),
        Just(ZlibStrategy::Fixed),
    ]
}

/// 0..=16384 bytes, a mix of fully random data and short repeated
/// patterns (which exercise DEFLATE's match-copying much harder than
/// uniform random noise would).
fn data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..=16384),
        (
            proptest::collection::vec(any::<u8>(), 1..=64),
            0..=16384usize
        )
            .prop_map(|(pattern, len)| pattern.iter().cycle().take(len).copied().collect()),
    ]
}

/// A chunk-size schedule: elements 1..=64, cycled as needed; a 1-in-10
/// chance of an all-1s schedule (the harshest case -- every call moves
/// exactly one byte).
fn chunk_schedule_strategy() -> impl Strategy<Value = Vec<usize>> {
    prop_oneof![
        1 => proptest::collection::vec(Just(1usize), 1..=32),
        9 => proptest::collection::vec(1usize..=64, 1..=32),
    ]
}

#[allow(clippy::too_many_arguments)]
fn params_strategy() -> impl Strategy<Value = (Framing, u8, ZlibStrategy, u8, u8)> {
    (
        framing_strategy(),
        0u8..=9,
        zlib_strategy_strategy(),
        9u8..=15,
        1u8..=9,
    )
}

/// Feed `compressed` to a fresh `Decompressor` using `in_schedule`
/// (input chunk sizes, cycled) and `out_schedule` (output buffer sizes,
/// cycled). Keeps offering input even after the core stream's own
/// `Done` -- exactly like the one-shot `decompress_with`, which always
/// hands the decoder its full remaining buffer in one call -- so any
/// trailing bytes left in `compressed` get the same chance to be
/// flagged as `Error::TrailingData` under either chunking scheme;
/// stops only once there is truly no more input to offer and the
/// decoder isn't just waiting for a fresh output buffer (D-19/D-20).
fn stream_decode(
    format: Format,
    compressed: &[u8],
    in_schedule: &[usize],
    out_schedule: &[usize],
) -> Result<Vec<u8>, Error> {
    let mut d = Decompressor::new(format, Options::default());
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut in_idx = 0usize;
    let mut out_idx = 0usize;

    loop {
        let in_len = in_schedule[in_idx % in_schedule.len()].max(1);
        in_idx += 1;
        let out_len = out_schedule[out_idx % out_schedule.len()].max(1);
        out_idx += 1;

        let end = (pos + in_len).min(compressed.len());
        let input_slice = compressed.get(pos..end).unwrap_or(&[]);
        let input_was_empty = input_slice.is_empty();
        let mut out_buf = vec![0u8; out_len];

        let progress = d.decompress(input_slice, &mut out_buf)?;
        pos += progress.consumed;
        out.extend_from_slice(out_buf.get(..progress.produced).unwrap_or(&[]));

        if input_was_empty && progress.status != Status::OutputFull {
            break;
        }
    }

    d.finish()?;
    Ok(out)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn chunking_invariance(
        data in data_strategy(),
        (framing, level, strategy, window_bits, mem_level) in params_strategy(),
        in_schedule in chunk_schedule_strategy(),
        out_schedule in chunk_schedule_strategy(),
    ) {
        let params = DeflateParams { level, strategy, window_bits, mem_level, framing };
        let compressed = match deflate_bytes(&data, &params, None) {
            Ok(v) => v,
            Err(_) => return Ok(()), // parameter combination C zlib itself rejects
        };
        let format = to_cleanflate_format(framing);

        let one_shot = decompress_with(format, &compressed, &Options::default())
            .map_err(|e| TestCaseError::fail(format!("one-shot decode failed: {e:?}")))?;
        prop_assert_eq!(&one_shot.data, &data, "one-shot output must equal the original data");

        let streamed = stream_decode(format, &compressed, &in_schedule, &out_schedule)
            .map_err(|e| TestCaseError::fail(format!("streamed decode failed: {e:?}")))?;
        prop_assert_eq!(&streamed, &data, "streamed output must equal the original data");
    }

    #[test]
    fn chunking_invariance_on_corrupted_streams(
        data in data_strategy(),
        (framing, level, strategy, window_bits, mem_level) in params_strategy(),
        in_schedule in chunk_schedule_strategy(),
        out_schedule in chunk_schedule_strategy(),
        flip_ratio in 0.0f64..1.0,
        flip_xor in 1u8..=255,
    ) {
        let params = DeflateParams { level, strategy, window_bits, mem_level, framing };
        let mut compressed = match deflate_bytes(&data, &params, None) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if compressed.is_empty() {
            return Ok(());
        }
        let idx = ((flip_ratio * compressed.len() as f64) as usize).min(compressed.len() - 1);
        if let Some(byte) = compressed.get_mut(idx) {
            *byte ^= flip_xor;
        }
        let format = to_cleanflate_format(framing);

        let one_shot = decompress_with(format, &compressed, &Options::default());
        let streamed = stream_decode(format, &compressed, &in_schedule, &out_schedule);

        match (&one_shot, &streamed) {
            (Ok(a), Ok(b)) => {
                prop_assert_eq!(&a.data, b, "Ok/Ok: streamed and one-shot output must match");
            }
            (Err(a), Err(b)) => {
                prop_assert_eq!(
                    a.kind(),
                    b.kind(),
                    "Err/Err: streamed and one-shot must fail with the same ErrorKind"
                );
            }
            _ => {
                prop_assert!(
                    false,
                    "streamed and one-shot disagree on Ok/Err: one_shot={one_shot:?} streamed={streamed:?}"
                );
            }
        }
    }
}

//! Differential fuzz target (b): a round-trip through C zlib's own
//! compressor must always decode back to the original data through
//! every `cleanflate` entry point (one-shot, streaming) and through the
//! oracle's own decompressor, for every framing/level/strategy/
//! windowBits/memLevel combination. Any mismatch is a real bug and
//! `panic!`s -- that panic *is* the fuzz failure (SPEC S5 M4 (b)).
#![no_main]

use libfuzzer_sys::fuzz_target;

use cleanflate::{decompress_with, Decompressor, Format, Options, Status};
use oracle::ffi::{deflate_bytes, inflate_bytes, DeflateParams, Framing, Strategy};

/// Not a `cleanflate` tunable (C8 only governs `cleanflate`'s own
/// defaults): just a cap so a single fuzz iteration can't spend
/// unbounded time.
const FUZZ_MAX_OUTPUT: u64 = 1 << 24;

fn framing_for(byte: u8) -> (Framing, Format) {
    match byte % 3 {
        0 => (Framing::Raw, Format::Raw),
        1 => (Framing::Zlib, Format::Zlib),
        _ => (Framing::Gzip, Format::Gzip),
    }
}

fn strategy_for(byte: u8) -> Strategy {
    match byte % 5 {
        0 => Strategy::Default,
        1 => Strategy::Filtered,
        2 => Strategy::HuffmanOnly,
        3 => Strategy::Rle,
        _ => Strategy::Fixed,
    }
}

/// Stream `compressed` through a fresh `Decompressor`, chunking both
/// input and output by `chunk` bytes at a time (mirroring the pattern
/// established in tests/stream_contract.rs and tests/chunking.rs:
/// keep offering input past `Done` -- there never is any trailing data
/// here, so this always terminates at `Status::Done` -- and never treat
/// `OutputFull` as a stopping point).
fn stream_decode(format: Format, compressed: &[u8], chunk: usize) -> Vec<u8> {
    let mut d = Decompressor::new(
        format,
        Options {
            max_output: FUZZ_MAX_OUTPUT,
            ..Options::default()
        },
    );
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let end = (pos + chunk).min(compressed.len());
        let input_slice = compressed.get(pos..end).unwrap_or(&[]);
        let input_was_empty = input_slice.is_empty();
        let mut out_buf = vec![0u8; chunk];
        let progress = match d.decompress(input_slice, &mut out_buf) {
            Ok(p) => p,
            Err(e) => panic!("streaming decode errored on a stream C zlib itself produced: {e:?}"),
        };
        pos += progress.consumed;
        out.extend_from_slice(out_buf.get(..progress.produced).unwrap_or(&[]));
        if input_was_empty && progress.status != Status::OutputFull {
            break;
        }
    }
    if let Err(e) = d.finish() {
        panic!("streaming finish() errored on a stream C zlib itself produced: {e:?}");
    }
    out
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }
    let (framing, format) = framing_for(data[0]);
    let level = data[1] % 10;
    let strategy = strategy_for(data[2]);
    let window_bits = 9 + (data[3] % 7);
    let mem_level = 1 + (data[4] % 9);
    let rest = &data[5..];
    let payload = rest.get(..65536.min(rest.len())).unwrap_or(rest);

    let params = DeflateParams {
        level,
        strategy,
        window_bits,
        mem_level,
        framing,
    };
    let compressed = match deflate_bytes(payload, &params, None) {
        Ok(v) => v,
        Err(_) => return, // this parameter combination C zlib itself rejects
    };

    let options = Options {
        max_output: FUZZ_MAX_OUTPUT,
        ..Options::default()
    };
    match decompress_with(format, &compressed, &options) {
        Ok(decoded) if decoded.data == payload => {}
        Ok(decoded) => panic!(
            "one-shot output diverged from the original {} bytes (got {})",
            payload.len(),
            decoded.data.len()
        ),
        Err(e) => panic!("one-shot decode errored on a stream C zlib itself produced: {e:?}"),
    }

    let chunk = (payload.len() % 251) + 1;
    let streamed = stream_decode(format, &compressed, chunk);
    if streamed != payload {
        panic!(
            "streamed output diverged from the original {} bytes (got {}, chunk {chunk})",
            payload.len(),
            streamed.len()
        );
    }

    let oracle_outcome = inflate_bytes(&compressed, framing, None, FUZZ_MAX_OUTPUT as usize);
    if oracle_outcome.output != payload {
        panic!(
            "oracle's own decoder diverged from the original {} bytes (got {}, status {:?})",
            payload.len(),
            oracle_outcome.output.len(),
            oracle_outcome.status
        );
    }
});

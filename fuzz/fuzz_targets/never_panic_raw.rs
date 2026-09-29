//! Never-panic fuzz target for `Format::Raw` (RFC 1951): arbitrary bytes
//! must never panic the one-shot or streaming decoders, regardless of
//! chunking. Results are ignored -- only panics/timeouts are failures
//! (SPEC S5 M4 (a)).
#![no_main]

use libfuzzer_sys::fuzz_target;

use cleanflate::{decompress_with, Decompressor, Format, Options, Status};

/// Not a tunable (C8 only governs `cleanflate`'s own defaults): this is
/// just a small cap so the fuzzer doesn't waste time materializing huge
/// outputs from a crafted bomb-shaped input.
const FUZZ_MAX_OUTPUT: u64 = 1 << 24;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let in_chunk = (data[0] as usize % 256) + 1;
    let out_chunk = (data[1] as usize % 256) + 1;
    let body = &data[2..];

    let options = Options {
        max_output: FUZZ_MAX_OUTPUT,
        ..Options::default()
    };
    let _ = decompress_with(Format::Raw, body, &options);

    let mut d = Decompressor::new(Format::Raw, options);
    let mut pos = 0usize;
    loop {
        let end = (pos + in_chunk).min(body.len());
        let input_slice = body.get(pos..end).unwrap_or(&[]);
        let input_was_empty = input_slice.is_empty();
        let mut out_buf = vec![0u8; out_chunk];
        match d.decompress(input_slice, &mut out_buf) {
            Ok(progress) => {
                pos += progress.consumed;
                if input_was_empty && progress.status != Status::OutputFull {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = d.finish();
});

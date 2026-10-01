//! Differential fuzz target: the Fast and Reference decode paths, and the
//! slice and Vec one-shot output sinks, must agree on arbitrary bytes
//! (SPEC S5). Both decode the input one-shot and under
//! an input-derived chunk schedule, and any difference in output, error,
//! per-call consumed/produced/status, or `finish()` is a panic.
//!
//! Input layout: `byte0 % 3` selects the format (raw, zlib, gzip);
//! `byte1` and `byte2` give the input and output chunk sizes (1..=256); the
//! rest is the stream.
#![no_main]

use libfuzzer_sys::fuzz_target;

use cleanflate::test_hooks::{
    decompress_with_path, decompress_with_sink, decompressor_with_path, DecodePath, OneShotSink,
};
use cleanflate::{Error, Format, Options, Status};

/// Target-local cap on decoded output, as in the other fuzz targets.
const FUZZ_MAX_OUTPUT: u64 = 1 << 24;

/// A streaming trace stops once this much output has been produced, so
/// one-byte output chunks cannot make a single input take millions of
/// calls. Both paths stop at the same point.
const TRACE_MAX_OUTPUT: usize = 1 << 16;

type Call = Result<(usize, usize, Status), Error>;

struct Trace {
    calls: Vec<Call>,
    output: Vec<u8>,
    finish: Result<(), Error>,
    total_in: u64,
    total_out: u64,
    trailing_start: Option<u64>,
}

fn trace(
    format: Format,
    options: &Options,
    path: DecodePath,
    body: &[u8],
    in_chunk: usize,
    out_chunk: usize,
) -> Trace {
    let mut d = decompressor_with_path(format, options.clone(), path);
    let mut calls = Vec::new();
    let mut output = Vec::new();
    let mut pos = 0usize;
    let mut no_progress = 0u32;
    let budget = 16 * (body.len() + TRACE_MAX_OUTPUT + 64);
    while calls.len() < budget && output.len() < TRACE_MAX_OUTPUT {
        let end = (pos + in_chunk).min(body.len());
        let chunk = body.get(pos..end).unwrap_or(&[]);
        let mut buf = vec![0u8; out_chunk];
        match d.decompress(chunk, &mut buf) {
            Err(e) => {
                calls.push(Err(e));
                break;
            }
            Ok(p) => {
                calls.push(Ok((p.consumed, p.produced, p.status)));
                pos += p.consumed;
                output.extend_from_slice(buf.get(..p.produced).unwrap_or(&[]));
                if p.consumed == 0 && p.produced == 0 {
                    no_progress += 1;
                } else {
                    no_progress = 0;
                }
                if p.status == Status::Done || no_progress >= 64 {
                    break;
                }
            }
        }
    }
    Trace {
        calls,
        output,
        finish: d.finish(),
        total_in: d.total_in(),
        total_out: d.total_out(),
        trailing_start: d.trailing_start(),
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }
    let format = match data[0] % 3 {
        0 => Format::Raw,
        1 => Format::Zlib,
        _ => Format::Gzip,
    };
    let in_chunk = (data[1] as usize % 256) + 1;
    let out_chunk = (data[2] as usize % 256) + 1;
    let body = &data[3..];
    let options = Options {
        max_output: FUZZ_MAX_OUTPUT,
        ..Options::default()
    };

    let fast = decompress_with_path(format, body, &options, DecodePath::Fast);
    let reference = decompress_with_path(format, body, &options, DecodePath::Reference);
    let same = match (&fast, &reference) {
        (Ok(a), Ok(b)) => a == b,
        (Err(a), Err(b)) => a == b,
        _ => false,
    };
    if !same {
        let short = |r: &Result<cleanflate::Decoded, Error>| match r {
            Ok(d) => format!("Ok(len {}, consumed {})", d.data.len(), d.consumed),
            Err(e) => format!("Err({e:?})"),
        };
        panic!(
            "one-shot mismatch ({format:?}): fast {} vs reference {}",
            short(&fast),
            short(&reference)
        );
    }

    // The Vec sink must agree with the slice sink on both paths.
    for (path, slice) in [(DecodePath::Fast, &fast), (DecodePath::Reference, &reference)] {
        let vec = decompress_with_sink(format, body, &options, path, OneShotSink::Vec);
        let same = match (slice, &vec) {
            (Ok(a), Ok(b)) => a == b,
            (Err(a), Err(b)) => a == b,
            _ => false,
        };
        if !same {
            panic!(
                "sink mismatch ({format:?}, {path:?}): slice {:?} vs vec {:?}",
                slice.as_ref().map(|d| (d.data.len(), d.consumed)),
                vec.as_ref().map(|d| (d.data.len(), d.consumed))
            );
        }
    }

    let a = trace(format, &options, DecodePath::Fast, body, in_chunk, out_chunk);
    let b = trace(format, &options, DecodePath::Reference, body, in_chunk, out_chunk);
    if let Some(i) = (0..a.calls.len().max(b.calls.len())).find(|&i| a.calls.get(i) != b.calls.get(i))
    {
        panic!(
            "call #{i} differs ({format:?}, in {in_chunk}, out {out_chunk}): fast {:?} vs reference {:?}",
            a.calls.get(i),
            b.calls.get(i)
        );
    }
    if a.output != b.output
        || a.finish != b.finish
        || a.total_in != b.total_in
        || a.total_out != b.total_out
        || a.trailing_start != b.trailing_start
    {
        panic!(
            "streaming state differs ({format:?}): finish {:?}/{:?}, in {}/{}, out {}/{}, trailing {:?}/{:?}, output lens {}/{}",
            a.finish,
            b.finish,
            a.total_in,
            b.total_in,
            a.total_out,
            b.total_out,
            a.trailing_start,
            b.trailing_start,
            a.output.len(),
            b.output.len()
        );
    }
});

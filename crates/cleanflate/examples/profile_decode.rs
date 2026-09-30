//! Cleanflate-only decode loop for profiling. Uses cleanflate and std
//! only, so a profiler attached to it sees no other decompressor. The
//! compressed inputs and their `index.tsv` come from `oracle perf-prep`,
//! an unprofiled step.
//!
//! Usage: `profile_decode --dir DIR --format rfc1951|rfc1950|rfc1952
//! --iters N [--mode oneshot|streaming]`.

use std::process::ExitCode;
use std::time::Instant;

use cleanflate::checksum::crc32;
use cleanflate::{Decompressor, Format, Options, Status};

struct Entry {
    name: String,
    compressed: Vec<u8>,
    original_len: usize,
    original_crc32: u32,
}

fn parse_format(s: &str) -> Option<Format> {
    match s {
        "rfc1951" | "raw" => Some(Format::Raw),
        "rfc1950" | "zlib" => Some(Format::Zlib),
        "rfc1952" | "gzip" => Some(Format::Gzip),
        _ => None,
    }
}

fn load(dir: &str) -> Result<Vec<Entry>, String> {
    let index = std::fs::read_to_string(format!("{dir}/index.tsv"))
        .map_err(|e| format!("reading {dir}/index.tsv: {e} (run `make profile-prep`)"))?;
    let mut entries = Vec::new();
    for line in index.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        let [file, clen, olen, crc] = cols.as_slice() else {
            return Err(format!("bad index line: {line}"));
        };
        let compressed =
            std::fs::read(format!("{dir}/{file}")).map_err(|e| format!("reading {file}: {e}"))?;
        let clen: usize = clen.parse().map_err(|e| format!("{file}: {e}"))?;
        if compressed.len() != clen {
            return Err(format!(
                "{file}: length {} != index {clen}",
                compressed.len()
            ));
        }
        entries.push(Entry {
            name: (*file).to_string(),
            compressed,
            original_len: olen.parse().map_err(|e| format!("{file}: {e}"))?,
            original_crc32: crc.parse().map_err(|e| format!("{file}: {e}"))?,
        });
    }
    if entries.is_empty() {
        return Err(format!("{dir}/index.tsv lists no files"));
    }
    Ok(entries)
}

/// Decode in 64 KiB chunks; returns the output and the decompressor.
fn decode_streaming(
    format: Format,
    input: &[u8],
    expected: usize,
) -> Result<(Vec<u8>, Decompressor), String> {
    let mut d = Decompressor::new(format, Options::default());
    let mut out = vec![0u8; expected + 65_536];
    let (mut in_pos, mut out_pos) = (0usize, 0usize);
    loop {
        let in_end = (in_pos + 65_536).min(input.len());
        let out_end = (out_pos + 65_536).min(out.len());
        let chunk = input.get(in_pos..in_end).unwrap_or(&[]);
        let window = out.get_mut(out_pos..out_end).unwrap_or(&mut []);
        let p = d.decompress(chunk, window).map_err(|e| format!("{e:?}"))?;
        in_pos += p.consumed;
        out_pos += p.produced;
        match p.status {
            Status::Done => break,
            Status::OutputFull => {}
            Status::NeedsInput => {
                if p.consumed == 0 && p.produced == 0 && in_pos >= input.len() {
                    break;
                }
            }
        }
    }
    d.finish().map_err(|e| format!("{e:?}"))?;
    out.truncate(out_pos);
    Ok((out, d))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir = None;
    let mut format = None;
    let mut iters = 3u32;
    let mut streaming = false;
    let mut i = 0;
    while i < args.len() {
        let flag = args.get(i).map(String::as_str).unwrap_or("");
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag {
            "--dir" => dir = Some(value.clone()),
            "--format" => {
                format = Some(parse_format(value).ok_or_else(|| format!("bad --format {value}"))?)
            }
            "--iters" => iters = value.parse().map_err(|e| format!("--iters: {e}"))?,
            "--mode" => match value.as_str() {
                "oneshot" => streaming = false,
                "streaming" => streaming = true,
                other => return Err(format!("bad --mode {other}")),
            },
            other => return Err(format!("unknown argument {other}")),
        }
        i += 2;
    }
    let dir = dir.ok_or("--dir is required")?;
    let format = format.ok_or("--format is required")?;
    let entries = load(&dir)?;

    let total_bytes: usize = entries.iter().map(|e| e.original_len).sum();
    let mut decoded_bytes = 0usize;
    #[cfg(feature = "stats")]
    let mut merged = cleanflate::Stats::default();
    let start = Instant::now();
    for iter in 0..iters {
        for e in &entries {
            let out = if streaming {
                let (out, _d) = decode_streaming(format, &e.compressed, e.original_len)?;
                #[cfg(feature = "stats")]
                if iter == 0 {
                    merged.merge(&_d.stats());
                }
                out
            } else {
                #[cfg(feature = "stats")]
                {
                    let (d, s) = cleanflate::decompress_with_stats(
                        format,
                        &e.compressed,
                        &Options::default(),
                    )
                    .map_err(|err| format!("{}: {err:?}", e.name))?;
                    if iter == 0 {
                        merged.merge(&s);
                    }
                    d.data
                }
                #[cfg(not(feature = "stats"))]
                cleanflate::decompress_with(format, &e.compressed, &Options::default())
                    .map_err(|err| format!("{}: {err:?}", e.name))?
                    .data
            };
            if iter == 0 && (out.len() != e.original_len || crc32(&out) != e.original_crc32) {
                return Err(format!("{}: output does not match the index", e.name));
            }
            decoded_bytes += out.len();
            std::hint::black_box(&out);
        }
    }
    let secs = start.elapsed().as_secs_f64();
    let mib = decoded_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "profile_decode: {} files, {} iters, {:.1} MiB in {:.2} s = {:.1} MiB/s (of {} bytes per pass)",
        entries.len(),
        iters,
        mib,
        secs,
        mib / secs.max(1e-9),
        total_bytes
    );
    #[cfg(feature = "stats")]
    println!("{merged:#?}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("profile_decode: {e}");
            ExitCode::FAILURE
        }
    }
}

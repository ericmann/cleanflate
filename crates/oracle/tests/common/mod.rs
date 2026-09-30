//! Shared helpers for the oracle integration tests: proptest strategies
//! and a deterministic call-by-call trace of a `Decompressor`, used to
//! compare the Fast and Reference decode paths.

#![allow(dead_code)]

use std::path::PathBuf;

use proptest::prelude::*;

use cleanflate::test_hooks::{decompress_with_path, decompressor_with_path, DecodePath};
use cleanflate::{Format, Options, Status};
use oracle::ffi::{Framing, Strategy as ZlibStrategy};
use oracle::manifest::sha256_hex;

/// What one `decompress` call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallOutcome {
    /// The call returned `Ok`.
    Progress {
        consumed: usize,
        produced: usize,
        status: Status,
    },
    /// The call returned `Err`; the payload is `format!("{e:?}")`.
    Err(String),
}

/// Everything observable about driving one decompressor over one input
/// with fixed chunk schedules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub calls: Vec<CallOutcome>,
    pub output: Vec<u8>,
    pub finish: Option<String>,
    pub total_in: u64,
    pub total_out: u64,
    pub trailing_start: Option<u64>,
    pub gzip_member_count: u64,
    pub stalled: bool,
}

/// Drive a fresh decompressor on `path`. Schedules (entries >= 1) are
/// cycled; each call gets the next input chunk from the current offset
/// and a fresh zeroed output buffer of the next size.
pub fn trace(
    format: Format,
    options: &Options,
    path: DecodePath,
    input: &[u8],
    in_sched: &[usize],
    out_sched: &[usize],
) -> Trace {
    assert!(
        !in_sched.is_empty() && !out_sched.is_empty(),
        "empty schedule"
    );
    let mut d = decompressor_with_path(format, options.clone(), path);
    let mut calls = Vec::new();
    let mut output = Vec::new();
    let mut pos = 0usize;
    let mut idx = 0usize;
    let mut no_progress = 0u32;
    let mut stalled = false;

    loop {
        let in_len = in_sched
            .get(idx % in_sched.len())
            .copied()
            .unwrap_or(1)
            .max(1);
        let out_len = out_sched
            .get(idx % out_sched.len())
            .copied()
            .unwrap_or(1)
            .max(1);
        idx += 1;
        let end = pos.saturating_add(in_len).min(input.len());
        let chunk = input.get(pos..end).unwrap_or(&[]);
        let mut buf = vec![0u8; out_len];
        match d.decompress(chunk, &mut buf) {
            Err(e) => {
                calls.push(CallOutcome::Err(format!("{e:?}")));
                break;
            }
            Ok(p) => {
                calls.push(CallOutcome::Progress {
                    consumed: p.consumed,
                    produced: p.produced,
                    status: p.status,
                });
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
        let budget =
            16usize.saturating_mul(input.len().saturating_add(output.len()).saturating_add(64));
        if calls.len() > budget {
            stalled = true;
            break;
        }
    }

    let finish = Some(format!("{:?}", d.finish()));
    Trace {
        calls,
        output,
        finish,
        total_in: d.total_in(),
        total_out: d.total_out(),
        trailing_start: d.trailing_start(),
        gzip_member_count: d.gzip_member_count(),
        stalled,
    }
}

/// One-shot decode on `path`: data, consumed, and the gzip headers'
/// Debug string, or the error's Debug string.
pub fn oneshot(
    format: Format,
    options: &Options,
    path: DecodePath,
    input: &[u8],
) -> Result<(Vec<u8>, usize, String), String> {
    decompress_with_path(format, input, options, path)
        .map(|d| (d.data, d.consumed, format!("{:?}", d.gzip_members)))
        .map_err(|e| format!("{e:?}"))
}

/// Describe the first difference between two traces, or `None`.
pub fn describe_diff(fast: &Trace, reference: &Trace) -> Option<String> {
    if fast == reference {
        return None;
    }
    let n = fast.calls.len().max(reference.calls.len());
    for i in 0..n {
        let a = fast.calls.get(i);
        let b = reference.calls.get(i);
        if a != b {
            return Some(format!(
                "first differing call #{i}: fast={a:?} reference={b:?}"
            ));
        }
    }
    Some(format!(
        "calls equal but state differs: fast(finish={:?} in={} out={} trailing={:?} members={} stalled={} out_len={}) \
         reference(finish={:?} in={} out={} trailing={:?} members={} stalled={} out_len={})",
        fast.finish,
        fast.total_in,
        fast.total_out,
        fast.trailing_start,
        fast.gzip_member_count,
        fast.stalled,
        fast.output.len(),
        reference.finish,
        reference.total_in,
        reference.total_out,
        reference.trailing_start,
        reference.gzip_member_count,
        reference.stalled,
        reference.output.len(),
    ))
}

/// Save a failing input under `target/regress-candidates/` (plus a note
/// file) so it can be promoted to a regression fixture.
pub fn dump_candidate(
    tag: &str,
    format: Format,
    input: &[u8],
    in_sched: &[usize],
    out_sched: &[usize],
    note: &str,
) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("regress-candidates");
    let _ = std::fs::create_dir_all(&dir);
    let hash = sha256_hex(input);
    let stem = format!("{tag}-{}", hash.get(..8).unwrap_or(&hash));
    let bin = dir.join(format!("{stem}.bin"));
    let _ = std::fs::write(&bin, input);
    let _ = std::fs::write(
        dir.join(format!("{stem}.txt")),
        format!(
            "format: {format:?}\nin_sched: {in_sched:?}\nout_sched: {out_sched:?}\nnote: {note}\n"
        ),
    );
    bin
}

pub fn to_cleanflate_format(framing: Framing) -> Format {
    match framing {
        Framing::Raw => Format::Raw,
        Framing::Zlib => Format::Zlib,
        Framing::Gzip => Format::Gzip,
    }
}

pub fn framing_strategy() -> impl Strategy<Value = Framing> {
    prop_oneof![Just(Framing::Raw), Just(Framing::Zlib), Just(Framing::Gzip),]
}

pub fn zlib_strategy_strategy() -> impl Strategy<Value = ZlibStrategy> {
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
pub fn data_strategy() -> impl Strategy<Value = Vec<u8>> {
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
pub fn chunk_schedule_strategy() -> impl Strategy<Value = Vec<usize>> {
    prop_oneof![
        1 => proptest::collection::vec(Just(1usize), 1..=32),
        9 => proptest::collection::vec(1usize..=64, 1..=32),
    ]
}

/// (framing, level, strategy, windowBits, memLevel).
pub fn params_strategy() -> impl Strategy<Value = (Framing, u8, ZlibStrategy, u8, u8)> {
    (
        framing_strategy(),
        0u8..=9,
        zlib_strategy_strategy(),
        9u8..=15,
        1u8..=9,
    )
}

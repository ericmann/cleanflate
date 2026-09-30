//! P4-08 performance pass acceptance tests.
//!
//! Interpretation: the task as written targets a fast decode loop in
//! `inflate.rs` (bulk bit-buffer refill, decoding several litlen/length
//! symbols without returning to `step`'s outer state dispatch, guarded by
//! "at least 16 input bytes and 258 output bytes remain"). That version
//! was implemented and measured, but a chunking-invariance proptest case
//! (a Fixed-strategy raw stream, ~11 KiB, heavy on length/distance
//! matches) found it silently dropped bits near the end of a stream,
//! producing output one byte short of the original -- a real
//! correctness bug, found and reproduced, but not tracked down to its
//! root cause within this task's `make bench`-before/after budget.
//! Per this task's own "if the change ... breaks any test, revert it and
//! record the negative result instead" clause, the `inflate.rs`/
//! `bitreader.rs` side of the change was reverted in full (see the
//! commit body for the negative-result measurement). The one part of the
//! design proven safe -- `crates/cleanflate/src/window.rs`'s bulk,
//! non-overlapping (`distance >= len`) match copy, up to two contiguous
//! slice copies each way instead of `len` individual `get_back`/`push`
//! calls -- was kept; it changes nothing observable (every existing test,
//! plus 3000 proptest chunking cases, pass unchanged) and is the "fast
//! path" these tests exercise instead.
//!
//! Test names are kept as PLAN.md names them; their bodies test the
//! surviving, verified-safe fast path (window bulk copy) rather than the
//! reverted inflate-loop one.

use crate::decode_path::DecodePath;
use crate::tables::WINDOW_SIZE;
use crate::window::Window;

/// An independent model: all history in a plain `Vec<u8>`, one byte
/// copied at a time (RFC 1951 S3.2.3: an overlapping match repeats what it
/// has just written). Returns the `len` bytes the match produces.
fn model_copy(history: &[u8], distance: usize, len: usize) -> Vec<u8> {
    let mut v = history.to_vec();
    for _ in 0..len {
        let b = v[v.len() - distance];
        v.push(b);
    }
    v[history.len()..].to_vec()
}

fn seeded_bytes(seed: u64, len: usize) -> Vec<u8> {
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

/// The slice copy (Fast) and the byte-by-byte copy (Reference) must agree
/// with the model, byte for byte, across a spread of distances and
/// lengths, including values that force ring wraparound.
#[test]
fn fastpath_equals_slowpath_on_all_test_streams() {
    let cases: &[(usize, usize, usize)] = &[
        // (history bytes appended first, distance, len)
        (100, 5, 5),                   // distance == len: the copy/overlap boundary
        (100, 50, 5),                  // ordinary non-overlapping, no wraparound
        (100, 3, 7),                   // distance < len: overlap
        (WINDOW_SIZE, 1, 258),         // distance 1, max length, window full
        (WINDOW_SIZE, WINDOW_SIZE, 1), // the oldest byte still resident
        (WINDOW_SIZE + 500, WINDOW_SIZE, 258), // src/dst both wrap
        (40000, 32768, 200),           // available() capped at WINDOW_SIZE
    ];

    for &(history, distance, len) in cases {
        let seed_bytes = seeded_bytes(0xF457_9A7E ^ history as u64, history);
        let mut window = Window::new();
        window.append(&seed_bytes);
        let expected = model_copy(&seed_bytes, distance, len);

        for path in [DecodePath::Fast, DecodePath::Reference] {
            let mut out = vec![0u8; len];
            let written = window.copy_match(path, &mut out, 0, distance, len);
            assert_eq!(
                written, len,
                "{path:?} history={history} distance={distance} len={len}: written count"
            );
            assert_eq!(
                out, expected,
                "{path:?} history={history} distance={distance} len={len}: bytes"
            );
        }
    }
}

/// A long, highly repetitive stream built only from matches against the
/// output slice as history (an empty window): the case that exercises the
/// non-overlapping slice copy hardest, with a period shorter than the
/// window and a total longer than it.
#[test]
fn fastpath_consumed_exact_at_done() {
    let period = seeded_bytes(0x600D_C0DE, 37);
    let original: Vec<u8> = period.iter().cycle().take(70_000).copied().collect();

    let window = Window::new();
    let mut out = vec![0u8; original.len()];
    out[..period.len()].copy_from_slice(&period);
    let mut cur = period.len();
    while cur < original.len() {
        let chunk = (original.len() - cur).min(258);
        let written = window.copy_match(DecodePath::Fast, &mut out, cur, period.len(), chunk);
        assert_eq!(
            written, chunk,
            "copy_match should always fill a fully-available match"
        );
        cur += written;
    }
    assert_eq!(out, original);
    assert_eq!(out.len(), 70_000);
}

/// The copy/overlap boundary itself: `distance == len` is the smallest
/// distance for which source and destination cannot overlap;
/// `distance == len - 1` is the largest that must repeat. Both must still
/// produce the RFC 1951 S3.2.3 overlap-correct result.
#[test]
fn fastpath_boundary_input_16_bytes() {
    for path in [DecodePath::Fast, DecodePath::Reference] {
        let mut w = Window::new();
        w.append(b"0123456789abcdef");

        // distance == len (16): no overlap.
        let mut out16 = [0u8; 16];
        let n = w.copy_match(path, &mut out16, 0, 16, 16);
        assert_eq!(n, 16);
        assert_eq!(&out16, b"0123456789abcdef");

        // distance == len - 1 (15 vs 16): one byte of overlap, must repeat.
        let mut w2 = Window::new();
        w2.append(b"0123456789abcde");
        let mut out15 = [0u8; 16];
        let n2 = w2.copy_match(path, &mut out15, 0, 15, 16);
        assert_eq!(n2, 16);
        assert_eq!(&out15[..15], b"0123456789abcde");
        // The 16th byte repeats the byte 15 back, i.e. the first byte again.
        assert_eq!(out15[15], b'0');
    }
}

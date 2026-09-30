use crate::config::{FAST_LOOP_MIN_INPUT, FAST_LOOP_MIN_OUTPUT};
use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::inflate::{InflateStatus, Inflater};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{
    emit_dynamic_block, huffman_lengths_from_freqs_u16, literal_cl_program, DynBlock, PayloadSym,
};
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal, emit_match};

/// Test-local xorshift64 generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

/// One symbol of a generated block.
#[derive(Clone, Copy)]
enum Sym {
    Lit(u8),
    Match { len: u16, dist: u32 },
}

/// Pick `n` symbols valid after `history` bytes of output, appending what
/// they decode to onto `out`.
fn pick_symbols(rng: &mut Rng, n: usize, out: &mut Vec<u8>) -> Vec<Sym> {
    let mut syms = Vec::new();
    for _ in 0..n {
        if out.is_empty() || rng.below(3) != 0 {
            let b = (rng.below(12) as u8) + b'a';
            out.push(b);
            syms.push(Sym::Lit(b));
        } else {
            let len = 3 + rng.below(256) as u16;
            let max_dist = out.len().min(32768);
            let dist = match rng.below(4) {
                0 => 1,
                1 => 1 + rng.below(max_dist.min(8)),
                _ => 1 + rng.below(max_dist),
            } as u32;
            for _ in 0..len {
                let b = out[out.len() - dist as usize];
                out.push(b);
            }
            syms.push(Sym::Match { len, dist });
        }
    }
    syms
}

fn emit_fixed(w: &mut BitWriter, syms: &[Sym], bfinal: bool) {
    emit_block_header(w, bfinal, 0b01);
    for s in syms {
        match *s {
            Sym::Lit(b) => emit_literal(w, b as u16),
            Sym::Match { len, dist } => emit_match(w, len, dist),
        }
    }
    emit_end_of_block(w);
}

fn emit_dynamic(w: &mut BitWriter, syms: &[Sym], bfinal: bool) {
    // A complete code over all 286 literal/length and 30 distance symbols.
    let lit_freqs: Vec<(u16, u32)> = (0..286u16).map(|s| (s, 1)).collect();
    let dist_freqs: Vec<(u16, u32)> = (0..30u16).map(|s| (s, 1)).collect();
    let mut litlen = vec![0u8; 286];
    for (s, l) in huffman_lengths_from_freqs_u16(&lit_freqs) {
        litlen[s as usize] = l;
    }
    let mut dist = vec![0u8; 30];
    for (s, l) in huffman_lengths_from_freqs_u16(&dist_freqs) {
        dist[s as usize] = l;
    }
    let mut all = litlen.clone();
    all.extend_from_slice(&dist);
    let block = DynBlock {
        bfinal,
        cl_program: literal_cl_program(&all),
        litlen_lengths: litlen,
        dist_lengths: dist,
    };
    let payload: Vec<PayloadSym> = syms
        .iter()
        .map(|s| match *s {
            Sym::Lit(b) => PayloadSym::Literal(b),
            Sym::Match { len, dist } => PayloadSym::Match { len, dist },
        })
        .collect();
    emit_dynamic_block(w, &block, &payload);
}

/// (stream, expected output) for a named hand-built shape.
fn stream(kind: &str) -> (Vec<u8>, Vec<u8>) {
    let mut rng = Rng(0xFA57_100F_u64 + kind.len() as u64);
    let mut out = Vec::new();
    let mut w = BitWriter::new();
    match kind {
        "fixed" => {
            let s = pick_symbols(&mut rng, 300, &mut out);
            emit_fixed(&mut w, &s, true);
        }
        "dynamic" => {
            let s = pick_symbols(&mut rng, 300, &mut out);
            emit_dynamic(&mut w, &s, true);
        }
        "multi" => {
            let s = pick_symbols(&mut rng, 120, &mut out);
            emit_fixed(&mut w, &s, false);
            let s = pick_symbols(&mut rng, 120, &mut out);
            emit_dynamic(&mut w, &s, false);
            let s = pick_symbols(&mut rng, 120, &mut out);
            emit_fixed(&mut w, &s, true);
        }
        "then-stored" => {
            let s = pick_symbols(&mut rng, 150, &mut out);
            emit_fixed(&mut w, &s, false);
            emit_block_header(&mut w, true, 0b00);
            w.align();
            let payload = b"stored payload after a compressed block";
            w.write_bits(payload.len() as u32, 16);
            w.write_bits(!(payload.len() as u32) & 0xFFFF, 16);
            for &b in payload.iter() {
                w.write_bits(b as u32, 8);
            }
            out.extend_from_slice(payload);
        }
        _ => panic!("unknown stream kind"),
    }
    (w.finish(), out)
}

type Call = Result<(usize, usize, InflateStatus), Error>;

/// Drive `input` in `in_chunk` pieces with `out_chunk`-byte output slices.
fn drive(
    path: DecodePath,
    input: &[u8],
    in_chunk: usize,
    out_chunk: usize,
) -> (Vec<Call>, Vec<u8>, Inflater) {
    let mut inf = Inflater::new_with_path(path);
    let mut calls = Vec::new();
    let mut out = Vec::new();
    let mut pos = 0usize;
    for _ in 0..1_000_000 {
        let end = pos.saturating_add(in_chunk).min(input.len());
        let mut buf = vec![0u8; out_chunk.min(1 << 20)];
        let res = inf.step(input.get(pos..end).unwrap_or(&[]), &mut buf);
        if let Ok((c, p, _)) = &res {
            pos += c;
            out.extend_from_slice(&buf[..*p]);
        }
        let stop = matches!(res, Ok((_, _, InflateStatus::Done)) | Err(_))
            || matches!(&res, Ok((0, 0, InflateStatus::NeedsInput)) if pos >= input.len());
        calls.push(res);
        if stop {
            break;
        }
    }
    (calls, out, inf)
}

#[cfg(feature = "stats")]
fn entries(inf: &Inflater) -> u64 {
    inf.stats().fast_loop_entries
}

const SCHEDULES: [(usize, usize); 8] = [
    (usize::MAX / 4, usize::MAX / 4),
    (usize::MAX / 4, 1),
    (1, usize::MAX / 4),
    (7, 13),
    (40, 300),
    (100, 259),
    (17, 258),
    (usize::MAX / 4, 1000),
];

#[test]
fn fast_loop_same_trace_as_reference_hand_built() {
    for kind in ["fixed", "dynamic", "multi", "then-stored"] {
        let (input, expected) = stream(kind);
        for (i, o) in SCHEDULES {
            let (fast_calls, fast_out, fast) = drive(DecodePath::Fast, &input, i, o);
            let (ref_calls, ref_out, _) = drive(DecodePath::Reference, &input, i, o);
            assert_eq!(fast_out, expected, "{kind} in {i} out {o}");
            assert_eq!(ref_out, expected, "{kind} in {i} out {o}");
            assert_eq!(
                fast_calls, ref_calls,
                "{kind} in {i} out {o}: per-call results"
            );
            #[cfg(feature = "stats")]
            if i > 1000 && o > 250 {
                assert!(entries(&fast) > 0, "{kind}: the fast loop must run");
            }
            #[cfg(not(feature = "stats"))]
            let _ = fast;
        }
    }
}

#[test]
fn fast_loop_input_margin_boundaries() {
    let (input, expected) = stream("fixed");
    // The 3-bit block header loads one byte, so `n` bytes remain when the
    // literal/length state starts.
    for n in [
        FAST_LOOP_MIN_INPUT - 1,
        FAST_LOOP_MIN_INPUT,
        FAST_LOOP_MIN_INPUT + 1,
    ] {
        let mut runs = Vec::new();
        for path in [DecodePath::Fast, DecodePath::Reference] {
            let mut inf = Inflater::new_with_path(path);
            let mut out = vec![0u8; 1 << 20];
            let first = inf.step(&input[..1 + n], &mut out).expect("valid");
            #[cfg(feature = "stats")]
            if path == DecodePath::Fast {
                if n < FAST_LOOP_MIN_INPUT {
                    assert_eq!(entries(&inf), 0, "n {n}");
                } else {
                    assert!(entries(&inf) > 0, "n {n}");
                }
            }
            let rest = inf
                .step(&input[first.0..], &mut out[first.1..])
                .expect("valid");
            assert_eq!(rest.2, InflateStatus::Done);
            assert_eq!(&out[..first.1 + rest.1], &expected[..], "n {n} {path:?}");
            runs.push((first, rest));
        }
        assert_eq!(runs[0], runs[1], "n {n}: per-call results");
    }
}

#[test]
fn fast_loop_output_margin_boundaries() {
    let (input, expected) = stream("dynamic");
    for room in [
        FAST_LOOP_MIN_OUTPUT - 1,
        FAST_LOOP_MIN_OUTPUT,
        FAST_LOOP_MIN_OUTPUT + 1,
    ] {
        let mut runs = Vec::new();
        for path in [DecodePath::Fast, DecodePath::Reference] {
            let mut inf = Inflater::new_with_path(path);
            let mut out = vec![0u8; 1 << 20];
            let first = inf.step(&input, &mut out[..room]).expect("valid");
            #[cfg(feature = "stats")]
            if path == DecodePath::Fast {
                if room < FAST_LOOP_MIN_OUTPUT {
                    assert_eq!(entries(&inf), 0, "room {room}");
                } else {
                    assert!(entries(&inf) > 0, "room {room}");
                }
            }
            let mut produced = first.1;
            let mut consumed = first.0;
            let mut last = first;
            while last.2 != InflateStatus::Done {
                last = inf
                    .step(&input[consumed..], &mut out[produced..])
                    .expect("valid");
                consumed += last.0;
                produced += last.1;
            }
            assert_eq!(&out[..produced], &expected[..], "room {room} {path:?}");
            runs.push((first, consumed, produced));
        }
        assert_eq!(runs[0], runs[1], "room {room}: per-call results");
    }
}

/// `stream` bytes followed by `pad` zero bytes, so the input margin holds
/// when the fast loop reaches the bad symbol.
fn padded(w: BitWriter, pad: usize) -> Vec<u8> {
    let mut v = w.finish();
    v.extend(std::iter::repeat_n(0u8, pad));
    v
}

#[test]
fn fast_loop_errors_identical_to_reference() {
    let mut cases: Vec<(&str, Vec<u8>, Error)> = Vec::new();

    // A distance that reaches before the start of the history so far.
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in b"0123456789" {
        emit_literal(&mut w, b as u16);
    }
    emit_match(&mut w, 5, 11);
    cases.push((
        "distance beyond history",
        padded(w, 48),
        Error::InvalidDistance {
            distance: 11,
            available: 10,
        },
    ));

    // A distance with an empty history: before the stream start.
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_match(&mut w, 3, 1);
    cases.push((
        "distance before stream start",
        padded(w, 48),
        Error::InvalidDistance {
            distance: 1,
            available: 0,
        },
    ));

    // Fixed distance code 30 (valid in the fixed table, never in data).
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'x' as u16);
    emit_literal(&mut w, 257);
    w.write_code(30, 5);
    cases.push(("fixed distance 30", padded(w, 48), Error::InvalidSymbol));

    // Fixed literal/length codes 286 and 287.
    for sym in [286u16, 287] {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_literal(&mut w, b'x' as u16);
        emit_literal(&mut w, sym);
        cases.push((
            if sym == 286 {
                "litlen 286"
            } else {
                "litlen 287"
            },
            padded(w, 48),
            Error::InvalidSymbol,
        ));
    }

    for (name, input, expected) in cases {
        for (i, o) in [(usize::MAX / 4, 4096usize), (64, 300), (usize::MAX / 4, 1)] {
            let (fast, _, fast_inf) = drive(DecodePath::Fast, &input, i, o);
            let (reference, _, _) = drive(DecodePath::Reference, &input, i, o);
            assert_eq!(fast, reference, "{name} in {i} out {o}: per-call results");
            assert_eq!(
                fast.last(),
                Some(&Err(expected.clone())),
                "{name} in {i} out {o}"
            );
            #[cfg(feature = "stats")]
            if i > 1000 && o > 1000 {
                assert!(entries(&fast_inf) > 0, "{name}: found by the fast loop");
            }
            #[cfg(not(feature = "stats"))]
            let _ = fast_inf;
        }
    }
}

#[test]
fn fast_loop_eob_then_trailer_consumed_exact() {
    let (stream, expected) = stream("multi");
    for garbage in [0usize, 1, 16, 40] {
        let mut input = stream.clone();
        input.extend(std::iter::repeat_n(0xA5u8, garbage));
        for (i, o) in [(usize::MAX / 4, 1 << 20), (40, 300), (usize::MAX / 4, 1)] {
            let mut totals = Vec::new();
            for path in [DecodePath::Fast, DecodePath::Reference] {
                let (calls, out, _) = drive(path, &input, i, o);
                assert_eq!(out, expected, "{path:?} garbage {garbage}");
                let consumed: usize = calls
                    .iter()
                    .filter_map(|c| c.as_ref().ok().map(|t| t.0))
                    .sum();
                assert_eq!(consumed, stream.len(), "{path:?} garbage {garbage} in {i}");
                totals.push(calls);
            }
            assert_eq!(totals[0], totals[1]);
        }
    }
}

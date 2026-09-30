use crate::bitreader::BitReader;
use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::inflate::{InflateStatus, Inflater};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{emit_dynamic_block, literal_cl_program, DynBlock, PayloadSym};
use crate::tests::inflate_fixed::{
    emit_block_header, emit_end_of_block, emit_literal, emit_match, length_symbol_and_extra,
};

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
}

fn bytes(n: usize, seed: u64) -> Vec<u8> {
    let mut r = Rng(seed | 1);
    (0..n).map(|_| r.next() as u8).collect()
}

#[test]
fn refill_bulk_word_and_bytewise() {
    // Word path: 20 bytes, empty buffer -> 7 whole bytes (56 bits).
    let input = bytes(20, 1);
    let mut r = BitReader::new();
    let mut pos = 0usize;
    r.refill_bulk(&input, &mut pos);
    assert_eq!((pos, r.available()), (7, 56));
    let low = (0..7).fold(0u64, |a, i| a | ((input[i] as u64) << (8 * i)));
    assert_eq!(r.peek(32) as u64, low & 0xFFFF_FFFF);

    // With some bits already buffered the count still never exceeds 63.
    for pre in 0..=32u32 {
        let mut r = BitReader::new();
        let mut pos = 0usize;
        assert!(r.fill(&input, &mut pos, pre));
        r.consume(r.available().min(pre));
        r.refill_bulk(&input, &mut pos);
        assert!(r.available() <= 63, "pre {pre}: {}", r.available());
        assert!(r.available() >= 56 - 7, "pre {pre}");
        assert_eq!(r.available() % 8, (8 - (pre % 8)) % 8);
    }

    // Byte path: fewer than 8 bytes left.
    for n in 0..8usize {
        let short = bytes(n, 9);
        let mut r = BitReader::new();
        let mut pos = 0usize;
        r.refill_bulk(&short, &mut pos);
        assert_eq!(pos, n, "all {n} bytes are loaded");
        assert_eq!(r.available(), 8 * n as u32);
    }
    // Byte path stops once more than 55 bits are buffered.
    let seven = bytes(7, 3);
    let mut r = BitReader::new();
    let mut pos = 0usize;
    r.refill_bulk(&seven, &mut pos);
    assert_eq!((pos, r.available()), (7, 56));
    // Calling again with a full buffer loads nothing.
    let more = bytes(7, 4);
    let mut pos2 = 0usize;
    r.refill_bulk(&more, &mut pos2);
    assert_eq!((pos2, r.available()), (0, 56));
}

#[test]
fn give_back_keeps_pending_bits() {
    let input = bytes(16, 5);
    for keep in [0u32, 9, 15] {
        let mut r = BitReader::new();
        let mut pos = 0usize;
        r.refill_bulk(&input, &mut pos);
        assert_eq!(r.available(), 56);
        let expected_k = ((56 - keep) / 8) as usize;
        let low = r.peek(keep.max(1));
        let k = r.give_back(keep);
        assert_eq!(k, expected_k, "keep {keep}");
        assert_eq!(r.available(), 56 - 8 * k as u32);
        assert!(r.available() >= keep && r.available() < keep + 8);
        if keep > 0 {
            assert_eq!(r.peek(keep), low, "kept bits unchanged");
        }
    }
    // Fewer than keep + 8 bits buffered: nothing to give back.
    let mut r = BitReader::new();
    let mut pos = 0usize;
    assert!(r.fill(&input, &mut pos, 12));
    assert_eq!(r.give_back(9), 0);
    assert_eq!(r.available(), 16);
    assert_eq!(r.give_back(20), 0);
}

#[test]
fn bulk_then_giveback_bit_sequence_equals_exact_fill() {
    for seed in 1..=40u64 {
        let input = bytes(300, seed);
        let mut rng = Rng(seed * 7919 + 1);
        let (mut a, mut pa) = (BitReader::new(), 0usize);
        let (mut b, mut pb) = (BitReader::new(), 0usize);
        loop {
            let n = 1 + (rng.next() % 16) as u32;
            let va = a.take(&input, &mut pa, n);
            if b.available() < n {
                b.refill_bulk(&input, &mut pb);
            }
            let vb = if b.available() >= n {
                let v = b.peek(n);
                b.consume(n);
                Some(v)
            } else {
                None
            };
            assert_eq!(va, vb, "seed {seed}");
            if va.is_none() {
                break;
            }
            // Give back at random moments; the exact reader holds at most
            // 7 bits, and the bulk reader must then hold the same.
            if rng.next() % 2 == 0 {
                let k = b.give_back(0);
                pb -= k;
                assert_eq!(pa, pb, "seed {seed}: positions agree after give-back");
                assert_eq!(a.available(), b.available(), "seed {seed}");
            }
        }
    }
}

/// A fixed-Huffman stream with literals and overlapping/distant matches.
fn fixed_stream(bfinal: bool) -> (Vec<u8>, Vec<u8>) {
    let mut w = BitWriter::new();
    emit_fixed(&mut w, bfinal);
    (w.finish(), fixed_expected())
}

fn emit_fixed(w: &mut BitWriter, bfinal: bool) {
    emit_block_header(w, bfinal, 0b01);
    for &b in b"abcabx" {
        emit_literal(w, b as u16);
    }
    emit_match(w, 20, 3);
    emit_match(w, 258, 1);
    emit_match(w, 7, 6);
    emit_literal(w, b'!' as u16);
    emit_end_of_block(w);
}

fn fixed_expected() -> Vec<u8> {
    let mut v = b"abcabx".to_vec();
    for (len, dist) in [(20usize, 3usize), (258, 1), (7, 6)] {
        for _ in 0..len {
            let b = v[v.len() - dist];
            v.push(b);
        }
    }
    v.push(b'!');
    v
}

fn dynamic_stream() -> (Vec<u8>, Vec<u8>) {
    let (lsym, _, _) = length_symbol_and_extra(4);
    let mut litlen = vec![0u8; 260];
    litlen[b'a' as usize] = 2;
    litlen[b'b' as usize] = 3;
    litlen[b'c' as usize] = 3;
    litlen[256] = 2;
    litlen[lsym as usize] = 2;
    let dist = vec![1u8];
    let mut all = litlen.clone();
    all.extend_from_slice(&dist);
    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&all),
        litlen_lengths: litlen,
        dist_lengths: dist,
    };
    let payload = [
        PayloadSym::Literal(b'a'),
        PayloadSym::Literal(b'b'),
        PayloadSym::Literal(b'c'),
        PayloadSym::Match { len: 4, dist: 1 },
        PayloadSym::Match { len: 4, dist: 1 },
        PayloadSym::Literal(b'a'),
    ];
    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    let mut expected = b"abc".to_vec();
    expected.extend(std::iter::repeat_n(b'c', 8));
    expected.push(b'a');
    (w.finish(), expected)
}

/// One step's observable result.
type Call = Result<(usize, usize, InflateStatus), Error>;

/// Drive `input` (in chunks of `in_chunk`) through an inflater with
/// `out_chunk`-byte output slices; return each call and the output.
fn drive(
    path: DecodePath,
    input: &[u8],
    in_chunk: usize,
    out_chunk: usize,
) -> (Vec<Call>, Vec<u8>) {
    let mut inf = Inflater::new_with_path(path);
    let mut calls = Vec::new();
    let mut out = Vec::new();
    let mut pos = 0usize;
    for _ in 0..100_000 {
        let end = (pos + in_chunk).min(input.len());
        let mut buf = vec![0u8; out_chunk];
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
    (calls, out)
}

#[test]
fn raw_done_consumed_exact_with_trailing_bytes() {
    for (stream, expected) in [fixed_stream(true), dynamic_stream()] {
        for garbage in 0..=16usize {
            let mut input = stream.clone();
            input.extend(bytes(garbage, 77 + garbage as u64));
            for path in [DecodePath::Fast, DecodePath::Reference] {
                for split in 0..=input.len() {
                    let (a, b) = input.split_at(split);
                    let mut inf = Inflater::new_with_path(path);
                    let mut out = vec![0u8; 4096];
                    let mut total = 0usize;
                    let mut produced = 0usize;
                    let mut done = false;
                    for chunk in [a, b] {
                        let mut off = 0usize;
                        while !done {
                            let rest = &chunk[off..];
                            let (c, p, st) = inf.step(rest, &mut out[produced..]).expect("valid");
                            off += c;
                            total += c;
                            produced += p;
                            if st == InflateStatus::Done {
                                done = true;
                            } else if (c == 0 && p == 0) || st == InflateStatus::NeedsInput {
                                break;
                            }
                        }
                    }
                    assert!(done, "{path:?} split {split} garbage {garbage}");
                    assert_eq!(&out[..produced], &expected[..]);
                    assert_eq!(
                        total,
                        stream.len(),
                        "{path:?} split {split} garbage {garbage}: consumed at Done"
                    );
                }
            }
        }
    }
}

#[test]
fn per_call_consumed_matches_reference() {
    for (stream, expected) in [fixed_stream(true), dynamic_stream()] {
        for out_chunk in [1usize, 3] {
            for in_chunk in [1usize, 2, 5, usize::MAX / 4] {
                let (fast, fout) = drive(DecodePath::Fast, &stream, in_chunk, out_chunk);
                let (reference, rout) = drive(DecodePath::Reference, &stream, in_chunk, out_chunk);
                assert_eq!(fout, expected);
                assert_eq!(rout, expected);
                assert_eq!(
                    fast, reference,
                    "in_chunk {in_chunk} out_chunk {out_chunk}: per-call results differ"
                );
            }
        }
    }
}

#[test]
fn stored_block_after_bulk_refill() {
    // A non-final fixed block, then a final stored block.
    let mut w = BitWriter::new();
    emit_fixed(&mut w, false);
    emit_block_header(&mut w, true, 0b00);
    w.align();
    let payload = b"stored payload bytes";
    w.write_bits(payload.len() as u32, 16);
    w.write_bits(!(payload.len() as u32) & 0xFFFF, 16);
    for &b in payload.iter() {
        w.write_bits(b as u32, 8);
    }
    let stream = w.finish();
    let mut expected = fixed_expected();
    expected.extend_from_slice(payload);

    for path in [DecodePath::Fast, DecodePath::Reference] {
        for split in 0..=stream.len() {
            let (a, b) = stream.split_at(split);
            let mut inf = Inflater::new_with_path(path);
            let mut out = vec![0u8; 4096];
            let mut produced = 0usize;
            let mut total = 0usize;
            let mut done = false;
            for chunk in [a, b] {
                let mut off = 0usize;
                while !done {
                    let (c, p, st) = inf
                        .step(&chunk[off..], &mut out[produced..])
                        .expect("valid");
                    off += c;
                    total += c;
                    produced += p;
                    if st == InflateStatus::Done {
                        done = true;
                    } else if st == InflateStatus::NeedsInput || (c == 0 && p == 0) {
                        break;
                    }
                }
            }
            assert!(done, "{path:?} split {split}");
            assert_eq!(&out[..produced], &expected[..], "{path:?} split {split}");
            assert_eq!(total, stream.len());
        }
    }
}

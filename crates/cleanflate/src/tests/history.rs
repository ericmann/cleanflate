use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::inflate::{InflateStatus, Inflater};
use crate::tables::WINDOW_SIZE;
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal, emit_match};
use crate::window::Window;

fn seeded_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}

/// Independent model of a match: `n` bytes copied one at a time from a
/// plain `Vec` holding the whole history (RFC 1951 S3.2.3).
fn model_copy(history: &[u8], dist: usize, n: usize) -> Vec<u8> {
    let mut v = history.to_vec();
    for _ in 0..n {
        let b = v[v.len() - dist];
        v.push(b);
    }
    v[history.len()..].to_vec()
}

#[test]
fn history_copy_matches_vec_model() {
    let lens = [3usize, 4, 10, 100, 257, 258];
    for fill in [0usize, 100, WINDOW_SIZE, 100_000] {
        let all = seeded_bytes(0xAB ^ fill as u64, fill);
        let mut window = Window::new();
        window.append(&all);
        for out_pos in (0..=40usize).chain([1000]) {
            let prefix = seeded_bytes(0xCD + out_pos as u64, out_pos);
            let mut history = all.clone();
            history.extend_from_slice(&prefix);
            let max_dist = window.history_available(out_pos);
            assert_eq!(max_dist, history.len().min(WINDOW_SIZE));

            let mut dists: Vec<usize> = (1..=40).collect();
            dists.extend([255, 256, 257, 32767, 32768]);
            dists.push(max_dist + 1);
            for dist in dists {
                for len in lens {
                    for room in [1usize, 7, len] {
                        let n = len.min(room);
                        for path in [DecodePath::Fast, DecodePath::Reference] {
                            let mut out = vec![0u8; out_pos + room];
                            out[..out_pos].copy_from_slice(&prefix);
                            let written = window.copy_match(path, &mut out, out_pos, dist, len);
                            if dist > max_dist {
                                assert_eq!(written, 0, "fill {fill} out_pos {out_pos} dist {dist}");
                                continue;
                            }
                            assert_eq!(
                                written, n,
                                "{path:?} fill {fill} out_pos {out_pos} d {dist}"
                            );
                            assert_eq!(
                                &out[out_pos..out_pos + n],
                                &model_copy(&history, dist, n)[..],
                                "{path:?} fill {fill} out_pos {out_pos} dist {dist} len {len} room {room}"
                            );
                            assert_eq!(&out[..out_pos], &prefix[..], "history untouched");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn window_append_wraps_and_truncates() {
    for start_fill in [0usize, 1, 100, 32767, 32768, 50_000] {
        let mut model = seeded_bytes(0x11 + start_fill as u64, start_fill);
        let mut w = Window::new();
        w.append(&model);
        for (i, size) in [0usize, 1, 32767, 32768, 32769, 100_000]
            .into_iter()
            .enumerate()
        {
            let chunk = seeded_bytes(0x77 + (i * 31 + start_fill) as u64, size);
            w.append(&chunk);
            model.extend_from_slice(&chunk);
            let avail = model.len().min(WINDOW_SIZE);
            assert_eq!(w.available(), avail, "start {start_fill} size {size}");
            for back in [1usize, 2, 100, 32767, 32768] {
                if back <= avail {
                    assert_eq!(
                        w.get_back(back),
                        Some(model[model.len() - back]),
                        "start {start_fill} size {size} back {back}"
                    );
                } else {
                    assert_eq!(w.get_back(back), None);
                }
            }
            // read_back: any span that ends at or before the window end.
            for (back, n) in [
                (1usize, 1usize),
                (100, 100),
                (200, 37),
                (32768, 5),
                (32768, 32768),
            ] {
                let mut dst = vec![0u8; n];
                let ok = w.read_back(back, &mut dst);
                if back <= avail && n <= back {
                    assert!(ok, "start {start_fill} size {size} back {back} n {n}");
                    let from = model.len() - back;
                    assert_eq!(&dst[..], &model[from..from + n]);
                } else {
                    assert!(!ok);
                }
            }
        }
    }
}

type Call = Result<(usize, usize, InflateStatus), Error>;

/// Drive an inflater (optionally with a preset dictionary) with all input
/// at once and `out_chunk`-byte output slices.
fn drive(
    path: DecodePath,
    dict: Option<&[u8]>,
    input: &[u8],
    out_chunk: usize,
) -> (Vec<Call>, Vec<u8>) {
    let mut inf = match dict {
        Some(d) => Inflater::with_dictionary(d, path),
        None => Inflater::new_with_path(path),
    };
    let mut calls = Vec::new();
    let mut out = Vec::new();
    let mut pos = 0usize;
    for _ in 0..1_000_000 {
        let mut buf = vec![0u8; out_chunk];
        let res = inf.step(input.get(pos..).unwrap_or(&[]), &mut buf);
        if let Ok((c, p, _)) = &res {
            pos += c;
            out.extend_from_slice(&buf[..*p]);
        }
        let stop = matches!(res, Ok((_, _, InflateStatus::Done)) | Err(_));
        calls.push(res);
        if stop {
            break;
        }
    }
    (calls, out)
}

#[test]
fn dictionary_with_one_byte_output() {
    let dict = seeded_bytes(0xD1C7, 400);
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_match(&mut w, 100, 300); // reaches into the dictionary
    emit_match(&mut w, 258, 400); // the oldest dictionary byte, then output
    emit_literal(&mut w, b'Z' as u16);
    emit_match(&mut w, 50, 450);
    emit_end_of_block(&mut w);
    let input = w.finish();

    let mut history = dict.clone();
    let mut expected = Vec::new();
    for (len, dist) in [(100usize, 300usize), (258, 400)] {
        let m = model_copy(&history, dist, len);
        history.extend_from_slice(&m);
        expected.extend_from_slice(&m);
    }
    history.push(b'Z');
    expected.push(b'Z');
    expected.extend_from_slice(&model_copy(&history, 450, 50));

    for out_chunk in [1usize, 2, 1000] {
        let (fast, fast_out) = drive(DecodePath::Fast, Some(&dict), &input, out_chunk);
        let (reference, ref_out) = drive(DecodePath::Reference, Some(&dict), &input, out_chunk);
        assert_eq!(fast_out, expected, "out_chunk {out_chunk}");
        assert_eq!(ref_out, expected, "out_chunk {out_chunk}");
        assert_eq!(fast, reference, "out_chunk {out_chunk}: per-call results");
    }
}

#[test]
fn match_resumes_across_calls_after_window_refresh() {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in b"abcdefghij" {
        emit_literal(&mut w, b as u16);
    }
    emit_match(&mut w, 258, 10); // overlapping
    emit_match(&mut w, 200, 3); // overlapping, tiny distance
    emit_match(&mut w, 258, 300); // reaches back across earlier calls
    emit_end_of_block(&mut w);
    let input = w.finish();

    let mut history = b"abcdefghij".to_vec();
    for (len, dist) in [(258usize, 10usize), (200, 3), (258, 300)] {
        let m = model_copy(&history, dist, len);
        history.extend_from_slice(&m);
    }

    for out_chunk in [1usize, 7, 13, 100, 257, 258, 259] {
        let (fast, fast_out) = drive(DecodePath::Fast, None, &input, out_chunk);
        let (reference, ref_out) = drive(DecodePath::Reference, None, &input, out_chunk);
        assert_eq!(fast_out, history, "out_chunk {out_chunk}");
        assert_eq!(ref_out, history, "out_chunk {out_chunk}");
        assert_eq!(fast, reference, "out_chunk {out_chunk}: per-call results");
    }
}

#[test]
fn decompressor_size_not_grown() {
    // Measured before P3-01 (default features): 33688 bytes. The history
    // change adds no field to the decoder, so allow 1 KiB of slack.
    let before = 33_688usize;
    assert!(
        std::mem::size_of::<crate::Decompressor>() <= before + 1024,
        "Decompressor is {} bytes",
        std::mem::size_of::<crate::Decompressor>()
    );
}

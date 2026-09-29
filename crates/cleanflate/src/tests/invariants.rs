//! Chunking-invariance and no-panic tests (SPEC S3.3, S1).

use crate::options::{Format, Options};
use crate::stream::{inflate, Decompressor, Status};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{
    emit_dynamic_block, huffman_lengths_from_freqs_u16, literal_cl_program, DynBlock, PayloadSym,
};
use crate::tests::inflate_fixed::{
    emit_block_header, emit_end_of_block, emit_literal, emit_match, length_symbol_and_extra,
};

/// Test-local xorshift64 generator (separate from the oracle's PRNG,
/// which cleanflate cannot depend on).
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        XorShift(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A value in `1..=max` (max >= 1).
    fn range1(&mut self, max: usize) -> usize {
        if max <= 1 {
            return 1;
        }
        1 + (self.next_u64() as usize % max)
    }
}

fn stored_block(bfinal: bool, data: &[u8]) -> Vec<u8> {
    let mut out = vec![bfinal as u8];
    let len = data.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn fixed_literals(bytes: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in bytes {
        emit_literal(&mut w, b as u16);
    }
    emit_end_of_block(&mut w);
    w.finish()
}

fn fixture_streams() -> Vec<Vec<u8>> {
    let mut streams = Vec::new();

    // 1. A single stored block.
    streams.push(stored_block(true, b"hello, world"));

    // 2. Two stored blocks, second final.
    let mut s2 = stored_block(false, b"first-");
    s2.extend_from_slice(&stored_block(true, b"second"));
    streams.push(s2);

    // 3. Fixed-Huffman literals only.
    streams.push(fixed_literals(b"the quick brown fox"));

    // 4. Fixed block, overlapping match distance 1 (run-length).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_literal(&mut w, b'q' as u16);
        emit_match(&mut w, 20, 1);
        emit_end_of_block(&mut w);
        streams.push(w.finish());
    }

    // 5. Fixed block, overlapping match distance 3.
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        for &b in b"abc" {
            emit_literal(&mut w, b as u16);
        }
        emit_match(&mut w, 15, 3);
        emit_end_of_block(&mut w);
        streams.push(w.finish());
    }

    // 6. Stored block then fixed block.
    {
        let mut s = stored_block(false, b"ab");
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_literal(&mut w, b'c' as u16);
        emit_match(&mut w, 4, 3);
        emit_end_of_block(&mut w);
        s.extend_from_slice(&w.finish());
        streams.push(s);
    }

    // 7. Length 258 via code 284 extra 31.
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_literal(&mut w, b'a' as u16);
        emit_match(&mut w, 258, 1);
        emit_end_of_block(&mut w);
        streams.push(w.finish());
    }

    // 8. Multiple literals and matches mixed together.
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        for &b in b"ab" {
            emit_literal(&mut w, b as u16);
        }
        emit_match(&mut w, 6, 2); // "ababab"
        for &b in b"XY" {
            emit_literal(&mut w, b as u16);
        }
        emit_match(&mut w, 3, 8);
        emit_end_of_block(&mut w);
        streams.push(w.finish());
    }

    // 9-12. Dynamic-Huffman blocks (RFC 1951 S3.2.7).
    streams.push(dynamic_literals(b"dynamic literals only"));
    streams.push(dynamic_with_match());
    streams.push(dynamic_single_distance_code());
    streams.push(dynamic_no_distance_codes());

    streams
}

fn small_litlen_lengths(used_symbols: &[u16], hlit: usize) -> Vec<u8> {
    let mut symbols: Vec<u16> = used_symbols.to_vec();
    if !symbols.contains(&256) {
        symbols.push(256);
    }
    symbols.sort_unstable();
    symbols.dedup();
    let freqs: Vec<(u16, u32)> = symbols.iter().map(|&s| (s, 1u32)).collect();
    let lens = huffman_lengths_from_freqs_u16(&freqs);
    let mut lengths = vec![0u8; hlit];
    for (sym, len) in lens {
        if let Some(slot) = lengths.get_mut(sym as usize) {
            *slot = len;
        }
    }
    lengths
}

fn dynamic_literals(bytes: &[u8]) -> Vec<u8> {
    let hlit = 257;
    let used: Vec<u16> = bytes.iter().map(|&b| b as u16).collect();
    let litlen_lengths = small_litlen_lengths(&used, hlit);
    let dist_lengths = vec![0u8]; // no distance codes

    let mut combined = litlen_lengths.clone();
    combined.extend_from_slice(&dist_lengths);
    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined),
        litlen_lengths,
        dist_lengths,
    };
    let payload: Vec<PayloadSym> = bytes.iter().map(|&b| PayloadSym::Literal(b)).collect();

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    w.finish()
}

fn dynamic_with_match() -> Vec<u8> {
    let hlit = 260;
    let (lsym, _, _) = length_symbol_and_extra(5);
    let litlen_lengths = small_litlen_lengths(&[b'q' as u16, lsym], hlit);
    let dist_lengths = vec![1u8]; // single distance code

    let mut combined = litlen_lengths.clone();
    combined.extend_from_slice(&dist_lengths);
    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'q'),
        PayloadSym::Match { len: 5, dist: 1 },
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    w.finish()
}

fn dynamic_single_distance_code() -> Vec<u8> {
    let hlit = 260;
    let (lsym, _, _) = length_symbol_and_extra(3);
    let litlen_lengths = small_litlen_lengths(&[b'x' as u16, b'y' as u16, lsym], hlit);
    let dist_lengths = vec![1u8];

    let mut combined = litlen_lengths.clone();
    combined.extend_from_slice(&dist_lengths);
    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'x'),
        PayloadSym::Literal(b'y'),
        PayloadSym::Match { len: 3, dist: 2 },
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    w.finish()
}

fn dynamic_no_distance_codes() -> Vec<u8> {
    let hlit = 257;
    let litlen_lengths = small_litlen_lengths(&[b'm' as u16, b'n' as u16, b'o' as u16], hlit);
    let dist_lengths = vec![0u8];

    let mut combined = litlen_lengths.clone();
    combined.extend_from_slice(&dist_lengths);
    let block = DynBlock {
        bfinal: true,
        cl_program: literal_cl_program(&combined),
        litlen_lengths,
        dist_lengths,
    };
    let payload = [
        PayloadSym::Literal(b'm'),
        PayloadSym::Literal(b'n'),
        PayloadSym::Literal(b'o'),
    ];

    let mut w = BitWriter::new();
    emit_dynamic_block(&mut w, &block, &payload);
    w.finish()
}

fn chunked_decode(stream: &[u8], in_chunk: usize, out_chunk: usize) -> Vec<u8> {
    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut buf = vec![0u8; out_chunk.max(1)];
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(guard < 1_000_000, "chunked_decode looped without progress");
        let end = (pos + in_chunk).min(stream.len());
        let input_slice = stream.get(pos..end).unwrap_or(&[]);
        let progress = d
            .decompress(input_slice, &mut buf)
            .expect("valid fixture stream");
        pos += progress.consumed;
        out.extend_from_slice(buf.get(..progress.produced).unwrap_or(&[]));
        match progress.status {
            Status::Done => break,
            Status::OutputFull => continue,
            Status::NeedsInput => {
                if progress.consumed == 0 && input_slice.is_empty() {
                    panic!("ran out of input before Done on a valid fixture stream");
                }
            }
        }
    }
    d.finish().expect("stream finished cleanly");
    out
}

fn chunked_decode_random(stream: &[u8], rng: &mut XorShift) -> Vec<u8> {
    let mut d = Decompressor::new(Format::Raw, Options::default());
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(
            guard < 1_000_000,
            "chunked_decode_random looped without progress"
        );
        let in_chunk = rng.range1(16);
        let out_chunk = rng.range1(16);
        let end = (pos + in_chunk).min(stream.len());
        let input_slice = stream.get(pos..end).unwrap_or(&[]);
        let mut buf = vec![0u8; out_chunk];
        let progress = d
            .decompress(input_slice, &mut buf)
            .expect("valid fixture stream");
        pos += progress.consumed;
        out.extend_from_slice(buf.get(..progress.produced).unwrap_or(&[]));
        match progress.status {
            Status::Done => break,
            Status::OutputFull => continue,
            Status::NeedsInput => {
                if progress.consumed == 0 && input_slice.is_empty() {
                    panic!("ran out of input before Done on a valid fixture stream");
                }
            }
        }
    }
    d.finish().expect("stream finished cleanly");
    out
}

#[test]
fn chunking_invariance_deterministic() {
    let streams = fixture_streams();
    assert!(streams.len() >= 8);

    for (idx, stream) in streams.iter().enumerate() {
        let expected =
            inflate(stream).unwrap_or_else(|e| panic!("stream {idx}: one-shot failed: {e:?}"));

        // All-1-byte input and output, deterministic.
        let got_1byte = chunked_decode(stream, 1, 1);
        assert_eq!(
            got_1byte, expected,
            "stream {idx}: 1-byte chunking mismatch"
        );

        // 200 randomized chunk schedules with a fixed seed per stream.
        let mut rng = XorShift::new(0x5EED_0000_0000_0001 ^ (idx as u64));
        for trial in 0..200 {
            let got = chunked_decode_random(stream, &mut rng);
            assert_eq!(
                got, expected,
                "stream {idx} trial {trial}: chunking mismatch"
            );
        }
    }
}

#[test]
fn garbage_never_panics() {
    let mut rng = XorShift::new(0x00BA_D5EE_DBAD_5EED_u64);
    for _ in 0..20_000u32 {
        let len = (rng.next_u64() as usize) % 513;
        let mut buf = vec![0u8; len];
        for byte in buf.iter_mut() {
            *byte = (rng.next_u64() & 0xFF) as u8;
        }

        // One-shot: any Result is fine, just must not panic.
        let _ = inflate(&buf);

        // 1-byte streaming: same requirement.
        let mut d = Decompressor::new(Format::Raw, Options::default());
        let mut out = [0u8; 1];
        let mut pos = 0usize;
        loop {
            let input_slice = buf.get(pos..(pos + 1).min(buf.len())).unwrap_or(&[]);
            let result = d.decompress(input_slice, &mut out);
            let progress = match result {
                Ok(p) => p,
                Err(_) => break,
            };
            pos += progress.consumed;
            if progress.status == Status::Done {
                break;
            }
            if progress.consumed == 0 && input_slice.is_empty() {
                break;
            }
        }
    }
}

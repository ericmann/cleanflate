use crate::options::{Format, Options};
use crate::stream::{decompress_with_stats, Decompressor, Status};
use crate::tests::bitwriter::BitWriter;
use crate::tests::dynbuilder::{emit_dynamic_block, literal_cl_program, DynBlock, PayloadSym};
use crate::tests::inflate_fixed::{emit_block_header, emit_end_of_block, emit_literal, emit_match};
use crate::Stats;

/// Decode `input` with the given chunking, returning output and stats.
fn run(format: Format, input: &[u8], in_chunk: usize) -> (Vec<u8>, Stats) {
    let mut d = Decompressor::new(format, Options::default());
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut buf = [0u8; 64];
    loop {
        let end = (pos + in_chunk).min(input.len());
        let p = d
            .decompress(input.get(pos..end).unwrap_or(&[]), &mut buf)
            .expect("decodes");
        pos += p.consumed;
        out.extend_from_slice(buf.get(..p.produced).unwrap_or(&[]));
        if p.status == Status::Done || (p.consumed == 0 && p.produced == 0 && pos >= input.len()) {
            break;
        }
    }
    d.finish().expect("finishes");
    let stats = d.stats();
    (out, stats)
}

fn stored_block(bfinal: bool, data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut v = vec![bfinal as u8];
    v.extend_from_slice(&len.to_le_bytes());
    v.extend_from_slice(&(!len).to_le_bytes());
    v.extend_from_slice(data);
    v
}

fn fixed_stream() -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for &b in b"abc" {
        emit_literal(&mut w, b as u16);
    }
    emit_match(&mut w, 3, 3); // non-overlapping
    emit_match(&mut w, 10, 1); // overlapping
    emit_match(&mut w, 258, 2); // overlapping, longest
    emit_end_of_block(&mut w);
    w.finish()
}

#[test]
fn stats_stored_block_counts() {
    let data = [7u8; 40];
    let (out, s) = run(Format::Raw, &stored_block(true, &data), 1000);
    assert_eq!(out, data);
    assert_eq!(s.blocks_stored, 1);
    assert_eq!(s.stored_bytes, 40);
    assert_eq!(s.output_bytes, 40);
    assert_eq!(s.window_bytes_written, 40);
    assert_eq!(s.blocks_fixed + s.blocks_dynamic, 0);
    assert_eq!(s.literals + s.matches, 0);
}

#[test]
fn stats_fixed_literals_and_matches() {
    let (out, s) = run(Format::Raw, &fixed_stream(), 1000);
    assert_eq!(out.len(), 3 + 3 + 10 + 258);
    assert_eq!(s.blocks_fixed, 1);
    assert_eq!(s.fixed_table_clones, 2);
    assert_eq!(s.literals, 3);
    assert_eq!(s.matches, 3);
    assert_eq!(s.match_bytes, 271);
    assert_eq!(s.overlap_matches, 2);
    // Lengths 3, 10, 258 -> buckets 0, 2, 6.
    assert_eq!(s.match_len_hist, [1, 0, 1, 0, 0, 0, 1]);
    // Distances 3, 1, 2 -> buckets 1, 0, 1.
    assert_eq!(s.match_dist_hist, [1, 2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(s.output_bytes, 274);
    assert_eq!(s.window_bytes_written, 274);
    assert!(s.step_calls >= 1);
    assert!(s.state_dispatches >= s.matches);
    assert!(s.refills >= 1);
    assert_eq!(s.table_builds, 0);
}

fn dynamic_block(bfinal: bool) -> (DynBlock, [PayloadSym; 1]) {
    let mut litlen = vec![0u8; 257];
    litlen[b'a' as usize] = 1;
    litlen[256] = 1;
    let dist = vec![0u8];
    let mut all = litlen.clone();
    all.extend_from_slice(&dist);
    (
        DynBlock {
            bfinal,
            cl_program: literal_cl_program(&all),
            litlen_lengths: litlen,
            dist_lengths: dist,
        },
        [PayloadSym::Literal(b'a')],
    )
}

#[test]
fn stats_dynamic_table_builds() {
    let mut w = BitWriter::new();
    let (b1, p1) = dynamic_block(false);
    emit_dynamic_block(&mut w, &b1, &p1);
    let (b2, p2) = dynamic_block(true);
    emit_dynamic_block(&mut w, &b2, &p2);
    let (out, s) = run(Format::Raw, &w.finish(), 1000);
    assert_eq!(out, b"aa");
    assert_eq!(s.blocks_dynamic, 2);
    assert_eq!(
        s.table_builds, 6,
        "code-length, litlen and distance per block"
    );
    assert!(s.table_entries_built > 0);
    assert_eq!(s.literals, 2);
}

#[test]
fn stats_symbol_decode_methods_sum() {
    for in_chunk in [1usize, 2, 3, 1000] {
        let (_, s) = run(Format::Raw, &fixed_stream(), in_chunk);
        let eob = s.blocks_fixed;
        assert_eq!(
            s.litlen_table_decodes + s.litlen_serial_decodes,
            s.literals + s.matches + eob,
            "in_chunk {in_chunk}"
        );
        assert_eq!(
            s.dist_table_decodes + s.dist_serial_decodes,
            s.matches,
            "in_chunk {in_chunk}"
        );
    }
    // Dynamic streams also decode code-length symbols.
    let mut w = BitWriter::new();
    let (b, p) = dynamic_block(true);
    emit_dynamic_block(&mut w, &b, &p);
    let (_, s) = run(Format::Raw, &w.finish(), 1);
    assert_eq!(s.litlen_table_decodes + s.litlen_serial_decodes, 2);
    assert_eq!(s.codelen_table_decodes + s.codelen_serial_decodes, 258);
    assert_eq!(s.codelen_serial_decodes, 0);
    // Every Fast-path symbol resolves through a lookup table, even when
    // one-byte input leaves few bits buffered (two-level tables, D-28).
    let (_, one) = run(Format::Raw, &fixed_stream(), 1);
    assert_eq!(one.litlen_serial_decodes + one.dist_serial_decodes, 0);
    assert!(one.litlen_table_decodes > 0);
}

#[test]
fn stats_accumulate_across_gzip_members() {
    let mut member = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03];
    member.extend_from_slice(&stored_block(true, b"abc"));
    // CRC-32 of "abc" and ISIZE 3, little endian.
    member.extend_from_slice(&[0xc2, 0x41, 0x24, 0x35, 0x03, 0x00, 0x00, 0x00]);
    let mut input = member.clone();
    input.extend_from_slice(&member);
    let (out, s) = run(Format::Gzip, &input, 5);
    assert_eq!(out, b"abcabc");
    assert_eq!(s.blocks_stored, 2);
    assert_eq!(s.stored_bytes, 6);
    assert_eq!(s.output_bytes, 6);
}

#[test]
fn stats_decompress_with_counts_resizes() {
    // Two stored blocks: the output (100000 bytes) is smaller than four
    // times the input, so the ratio hint sizes the buffer in one step.
    let zeros = vec![0u8; 50_000];
    let mut input = stored_block(false, &zeros);
    input.extend_from_slice(&stored_block(true, &zeros));
    let (decoded, s) =
        decompress_with_stats(Format::Raw, &input, &Options::default()).expect("decodes");
    assert_eq!(decoded.data.len(), 100_000);
    assert_eq!(s.output_bytes, 100_000);
    assert_eq!(s.oneshot_resizes, 1);
    assert_eq!(s.oneshot_bytes_allocated, 4 * input.len() as u64);

    // A highly compressible stream: 77401 bytes from a few hundred input
    // bytes, far above the hint, so the buffer grows geometrically from the
    // growth floor: 32768, 65536, 131072.
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    emit_literal(&mut w, b'a' as u16);
    for _ in 0..300 {
        emit_match(&mut w, 258, 1);
    }
    emit_end_of_block(&mut w);
    let input = w.finish();
    let (decoded, s) =
        decompress_with_stats(Format::Raw, &input, &Options::default()).expect("decodes");
    assert_eq!(decoded.data.len(), 1 + 300 * 258);
    assert_eq!(s.oneshot_resizes, 3);
    assert_eq!(s.oneshot_bytes_allocated, 131_072);
}

#[test]
fn reference_path_never_gives_back() {
    use crate::decode_path::DecodePath;
    let mut w = BitWriter::new();
    emit_block_header(&mut w, true, 0b01);
    for i in 0..96u16 {
        emit_literal(&mut w, 144 + i);
    }
    emit_end_of_block(&mut w);
    let literals = w.finish();
    let mut w = BitWriter::new();
    let (b1, p1) = dynamic_block(false);
    emit_dynamic_block(&mut w, &b1, &p1);
    let (b2, p2) = dynamic_block(true);
    emit_dynamic_block(&mut w, &b2, &p2);
    let dynamic = w.finish();
    for stream in [literals, fixed_stream(), dynamic] {
        for out_chunk in [1usize, 3] {
            let mut d =
                Decompressor::new_with_path(Format::Raw, Options::default(), DecodePath::Reference);
            let mut pos = 0usize;
            let mut buf = vec![0u8; out_chunk];
            for _ in 0..100_000 {
                let p = d
                    .decompress(stream.get(pos..).unwrap_or(&[]), &mut buf)
                    .expect("decodes");
                pos += p.consumed;
                if p.status == Status::Done {
                    break;
                }
            }
            d.finish().expect("finishes");
            assert_eq!(d.stats().given_back_bytes, 0, "chunk {out_chunk}");
        }
    }
}

//! Test-only builder for dynamic-Huffman (BTYPE=10) fixture streams
//! (RFC 1951 S3.2.7). Computes canonical codes for arbitrary length
//! arrays itself (an independent implementation of the RFC's own
//! construction algorithm, used only to build test inputs).

use std::collections::BTreeMap;

use crate::tables::CODE_LENGTH_ORDER;
use crate::tests::bitwriter::BitWriter;
use crate::tests::inflate_fixed::{
    distance_symbol_and_extra, emit_block_header, length_symbol_and_extra,
};

/// One instruction in the code-length-alphabet transmission sequence
/// (RFC 1951 S3.2.7).
#[derive(Debug, Clone, Copy)]
pub(crate) enum ClSymbol {
    /// Transmit code length `0` (literal code-length alphabet symbols
    /// 0..=15 double as "this code has length N").
    Lit(u8),
    /// Symbol 16: repeat the previous length `3 + extra` times, extra in
    /// 0..=3.
    Rep16(u8),
    /// Symbol 17: repeat length 0 `3 + extra` times, extra in 0..=7.
    Rep17(u8),
    /// Symbol 18: repeat length 0 `11 + extra` times, extra in 0..=127.
    Rep18(u8),
}

/// A payload symbol: either a literal byte or a length/distance match.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PayloadSym {
    Literal(u8),
    Match { len: u16, dist: u32 },
}

/// A complete dynamic block specification.
pub(crate) struct DynBlock {
    pub(crate) bfinal: bool,
    pub(crate) litlen_lengths: Vec<u8>,
    pub(crate) dist_lengths: Vec<u8>,
    pub(crate) cl_program: Vec<ClSymbol>,
}

/// Turn a plain lengths array into a code-length program with no repeat
/// codes (transmit every length literally).
pub(crate) fn literal_cl_program(lengths: &[u8]) -> Vec<ClSymbol> {
    lengths.iter().map(|&l| ClSymbol::Lit(l)).collect()
}

/// RFC 1951 S3.2.2's canonical-code construction algorithm: given
/// per-symbol code lengths, return each symbol's `(code, bit_length)`.
pub(crate) fn canonical_codes(lengths: &[u8]) -> Vec<Option<(u32, u32)>> {
    let mut bl_count = [0u32; 16];
    for &l in lengths {
        if l > 0 {
            *bl_count.get_mut(l as usize).expect("length <= 15") += 1;
        }
    }
    let mut next_code = [0u32; 16];
    let mut code = 0u32;
    for bits in 1..=15usize {
        code = (code + *bl_count.get(bits - 1).expect("in range")) << 1;
        *next_code.get_mut(bits).expect("in range") = code;
    }
    let mut result = vec![None; lengths.len()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l > 0 {
            let slot = next_code.get_mut(l as usize).expect("in range");
            *result.get_mut(sym).expect("in range") = Some((*slot, l as u32));
            *slot += 1;
        }
    }
    result
}

/// Standard (textbook) Huffman code-length assignment from symbol
/// frequencies: repeatedly merge the two lowest-frequency nodes. Used
/// only to build a valid Huffman table for test fixtures. Guarantees a
/// *complete* prefix code (Kraft's inequality holds with equality) for
/// any `freqs.len() >= 2`, which is what real canonical construction
/// requires -- a length assignment that merely looks "balanced" (e.g.
/// every symbol at the same depth) is not complete unless the symbol
/// count is a power of two.
pub(crate) fn huffman_lengths_from_freqs_u16(freqs: &[(u16, u32)]) -> BTreeMap<u16, u8> {
    if freqs.is_empty() {
        return BTreeMap::new();
    }
    if freqs.len() == 1 {
        let mut m = BTreeMap::new();
        m.insert(freqs[0].0, 1u8);
        return m;
    }

    struct Node {
        freq: u64,
        symbols: Vec<u16>,
    }

    let mut nodes: Vec<Node> = freqs
        .iter()
        .map(|&(s, f)| Node {
            freq: f as u64,
            symbols: vec![s],
        })
        .collect();
    let mut lengths: BTreeMap<u16, u8> = freqs.iter().map(|&(s, _)| (s, 0u8)).collect();

    while nodes.len() > 1 {
        nodes.sort_by_key(|n| n.freq);
        let a = nodes.remove(0);
        let b = nodes.remove(0);
        for &s in a.symbols.iter().chain(b.symbols.iter()) {
            if let Some(l) = lengths.get_mut(&s) {
                *l += 1;
            }
        }
        let merged_symbols: Vec<u16> = a.symbols.into_iter().chain(b.symbols).collect();
        nodes.push(Node {
            freq: a.freq + b.freq,
            symbols: merged_symbols,
        });
    }

    lengths
}

fn huffman_lengths_from_freqs(freqs: &[(u8, u32)]) -> BTreeMap<u8, u8> {
    let freqs16: Vec<(u16, u32)> = freqs.iter().map(|&(s, f)| (s as u16, f)).collect();
    huffman_lengths_from_freqs_u16(&freqs16)
        .into_iter()
        .map(|(s, l)| (s as u8, l))
        .collect()
}

/// Emit a complete, valid dynamic-Huffman block (header, code-length
/// table, code lengths, and `payload`), followed by end-of-block.
pub(crate) fn emit_dynamic_block(w: &mut BitWriter, block: &DynBlock, payload: &[PayloadSym]) {
    let hlit = block.litlen_lengths.len();
    let hdist = block.dist_lengths.len();
    assert!((257..=286).contains(&hlit), "hlit out of range: {hlit}");
    assert!((1..=32).contains(&hdist), "hdist out of range: {hdist}");

    let mut freq_map: BTreeMap<u8, u32> = BTreeMap::new();
    for instr in &block.cl_program {
        let sym: u8 = match instr {
            ClSymbol::Lit(v) => *v,
            ClSymbol::Rep16(_) => 16,
            ClSymbol::Rep17(_) => 17,
            ClSymbol::Rep18(_) => 18,
        };
        *freq_map.entry(sym).or_insert(0) += 1;
    }
    let freqs: Vec<(u8, u32)> = freq_map.into_iter().collect();
    let cl_len_map = huffman_lengths_from_freqs(&freqs);
    let mut cl_lengths = [0u8; 19];
    for (&sym, &len) in &cl_len_map {
        if let Some(slot) = cl_lengths.get_mut(sym as usize) {
            *slot = len;
        }
    }

    let mut hclen_count = 4usize;
    for (i, &sym) in CODE_LENGTH_ORDER.iter().enumerate() {
        if *cl_lengths.get(sym as usize).unwrap_or(&0) != 0 {
            hclen_count = hclen_count.max(i + 1);
        }
    }

    emit_block_header(w, block.bfinal, 0b10);
    let hlit_field = (hlit - 257) as u32;
    let hdist_field = (hdist - 1) as u32;
    let hclen_field = (hclen_count - 4) as u32;
    w.write_bits(hlit_field | (hdist_field << 5) | (hclen_field << 10), 14);

    for &sym in CODE_LENGTH_ORDER.iter().take(hclen_count) {
        w.write_bits(*cl_lengths.get(sym as usize).unwrap_or(&0) as u32, 3);
    }

    let cl_codes = canonical_codes(&cl_lengths);
    for instr in &block.cl_program {
        match instr {
            ClSymbol::Lit(v) => {
                let (code, len) = cl_codes
                    .get(*v as usize)
                    .copied()
                    .flatten()
                    .expect("used symbol has a code");
                w.write_code(code, len);
            }
            ClSymbol::Rep16(extra) => {
                let (code, len) = cl_codes
                    .get(16)
                    .copied()
                    .flatten()
                    .expect("code 16 has a code");
                w.write_code(code, len);
                w.write_bits(*extra as u32, 2);
            }
            ClSymbol::Rep17(extra) => {
                let (code, len) = cl_codes
                    .get(17)
                    .copied()
                    .flatten()
                    .expect("code 17 has a code");
                w.write_code(code, len);
                w.write_bits(*extra as u32, 3);
            }
            ClSymbol::Rep18(extra) => {
                let (code, len) = cl_codes
                    .get(18)
                    .copied()
                    .flatten()
                    .expect("code 18 has a code");
                w.write_code(code, len);
                w.write_bits(*extra as u32, 7);
            }
        }
    }

    let litlen_codes = canonical_codes(&block.litlen_lengths);
    let dist_codes = canonical_codes(&block.dist_lengths);

    let emit_litlen = |w: &mut BitWriter, sym: u16| {
        let (code, len) = litlen_codes
            .get(sym as usize)
            .copied()
            .flatten()
            .unwrap_or_else(|| panic!("litlen symbol {sym} has no code"));
        w.write_code(code, len);
    };

    for sym in payload {
        match *sym {
            PayloadSym::Literal(b) => emit_litlen(w, b as u16),
            PayloadSym::Match { len, dist } => {
                let (lsym, lextra_bits, lextra_val) = length_symbol_and_extra(len);
                emit_litlen(w, lsym);
                w.write_bits(lextra_val, lextra_bits);
                let (dsym, dextra_bits, dextra_val) = distance_symbol_and_extra(dist);
                match dist_codes.get(dsym as usize).copied().flatten() {
                    Some((dcode, dlen)) => {
                        w.write_code(dcode, dlen);
                        w.write_bits(dextra_val, dextra_bits);
                    }
                    None => {
                        // No code exists for this distance symbol (e.g.
                        // a deliberately empty/all-invalid distance
                        // table, for a test that expects decode to fail
                        // here). Supply enough filler bits (15) for a
                        // full Huffman::decode walk to complete and
                        // observe Invalid; their value does not matter.
                        w.write_bits(0, 15);
                    }
                }
            }
        }
    }

    // If symbol 256 has no code (a deliberately malformed fixture, e.g.
    // for a MissingEndOfBlock test), there is nothing valid to emit;
    // decoding is expected to fail before an end-of-block code would
    // ever be needed.
    if litlen_codes.get(256).copied().flatten().is_some() {
        emit_litlen(w, 256);
    }
}

/// Emit just a dynamic block's header (block header + HLIT/HDIST/HCLEN +
/// raw code-length lengths), with no validity guarantees -- used to
/// build malformed fixtures (e.g. a deliberately over-subscribed
/// code-length table).
pub(crate) fn emit_dynamic_header_raw(
    w: &mut BitWriter,
    bfinal: bool,
    hlit: usize,
    hdist: usize,
    cl_lengths: &[u8; 19],
) {
    emit_block_header(w, bfinal, 0b10);
    let mut hclen_count = 4usize;
    for (i, &sym) in CODE_LENGTH_ORDER.iter().enumerate() {
        if *cl_lengths.get(sym as usize).unwrap_or(&0) != 0 {
            hclen_count = hclen_count.max(i + 1);
        }
    }
    let hlit_field = (hlit.max(257) - 257) as u32;
    let hdist_field = (hdist.max(1) - 1) as u32;
    let hclen_field = (hclen_count - 4) as u32;
    w.write_bits(hlit_field | (hdist_field << 5) | (hclen_field << 10), 14);
    for &sym in CODE_LENGTH_ORDER.iter().take(hclen_count) {
        w.write_bits(*cl_lengths.get(sym as usize).unwrap_or(&0) as u32, 3);
    }
}

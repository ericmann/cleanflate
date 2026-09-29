//! Hand-built raw-DEFLATE malformed corpus (SPEC S4: "hand-built from RFC
//! rules, each file records its expected error variant"). This is a
//! fully independent DEFLATE-stream builder: it does not import
//! `cleanflate`'s test helpers (which are private to that crate and
//! `#[cfg(test)]`-only in any case) or any of `cleanflate`'s internal
//! encoding logic, only the RFC 1951 text's own constant tables and
//! construction algorithm, so the two sides of every conformance check
//! stay independent.

use std::collections::BTreeMap;

use crate::ffi;

/// LSB-first resumable-style bit writer (RFC 1951 S3.1.1), the inverse
/// of the decoder's bit-packing rules.
pub struct BitWriter {
    buffer: u64,
    count: u32,
    bytes: Vec<u8>,
}

impl BitWriter {
    pub fn new() -> Self {
        BitWriter {
            buffer: 0,
            count: 0,
            bytes: Vec::new(),
        }
    }

    /// Append the low `n` bits of `value`, LSB-first.
    pub fn write_bits(&mut self, value: u32, n: u32) {
        for i in 0..n {
            let bit = (value >> i) & 1;
            self.buffer |= (bit as u64) << self.count;
            self.count += 1;
            if self.count == 8 {
                self.bytes.push((self.buffer & 0xFF) as u8);
                self.buffer >>= 8;
                self.count = 0;
            }
        }
    }

    /// Append a Huffman code, MSB-first (RFC 1951 S3.1.1).
    pub fn write_code(&mut self, code: u32, len: u32) {
        for i in (0..len).rev() {
            let bit = (code >> i) & 1;
            self.write_bits(bit, 1);
        }
    }

    /// Pad the current partial byte with zero bits.
    pub fn align(&mut self) {
        if self.count > 0 {
            self.bytes.push((self.buffer & 0xFF) as u8);
            self.buffer = 0;
            self.count = 0;
        }
    }

    /// Flush any partial byte and return the accumulated bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.align();
        self.bytes
    }

    /// Bytes written so far, not including any unflushed partial byte.
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

impl Default for BitWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// RFC 1951 S3.2.6: fixed literal/length code lengths.
pub fn fixed_litlen_lengths() -> Vec<u8> {
    let mut lengths = vec![0u8; 288];
    for (i, slot) in lengths.iter_mut().enumerate() {
        *slot = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            280..=287 => 8,
            _ => 0,
        };
    }
    lengths
}

/// RFC 1951 S3.2.6: fixed distance code lengths (all 5 bits).
pub fn fixed_dist_lengths() -> Vec<u8> {
    vec![5u8; 32]
}

/// RFC 1951 S3.2.5: length code base/extra-bits, indexed by `code - 257`.
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// RFC 1951 S3.2.5: distance code base/extra-bits, indexed by `code`.
const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// RFC 1951 S3.2.7: order in which HCLEN code-length lengths are sent.
const CODE_LENGTH_ORDER: [u8; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Given a length, return its (litlen code symbol, extra bit count, extra
/// value) per the RFC 1951 S3.2.5 table.
pub fn length_symbol_and_extra(length: u16) -> (u16, u32, u32) {
    for (i, &base) in LENGTH_BASE.iter().enumerate() {
        let extra = LENGTH_EXTRA[i];
        let max = base + ((1u32 << extra) - 1) as u16;
        if length >= base && length <= max {
            return (257 + i as u16, extra, (length - base) as u32);
        }
    }
    panic!("length {length} out of range");
}

/// Given a distance, return its (distance code symbol, extra bit count,
/// extra value) per the RFC 1951 S3.2.5 table.
pub fn distance_symbol_and_extra(dist: u32) -> (u16, u32, u32) {
    for (i, &base) in DIST_BASE.iter().enumerate() {
        let extra = DIST_EXTRA[i];
        let max = base + (1u32 << extra) - 1;
        if dist >= base && dist <= max {
            return (i as u16, extra, dist - base);
        }
    }
    panic!("distance {dist} out of range");
}

/// RFC 1951 S3.2.2's canonical-code construction algorithm.
pub fn canonical_codes(lengths: &[u8]) -> Vec<Option<(u32, u32)>> {
    let mut bl_count = [0u32; 16];
    for &l in lengths {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }
    let mut next_code = [0u32; 16];
    let mut code = 0u32;
    for bits in 1..=15usize {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    let mut result = vec![None; lengths.len()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l > 0 {
            result[sym] = Some((next_code[l as usize], l as u32));
            next_code[l as usize] += 1;
        }
    }
    result
}

/// Textbook Huffman code-length assignment from symbol frequencies
/// (repeatedly merge the two lowest-frequency nodes), used to build a
/// valid code-length-alphabet table for the fixtures below. Guarantees a
/// complete prefix code for any `freqs.len() >= 2`.
fn huffman_lengths_from_freqs(freqs: &[(u16, u32)]) -> BTreeMap<u16, u8> {
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
        let merged: Vec<u16> = a.symbols.into_iter().chain(b.symbols).collect();
        nodes.push(Node {
            freq: a.freq + b.freq,
            symbols: merged,
        });
    }
    lengths
}

/// Build a valid, complete length assignment (equal-frequency Huffman
/// construction) for `used_symbols` plus symbol 256 (end-of-block),
/// padded to `hlit` entries.
pub fn small_litlen_lengths(used_symbols: &[u16], hlit: usize) -> Vec<u8> {
    let mut symbols: Vec<u16> = used_symbols.to_vec();
    if !symbols.contains(&256) {
        symbols.push(256);
    }
    symbols.sort_unstable();
    symbols.dedup();
    let freqs: Vec<(u16, u32)> = symbols.iter().map(|&s| (s, 1u32)).collect();
    let lens = huffman_lengths_from_freqs(&freqs);
    let mut lengths = vec![0u8; hlit];
    for (sym, len) in lens {
        if let Some(slot) = lengths.get_mut(sym as usize) {
            *slot = len;
        }
    }
    lengths
}

pub fn emit_block_header(w: &mut BitWriter, bfinal: bool, btype: u32) {
    let value = (bfinal as u32) | (btype << 1);
    w.write_bits(value, 3);
}

/// A full, valid stored block (RFC 1951 S3.2.4).
pub fn stored_block(bfinal: bool, data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    emit_block_header(&mut w, bfinal, 0b00);
    w.align();
    let len = data.len() as u16;
    w.write_bits(len as u32, 16);
    w.write_bits((!len) as u32, 16);
    let mut bytes = w.finish();
    bytes.extend_from_slice(data);
    bytes
}

fn fixed_litlen_code(symbol: u16) -> (u32, u32) {
    match symbol {
        0..=143 => (0x30 + symbol as u32, 8),
        144..=255 => (0x190 + (symbol - 144) as u32, 9),
        256..=279 => (symbol as u32 - 256, 7),
        280..=287 => (0xC0 + (symbol - 280) as u32, 8),
        _ => panic!("symbol out of range"),
    }
}

pub fn emit_fixed_literal(w: &mut BitWriter, symbol: u16) {
    let (code, len) = fixed_litlen_code(symbol);
    w.write_code(code, len);
}

pub fn emit_fixed_end_of_block(w: &mut BitWriter) {
    emit_fixed_literal(w, 256);
}

pub fn emit_fixed_match(w: &mut BitWriter, length: u16, dist: u32) {
    let (lsym, lextra_bits, lextra_val) = length_symbol_and_extra(length);
    emit_fixed_literal(w, lsym);
    w.write_bits(lextra_val, lextra_bits);
    let (dsym, dextra_bits, dextra_val) = distance_symbol_and_extra(dist);
    w.write_code(dsym as u32, 5); // fixed distance codes are always 5 bits
    w.write_bits(dextra_val, dextra_bits);
}

/// One instruction in the code-length-alphabet transmission sequence
/// (RFC 1951 S3.2.7).
#[derive(Clone, Copy)]
pub enum ClSymbol {
    Lit(u8),
    #[allow(dead_code)]
    Rep16(u8),
    #[allow(dead_code)]
    Rep17(u8),
    Rep18(u8),
}

#[derive(Clone, Copy)]
pub enum PayloadSym {
    Literal(u8),
    Match { len: u16, dist: u32 },
}

pub struct DynBlock {
    pub bfinal: bool,
    pub litlen_lengths: Vec<u8>,
    pub dist_lengths: Vec<u8>,
    pub cl_program: Vec<ClSymbol>,
}

pub fn literal_cl_program(lengths: &[u8]) -> Vec<ClSymbol> {
    lengths.iter().map(|&l| ClSymbol::Lit(l)).collect()
}

pub fn combined_lengths(litlen: &[u8], dist: &[u8]) -> Vec<u8> {
    let mut v = litlen.to_vec();
    v.extend_from_slice(dist);
    v
}

/// Emit a complete, valid dynamic-Huffman block plus `payload` and an
/// automatic end-of-block (skipped if symbol 256 has no code, for
/// deliberately malformed fixtures that never need a real EOB).
pub fn emit_dynamic_block(w: &mut BitWriter, block: &DynBlock, payload: &[PayloadSym]) {
    let hlit = block.litlen_lengths.len();
    let hdist = block.dist_lengths.len();

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
    let freqs16: Vec<(u16, u32)> = freq_map.into_iter().map(|(s, f)| (s as u16, f)).collect();
    let cl_len_map = huffman_lengths_from_freqs(&freqs16);
    let mut cl_lengths = [0u8; 19];
    for (sym, len) in &cl_len_map {
        if let Some(slot) = cl_lengths.get_mut(*sym as usize) {
            *slot = *len;
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
                let (code, len) = cl_codes[*v as usize].expect("used symbol has a code");
                w.write_code(code, len);
            }
            ClSymbol::Rep16(extra) => {
                let (code, len) = cl_codes[16].expect("code 16 has a code");
                w.write_code(code, len);
                w.write_bits(*extra as u32, 2);
            }
            ClSymbol::Rep17(extra) => {
                let (code, len) = cl_codes[17].expect("code 17 has a code");
                w.write_code(code, len);
                w.write_bits(*extra as u32, 3);
            }
            ClSymbol::Rep18(extra) => {
                let (code, len) = cl_codes[18].expect("code 18 has a code");
                w.write_code(code, len);
                w.write_bits(*extra as u32, 7);
            }
        }
    }

    let litlen_codes = canonical_codes(&block.litlen_lengths);
    let dist_codes = canonical_codes(&block.dist_lengths);
    let emit_litlen = |w: &mut BitWriter, sym: u16| {
        let (code, len) =
            litlen_codes[sym as usize].unwrap_or_else(|| panic!("litlen symbol {sym} has no code"));
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
                        // No code exists for this distance symbol (a
                        // deliberately empty/all-invalid distance table
                        // fixture): supply enough filler bits (15) for a
                        // full bit-serial decode walk to complete.
                        w.write_bits(0, 15);
                    }
                }
            }
        }
    }

    if litlen_codes.get(256).copied().flatten().is_some() {
        emit_litlen(w, 256);
    }
}

/// Emit just a dynamic block's header (block header + HLIT/HDIST/HCLEN +
/// raw code-length lengths), with no validity guarantees -- used to
/// build malformed fixtures (e.g. a deliberately over-subscribed
/// code-length table).
pub fn emit_dynamic_header_raw(
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

/// Which RFC (and therefore which corpus subdirectory and decoder entry
/// point) a [`Case`] belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Raw RFC 1951 DEFLATE, decoded with `cleanflate::inflate`.
    Rfc1951,
    /// RFC 1950 zlib, decoded with `cleanflate::zlib_decompress`.
    Rfc1950,
    /// RFC 1952 gzip, decoded with `cleanflate::gzip_decompress`.
    Rfc1952,
}

impl Family {
    /// The corpus subdirectory this family's fixtures live under.
    pub fn dir_name(self) -> &'static str {
        match self {
            Family::Rfc1951 => "rfc1951",
            Family::Rfc1950 => "rfc1950",
            Family::Rfc1952 => "rfc1952",
        }
    }
}

/// One malformed (or deliberately valid) fixture.
pub struct Case {
    pub name: &'static str,
    pub bytes: Vec<u8>,
    pub expected: &'static str,
    pub rule: &'static str,
    /// Whether this case's file should also get a `sweep:` row asserting
    /// every strict prefix fails with `Truncated`.
    pub sweep: bool,
    /// Which RFC/decoder this case targets.
    pub family: Family,
}

/// Build every named raw-DEFLATE malformed (and control-valid) case.
pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // btype3: reserved block type (RFC 1951 S3.2.3).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b11);
        cases.push(Case {
            name: "btype3",
            bytes: w.finish(),
            expected: "BadBlockType",
            rule: "RFC 1951 S3.2.3: BTYPE 11 is reserved",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // stored_nlen_mismatch (RFC 1951 S3.2.4).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b00);
        w.align();
        w.write_bits(5, 16);
        w.write_bits(5, 16); // should be !5
        let mut bytes = w.finish();
        bytes.extend_from_slice(b"hello");
        cases.push(Case {
            name: "stored_nlen_mismatch",
            bytes,
            expected: "InvalidStoredLength",
            rule: "RFC 1951 S3.2.4: NLEN must be the one's complement of LEN",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // fixed_litlen_286 (RFC 1951 S3.2.6: 286/287 never occur in real data).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_fixed_literal(&mut w, 286);
        cases.push(Case {
            name: "fixed_litlen_286",
            bytes: w.finish(),
            expected: "InvalidSymbol",
            rule: "RFC 1951 S3.2.6: litlen codes 286/287 never occur in real data",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // fixed_dist_code_30 (RFC 1951 S3.2.6: distance codes 30/31 never occur).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_fixed_literal(&mut w, b'a' as u16);
        let (lsym, lextra_bits, lextra_val) = length_symbol_and_extra(3);
        emit_fixed_literal(&mut w, lsym);
        w.write_bits(lextra_val, lextra_bits);
        w.write_code(30, 5);
        cases.push(Case {
            name: "fixed_dist_code_30",
            bytes: w.finish(),
            expected: "InvalidSymbol",
            rule: "RFC 1951 S3.2.6: distance codes 30/31 never occur in real data",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // fixed_distance_at_start (RFC 1951 S3.2.5: distance cannot refer
    // past the beginning of the output stream).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_fixed_match(&mut w, 3, 1);
        cases.push(Case {
            name: "fixed_distance_at_start",
            bytes: w.finish(),
            expected: "InvalidDistance",
            rule: "RFC 1951 S3.2.5: a distance cannot refer past the start of output",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // fixed_distance_too_far.
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        for &b in b"abcd" {
            emit_fixed_literal(&mut w, b as u16);
        }
        emit_fixed_match(&mut w, 3, 5); // only 4 bytes available
        cases.push(Case {
            name: "fixed_distance_too_far",
            bytes: w.finish(),
            expected: "InvalidDistance",
            rule: "RFC 1951 S3.2.5: distance exceeds bytes produced so far",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // fixed_len284_extra31: OK (length 258 via code 284 extra 31).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        emit_fixed_literal(&mut w, b'a' as u16);
        emit_fixed_match(&mut w, 258, 1);
        emit_fixed_end_of_block(&mut w);
        cases.push(Case {
            name: "fixed_len284_extra31",
            bytes: w.finish(),
            expected: "OK",
            rule: "RFC 1951 S3.2.5: code 284 extra value 31 decodes to length 258",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // valid_stored_trailing_byte: TrailingData (default Reject).
    {
        let mut bytes = stored_block(true, b"hi");
        bytes.push(b'X');
        cases.push(Case {
            name: "valid_stored_trailing_byte",
            bytes,
            expected: "TrailingData",
            rule: "SPEC S3.3: trailing data is rejected by default",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_hlit_287 (RFC 1951 S3.2.7).
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b10);
        w.write_bits(30, 14); // hlit field 30 -> hlit = 287
        cases.push(Case {
            name: "dynamic_hlit_287",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.7: HLIT+257 must not exceed 286",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_codelength_oversubscribed.
    {
        let mut cl_lengths = [0u8; 19];
        cl_lengths[0] = 1;
        cl_lengths[1] = 1;
        cl_lengths[2] = 1;
        cl_lengths[3] = 1;
        let mut w = BitWriter::new();
        emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
        cases.push(Case {
            name: "dynamic_codelength_oversubscribed",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.2: code-length alphabet table is over-subscribed",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_codelength_incomplete.
    {
        let mut cl_lengths = [0u8; 19];
        cl_lengths[0] = 2; // a single length-2 code: incomplete (sum 0.25)
        let mut w = BitWriter::new();
        emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
        cases.push(Case {
            name: "dynamic_codelength_incomplete",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.2: code-length alphabet table is incomplete",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_litlen_oversubscribed: a valid CL table transmitting an
    // over-subscribed litlen length sequence (four symbols at length 1).
    {
        let hlit = 257;
        let hdist = 1;
        let mut litlen_lengths = vec![0u8; hlit];
        litlen_lengths[0] = 1;
        litlen_lengths[1] = 1;
        litlen_lengths[2] = 1;
        litlen_lengths[3] = 1;
        let dist_lengths = vec![0u8; hdist];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &[]);
        cases.push(Case {
            name: "dynamic_litlen_oversubscribed",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.2: literal/length table is over-subscribed",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_litlen_incomplete: two symbols at length 2 (only using
    // half the length-2 codes; not the distance-only exception).
    {
        let hlit = 257;
        let hdist = 1;
        let mut litlen_lengths = vec![0u8; hlit];
        litlen_lengths[0] = 2;
        litlen_lengths[1] = 2;
        let dist_lengths = vec![0u8; hdist];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &[]);
        cases.push(Case {
            name: "dynamic_litlen_incomplete",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.2: literal/length table is incomplete (no exception applies)",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_dist_oversubscribed: four distance symbols at length 1.
    {
        let hlit = 257;
        let litlen_lengths = small_litlen_lengths(&[b'a' as u16], hlit);
        let dist_lengths = vec![1u8, 1, 1, 1];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &[]);
        cases.push(Case {
            name: "dynamic_dist_oversubscribed",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.2: distance table is over-subscribed",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_dist_incomplete_two_codes: 2 distance codes, incomplete
    // (lengths [1,3]) -- neither exception (single code / all zero)
    // applies to two codes.
    {
        let hlit = 257;
        let litlen_lengths = small_litlen_lengths(&[b'a' as u16], hlit);
        let dist_lengths = vec![1u8, 3];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &[]);
        cases.push(Case {
            name: "dynamic_dist_incomplete_two_codes",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.7: two distance codes still must form a complete table",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_repeat16_first: code 16 with no previous length.
    {
        let mut cl_lengths = [0u8; 19];
        cl_lengths[16] = 1;
        cl_lengths[0] = 1;
        let mut w = BitWriter::new();
        emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
        let codes = canonical_codes(&cl_lengths);
        let (code, len) = codes[16].expect("symbol 16 has a code");
        w.write_code(code, len);
        cases.push(Case {
            name: "dynamic_repeat16_first",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.7: repeat code 16 with no previous length to repeat",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_repeat_overflow: 200 zeros then a repeat-18 of 138 (258 needed total).
    {
        let mut cl_lengths = [0u8; 19];
        cl_lengths[0] = 1;
        cl_lengths[18] = 1;
        let mut w = BitWriter::new();
        emit_dynamic_header_raw(&mut w, true, 257, 1, &cl_lengths);
        let codes = canonical_codes(&cl_lengths);
        let (zero_code, zero_len) = codes[0].expect("symbol 0 has a code");
        let (rep_code, rep_len) = codes[18].expect("symbol 18 has a code");
        for _ in 0..200 {
            w.write_code(zero_code, zero_len);
        }
        w.write_code(rep_code, rep_len);
        w.write_bits(127, 7); // extra=127 -> repeat count 138
        cases.push(Case {
            name: "dynamic_repeat_overflow",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.7: a repeat run must not exceed HLIT+HDIST lengths",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_missing_eob: symbol 256 has length 0.
    {
        let hlit = 257;
        let mut litlen_lengths = small_litlen_lengths(&[b'a' as u16, b'b' as u16], hlit);
        litlen_lengths[256] = 0;
        let dist_lengths = vec![0u8];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &[PayloadSym::Literal(b'a')]);
        cases.push(Case {
            name: "dynamic_missing_eob",
            bytes: w.finish(),
            expected: "InvalidHuffmanTable",
            rule: "RFC 1951 S3.2.3: every block needs an end-of-block code",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_no_dist_then_length: no distance codes, then a length code.
    {
        let hlit = 260;
        let (lsym, _, _) = length_symbol_and_extra(3);
        let litlen_lengths = small_litlen_lengths(&[b'a' as u16, lsym], hlit);
        let dist_lengths = vec![0u8];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let payload = [
            PayloadSym::Literal(b'a'),
            PayloadSym::Match { len: 3, dist: 1 },
        ];
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &payload);
        cases.push(Case {
            name: "dynamic_no_dist_then_length",
            bytes: w.finish(),
            expected: "InvalidSymbol",
            rule: "RFC 1951 S3.2.7: a length code with an empty distance table has no valid decode",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_single_dist_code: OK.
    {
        let hlit = 260;
        let (lsym, _, _) = length_symbol_and_extra(3);
        let litlen_lengths = small_litlen_lengths(&[b'a' as u16, lsym], hlit);
        let dist_lengths = vec![1u8];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let payload = [
            PayloadSym::Literal(b'a'),
            PayloadSym::Match { len: 3, dist: 1 },
        ];
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &payload);
        cases.push(Case {
            name: "dynamic_single_dist_code",
            bytes: w.finish(),
            expected: "OK",
            rule: "RFC 1951 S3.2.7: a single distance code (length 1) is valid",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // dynamic_no_dist_literals: OK.
    {
        let hlit = 257;
        let litlen_lengths = small_litlen_lengths(&[b'a' as u16, b'b' as u16], hlit);
        let dist_lengths = vec![0u8];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let payload = [PayloadSym::Literal(b'a'), PayloadSym::Literal(b'b')];
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &payload);
        cases.push(Case {
            name: "dynamic_no_dist_literals",
            bytes: w.finish(),
            expected: "OK",
            rule: "RFC 1951 S3.2.7: no distance codes at all is valid for an all-literal block",
            sweep: false,
            family: Family::Rfc1951,
        });
    }

    // valid_small_stored: OK, and swept for Truncated on every prefix.
    {
        let bytes = stored_block(true, b"the quick fox");
        cases.push(Case {
            name: "valid_small_stored",
            bytes,
            expected: "OK",
            rule: "control case: a valid stored block",
            sweep: true,
            family: Family::Rfc1951,
        });
    }

    // valid_small_fixed: OK, swept.
    {
        let mut w = BitWriter::new();
        emit_block_header(&mut w, true, 0b01);
        for &b in b"abcabc" {
            emit_fixed_literal(&mut w, b as u16);
        }
        emit_fixed_match(&mut w, 5, 3);
        emit_fixed_end_of_block(&mut w);
        cases.push(Case {
            name: "valid_small_fixed",
            bytes: w.finish(),
            expected: "OK",
            rule: "control case: a valid fixed-Huffman block",
            sweep: true,
            family: Family::Rfc1951,
        });
    }

    // valid_small_dynamic: OK, swept.
    {
        let hlit = 260;
        let (lsym, _, _) = length_symbol_and_extra(4);
        let litlen_lengths = small_litlen_lengths(&[b'x' as u16, b'y' as u16, lsym], hlit);
        let dist_lengths = vec![1u8];
        let program = literal_cl_program(&combined_lengths(&litlen_lengths, &dist_lengths));
        let block = DynBlock {
            bfinal: true,
            litlen_lengths,
            dist_lengths,
            cl_program: program,
        };
        let payload = [
            PayloadSym::Literal(b'x'),
            PayloadSym::Literal(b'y'),
            PayloadSym::Match { len: 4, dist: 1 },
        ];
        let mut w = BitWriter::new();
        emit_dynamic_block(&mut w, &block, &payload);
        cases.push(Case {
            name: "valid_small_dynamic",
            bytes: w.finish(),
            expected: "OK",
            rule: "control case: a valid dynamic-Huffman block",
            sweep: true,
            family: Family::Rfc1951,
        });
    }

    cases
}

// --- RFC 1950 (zlib) and RFC 1952 (gzip) wrapper fixtures ------------
//
// These reuse the RFC-1951-only builders above for the raw DEFLATE body
// but build the zlib/gzip framing entirely independently, and take every
// checksum from `oracle::ffi` (C zlib), never from `cleanflate` itself
// (SPEC S4 / C16).

/// Find an FLG byte (given FDICT) making `(cmf*256 + flg) % 31 == 0`
/// (RFC 1950 S2.2), with FLEVEL left at 0.
fn zlib_flg_for(cmf: u8, fdict: bool) -> u8 {
    for fcheck in 0u8..32 {
        let flg = fcheck | ((fdict as u8) << 5);
        if (cmf as u32 * 256 + flg as u32) % 31 == 0 {
            return flg;
        }
    }
    unreachable!("some FCHECK value always satisfies the mod-31 constraint")
}

/// Wrap raw DEFLATE `body` (already decoding to `data`) in an RFC 1950
/// zlib header/trailer: CMF (CM=8, CINFO=7) / FLG / optional DICTID /
/// body / Adler-32 trailer, all big-endian per RFC 1950 S2.2/S2.3.
fn zlib_wrap(body: &[u8], data: &[u8], dict: Option<&[u8]>) -> Vec<u8> {
    let cmf = 0x78u8; // CM=8, CINFO=7
    let flg = zlib_flg_for(cmf, dict.is_some());
    let mut out = vec![cmf, flg];
    if let Some(d) = dict {
        out.extend_from_slice(&ffi::adler32(d).to_be_bytes());
    }
    out.extend_from_slice(body);
    out.extend_from_slice(&ffi::adler32(data).to_be_bytes());
    out
}

const GZ_FTEXT: u8 = 0x01;
const GZ_FHCRC: u8 = 0x02;
#[allow(dead_code)]
const GZ_FEXTRA: u8 = 0x04;
#[allow(dead_code)]
const GZ_FNAME: u8 = 0x08;
#[allow(dead_code)]
const GZ_FCOMMENT: u8 = 0x10;

/// Build one RFC 1952 gzip member: ID1/ID2/CM/FLG/MTIME/XFL/OS, no
/// optional fields, `body` (already decoding to `data`), then
/// CRC32+ISIZE (little-endian, RFC 1952 S2.3/S2.3.1).
#[allow(clippy::too_many_arguments)]
fn gzip_member_bytes(
    cm: u8,
    flg: u8,
    mtime: u32,
    xfl: u8,
    os: u8,
    body: &[u8],
    data: &[u8],
    hcrc_override: Option<u16>,
) -> Vec<u8> {
    let mut header = vec![0x1fu8, 0x8b, cm, flg];
    header.extend_from_slice(&mtime.to_le_bytes());
    header.push(xfl);
    header.push(os);
    if flg & GZ_FHCRC != 0 {
        let hcrc = hcrc_override.unwrap_or_else(|| (ffi::crc32(&header) & 0xFFFF) as u16);
        header.extend_from_slice(&hcrc.to_le_bytes());
    }
    let mut out = header;
    out.extend_from_slice(body);
    out.extend_from_slice(&ffi::crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

/// A minimal, well-formed gzip member wrapping `body`/`data` (FLG=0,
/// MTIME=0, XFL=0, OS=255).
fn gzip_minimal_member(body: &[u8], data: &[u8]) -> Vec<u8> {
    gzip_member_bytes(8, 0, 0, 0, 255, body, data, None)
}

/// Build every named RFC 1950 (zlib) malformed (and control-valid) case.
pub fn rfc1950_cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // bad_fcheck: flip a FCHECK bit so the mod-31 check fails.
    {
        let body = stored_block(true, b"hi");
        let mut bytes = zlib_wrap(&body, b"hi", None);
        bytes[1] ^= 0x01;
        cases.push(Case {
            name: "bad_fcheck",
            bytes,
            expected: "BadHeader",
            rule: "RFC 1950 S2.2: (CMF*256+FLG) must be a multiple of 31",
            sweep: false,
            family: Family::Rfc1950,
        });
    }

    // cm_7: CM=7 in the low nibble of CMF (not 8).
    {
        let cmf = 0x77u8; // CM=7, CINFO=7
        let flg = zlib_flg_for(cmf, false);
        cases.push(Case {
            name: "cm_7",
            bytes: vec![cmf, flg],
            expected: "BadHeader",
            rule: "RFC 1950 S2.2: CM must be 8 (deflate)",
            sweep: false,
            family: Family::Rfc1950,
        });
    }

    // cinfo_8: CINFO=8 (> 7) in the high nibble of CMF.
    {
        let cmf = 0x88u8; // CM=8, CINFO=8
        let flg = zlib_flg_for(cmf, false);
        cases.push(Case {
            name: "cinfo_8",
            bytes: vec![cmf, flg],
            expected: "BadHeader",
            rule: "RFC 1950 S2.2: CINFO must not exceed 7",
            sweep: false,
            family: Family::Rfc1950,
        });
    }

    // fdict_no_dictionary: FDICT set, but no dictionary is ever supplied.
    {
        let body = stored_block(true, b"hi");
        let bytes = zlib_wrap(&body, b"hi", Some(b"some dictionary"));
        cases.push(Case {
            name: "fdict_no_dictionary",
            bytes,
            expected: "PresetDictionaryRequired",
            rule: "RFC 1950 S2.2: FDICT set requires a preset dictionary",
            sweep: false,
            family: Family::Rfc1950,
        });
    }

    // adler_mismatch: corrupt the trailer.
    {
        let body = stored_block(true, b"hello");
        let mut bytes = zlib_wrap(&body, b"hello", None);
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        cases.push(Case {
            name: "adler_mismatch",
            bytes,
            expected: "ChecksumMismatch",
            rule: "RFC 1950 S2.3: Adler-32 trailer must match the decoded data",
            sweep: false,
            family: Family::Rfc1950,
        });
    }

    // trailing_byte: TrailingData (default Reject).
    {
        let body = stored_block(true, b"hi");
        let mut bytes = zlib_wrap(&body, b"hi", None);
        bytes.push(b'X');
        cases.push(Case {
            name: "trailing_byte",
            bytes,
            expected: "TrailingData",
            rule: "SPEC S3.3: trailing data is rejected by default",
            sweep: false,
            family: Family::Rfc1950,
        });
    }

    // valid_small: OK, swept for Truncated on every prefix.
    {
        let body = stored_block(true, b"the quick fox");
        let bytes = zlib_wrap(&body, b"the quick fox", None);
        cases.push(Case {
            name: "valid_small",
            bytes,
            expected: "OK",
            rule: "control case: a valid minimal zlib stream",
            sweep: true,
            family: Family::Rfc1950,
        });
    }

    cases
}

/// Build every named RFC 1952 (gzip) malformed (and control-valid) case.
pub fn rfc1952_cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // bad_magic: ID1 wrong.
    {
        let body = stored_block(true, b"hi");
        let mut bytes = gzip_minimal_member(&body, b"hi");
        bytes[0] = 0x00;
        cases.push(Case {
            name: "bad_magic",
            bytes,
            expected: "BadHeader",
            rule: "RFC 1952 S2.3.1: ID1/ID2 must be 1f 8b",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // cm_7: CM=7, not 8.
    {
        let body = stored_block(true, b"hi");
        let bytes = gzip_member_bytes(7, 0, 0, 0, 255, &body, b"hi", None);
        cases.push(Case {
            name: "cm_7",
            bytes,
            expected: "BadHeader",
            rule: "RFC 1952 S2.3.1: CM must be 8 (deflate)",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // reserved_flag_bit5.
    {
        let body = stored_block(true, b"hi");
        let bytes = gzip_member_bytes(8, 0x20, 0, 0, 255, &body, b"hi", None);
        cases.push(Case {
            name: "reserved_flag_bit5",
            bytes,
            expected: "BadHeader",
            rule: "RFC 1952 S2.3.1: FLG bits 5-7 are reserved and must be zero",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // reserved_flag_bit7.
    {
        let body = stored_block(true, b"hi");
        let bytes = gzip_member_bytes(8, 0x80, 0, 0, 255, &body, b"hi", None);
        cases.push(Case {
            name: "reserved_flag_bit7",
            bytes,
            expected: "BadHeader",
            rule: "RFC 1952 S2.3.1: FLG bits 5-7 are reserved and must be zero",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // fhcrc_mismatch: FHCRC set, but the stored CRC16 is wrong.
    {
        let body = stored_block(true, b"hi");
        let bytes = gzip_member_bytes(8, GZ_FHCRC, 0, 0, 255, &body, b"hi", Some(0xFFFF));
        cases.push(Case {
            name: "fhcrc_mismatch",
            bytes,
            expected: "ChecksumMismatch",
            rule: "RFC 1952 S2.3.1: FHCRC must match the header CRC-32's low 16 bits",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // crc32_mismatch: corrupt a trailer CRC32 byte.
    {
        let body = stored_block(true, b"hi");
        let mut bytes = gzip_minimal_member(&body, b"hi");
        let len = bytes.len();
        bytes[len - 8] ^= 0xFF;
        cases.push(Case {
            name: "crc32_mismatch",
            bytes,
            expected: "ChecksumMismatch",
            rule: "RFC 1952 S2.3.1: member CRC32 must match the decoded data",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // isize_mismatch: corrupt a trailer ISIZE byte.
    {
        let body = stored_block(true, b"hi");
        let mut bytes = gzip_minimal_member(&body, b"hi");
        let len = bytes.len();
        bytes[len - 1] ^= 0xFF;
        cases.push(Case {
            name: "isize_mismatch",
            bytes,
            expected: "ChecksumMismatch",
            rule: "RFC 1952 S2.3.1: ISIZE must match the decoded length mod 2^32",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // trailing_garbage: bytes after a complete member that are not
    // another member's magic.
    {
        let body = stored_block(true, b"hi");
        let mut bytes = gzip_minimal_member(&body, b"hi");
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        cases.push(Case {
            name: "trailing_garbage",
            bytes,
            expected: "TrailingData",
            rule: "SPEC S3.3: trailing data is rejected by default",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // second_member_bad_cm: a valid first member, then a second member
    // whose header fails validation (CM=7).
    {
        let body = stored_block(true, b"hi");
        let mut bytes = gzip_minimal_member(&body, b"hi");
        bytes.extend_from_slice(&gzip_member_bytes(7, 0, 0, 0, 255, &body, b"hi", None));
        cases.push(Case {
            name: "second_member_bad_cm",
            bytes,
            expected: "BadHeader",
            rule: "RFC 1952 S2.2: each concatenated member's header is validated independently",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    // all_flags_valid: OK, every optional field present, swept.
    {
        let body = stored_block(true, b"hi");
        let flg = GZ_FTEXT | GZ_FHCRC | GZ_FEXTRA | GZ_FNAME | GZ_FCOMMENT;
        let mut header = vec![0x1fu8, 0x8b, 8, flg];
        header.extend_from_slice(&123456u32.to_le_bytes());
        header.push(2);
        header.push(3);
        let extra = b"EXTR";
        header.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        header.extend_from_slice(extra);
        header.extend_from_slice(b"file.txt");
        header.push(0);
        header.extend_from_slice(b"a comment");
        header.push(0);
        let hcrc = (ffi::crc32(&header) & 0xFFFF) as u16;
        header.extend_from_slice(&hcrc.to_le_bytes());
        let mut bytes = header;
        bytes.extend_from_slice(&body);
        bytes.extend_from_slice(&ffi::crc32(b"hi").to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        cases.push(Case {
            name: "all_flags_valid",
            bytes,
            expected: "OK",
            rule: "control case: a valid gzip member using every optional header field",
            sweep: true,
            family: Family::Rfc1952,
        });
    }

    // two_members_valid: OK, two concatenated members.
    {
        let body_a = stored_block(true, b"foo");
        let body_b = stored_block(true, b"bar");
        let mut bytes = gzip_minimal_member(&body_a, b"foo");
        bytes.extend_from_slice(&gzip_minimal_member(&body_b, b"bar"));
        cases.push(Case {
            name: "two_members_valid",
            bytes,
            expected: "OK",
            rule: "RFC 1952 S2.2: members are simply concatenated",
            sweep: false,
            family: Family::Rfc1952,
        });
    }

    cases
}

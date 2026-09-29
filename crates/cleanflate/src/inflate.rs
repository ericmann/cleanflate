//! `Inflater`: the resumable raw DEFLATE (RFC 1951) state machine. Block
//! headers, stored (BTYPE=00), fixed Huffman (BTYPE=01) and dynamic
//! Huffman (BTYPE=10) blocks are all implemented.

use crate::bitreader::BitReader;
use crate::error::{Error, HuffmanError};
use crate::huffman::{Decoded, Huffman, TableKind};
use crate::tables::{
    fixed_dist_lengths, fixed_litlen_lengths, CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA,
    END_OF_BLOCK, LENGTH_BASE, LENGTH_EXTRA,
};
use crate::window::Window;

/// What happened during one [`Inflater::step`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InflateStatus {
    /// A sub-step needed more bits/bytes than `input` had left. All of
    /// `input` was consumed into the bit buffer.
    NeedsInput,
    /// A byte was ready to write but `output` had no room left.
    OutputFull,
    /// The final block has been fully decoded.
    Done,
}

#[derive(Debug, Clone)]
enum State {
    /// Read the 3-bit block header (BFINAL, BTYPE) (RFC 1951 S3.2.3).
    BlockHeader,
    /// Align to a byte boundary, then read LEN/NLEN (RFC 1951 S3.2.4).
    StoredLen,
    /// Copy `remaining` literal bytes straight from input to output.
    StoredCopy { remaining: u32 },
    /// Read HLIT(5)+HDIST(5)+HCLEN(4) (RFC 1951 S3.2.7).
    DynHeader,
    /// Read the `i`-th (of `dyn_hclen`) 3-bit code-length-alphabet length.
    CodeLenLens { i: usize },
    /// Decode literal/length+distance code lengths using the code-length
    /// table, including repeat codes 16/17/18.
    CodeLens,
    /// A repeat code (16/17/18) was decoded; read its extra bits.
    Repeat { sym: u16 },
    /// Decode the next literal/length symbol (RFC 1951 S3.2.5/S3.2.6/S3.2.7).
    LitLen,
    /// A length code (257..=285) was decoded; read its extra bits.
    LenExtra { sym: u16 },
    /// A length is known; decode the distance symbol (0..=31).
    Dist { len: u16 },
    /// A distance code was decoded; read its extra bits.
    DistExtra { len: u16, dsym: u16 },
    /// Copy `remaining` bytes from `dist` back in the window.
    Copy { remaining: u16, dist: u16 },
    /// The final block has ended.
    Done,
    /// A previous step failed; every later step returns a clone of the
    /// same error (errors are sticky).
    Failed(Error),
}

/// The result of decoding one Huffman symbol, with `NeedMoreBits`
/// resolved away (the caller loops on that internally).
enum SymbolOutcome {
    Symbol { symbol: u16, len: u32 },
    Invalid,
}

/// The resumable raw-DEFLATE decoder core. One `Inflater` drives both the
/// one-shot and the streaming public APIs.
#[derive(Debug, Clone)]
pub(crate) struct Inflater {
    br: BitReader,
    window: Window,
    state: State,
    total_out: u64,
    /// BFINAL of the block currently being decoded; consulted when that
    /// block's end-of-block is reached.
    bfinal: bool,
    /// The fixed literal/length table (RFC 1951 S3.2.6), built once.
    fixed_litlen: Huffman,
    /// The fixed distance table (RFC 1951 S3.2.6), built once.
    fixed_dist: Huffman,
    /// The literal/length table in use for the current block (a clone of
    /// `fixed_litlen`, or a freshly built dynamic table).
    litlen_table: Huffman,
    /// The distance table in use for the current block.
    dist_table: Huffman,
    /// HLIT+257 for the dynamic block currently being parsed.
    dyn_hlit: usize,
    /// HDIST+1 for the dynamic block currently being parsed.
    dyn_hdist: usize,
    /// HCLEN+4 for the dynamic block currently being parsed.
    dyn_hclen: usize,
    /// Code-length-alphabet lengths (index = alphabet symbol 0..=18),
    /// filled in `CODE_LENGTH_ORDER` order.
    dyn_cl_lengths: [u8; 19],
    /// The code-length-alphabet table built from `dyn_cl_lengths`.
    dyn_cl_table: Huffman,
    /// The combined literal/length + distance code lengths accumulated
    /// while decoding with `dyn_cl_table` (RFC 1951 S3.2.7: "all code
    /// lengths form a single sequence of HLIT + HDIST + 258 values").
    dyn_lengths: Vec<u8>,
    /// The last length value decoded (or expanded by a repeat), used by
    /// repeat code 16.
    dyn_prev_len: u8,
}

impl Inflater {
    /// A fresh decoder, ready to read the first block header.
    pub(crate) fn new() -> Self {
        let fixed_litlen = Huffman::from_lengths(&fixed_litlen_lengths(), TableKind::LitLen)
            .unwrap_or_else(|_| Huffman::empty());
        let fixed_dist = Huffman::from_lengths(&fixed_dist_lengths(), TableKind::Distance)
            .unwrap_or_else(|_| Huffman::empty());
        Inflater {
            br: BitReader::new(),
            window: Window::new(),
            state: State::BlockHeader,
            total_out: 0,
            bfinal: false,
            fixed_litlen: fixed_litlen.clone(),
            fixed_dist: fixed_dist.clone(),
            litlen_table: fixed_litlen,
            dist_table: fixed_dist,
            dyn_hlit: 0,
            dyn_hdist: 0,
            dyn_hclen: 0,
            dyn_cl_lengths: [0u8; 19],
            dyn_cl_table: Huffman::empty(),
            dyn_lengths: Vec::new(),
            dyn_prev_len: 0,
        }
    }

    /// A fresh decoder whose window is preloaded with `dict` (keeping
    /// only the last 32 KiB), so early back-references can copy from it
    /// (RFC 1950 S2.2's preset dictionary, or a raw-format caller that
    /// supplies one directly).
    pub(crate) fn with_dictionary(dict: &[u8]) -> Self {
        let mut inflater = Self::new();
        inflater.window.prefill(dict);
        inflater
    }

    /// Total bytes produced so far across all calls to `step`.
    pub(crate) fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Whether the final block has been fully decoded.
    pub(crate) fn is_done(&self) -> bool {
        matches!(self.state, State::Done)
    }

    /// Decode the next symbol from `table`, filling `self.br` from
    /// `input` as needed. Returns `None` if input ran out before a
    /// decision could be made (the caller should return `NeedsInput`);
    /// otherwise the decoded symbol/length or `Invalid`, with the bits
    /// **not yet consumed** (the caller consumes once it knows the
    /// symbol is actionable).
    fn peek_symbol(
        table: &Huffman,
        br: &mut BitReader,
        input: &[u8],
        pos: &mut usize,
    ) -> Option<SymbolOutcome> {
        loop {
            match table.decode_table(br) {
                Decoded::Symbol { symbol, len } => {
                    return Some(SymbolOutcome::Symbol { symbol, len })
                }
                Decoded::Invalid => return Some(SymbolOutcome::Invalid),
                Decoded::NeedMoreBits => {
                    let need = br.available() + 1;
                    if !br.fill(input, pos, need) {
                        return None;
                    }
                }
            }
        }
    }

    /// Advance the state machine as far as `input` and `output` allow.
    /// Returns `(consumed, produced, status)`: bytes read from `input`,
    /// bytes written to `output`, and what stopped this call.
    pub(crate) fn step(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, InflateStatus), Error> {
        if let State::Failed(e) = &self.state {
            return Err(e.clone());
        }

        let mut pos = 0usize;
        let mut out_pos = 0usize;

        loop {
            match &self.state {
                State::Done => return Ok((pos, out_pos, InflateStatus::Done)),
                State::Failed(e) => return Err(e.clone()),
                _ => {}
            }

            match self.state.clone() {
                State::BlockHeader => match self.br.take(input, &mut pos, 3) {
                    None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                    Some(bits) => {
                        self.bfinal = (bits & 1) != 0;
                        let btype = (bits >> 1) & 0b11;
                        match btype {
                            0 => {
                                self.state = State::StoredLen;
                            }
                            1 => {
                                self.litlen_table = self.fixed_litlen.clone();
                                self.dist_table = self.fixed_dist.clone();
                                self.state = State::LitLen;
                            }
                            2 => {
                                self.dyn_hlit = 0;
                                self.dyn_hdist = 0;
                                self.dyn_hclen = 0;
                                self.dyn_cl_lengths = [0u8; 19];
                                self.dyn_lengths.clear();
                                self.dyn_prev_len = 0;
                                self.state = State::DynHeader;
                            }
                            _ => {
                                // BTYPE 11: reserved (RFC 1951 S3.2.3).
                                let e = Error::BadBlockType;
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                        }
                    }
                },
                State::StoredLen => {
                    self.br.align_to_byte();
                    match self.br.take(input, &mut pos, 32) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(v) => {
                            let len = (v & 0xFFFF) as u16;
                            let nlen = ((v >> 16) & 0xFFFF) as u16;
                            if len != !nlen {
                                let e = Error::InvalidStoredLength { len, nlen };
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                            self.state = State::StoredCopy {
                                remaining: len as u32,
                            };
                        }
                    }
                }
                State::StoredCopy { remaining } => {
                    if remaining == 0 {
                        self.state = self.next_block_state();
                        continue;
                    }
                    if out_pos >= output.len() {
                        return Ok((pos, out_pos, InflateStatus::OutputFull));
                    }
                    if pos >= input.len() {
                        return Ok((pos, out_pos, InflateStatus::NeedsInput));
                    }

                    let available_in = (input.len() - pos) as u32;
                    let available_out = (output.len() - out_pos) as u32;
                    let n = remaining.min(available_in).min(available_out) as usize;

                    for i in 0..n {
                        let byte = input.get(pos + i).copied().unwrap_or(0);
                        if let Some(slot) = output.get_mut(out_pos + i) {
                            *slot = byte;
                        }
                        self.window.push(byte);
                    }

                    pos += n;
                    out_pos += n;
                    self.total_out += n as u64;
                    self.state = State::StoredCopy {
                        remaining: remaining - n as u32,
                    };
                }
                State::DynHeader => match self.br.take(input, &mut pos, 14) {
                    None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                    Some(v) => {
                        let hlit_field = v & 0x1F;
                        let hdist_field = (v >> 5) & 0x1F;
                        let hclen_field = (v >> 10) & 0xF;
                        let hlit = hlit_field as usize + 257;
                        let hdist = hdist_field as usize + 1;
                        let hclen = hclen_field as usize + 4;
                        if hlit > 286 {
                            let e = Error::InvalidHuffmanTable(HuffmanError::TooManyLengthCodes);
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        self.dyn_hlit = hlit;
                        self.dyn_hdist = hdist;
                        self.dyn_hclen = hclen;
                        self.state = State::CodeLenLens { i: 0 };
                    }
                },
                State::CodeLenLens { i } => {
                    if i >= self.dyn_hclen {
                        match Huffman::from_lengths(&self.dyn_cl_lengths, TableKind::CodeLength) {
                            Ok(t) => {
                                self.dyn_cl_table = t;
                                self.state = State::CodeLens;
                            }
                            Err(he) => {
                                let e = Error::InvalidHuffmanTable(he);
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                        }
                        continue;
                    }
                    match self.br.take(input, &mut pos, 3) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(v) => {
                            let sym = *CODE_LENGTH_ORDER.get(i).unwrap_or(&0);
                            if let Some(slot) = self.dyn_cl_lengths.get_mut(sym as usize) {
                                *slot = v as u8;
                            }
                            self.state = State::CodeLenLens { i: i + 1 };
                        }
                    }
                }
                State::CodeLens => {
                    let total = self.dyn_hlit + self.dyn_hdist;
                    if self.dyn_lengths.len() >= total {
                        let litlen_lengths: Vec<u8> = self
                            .dyn_lengths
                            .get(..self.dyn_hlit)
                            .unwrap_or(&[])
                            .to_vec();
                        let dist_lengths: Vec<u8> = self
                            .dyn_lengths
                            .get(self.dyn_hlit..)
                            .unwrap_or(&[])
                            .to_vec();
                        if litlen_lengths
                            .get(END_OF_BLOCK as usize)
                            .copied()
                            .unwrap_or(0)
                            == 0
                        {
                            let e = Error::InvalidHuffmanTable(HuffmanError::MissingEndOfBlock);
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        let litlen_table =
                            match Huffman::from_lengths(&litlen_lengths, TableKind::LitLen) {
                                Ok(t) => t,
                                Err(he) => {
                                    let e = Error::InvalidHuffmanTable(he);
                                    self.state = State::Failed(e.clone());
                                    return Err(e);
                                }
                            };
                        let dist_table =
                            match Huffman::from_lengths(&dist_lengths, TableKind::Distance) {
                                Ok(t) => t,
                                Err(he) => {
                                    let e = Error::InvalidHuffmanTable(he);
                                    self.state = State::Failed(e.clone());
                                    return Err(e);
                                }
                            };
                        self.litlen_table = litlen_table;
                        self.dist_table = dist_table;
                        self.dyn_lengths.clear();
                        self.state = State::LitLen;
                        continue;
                    }

                    let outcome = match Self::peek_symbol(
                        &self.dyn_cl_table,
                        &mut self.br,
                        input,
                        &mut pos,
                    ) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(o) => o,
                    };
                    match outcome {
                        SymbolOutcome::Invalid => {
                            let e = Error::InvalidSymbol;
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        SymbolOutcome::Symbol { symbol, len } => {
                            if symbol <= 15 {
                                self.br.consume(len);
                                self.dyn_lengths.push(symbol as u8);
                                self.dyn_prev_len = symbol as u8;
                            } else if symbol == 16 {
                                if self.dyn_lengths.is_empty() {
                                    let e = Error::InvalidHuffmanTable(
                                        HuffmanError::RepeatWithoutPrevious,
                                    );
                                    self.state = State::Failed(e.clone());
                                    return Err(e);
                                }
                                self.br.consume(len);
                                self.state = State::Repeat { sym: 16 };
                            } else if symbol == 17 {
                                self.br.consume(len);
                                self.state = State::Repeat { sym: 17 };
                            } else if symbol == 18 {
                                self.br.consume(len);
                                self.state = State::Repeat { sym: 18 };
                            } else {
                                // The code-length alphabet only has
                                // symbols 0..=18; a table built from a
                                // 19-length array cannot decode anything
                                // else.
                                let e = Error::InvalidSymbol;
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                        }
                    }
                }
                State::Repeat { sym } => {
                    let extra_bits: u32 = match sym {
                        16 => 2,
                        17 => 3,
                        18 => 7,
                        _ => 0,
                    };
                    match self.br.take(input, &mut pos, extra_bits) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(extra) => {
                            let (count, value): (usize, u8) = match sym {
                                16 => (3 + extra as usize, self.dyn_prev_len),
                                17 => (3 + extra as usize, 0u8),
                                18 => (11 + extra as usize, 0u8),
                                _ => (0, 0u8),
                            };
                            let total = self.dyn_hlit + self.dyn_hdist;
                            if self.dyn_lengths.len() + count > total {
                                let e = Error::InvalidHuffmanTable(HuffmanError::RepeatOverflow);
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                            for _ in 0..count {
                                self.dyn_lengths.push(value);
                            }
                            self.dyn_prev_len = value;
                            self.state = State::CodeLens;
                        }
                    }
                }
                State::LitLen => {
                    let outcome = match Self::peek_symbol(
                        &self.litlen_table,
                        &mut self.br,
                        input,
                        &mut pos,
                    ) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(o) => o,
                    };
                    match outcome {
                        SymbolOutcome::Invalid => {
                            let e = Error::InvalidSymbol;
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        SymbolOutcome::Symbol { symbol, len } => {
                            if symbol < 256 {
                                // A literal: only commit (consume bits,
                                // write the byte) once we know there is
                                // room, so a retry re-decodes the same
                                // (still-buffered) bits for free.
                                if out_pos >= output.len() {
                                    return Ok((pos, out_pos, InflateStatus::OutputFull));
                                }
                                self.br.consume(len);
                                let byte = symbol as u8;
                                if let Some(slot) = output.get_mut(out_pos) {
                                    *slot = byte;
                                }
                                self.window.push(byte);
                                out_pos += 1;
                                self.total_out += 1;
                            } else if symbol == END_OF_BLOCK {
                                self.br.consume(len);
                                self.state = self.next_block_state();
                            } else if (257..=285).contains(&symbol) {
                                self.br.consume(len);
                                self.state = State::LenExtra { sym: symbol };
                            } else {
                                // 286/287: valid codes in the fixed table
                                // that never occur in real data (RFC 1951
                                // S3.2.6); a dynamic table rejects them
                                // via MissingEndOfBlock/other validation
                                // long before decode could reach them.
                                let e = Error::InvalidSymbol;
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                        }
                    }
                }
                State::LenExtra { sym } => {
                    let idx = (sym - 257) as usize;
                    let extra_bits = *LENGTH_EXTRA.get(idx).unwrap_or(&0) as u32;
                    let base = *LENGTH_BASE.get(idx).unwrap_or(&0);
                    match self.br.take(input, &mut pos, extra_bits) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(extra) => {
                            let len = base.saturating_add(extra as u16);
                            self.state = State::Dist { len };
                        }
                    }
                }
                State::Dist { len } => {
                    let outcome =
                        match Self::peek_symbol(&self.dist_table, &mut self.br, input, &mut pos) {
                            None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                            Some(o) => o,
                        };
                    match outcome {
                        SymbolOutcome::Invalid => {
                            let e = Error::InvalidSymbol;
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        SymbolOutcome::Symbol {
                            symbol: dsym,
                            len: bit_len,
                        } => {
                            if dsym >= 30 {
                                // Distance codes 30/31 never occur in real
                                // data (RFC 1951 S3.2.6).
                                let e = Error::InvalidSymbol;
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                            self.br.consume(bit_len);
                            self.state = State::DistExtra { len, dsym };
                        }
                    }
                }
                State::DistExtra { len, dsym } => {
                    let idx = dsym as usize;
                    let extra_bits = *DIST_EXTRA.get(idx).unwrap_or(&0) as u32;
                    let base = *DIST_BASE.get(idx).unwrap_or(&0);
                    match self.br.take(input, &mut pos, extra_bits) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(extra) => {
                            let distance = (base as u32).saturating_add(extra);
                            let available = self.window.available() as u64;
                            if distance as u64 > available {
                                let e = Error::InvalidDistance {
                                    distance,
                                    available,
                                };
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                            self.state = State::Copy {
                                remaining: len,
                                dist: distance as u16,
                            };
                        }
                    }
                }
                State::Copy { remaining, dist } => {
                    if remaining == 0 {
                        self.state = State::LitLen;
                        continue;
                    }
                    if out_pos >= output.len() {
                        return Ok((pos, out_pos, InflateStatus::OutputFull));
                    }
                    let out_slice = output.get_mut(out_pos..).unwrap_or(&mut []);
                    let written =
                        self.window
                            .copy_match(dist as usize, remaining as usize, out_slice);
                    out_pos += written;
                    self.total_out += written as u64;
                    let new_remaining = remaining.saturating_sub(written as u16);
                    self.state = State::Copy {
                        remaining: new_remaining,
                        dist,
                    };
                    if written == 0 {
                        // Should not happen: distance was validated when
                        // entering Copy and only grows more available as
                        // bytes are pushed. Treat as needing more output
                        // room to avoid spinning.
                        return Ok((pos, out_pos, InflateStatus::OutputFull));
                    }
                }
                // Done/Failed are handled by the check at the top of this
                // loop; this arm is unreachable in practice but keeps the
                // match total without a forbidden panicking macro call.
                State::Done | State::Failed(_) => {}
            }
        }
    }

    /// After an end-of-block (stored copy exhausted, or literal/length
    /// symbol 256), decide whether the stream is done or another block
    /// header follows (RFC 1951 S3.2.3's "while not last block").
    fn next_block_state(&self) -> State {
        if self.bfinal {
            State::Done
        } else {
            State::BlockHeader
        }
    }
}

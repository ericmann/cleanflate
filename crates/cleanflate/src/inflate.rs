//! `Inflater`: the resumable raw DEFLATE (RFC 1951) state machine. This
//! task adds block headers, stored (BTYPE=00) blocks, and fixed Huffman
//! (BTYPE=01) blocks; dynamic Huffman (BTYPE=10) arrives in P2-02.

use crate::bitreader::BitReader;
use crate::error::Error;
use crate::huffman::{Decoded, Huffman};
use crate::tables::{
    fixed_dist_lengths, fixed_litlen_lengths, DIST_BASE, DIST_EXTRA, END_OF_BLOCK, LENGTH_BASE,
    LENGTH_EXTRA,
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
    /// Decode the next literal/length symbol (RFC 1951 S3.2.5/S3.2.6).
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
}

impl Inflater {
    /// A fresh decoder, ready to read the first block header.
    pub(crate) fn new() -> Self {
        let fixed_litlen =
            Huffman::from_lengths(&fixed_litlen_lengths()).unwrap_or_else(|_| Huffman::empty());
        let fixed_dist =
            Huffman::from_lengths(&fixed_dist_lengths()).unwrap_or_else(|_| Huffman::empty());
        Inflater {
            br: BitReader::new(),
            window: Window::new(),
            state: State::BlockHeader,
            total_out: 0,
            bfinal: false,
            fixed_litlen,
            fixed_dist,
        }
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
            match table.decode(br) {
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
                                self.state = State::LitLen;
                            }
                            2 => {
                                // replaced by P2-02 (dynamic Huffman blocks)
                                let e = Error::BadBlockType;
                                self.state = State::Failed(e.clone());
                                return Err(e);
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
                State::LitLen => {
                    let outcome = match Self::peek_symbol(
                        &self.fixed_litlen,
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
                                // S3.2.6).
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
                        match Self::peek_symbol(&self.fixed_dist, &mut self.br, input, &mut pos) {
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

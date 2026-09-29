//! `Inflater`: the resumable raw DEFLATE (RFC 1951) state machine. This
//! task implements block headers and stored (BTYPE=00) blocks; fixed
//! Huffman (BTYPE=01) arrives in P1-09, dynamic Huffman (BTYPE=10) in
//! P2-02.

use crate::bitreader::BitReader;
use crate::error::Error;
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
    StoredLen { bfinal: bool },
    /// Copy `remaining` literal bytes straight from input to output.
    StoredCopy { remaining: u32, bfinal: bool },
    /// The final block has ended.
    Done,
    /// A previous step failed; every later step returns a clone of the
    /// same error (errors are sticky).
    Failed(Error),
}

/// The resumable raw-DEFLATE decoder core. One `Inflater` drives both the
/// one-shot and the streaming public APIs.
#[derive(Debug, Clone)]
pub(crate) struct Inflater {
    br: BitReader,
    window: Window,
    state: State,
    total_out: u64,
}

impl Inflater {
    /// A fresh decoder, ready to read the first block header.
    pub(crate) fn new() -> Self {
        Inflater {
            br: BitReader::new(),
            window: Window::new(),
            state: State::BlockHeader,
            total_out: 0,
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
                        let bfinal = (bits & 1) != 0;
                        let btype = (bits >> 1) & 0b11;
                        match btype {
                            0 => {
                                self.state = State::StoredLen { bfinal };
                            }
                            1 => {
                                // replaced by P1-09 (fixed Huffman blocks)
                                let e = Error::BadBlockType;
                                self.state = State::Failed(e.clone());
                                return Err(e);
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
                State::StoredLen { bfinal } => {
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
                                bfinal,
                            };
                        }
                    }
                }
                State::StoredCopy { remaining, bfinal } => {
                    if remaining == 0 {
                        self.state = if bfinal {
                            State::Done
                        } else {
                            State::BlockHeader
                        };
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
                        bfinal,
                    };
                }
                // Done/Failed are handled by the check at the top of this
                // loop; this arm is unreachable in practice but keeps the
                // match total without a forbidden `unreachable!()` call.
                State::Done | State::Failed(_) => {}
            }
        }
    }
}

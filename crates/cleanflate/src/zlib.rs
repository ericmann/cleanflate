//! The RFC 1950 zlib wrapper: CMF/FLG header, optional preset dictionary,
//! and the Adler-32 trailer, around a raw DEFLATE [`Inflater`].

use crate::checksum::Adler32;
use crate::decode_path::DecodePath;
use crate::error::{ChecksumKind, Error, HeaderError};
use crate::inflate::{InflateStatus, Inflater};
use crate::sink::OutputSink;

#[derive(Debug, Clone)]
enum State {
    /// Read CMF (byte 0).
    Cmf,
    /// Read FLG (byte 1); `cmf` is what `Cmf` read.
    Flg { cmf: u8 },
    /// FDICT was set: read DICTID (4 bytes, big-endian).
    DictId { collected: usize },
    /// Decoding the raw DEFLATE body.
    Body,
    /// Read the Adler-32 trailer (4 bytes, big-endian).
    Trailer { collected: usize },
    /// The stream is fully decoded and its trailer verified.
    Done,
    /// A previous step failed; sticky, like [`Inflater`]'s own errors.
    Failed(Error),
}

/// The resumable RFC 1950 zlib decoder: header, optional preset
/// dictionary, raw DEFLATE body, Adler-32 trailer.
#[derive(Debug, Clone)]
pub(crate) struct ZlibDecoder {
    state: State,
    inflater: Inflater,
    path: DecodePath,
    adler: Adler32,
    dictionary: Option<Vec<u8>>,
    dict_id_buf: [u8; 4],
    trailer_buf: [u8; 4],
}

impl ZlibDecoder {
    /// A fresh decoder, ready to read CMF. `dictionary`, if given, is
    /// used only if the stream's FDICT bit is set (RFC 1950 S2.2); if
    /// FDICT is clear, a supplied dictionary is ignored.
    pub(crate) fn new(dictionary: Option<Vec<u8>>, path: DecodePath) -> Self {
        ZlibDecoder {
            state: State::Cmf,
            inflater: Inflater::new_with_path(path),
            path,
            adler: Adler32::new(),
            dictionary,
            dict_id_buf: [0u8; 4],
            trailer_buf: [0u8; 4],
        }
    }

    /// The decode path of the current inflater.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn inflater_path(&self) -> DecodePath {
        self.inflater.path()
    }

    /// A snapshot of the counters.
    #[cfg(feature = "stats")]
    pub(crate) fn stats(&self) -> crate::stats::Stats {
        self.inflater.stats()
    }

    pub(crate) fn total_out(&self) -> u64 {
        self.inflater.total_out()
    }

    pub(crate) fn is_done(&self) -> bool {
        matches!(self.state, State::Done)
    }

    pub(crate) fn step_into<S: OutputSink>(
        &mut self,
        input: &[u8],
        sink: &mut S,
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
                State::Cmf => match input.get(pos) {
                    None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                    Some(&cmf) => {
                        pos += 1;
                        self.state = State::Flg { cmf };
                    }
                },
                State::Flg { cmf } => match input.get(pos) {
                    None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                    Some(&flg) => {
                        pos += 1;
                        // RFC 1950 S2.2: checked in order FCHECK, CM, CINFO.
                        let check = (cmf as u32) * 256 + flg as u32;
                        if check % 31 != 0 {
                            let e = Error::BadHeader(HeaderError::ZlibFcheck);
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        let cm = cmf & 0x0F;
                        if cm != 8 {
                            let e = Error::BadHeader(HeaderError::ZlibCompressionMethod(cm));
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        let cinfo = (cmf >> 4) & 0x0F;
                        if cinfo > 7 {
                            let e = Error::BadHeader(HeaderError::ZlibWindowSize(cinfo));
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        let fdict = (flg & 0x20) != 0;
                        if fdict {
                            self.state = State::DictId { collected: 0 };
                        } else {
                            // FDICT clear: a supplied dictionary is
                            // ignored (RFC 1950 has no way to use one
                            // without FDICT/DICTID).
                            self.state = State::Body;
                        }
                    }
                },
                State::DictId { collected } => {
                    if collected >= 4 {
                        let dict_id = u32::from_be_bytes(self.dict_id_buf);
                        match &self.dictionary {
                            None => {
                                let e = Error::PresetDictionaryRequired { dict_id };
                                self.state = State::Failed(e.clone());
                                return Err(e);
                            }
                            Some(dict) => {
                                let actual = crate::checksum::adler32(dict);
                                if actual != dict_id {
                                    let e = Error::DictionaryMismatch {
                                        expected: dict_id,
                                        actual,
                                    };
                                    self.state = State::Failed(e.clone());
                                    return Err(e);
                                }
                                self.inflater = Inflater::with_dictionary(dict, self.path);
                                self.state = State::Body;
                            }
                        }
                        continue;
                    }
                    match input.get(pos) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(&b) => {
                            pos += 1;
                            if let Some(slot) = self.dict_id_buf.get_mut(collected) {
                                *slot = b;
                            }
                            self.state = State::DictId {
                                collected: collected + 1,
                            };
                        }
                    }
                }
                State::Body => {
                    let remaining_input = input.get(pos..).unwrap_or(&[]);
                    let before = sink.written();
                    match self.inflater.step_into(remaining_input, sink) {
                        Err(e) => {
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        Ok((consumed, produced, status)) => {
                            self.adler.update(sink.written_since(before));
                            pos += consumed;
                            out_pos += produced;
                            match status {
                                InflateStatus::NeedsInput => {
                                    return Ok((pos, out_pos, InflateStatus::NeedsInput))
                                }
                                InflateStatus::OutputFull => {
                                    return Ok((pos, out_pos, InflateStatus::OutputFull))
                                }
                                InflateStatus::Done => {
                                    self.state = State::Trailer { collected: 0 };
                                }
                            }
                        }
                    }
                }
                State::Trailer { collected } => {
                    if collected >= 4 {
                        let expected = u32::from_be_bytes(self.trailer_buf);
                        let actual = self.adler.finish();
                        if expected != actual {
                            let e = Error::ChecksumMismatch {
                                kind: ChecksumKind::Adler32,
                                expected,
                                actual,
                            };
                            self.state = State::Failed(e.clone());
                            return Err(e);
                        }
                        self.state = State::Done;
                        continue;
                    }
                    match input.get(pos) {
                        None => return Ok((pos, out_pos, InflateStatus::NeedsInput)),
                        Some(&b) => {
                            pos += 1;
                            if let Some(slot) = self.trailer_buf.get_mut(collected) {
                                *slot = b;
                            }
                            self.state = State::Trailer {
                                collected: collected + 1,
                            };
                        }
                    }
                }
                State::Done | State::Failed(_) => {}
            }
        }
    }
}

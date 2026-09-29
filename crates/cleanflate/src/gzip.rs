//! The RFC 1952 gzip member header parser: ID1/ID2/CM/FLG, MTIME/XFL/OS,
//! the optional FEXTRA/FNAME/FCOMMENT fields (bounded to
//! `options.max_gzip_field` bytes each), and the optional FHCRC check.

use crate::checksum::Crc32;
use crate::error::{ChecksumKind, Error, HeaderError};

/// A parsed gzip member header (RFC 1952 S2.3.1). `Zlib`/`Gzip` and
/// member-body/trailer handling arrive in P3-03.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GzipHeader {
    /// FTEXT: the compressor's guess that the data is ASCII text.
    pub text: bool,
    /// Modification time (Unix epoch seconds), 0 if unavailable.
    pub mtime: u32,
    /// Extra flags (compression-method-specific).
    pub xfl: u8,
    /// Operating system that produced the member.
    pub os: u8,
    /// FEXTRA content, if present, up to `max_gzip_field` bytes.
    pub extra: Option<Vec<u8>>,
    /// FNAME content (without the terminating zero byte), if present, up
    /// to `max_gzip_field` bytes.
    pub name: Option<Vec<u8>>,
    /// FCOMMENT content (without the terminating zero byte), if present,
    /// up to `max_gzip_field` bytes.
    pub comment: Option<Vec<u8>>,
    /// FHCRC: whether a header CRC16 was present (and, since parsing
    /// succeeded, verified).
    pub header_crc: bool,
    /// Whether any of `extra`/`name`/`comment` was longer than
    /// `max_gzip_field` and got truncated (the full field was still
    /// parsed and CRC-checked; only the stored copy is short).
    pub truncated_fields: bool,
}

#[derive(Debug, Clone)]
enum HState {
    Id1,
    Id2,
    Cm,
    Flg,
    Mtime { collected: usize },
    Xfl,
    Os,
    ExtraLen { collected: usize },
    ExtraData,
    Name,
    Comment,
    Hcrc { collected: usize },
    Done,
    Failed(Error),
}

/// Resumable RFC 1952 gzip member-header parser.
#[derive(Debug, Clone)]
pub(crate) struct GzipHeaderParser {
    state: HState,
    flg: u8,
    mtime_buf: [u8; 4],
    mtime: u32,
    xfl: u8,
    os: u8,
    xlen_buf: [u8; 2],
    extra_remaining: usize,
    extra_buf: Vec<u8>,
    name_buf: Vec<u8>,
    comment_buf: Vec<u8>,
    hcrc_buf: [u8; 2],
    crc: Crc32,
    truncated_fields: bool,
    max_field: usize,
}

const FTEXT: u8 = 0x01;
const FHCRC: u8 = 0x02;
const FEXTRA: u8 = 0x04;
const FNAME: u8 = 0x08;
const FCOMMENT: u8 = 0x10;
const RESERVED: u8 = 0xE0;

impl GzipHeaderParser {
    /// A fresh parser, keeping at most `max_field` bytes of each of
    /// FEXTRA/FNAME/FCOMMENT.
    pub(crate) fn new(max_field: usize) -> Self {
        GzipHeaderParser {
            state: HState::Id1,
            flg: 0,
            mtime_buf: [0u8; 4],
            mtime: 0,
            xfl: 0,
            os: 0,
            xlen_buf: [0u8; 2],
            extra_remaining: 0,
            extra_buf: Vec::new(),
            name_buf: Vec::new(),
            comment_buf: Vec::new(),
            hcrc_buf: [0u8; 2],
            crc: Crc32::new(),
            truncated_fields: false,
            max_field,
        }
    }

    /// After OS (stage 0), after FEXTRA (stage 1), after FNAME (stage 2),
    /// after FCOMMENT (stage 3): decide the next state given which FLG
    /// bits are set, skipping fields that are absent.
    fn decide_next(&self, stage: u8) -> HState {
        if stage == 0 && self.flg & FEXTRA != 0 {
            return HState::ExtraLen { collected: 0 };
        }
        if stage <= 1 && self.flg & FNAME != 0 {
            return HState::Name;
        }
        if stage <= 2 && self.flg & FCOMMENT != 0 {
            return HState::Comment;
        }
        if stage <= 3 && self.flg & FHCRC != 0 {
            return HState::Hcrc { collected: 0 };
        }
        HState::Done
    }

    fn build_header(&self) -> GzipHeader {
        GzipHeader {
            text: self.flg & FTEXT != 0,
            mtime: self.mtime,
            xfl: self.xfl,
            os: self.os,
            extra: if self.flg & FEXTRA != 0 {
                Some(self.extra_buf.clone())
            } else {
                None
            },
            name: if self.flg & FNAME != 0 {
                Some(self.name_buf.clone())
            } else {
                None
            },
            comment: if self.flg & FCOMMENT != 0 {
                Some(self.comment_buf.clone())
            } else {
                None
            },
            header_crc: self.flg & FHCRC != 0,
            truncated_fields: self.truncated_fields,
        }
    }

    /// Feed `input`, consuming as much as it takes to make progress.
    /// Returns `(consumed, Some(header))` once the header is fully
    /// parsed (and its FHCRC, if present, verified), or `(consumed,
    /// None)` if more input is needed (in which case `consumed ==
    /// input.len()`).
    pub(crate) fn feed(&mut self, input: &[u8]) -> Result<(usize, Option<GzipHeader>), Error> {
        if let HState::Failed(e) = &self.state {
            return Err(e.clone());
        }

        let mut pos = 0usize;

        loop {
            if matches!(self.state, HState::Done) {
                return Ok((pos, Some(self.build_header())));
            }
            if let HState::Failed(e) = &self.state {
                return Err(e.clone());
            }

            match self.state.clone() {
                HState::Id1 => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        if b != 0x1f {
                            return self.fail(Error::BadHeader(HeaderError::GzipMagic));
                        }
                        self.state = HState::Id2;
                    }
                },
                HState::Id2 => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        if b != 0x8b {
                            return self.fail(Error::BadHeader(HeaderError::GzipMagic));
                        }
                        self.state = HState::Cm;
                    }
                },
                HState::Cm => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        if b != 8 {
                            return self
                                .fail(Error::BadHeader(HeaderError::GzipCompressionMethod(b)));
                        }
                        self.state = HState::Flg;
                    }
                },
                HState::Flg => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        if b & RESERVED != 0 {
                            return self.fail(Error::BadHeader(HeaderError::GzipReservedFlags(b)));
                        }
                        self.flg = b;
                        self.state = HState::Mtime { collected: 0 };
                    }
                },
                HState::Mtime { collected } => {
                    if collected >= 4 {
                        self.mtime = u32::from_le_bytes(self.mtime_buf);
                        self.state = HState::Xfl;
                        continue;
                    }
                    match input.get(pos) {
                        None => return Ok((pos, None)),
                        Some(&b) => {
                            pos += 1;
                            self.crc.update(&[b]);
                            if let Some(slot) = self.mtime_buf.get_mut(collected) {
                                *slot = b;
                            }
                            self.state = HState::Mtime {
                                collected: collected + 1,
                            };
                        }
                    }
                }
                HState::Xfl => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        self.xfl = b;
                        self.state = HState::Os;
                    }
                },
                HState::Os => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        self.os = b;
                        self.state = self.decide_next(0);
                    }
                },
                HState::ExtraLen { collected } => {
                    if collected >= 2 {
                        self.extra_remaining = u16::from_le_bytes(self.xlen_buf) as usize;
                        self.state = HState::ExtraData;
                        continue;
                    }
                    match input.get(pos) {
                        None => return Ok((pos, None)),
                        Some(&b) => {
                            pos += 1;
                            self.crc.update(&[b]);
                            if let Some(slot) = self.xlen_buf.get_mut(collected) {
                                *slot = b;
                            }
                            self.state = HState::ExtraLen {
                                collected: collected + 1,
                            };
                        }
                    }
                }
                HState::ExtraData => {
                    if self.extra_remaining == 0 {
                        self.state = self.decide_next(1);
                        continue;
                    }
                    match input.get(pos) {
                        None => return Ok((pos, None)),
                        Some(&b) => {
                            pos += 1;
                            self.crc.update(&[b]);
                            if self.extra_buf.len() < self.max_field {
                                self.extra_buf.push(b);
                            } else {
                                self.truncated_fields = true;
                            }
                            self.extra_remaining -= 1;
                        }
                    }
                }
                HState::Name => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        if b == 0 {
                            self.state = self.decide_next(2);
                        } else if self.name_buf.len() < self.max_field {
                            self.name_buf.push(b);
                        } else {
                            self.truncated_fields = true;
                        }
                    }
                },
                HState::Comment => match input.get(pos) {
                    None => return Ok((pos, None)),
                    Some(&b) => {
                        pos += 1;
                        self.crc.update(&[b]);
                        if b == 0 {
                            self.state = self.decide_next(3);
                        } else if self.comment_buf.len() < self.max_field {
                            self.comment_buf.push(b);
                        } else {
                            self.truncated_fields = true;
                        }
                    }
                },
                HState::Hcrc { collected } => {
                    if collected >= 2 {
                        let expected = u16::from_le_bytes(self.hcrc_buf) as u32;
                        let actual = self.crc.finish() & 0xFFFF;
                        if expected != actual {
                            return self.fail(Error::ChecksumMismatch {
                                kind: ChecksumKind::GzipHeaderCrc,
                                expected,
                                actual,
                            });
                        }
                        self.state = HState::Done;
                        continue;
                    }
                    match input.get(pos) {
                        None => return Ok((pos, None)),
                        Some(&b) => {
                            pos += 1;
                            // The CRC16 field's own bytes are excluded
                            // from the header CRC (RFC 1952 S2.3.1: "for
                            // all bytes of the gzip header up to and not
                            // including the CRC16").
                            if let Some(slot) = self.hcrc_buf.get_mut(collected) {
                                *slot = b;
                            }
                            self.state = HState::Hcrc {
                                collected: collected + 1,
                            };
                        }
                    }
                }
                HState::Done | HState::Failed(_) => {}
            }
        }
    }

    fn fail(&mut self, e: Error) -> Result<(usize, Option<GzipHeader>), Error> {
        self.state = HState::Failed(e.clone());
        Err(e)
    }
}

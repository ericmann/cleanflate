//! The RFC 1952 gzip member header parser and the resumable multi-member
//! decoder: ID1/ID2/CM/FLG, MTIME/XFL/OS, the optional
//! FEXTRA/FNAME/FCOMMENT fields (bounded to `options.max_gzip_field`
//! bytes each), the optional FHCRC check, and the member body/trailer
//! and inter-member boundary logic (RFC 1952 S2.2, S2.3, S2.3.1).

use crate::checksum::Crc32;
use crate::decode_path::DecodePath;
use crate::error::{ChecksumKind, Error, HeaderError};
use crate::inflate::{InflateStatus, Inflater};
use crate::options::TrailingData;
use crate::sink::OutputSink;

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

    /// A fresh parser that already consumed and CRC'd a confirmed ID1/ID2
    /// magic pair (used when a multi-member gzip boundary already
    /// examined those two bytes to decide "this is a new member").
    pub(crate) fn new_primed(max_field: usize) -> Self {
        let mut parser = Self::new(max_field);
        parser.crc.update(&[0x1f, 0x8b]);
        parser.state = HState::Cm;
        parser
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

#[derive(Debug, Clone)]
enum GState {
    /// Parsing one member's header.
    MemberHeader(GzipHeaderParser),
    /// Decoding one member's raw DEFLATE body; `inflater`/`member_crc`/
    /// `member_len` on [`GzipDecoder`] carry the per-member state (kept
    /// out of this variant to avoid an oversized enum).
    Body,
    /// Reading one member's 8-byte trailer (CRC32 then ISIZE, both
    /// little-endian).
    Trailer { collected: usize, buf: [u8; 8] },
    /// Between members: deciding whether a new member follows or the
    /// stream has ended (RFC 1952 S2.2: members are simply concatenated,
    /// "with no additional information before, between, or after them").
    Boundary,
    /// The stream ended cleanly (no trailing data, or trailing data was
    /// allowed).
    Done,
    /// A previous step failed; sticky, like the other backends.
    Failed(Error),
}

/// The resumable RFC 1952 gzip decoder: one or more concatenated
/// members, each with its own header/body/trailer, followed by gzip's
/// own trailing-data resolution at each inter-member boundary.
#[derive(Debug, Clone)]
pub(crate) struct GzipDecoder {
    state: GState,
    inflater: Inflater,
    path: DecodePath,
    member_crc: Crc32,
    member_len: u64,
    headers: Vec<GzipHeader>,
    /// Every member started, retained or not (SPEC S1: bounded memory
    /// via `max_gzip_headers`, while still reporting the true count).
    member_count: u64,
    trailing: TrailingData,
    max_gzip_field: usize,
    max_gzip_headers: usize,
    total_in: u64,
    total_out: u64,
    trailing_start: Option<u64>,
    /// Bytes consumed so far while disambiguating the current boundary
    /// (0 or 1: a lone `0x1f` awaiting a second byte).
    boundary_pending: Vec<u8>,
    /// `total_in` at the moment the current boundary examination began,
    /// used as the trailing-data offset if it turns out not to be a new
    /// member.
    boundary_start: u64,
    /// Counters of the members already finished (feature `stats` only).
    #[cfg(feature = "stats")]
    stats_done: Box<crate::stats::Stats>,
}

impl GzipDecoder {
    /// Counters accumulated across every member so far.
    #[cfg(feature = "stats")]
    pub(crate) fn stats(&self) -> crate::stats::Stats {
        let mut s = (*self.stats_done).clone();
        s.merge(&self.inflater.stats());
        s
    }

    /// The decode path of the current member's inflater.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn inflater_path(&self) -> DecodePath {
        self.inflater.path()
    }

    pub(crate) fn new(
        trailing: TrailingData,
        max_gzip_field: usize,
        max_gzip_headers: usize,
        path: DecodePath,
    ) -> Self {
        GzipDecoder {
            state: GState::MemberHeader(GzipHeaderParser::new(max_gzip_field)),
            inflater: Inflater::new_with_path(path),
            path,
            member_crc: Crc32::new(),
            member_len: 0,
            headers: Vec::new(),
            member_count: 0,
            trailing,
            max_gzip_field,
            max_gzip_headers,
            total_in: 0,
            total_out: 0,
            trailing_start: None,
            boundary_pending: Vec::new(),
            boundary_start: 0,
            #[cfg(feature = "stats")]
            stats_done: Box::default(),
        }
    }

    /// Return to the freshly-constructed state, keeping the allocations
    /// (the inflater's window and tables, the header list's capacity).
    pub(crate) fn reset(&mut self) {
        self.state = GState::MemberHeader(GzipHeaderParser::new(self.max_gzip_field));
        self.inflater.reset();
        self.member_crc = Crc32::new();
        self.member_len = 0;
        self.headers.clear();
        self.member_count = 0;
        self.total_in = 0;
        self.total_out = 0;
        self.trailing_start = None;
        self.boundary_pending.clear();
        self.boundary_start = 0;
        #[cfg(feature = "stats")]
        {
            *self.stats_done = crate::stats::Stats::default();
        }
    }

    pub(crate) fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Every member started so far, retained or not.
    pub(crate) fn member_count(&self) -> u64 {
        self.member_count
    }

    pub(crate) fn is_done(&self) -> bool {
        matches!(self.state, GState::Done)
    }

    pub(crate) fn headers(&self) -> &[GzipHeader] {
        &self.headers
    }

    pub(crate) fn trailing_start(&self) -> Option<u64> {
        self.trailing_start
    }

    fn fail(&mut self, e: Error) -> Result<(usize, usize, InflateStatus), Error> {
        self.state = GState::Failed(e.clone());
        Err(e)
    }

    pub(crate) fn step_into<S: OutputSink>(
        &mut self,
        input: &[u8],
        sink: &mut S,
    ) -> Result<(usize, usize, InflateStatus), Error> {
        if let GState::Failed(e) = &self.state {
            return Err(e.clone());
        }

        let mut pos = 0usize;
        let mut out_pos = 0usize;

        loop {
            match &self.state {
                GState::Done => return Ok((pos, out_pos, InflateStatus::Done)),
                GState::Failed(e) => return Err(e.clone()),
                _ => {}
            }

            match self.state.clone() {
                GState::MemberHeader(mut parser) => {
                    let remaining = input.get(pos..).unwrap_or(&[]);
                    match parser.feed(remaining) {
                        Err(e) => return self.fail(e),
                        Ok((consumed, header_opt)) => {
                            pos += consumed;
                            self.total_in += consumed as u64;
                            match header_opt {
                                None => {
                                    self.state = GState::MemberHeader(parser);
                                    return Ok((pos, out_pos, InflateStatus::NeedsInput));
                                }
                                Some(header) => {
                                    self.member_count += 1;
                                    if self.headers.len() < self.max_gzip_headers {
                                        self.headers.push(header);
                                    }
                                    #[cfg(feature = "stats")]
                                    self.stats_done.merge(&self.inflater.stats());
                                    self.inflater = Inflater::new_with_path(self.path);
                                    self.member_crc = Crc32::new();
                                    self.member_len = 0;
                                    self.state = GState::Body;
                                }
                            }
                        }
                    }
                }
                GState::Body => {
                    let remaining_input = input.get(pos..).unwrap_or(&[]);
                    let before = sink.written();
                    match self.inflater.step_into(remaining_input, sink) {
                        Err(e) => return self.fail(e),
                        Ok((consumed, produced, status)) => {
                            self.member_crc.update(sink.written_since(before));
                            self.member_len += produced as u64;
                            pos += consumed;
                            self.total_in += consumed as u64;
                            out_pos += produced;
                            self.total_out += produced as u64;
                            match status {
                                InflateStatus::NeedsInput => {
                                    return Ok((pos, out_pos, InflateStatus::NeedsInput));
                                }
                                InflateStatus::OutputFull => {
                                    return Ok((pos, out_pos, InflateStatus::OutputFull));
                                }
                                InflateStatus::Done => {
                                    self.state = GState::Trailer {
                                        collected: 0,
                                        buf: [0u8; 8],
                                    };
                                }
                            }
                        }
                    }
                }
                GState::Trailer { collected, mut buf } => {
                    if collected >= 8 {
                        let crc_bytes: [u8; 4] = [
                            *buf.first().unwrap_or(&0),
                            *buf.get(1).unwrap_or(&0),
                            *buf.get(2).unwrap_or(&0),
                            *buf.get(3).unwrap_or(&0),
                        ];
                        let isize_bytes: [u8; 4] = [
                            *buf.get(4).unwrap_or(&0),
                            *buf.get(5).unwrap_or(&0),
                            *buf.get(6).unwrap_or(&0),
                            *buf.get(7).unwrap_or(&0),
                        ];
                        let expected_crc = u32::from_le_bytes(crc_bytes);
                        let expected_isize = u32::from_le_bytes(isize_bytes);
                        let crc32_accum = self.member_crc.finish();
                        if expected_crc != crc32_accum {
                            return self.fail(Error::ChecksumMismatch {
                                kind: ChecksumKind::Crc32,
                                expected: expected_crc,
                                actual: crc32_accum,
                            });
                        }
                        let actual_isize = (self.member_len % (1u64 << 32)) as u32;
                        if expected_isize != actual_isize {
                            return self.fail(Error::ChecksumMismatch {
                                kind: ChecksumKind::Isize,
                                expected: expected_isize,
                                actual: actual_isize,
                            });
                        }
                        self.state = GState::Boundary;
                        self.boundary_pending.clear();
                        continue;
                    }
                    match input.get(pos) {
                        None => {
                            self.state = GState::Trailer { collected, buf };
                            return Ok((pos, out_pos, InflateStatus::NeedsInput));
                        }
                        Some(&b) => {
                            pos += 1;
                            self.total_in += 1;
                            if let Some(slot) = buf.get_mut(collected) {
                                *slot = b;
                            }
                            self.state = GState::Trailer {
                                collected: collected + 1,
                                buf,
                            };
                        }
                    }
                }
                GState::Boundary => {
                    if self.boundary_pending.is_empty() {
                        // R1-03: decide by peeking, not consuming. A
                        // byte examined here and found not to start a
                        // new member was never part of any member, so
                        // it must not be counted as consumed -- only
                        // `resolve_trailing`'s caller-visible `pos`
                        // matters, and it must land exactly on
                        // `boundary_start` for `consumed` to agree with
                        // `trailing_start()`.
                        self.boundary_start = self.total_in;
                        let Some(&b) = input.get(pos) else {
                            return Ok((pos, out_pos, InflateStatus::NeedsInput));
                        };
                        if b != 0x1f {
                            return self.resolve_trailing(pos, out_pos);
                        }
                        match input.get(pos + 1) {
                            Some(&0x8b) => {
                                pos += 2;
                                self.total_in += 2;
                                self.state = GState::MemberHeader(GzipHeaderParser::new_primed(
                                    self.max_gzip_field,
                                ));
                            }
                            Some(_) => {
                                // 0x1f not followed by 0x8b: neither
                                // byte belongs to a member. Leave both
                                // unconsumed.
                                return self.resolve_trailing(pos, out_pos);
                            }
                            None => {
                                // The one documented exception: a lone
                                // 0x1f that is the last byte this call
                                // offers cannot be disambiguated yet, so
                                // it must be consumed now (the
                                // NeedsInput-consumes-everything
                                // invariant) and buffered for the next
                                // call or `finish()` to resolve.
                                pos += 1;
                                self.total_in += 1;
                                self.boundary_pending.push(b);
                                return Ok((pos, out_pos, InflateStatus::NeedsInput));
                            }
                        }
                    } else {
                        let Some(&b) = input.get(pos) else {
                            return Ok((pos, out_pos, InflateStatus::NeedsInput));
                        };
                        if b == 0x8b {
                            pos += 1;
                            self.total_in += 1;
                            self.boundary_pending.clear();
                            self.state = GState::MemberHeader(GzipHeaderParser::new_primed(
                                self.max_gzip_field,
                            ));
                        } else {
                            // Not consumed: the buffered 0x1f was
                            // already counted as consumed by the
                            // earlier call that had no choice; this new
                            // byte is only peeked.
                            return self.resolve_trailing(pos, out_pos);
                        }
                    }
                }
                GState::Done | GState::Failed(_) => {}
            }
        }
    }

    /// The boundary examination concluded this is not a new member:
    /// apply the trailing-data policy (RFC 1952 S2.2 concatenation ends
    /// here; SPEC S3.3 trailing-data policy applies to what follows).
    fn resolve_trailing(
        &mut self,
        pos: usize,
        out_pos: usize,
    ) -> Result<(usize, usize, InflateStatus), Error> {
        self.boundary_pending.clear();
        match self.trailing {
            TrailingData::Reject => {
                let offset = self.boundary_start;
                self.fail(Error::TrailingData { offset })
            }
            TrailingData::Allow => {
                self.trailing_start = Some(self.boundary_start);
                self.state = GState::Done;
                Ok((pos, out_pos, InflateStatus::Done))
            }
        }
    }

    /// Called by `Decompressor::finish`: gzip's own notion of "cleanly
    /// stoppable" (SPEC S3.3, RFC 1952 S2.2's Boundary rules), which
    /// differs from `is_done()` because a stream can stop exactly at a
    /// member boundary (no pending disambiguation) once at least one
    /// member is complete, or resolve a still-ambiguous lone `0x1f` via
    /// the trailing-data policy.
    pub(crate) fn finish(&mut self) -> Result<(), Error> {
        if let GState::Failed(e) = &self.state {
            return Err(e.clone());
        }
        if matches!(self.state, GState::Done) {
            return Ok(());
        }
        if matches!(self.state, GState::Boundary) && self.member_count > 0 {
            if self.boundary_pending.is_empty() {
                return Ok(());
            }
            // A lone buffered 0x1f: the stream ended before a second
            // byte could disambiguate it. RFC 1952 gives no meaning to
            // a stray byte here, so it is trailing data either way.
            return match self.trailing {
                TrailingData::Reject => {
                    let offset = self.boundary_start;
                    let e = Error::TrailingData { offset };
                    self.state = GState::Failed(e.clone());
                    Err(e)
                }
                TrailingData::Allow => {
                    self.trailing_start = Some(self.boundary_start);
                    self.state = GState::Done;
                    Ok(())
                }
            };
        }
        let e = Error::Truncated;
        self.state = GState::Failed(e.clone());
        Err(e)
    }
}

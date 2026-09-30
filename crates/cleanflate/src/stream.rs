//! The public streaming (`Decompressor`) and one-shot (`inflate`,
//! `decompress_with`) APIs (SPEC S3.3).

use crate::config::{
    MAX_DEFLATE_EXPANSION, ONESHOT_GROWTH_FLOOR, ONESHOT_INITIAL_RATIO, ONESHOT_MAX_PREALLOC,
};
use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::gzip::{GzipDecoder, GzipHeader};
use crate::inflate::{InflateStatus, Inflater};
use crate::options::{Format, Options, TrailingData};
use crate::zlib::ZlibDecoder;

/// The format-specific decoder a [`Decompressor`] drives. All variants
/// expose the same `step`/`is_done`/`total_out` shape as [`Inflater`]
/// itself, so `Decompressor`'s own logic (limits, trailing data,
/// stickiness) stays format-agnostic; gzip additionally owns its own
/// trailing-data resolution (see `finish_ok`/`skips_generic_trailing`).
#[derive(Debug, Clone)]
enum Backend {
    Raw(Inflater),
    Zlib(ZlibDecoder),
    Gzip(GzipDecoder),
}

impl Backend {
    fn step(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, InflateStatus), Error> {
        match self {
            Backend::Raw(i) => i.step(input, output),
            Backend::Zlib(z) => z.step(input, output),
            Backend::Gzip(g) => g.step(input, output),
        }
    }

    fn is_done(&self) -> bool {
        match self {
            Backend::Raw(i) => i.is_done(),
            Backend::Zlib(z) => z.is_done(),
            Backend::Gzip(g) => g.is_done(),
        }
    }

    fn total_out(&self) -> u64 {
        match self {
            Backend::Raw(i) => i.total_out(),
            Backend::Zlib(z) => z.total_out(),
            Backend::Gzip(g) => g.total_out(),
        }
    }

    /// Gzip resolves its own trailing-data policy inline (it must
    /// consume disambiguating bytes at each member boundary to tell "new
    /// member" from "trailing garbage"), so `Decompressor`'s generic
    /// post-`Done` leftover check must not also apply to it.
    fn owns_trailing_data(&self) -> bool {
        matches!(self, Backend::Gzip(_))
    }

    /// Backend-specific "is this a valid stopping point" check used by
    /// `Decompressor::finish`.
    fn finish(&mut self) -> Result<(), Error> {
        match self {
            Backend::Raw(i) => {
                if i.is_done() {
                    Ok(())
                } else {
                    Err(Error::Truncated)
                }
            }
            Backend::Zlib(z) => {
                if z.is_done() {
                    Ok(())
                } else {
                    Err(Error::Truncated)
                }
            }
            Backend::Gzip(g) => g.finish(),
        }
    }

    #[cfg(feature = "stats")]
    fn stats(&self) -> crate::stats::Stats {
        match self {
            Backend::Raw(i) => i.stats(),
            Backend::Zlib(z) => z.stats(),
            Backend::Gzip(g) => g.stats(),
        }
    }

    fn trailing_start(&self) -> Option<u64> {
        match self {
            Backend::Gzip(g) => g.trailing_start(),
            _ => None,
        }
    }

    fn gzip_headers(&self) -> &[GzipHeader] {
        match self {
            Backend::Gzip(g) => g.headers(),
            _ => &[],
        }
    }

    /// Every gzip member started so far, retained or not (0 for
    /// non-gzip formats).
    fn gzip_member_count(&self) -> u64 {
        match self {
            Backend::Gzip(g) => g.member_count(),
            _ => 0,
        }
    }
}

/// What happened during one [`Decompressor::decompress`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// All of `input` was consumed; more is needed to make progress.
    NeedsInput,
    /// `output` filled up before the stream finished.
    OutputFull,
    /// The stream is fully decoded.
    Done,
}

/// The result of one [`Decompressor::decompress`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes read from `input`.
    pub consumed: usize,
    /// Bytes written to `output`.
    pub produced: usize,
    /// What stopped this call.
    pub status: Status,
}

/// The result of a one-shot [`decompress_with`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The decompressed bytes.
    pub data: Vec<u8>,
    /// Bytes consumed from the input.
    pub consumed: usize,
    /// Parsed headers of every gzip member seen (empty for non-gzip
    /// formats), bounded by [`Options::max_gzip_headers`].
    pub gzip_members: Vec<GzipHeader>,
}

/// A resumable decompressor: feed it arbitrarily chunked input and
/// output buffers (including 1-byte chunks) and it produces identical
/// output regardless of chunking (SPEC S3.3).
#[derive(Debug, Clone)]
pub struct Decompressor {
    backend: Backend,
    options: Options,
    total_in: u64,
    trailing_start: Option<u64>,
    error: Option<Error>,
}

impl Decompressor {
    /// Create a decompressor for `format` with `options`.
    pub fn new(format: Format, options: Options) -> Self {
        Self::new_with_path(format, options, DecodePath::Fast)
    }

    /// [`Self::new`] with an explicit decode path.
    pub(crate) fn new_with_path(format: Format, options: Options, path: DecodePath) -> Self {
        let backend = match format {
            Format::Raw => match &options.dictionary {
                Some(dict) => Backend::Raw(Inflater::with_dictionary(dict, path)),
                None => Backend::Raw(Inflater::new_with_path(path)),
            },
            Format::Zlib => Backend::Zlib(ZlibDecoder::new(options.dictionary.clone(), path)),
            Format::Gzip => Backend::Gzip(GzipDecoder::new(
                options.trailing,
                options.max_gzip_field,
                options.max_gzip_headers,
                path,
            )),
        };
        Decompressor {
            backend,
            options,
            total_in: 0,
            trailing_start: None,
            error: None,
        }
    }

    /// The decode path of the current backend inflater.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn inflater_path(&self) -> DecodePath {
        match &self.backend {
            Backend::Raw(i) => i.path(),
            Backend::Zlib(z) => z.inflater_path(),
            Backend::Gzip(g) => g.inflater_path(),
        }
    }

    /// Feed `input` and write decompressed bytes into `output`. Returns
    /// how much of each was used and why the call stopped. With
    /// [`Status::NeedsInput`], all of `input` was consumed; input is left
    /// unconsumed only with [`Status::OutputFull`] or [`Status::Done`].
    /// Errors are sticky: every call after the first error returns a
    /// clone of it.
    pub fn decompress(&mut self, input: &[u8], output: &mut [u8]) -> Result<Progress, Error> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }

        if self.backend.is_done() {
            return self.handle_post_done_input(input);
        }

        let already_out = self.backend.total_out();
        let remaining_allowance = self.options.max_output.saturating_sub(already_out);
        // The limit is what bounded this call -- not the caller's own
        // output slice -- iff there was no allowance left at all, or the
        // allowance was strictly smaller than what the caller offered.
        // Comparing directly against `output.len()` (rather than the
        // already-clamped `allowance` below) is what makes a zero-length
        // `output` slice distinguishable from a zero-length *allowance*:
        // the former is just the caller asking for no bytes this call
        // (resumable `OutputFull`), the latter is the decompression-bomb
        // guard actually having nothing left to give.
        let limit_bound = remaining_allowance == 0 || remaining_allowance < output.len() as u64;
        let allowance = remaining_allowance.min(output.len() as u64) as usize;
        let output_slice = output.get_mut(..allowance).unwrap_or(&mut []);

        let step_result = self.backend.step(input, output_slice);
        let (consumed, produced, inflate_status) = match step_result {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e.clone());
                return Err(e);
            }
        };
        self.total_in += consumed as u64;

        match inflate_status {
            InflateStatus::NeedsInput => Ok(Progress {
                consumed,
                produced,
                status: Status::NeedsInput,
            }),
            InflateStatus::OutputFull => {
                if limit_bound {
                    let e = Error::OutputLimitExceeded {
                        limit: self.options.max_output,
                    };
                    self.error = Some(e.clone());
                    Err(e)
                } else {
                    Ok(Progress {
                        consumed,
                        produced,
                        status: Status::OutputFull,
                    })
                }
            }
            InflateStatus::Done => {
                let leftover = input.len().saturating_sub(consumed);
                let mut status = Status::Done;
                if leftover > 0 && !self.backend.owns_trailing_data() {
                    match self.options.trailing {
                        TrailingData::Reject => {
                            let offset = self.total_in;
                            let e = Error::TrailingData { offset };
                            self.error = Some(e.clone());
                            return Err(e);
                        }
                        TrailingData::Allow => {
                            if self.trailing_start.is_none() {
                                self.trailing_start = Some(self.total_in);
                            }
                            status = Status::Done;
                        }
                    }
                }
                Ok(Progress {
                    consumed,
                    produced,
                    status,
                })
            }
        }
    }

    /// Handle a `decompress` call made after the stream already reached
    /// `Done`: any input at all is trailing data.
    fn handle_post_done_input(&mut self, input: &[u8]) -> Result<Progress, Error> {
        if !input.is_empty() {
            match self.options.trailing {
                TrailingData::Reject => {
                    let offset = self.total_in;
                    let e = Error::TrailingData { offset };
                    self.error = Some(e.clone());
                    return Err(e);
                }
                TrailingData::Allow => {
                    if self.trailing_start.is_none() {
                        self.trailing_start = Some(self.total_in);
                    }
                }
            }
        }
        Ok(Progress {
            consumed: 0,
            produced: 0,
            status: Status::Done,
        })
    }

    /// Confirm the stream finished cleanly. `Ok(())` if the backend
    /// reached a valid stopping point (for gzip, this may resolve a
    /// still-pending trailing-data decision), else `Err`.
    pub fn finish(&mut self) -> Result<(), Error> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        match self.backend.finish() {
            Ok(()) => Ok(()),
            Err(e) => {
                self.error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Parsed headers of every gzip member seen so far (empty for
    /// non-gzip formats). Bounded by [`Options::max_gzip_headers`]; use
    /// [`Self::gzip_member_count`] for the true number of members seen,
    /// including ones past that cap.
    pub fn gzip_headers(&self) -> &[GzipHeader] {
        self.backend.gzip_headers()
    }

    /// Every gzip member started so far, retained or not (0 for
    /// non-gzip formats). Every member counted here was fully decoded
    /// and checksummed; only its header may have been dropped once
    /// [`Options::max_gzip_headers`] was reached.
    pub fn gzip_member_count(&self) -> u64 {
        self.backend.gzip_member_count()
    }

    /// A snapshot of this decoder's counters (feature `stats`). Gzip
    /// accumulates across members.
    #[cfg(feature = "stats")]
    pub fn stats(&self) -> crate::stats::Stats {
        self.backend.stats()
    }

    /// Total input bytes consumed so far.
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Total output bytes produced so far.
    pub fn total_out(&self) -> u64 {
        self.backend.total_out()
    }

    /// The input offset where trailing data was first observed, if any
    /// (only ever set under [`TrailingData::Allow`]).
    pub fn trailing_start(&self) -> Option<u64> {
        self.backend.trailing_start().or(self.trailing_start)
    }
}

/// Growth schedule for the one-shot decompressor's output buffer: grow by
/// `max(len, ONESHOT_GROWTH_FLOOR)`, never past `limit`, never below `len`.
pub(crate) fn next_len(len: usize, limit: u64) -> usize {
    let growth = (len.max(ONESHOT_GROWTH_FLOOR)) as u64;
    let grown = (len as u64).saturating_add(growth);
    let capped = grown.min(limit).min(usize::MAX as u64);
    capped.max(len as u64) as usize
}

/// Size of the first output allocation of a one-shot decode, from hints
/// only (DESIGN.md D-31): for gzip, the last four bytes read as a
/// little-endian ISIZE (the last member's uncompressed length modulo 2^32,
/// RFC 1952 S2.3.1, unreliable for multi-member streams and trailing data);
/// otherwise `ONESHOT_INITIAL_RATIO` output bytes per input byte. The
/// result never exceeds `max_output`, `ONESHOT_MAX_PREALLOC`, or the most
/// the input could possibly decode to (`MAX_DEFLATE_EXPANSION` bytes per
/// input byte, D-33), and only sets the first allocation: geometric growth
/// continues from there and the output never depends on it.
pub(crate) fn initial_len(format: Format, input: &[u8], max_output: u64) -> usize {
    let isize_hint = match format {
        Format::Gzip if input.len() >= 18 => input
            .len()
            .checked_sub(4)
            .and_then(|start| input.get(start..))
            .and_then(|tail| <[u8; 4]>::try_from(tail).ok())
            .map(|bytes| u32::from_le_bytes(bytes) as usize),
        _ => None,
    };
    let hint = isize_hint.unwrap_or_else(|| input.len().saturating_mul(ONESHOT_INITIAL_RATIO));
    let cap = max_output.min(usize::MAX as u64) as usize;
    let possible = input.len().saturating_mul(MAX_DEFLATE_EXPANSION);
    hint.min(possible).min(cap).min(ONESHOT_MAX_PREALLOC)
}

/// Decompress `input` as `format` with `options`, growing the output
/// buffer as needed up to `options.max_output`.
pub fn decompress_with(format: Format, input: &[u8], options: &Options) -> Result<Decoded, Error> {
    decompress_with_path(format, input, options, DecodePath::Fast)
}

/// [`decompress_with`] with an explicit decode path.
pub(crate) fn decompress_with_path(
    format: Format,
    input: &[u8],
    options: &Options,
    path: DecodePath,
) -> Result<Decoded, Error> {
    decompress_core(format, input, options, path).map(|core| core.decoded)
}

/// [`decompress_with`], also returning the decoder's counters plus the
/// output-buffer growth this one-shot call performed (feature `stats`).
#[cfg(feature = "stats")]
pub fn decompress_with_stats(
    format: Format,
    input: &[u8],
    options: &Options,
) -> Result<(Decoded, crate::stats::Stats), Error> {
    decompress_core(format, input, options, DecodePath::Fast).map(|core| (core.decoded, core.stats))
}

/// What the shared one-shot implementation produces.
struct Core {
    decoded: Decoded,
    #[cfg(feature = "stats")]
    stats: crate::stats::Stats,
}

/// The one implementation behind every one-shot entry point.
fn decompress_core(
    format: Format,
    input: &[u8],
    options: &Options,
    path: DecodePath,
) -> Result<Core, Error> {
    if input.is_empty() {
        return Err(Error::Truncated);
    }
    #[cfg(feature = "stats")]
    let mut resizes = 0u64;
    #[cfg(feature = "stats")]
    let mut allocated = 0u64;

    let mut decompressor = Decompressor::new_with_path(format, options.clone(), path);
    let mut buf: Vec<u8> = Vec::new();
    // Bytes of `buf` holding output; the rest is room still to be filled.
    let mut filled = 0usize;
    let mut in_pos = 0usize;

    // The first allocation comes from the size hints; every later one is a
    // geometric growth step, taken only when the decoder reports that it
    // has run out of room (`OutputFull`), so a decode that ends with room
    // to spare -- or exactly full, as a gzip stream does at a member
    // boundary -- never pays for a buffer it does not need.
    let first = next_len(0, options.max_output).max(initial_len(format, input, options.max_output));
    if first > 0 {
        stat!(resizes, 1);
        stat!(allocated, first);
        buf.resize(first, 0);
    }

    loop {
        let remaining_input = input.get(in_pos..).unwrap_or(&[]);
        let write_region = buf.get_mut(filled..).unwrap_or(&mut []);
        let progress = decompressor.decompress(remaining_input, write_region)?;
        in_pos += progress.consumed;
        filled += progress.produced;

        match progress.status {
            Status::Done => break,
            Status::OutputFull => {
                // The room ran out. Grow, unless the buffer is already at
                // the output limit: it cannot grow past `next_len`'s cap,
                // so the next call would offer an empty region, get
                // `OutputFull` again with nothing consumed or produced, and
                // `continue` would spin forever. Decide this from the
                // buffer itself, not from re-deriving the limit arithmetic
                // `decompress` already did, so a bug in that arithmetic
                // cannot reintroduce the hang here too.
                let target_len = next_len(buf.len(), options.max_output);
                if target_len <= buf.len() {
                    return Err(Error::OutputLimitExceeded {
                        limit: options.max_output,
                    });
                }
                stat!(resizes, 1);
                stat!(allocated, target_len - buf.len());
                buf.resize(target_len, 0);
            }
            Status::NeedsInput => {
                if progress.consumed == 0 {
                    break;
                }
            }
        }
    }

    buf.truncate(filled);
    decompressor.finish()?;
    // `trailing_start()`, when set, is the exact offset where the member
    // stream ended -- gzip may have had to consume one more byte than
    // that to disambiguate a lone trailing `0x1f` (RFC 1952 gives no
    // meaning to a stray byte at a member boundary; PLAN R1-03), and
    // that extra consumed byte must not be reported as part of the
    // decoded stream since the one-shot caller sees the whole input at
    // once and has no "streaming exception" to excuse it.
    let consumed = decompressor
        .trailing_start()
        .map(|offset| offset as usize)
        .unwrap_or(in_pos);
    #[cfg(feature = "stats")]
    let stats = {
        let mut s = decompressor.stats();
        s.oneshot_resizes = resizes;
        s.oneshot_bytes_allocated = allocated;
        s
    };
    Ok(Core {
        decoded: Decoded {
            data: buf,
            consumed,
            gzip_members: decompressor.gzip_headers().to_vec(),
        },
        #[cfg(feature = "stats")]
        stats,
    })
}

/// Decompress a raw RFC 1951 DEFLATE stream with default options.
pub fn inflate(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with(Format::Raw, input, &Options::default()).map(|d| d.data)
}

/// Decompress an RFC 1950 zlib stream with default options. A stream
/// requiring a preset dictionary fails with
/// [`Error::PresetDictionaryRequired`]; use [`decompress_with`] with
/// [`Options::dictionary`] set to supply one.
pub fn zlib_decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with(Format::Zlib, input, &Options::default()).map(|d| d.data)
}

/// Decompress an RFC 1952 gzip stream (one or more concatenated
/// members) with default options.
pub fn gzip_decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with(Format::Gzip, input, &Options::default()).map(|d| d.data)
}

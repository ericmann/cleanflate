//! The public streaming (`Decompressor`) and one-shot (`inflate`,
//! `decompress_with`) APIs (SPEC S3.3).

use crate::error::Error;
use crate::inflate::{InflateStatus, Inflater};
use crate::options::{Format, Options, TrailingData};

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
}

/// A resumable decompressor: feed it arbitrarily chunked input and
/// output buffers (including 1-byte chunks) and it produces identical
/// output regardless of chunking (SPEC S3.3).
#[derive(Debug, Clone)]
pub struct Decompressor {
    inflater: Inflater,
    options: Options,
    total_in: u64,
    trailing_start: Option<u64>,
    error: Option<Error>,
}

impl Decompressor {
    /// Create a decompressor for `format` with `options`. Only
    /// [`Format::Raw`] exists so far; zlib and gzip arrive in Phase 3.
    pub fn new(format: Format, options: Options) -> Self {
        let Format::Raw = format;
        Decompressor {
            inflater: Inflater::new(),
            options,
            total_in: 0,
            trailing_start: None,
            error: None,
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

        if self.inflater.is_done() {
            return self.handle_post_done_input(input);
        }

        let already_out = self.inflater.total_out();
        let remaining_allowance = self.options.max_output.saturating_sub(already_out);
        let allowance = remaining_allowance.min(output.len() as u64) as usize;
        let allowance_bound_output = allowance < output.len();
        let output_slice = output.get_mut(..allowance).unwrap_or(&mut []);

        let step_result = self.inflater.step(input, output_slice);
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
                if allowance_bound_output || allowance == 0 {
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
                if leftover > 0 {
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

    /// Confirm the stream finished cleanly. `Ok(())` if [`Status::Done`]
    /// was reached, else `Err(Error::Truncated)`.
    pub fn finish(&mut self) -> Result<(), Error> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        if self.inflater.is_done() {
            Ok(())
        } else {
            let e = Error::Truncated;
            self.error = Some(e.clone());
            Err(e)
        }
    }

    /// Total input bytes consumed so far.
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Total output bytes produced so far.
    pub fn total_out(&self) -> u64 {
        self.inflater.total_out()
    }

    /// The input offset where trailing data was first observed, if any
    /// (only ever set under [`TrailingData::Allow`]).
    pub fn trailing_start(&self) -> Option<u64> {
        self.trailing_start
    }
}

/// Growth schedule for the one-shot decompressor's output buffer: grow by
/// `max(len, 32768)`, never past `limit`, never below `len`.
pub(crate) fn next_len(len: usize, limit: u64) -> usize {
    let growth = (len.max(32768)) as u64;
    let grown = (len as u64).saturating_add(growth);
    let capped = grown.min(limit).min(usize::MAX as u64);
    capped.max(len as u64) as usize
}

/// Decompress `input` as `format` with `options`, growing the output
/// buffer as needed up to `options.max_output`.
pub fn decompress_with(format: Format, input: &[u8], options: &Options) -> Result<Decoded, Error> {
    if input.is_empty() {
        return Err(Error::Truncated);
    }

    let mut decompressor = Decompressor::new(format, options.clone());
    let mut buf: Vec<u8> = Vec::new();
    let mut in_pos = 0usize;

    loop {
        let cur_len = buf.len();
        let target_len = next_len(cur_len, options.max_output);
        if target_len > cur_len {
            buf.resize(target_len, 0);
        }
        let remaining_input = input.get(in_pos..).unwrap_or(&[]);
        let write_region = buf.get_mut(cur_len..).unwrap_or(&mut []);
        let progress = decompressor.decompress(remaining_input, write_region)?;
        in_pos += progress.consumed;
        buf.truncate(cur_len + progress.produced);

        match progress.status {
            Status::Done => break,
            Status::OutputFull => continue,
            Status::NeedsInput => {
                if progress.consumed == 0 {
                    break;
                }
            }
        }
    }

    decompressor.finish()?;
    Ok(Decoded {
        data: buf,
        consumed: in_pos,
    })
}

/// Decompress a raw RFC 1951 DEFLATE stream with default options.
pub fn inflate(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with(Format::Raw, input, &Options::default()).map(|d| d.data)
}

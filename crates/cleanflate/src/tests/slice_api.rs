//! Slice-shaped entry points for the unit tests. The decoder core writes
//! through an `OutputSink`; these inherent methods wrap a caller slice in a
//! `SliceSink` so the tests can keep driving the core the way the streaming
//! path does.

use crate::decode_path::DecodePath;
use crate::error::Error;
use crate::gzip::GzipDecoder;
use crate::inflate::{InflateStatus, Inflater};
use crate::sink::SliceSink;
use crate::window::Window;
use crate::zlib::ZlibDecoder;

impl Inflater {
    /// [`Inflater::step_into`] over a plain slice.
    pub(crate) fn step(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, InflateStatus), Error> {
        let mut sink = SliceSink::new(output);
        self.step_into(input, &mut sink)
    }
}

impl ZlibDecoder {
    /// [`ZlibDecoder::step_into`] over a plain slice.
    pub(crate) fn step(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, InflateStatus), Error> {
        let mut sink = SliceSink::new(output);
        self.step_into(input, &mut sink)
    }
}

impl GzipDecoder {
    /// [`GzipDecoder::step_into`] over a plain slice.
    pub(crate) fn step(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, InflateStatus), Error> {
        let mut sink = SliceSink::new(output);
        self.step_into(input, &mut sink)
    }
}

impl Window {
    /// [`Window::copy_match_sink`] over a plain slice: the match is written
    /// at `output[cur..]` and its history is the window followed by
    /// `output[..cur]`.
    pub(crate) fn copy_match(
        &self,
        path: DecodePath,
        output: &mut [u8],
        cur: usize,
        dist: usize,
        n: usize,
    ) -> usize {
        let mut sink = SliceSink::at(output, cur);
        self.copy_match_sink(path, &mut sink, 0, dist, n)
    }
}

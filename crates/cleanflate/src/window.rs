//! The 32 KiB DEFLATE history (RFC 1951 S2: "distance is drawn from
//! (1..32,768)"), including overlapping-copy semantics.
//!
//! Within one `step` call the output sink is the match history: history is
//! the window as it stood at call entry followed by the bytes the sink
//! received since then. The window itself only keeps what must survive
//! across calls, and is refreshed from the output at every exit (DESIGN.md
//! D-30).

use crate::decode_path::DecodePath;
use crate::sink::OutputSink;
use crate::tables::WINDOW_SIZE;

/// A ring buffer of the last `WINDOW_SIZE` bytes of history, used to
/// resolve length/distance back-references that reach back before the
/// current call's output.
#[derive(Debug, Clone)]
pub(crate) struct Window {
    buf: [u8; WINDOW_SIZE],
    /// Total bytes ever appended (including bytes now overwritten in the
    /// ring, and a preset dictionary), so distance and availability
    /// arithmetic is right even past the first `WINDOW_SIZE` bytes. The
    /// byte with absolute index `i` lives at `buf[i % WINDOW_SIZE]`.
    total: u64,
    /// Bytes actually copied into the ring by `append` (at most
    /// `WINDOW_SIZE` per call), not counting dictionary prefill.
    #[cfg(feature = "stats")]
    pub(crate) written: u64,
}

impl Window {
    /// An empty window.
    pub(crate) fn new() -> Self {
        Window {
            buf: [0u8; WINDOW_SIZE],
            total: 0,
            #[cfg(feature = "stats")]
            written: 0,
        }
    }

    /// Forget all history, keeping the storage. The ring's old bytes stay
    /// in place but are unreachable: every read is bounded by
    /// [`Self::available`], which is 0 again.
    pub(crate) fn reset(&mut self) {
        self.total = 0;
        #[cfg(feature = "stats")]
        {
            self.written = 0;
        }
    }

    /// Append `data`, keeping only the last `WINDOW_SIZE` bytes, in at most
    /// two slice copies.
    pub(crate) fn append(&mut self, data: &[u8]) {
        let n = data.len();
        if n == 0 {
            return;
        }
        // Only the last WINDOW_SIZE bytes can survive.
        let skip = n.saturating_sub(WINDOW_SIZE);
        let src = data.get(skip..).unwrap_or(&[]);
        let start = ((self.total + skip as u64) % WINDOW_SIZE as u64) as usize;
        let first = src.len().min(WINDOW_SIZE - start);
        if let (Some(dst), Some(s)) = (self.buf.get_mut(start..start + first), src.get(..first)) {
            dst.copy_from_slice(s);
        }
        let rest = src.len() - first;
        if rest > 0 {
            if let (Some(dst), Some(s)) = (self.buf.get_mut(..rest), src.get(first..)) {
                dst.copy_from_slice(s);
            }
        }
        self.total += n as u64;
        stat!(self.written, src.len());
    }

    /// Bytes currently available: `min(total, WINDOW_SIZE)`.
    pub(crate) fn available(&self) -> usize {
        self.total.min(WINDOW_SIZE as u64) as usize
    }

    /// History available to a match at output position `out_pos` of the
    /// current call: the window plus the output so far, capped at what a
    /// distance can reach.
    pub(crate) fn history_available(&self, out_pos: usize) -> usize {
        self.available().saturating_add(out_pos).min(WINDOW_SIZE)
    }

    /// The byte `distance` positions back from the end of the window
    /// (distance 1 = the last byte appended). `None` if `distance` is 0 or
    /// exceeds what is available.
    pub(crate) fn get_back(&self, distance: usize) -> Option<u8> {
        if distance == 0 || distance > self.available() {
            return None;
        }
        let idx = (self.total - distance as u64) % WINDOW_SIZE as u64;
        self.buf.get(idx as usize).copied()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    /// Copy `dst.len()` bytes that start `back` bytes before the end of
    /// the window, in at most two slice copies. Requires
    /// `dst.len() <= back <= available()`; returns `false` and leaves `dst`
    /// alone otherwise.
    pub(crate) fn read_back(&self, back: usize, dst: &mut [u8]) -> bool {
        let n = dst.len();
        if n == 0 {
            return true;
        }
        if back < n || back > self.available() {
            return false;
        }
        let start = ((self.total - back as u64) % WINDOW_SIZE as u64) as usize;
        let first = n.min(WINDOW_SIZE - start);
        if let (Some(d), Some(s)) = (dst.get_mut(..first), self.buf.get(start..start + first)) {
            d.copy_from_slice(s);
        }
        let rest = n - first;
        if rest > 0 {
            if let (Some(d), Some(s)) = (dst.get_mut(first..), self.buf.get(..rest)) {
                d.copy_from_slice(s);
            }
        }
        true
    }

    /// Seed the window with `data`, keeping only the last `WINDOW_SIZE`
    /// bytes (used for preset dictionaries).
    pub(crate) fn prefill(&mut self, data: &[u8]) {
        self.append(data);
        #[cfg(feature = "stats")]
        {
            self.written = 0;
        }
    }

    /// The two contiguous pieces of the ring that hold the `n` bytes that
    /// start `back` bytes before the end of the window, in order (the
    /// second is empty unless the span wraps). Requires `n <= back <=
    /// available()`; `None` otherwise.
    pub(crate) fn back_parts(&self, back: usize, n: usize) -> Option<(&[u8], &[u8])> {
        if back < n || back > self.available() {
            return None;
        }
        let start = ((self.total - back as u64) % WINDOW_SIZE as u64) as usize;
        let first = n.min(WINDOW_SIZE - start);
        let a = self.buf.get(start..start + first)?;
        let b = self.buf.get(..n - first)?;
        Some((a, b))
    }

    /// Copy a length/distance match into the sink: up to `n` bytes,
    /// bounded by the sink's room, returning how many were written (0 if
    /// `dist` is 0 or reaches beyond the history available). The history
    /// is the window followed by the sink's bytes since offset `base` (the
    /// caller's own part of the output), and an overlapping match (`dist`
    /// smaller than the length) repeats the pattern it has just written,
    /// as RFC 1951 S3.2.3 describes.
    ///
    /// `Fast` copies in slices: bytes that reach into the window come from
    /// [`Self::back_parts`], the rest from earlier in the sink's output with
    /// `copy_within_back`, at most `dist` bytes at a time so that source
    /// and destination never overlap. `Reference` copies byte by byte.
    #[inline]
    pub(crate) fn copy_match_sink<S: OutputSink>(
        &self,
        path: DecodePath,
        sink: &mut S,
        base: usize,
        dist: usize,
        n: usize,
    ) -> usize {
        let cur = sink.written().saturating_sub(base);
        let n = n.min(sink.room());
        if n == 0 || dist == 0 || dist > self.history_available(cur) {
            return 0;
        }
        match path {
            DecodePath::Fast => self.copy_match_slices(sink, cur, dist, n),
            DecodePath::Reference => self.copy_match_bytewise(sink, cur, dist, n),
        }
    }

    #[inline]
    fn copy_match_slices<S: OutputSink>(
        &self,
        sink: &mut S,
        cur: usize,
        dist: usize,
        n: usize,
    ) -> usize {
        let mut c = cur;
        let mut left = n;
        while left > 0 {
            if dist > c {
                // Reaches into the window: `dist - c` bytes before its end.
                let chunk = left.min(dist - c);
                let Some((a, b)) = self.back_parts(dist - c, chunk) else {
                    break;
                };
                let wrote = sink.extend_from_slice(a) + sink.extend_from_slice(b);
                c += wrote;
                left -= wrote;
                if wrote < chunk {
                    break;
                }
            } else {
                let wrote = sink.copy_within_back(dist, left.min(dist));
                if wrote == 0 {
                    break;
                }
                c += wrote;
                left -= wrote;
            }
        }
        c - cur
    }

    fn copy_match_bytewise<S: OutputSink>(
        &self,
        sink: &mut S,
        cur: usize,
        dist: usize,
        n: usize,
    ) -> usize {
        let mut written = 0;
        for i in 0..n {
            let c = cur + i;
            let byte = if dist <= c {
                sink.get_back(dist)
            } else {
                self.get_back(dist - c)
            };
            let Some(byte) = byte else { break };
            if !sink.push(byte) {
                break;
            }
            written += 1;
        }
        written
    }
}

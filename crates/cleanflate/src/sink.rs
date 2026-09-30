//! Where the decoder core writes its output (DESIGN.md "Output sinks for
//! the decoder core").
//!
//! The core never touches the caller's buffer directly: every output write
//! and every look-back at this call's own output goes through an
//! [`OutputSink`]. The streaming path writes into the caller's slice
//! ([`SliceSink`]); the trait is what lets the one-shot path append to a
//! growable buffer instead without a second decoder. The trait is used
//! generically (monomorphised), so a sink costs no dynamic dispatch.

/// A destination for decoded bytes. "This call" means everything written
/// through the sink since it was created; the decoder core remembers where
/// its own part of that output starts.
pub(crate) trait OutputSink {
    /// Bytes written in this call so far.
    fn written(&self) -> usize;

    /// Bytes that may still be written in this call.
    fn room(&self) -> usize;

    /// Append one byte. `false` means there was no room and nothing was
    /// written.
    fn push(&mut self, byte: u8) -> bool;

    /// Append `min(data.len(), room())` bytes of `data`; returns how many.
    fn extend_from_slice(&mut self, data: &[u8]) -> usize;

    /// Append `n` bytes copied from `dist` bytes back in this call's
    /// output, at most `room()` of them; returns how many were written.
    /// Requires `1 <= dist <= written()` and `n <= dist` (source and
    /// destination do not overlap); returns 0 if a precondition fails.
    fn copy_within_back(&mut self, dist: usize, n: usize) -> usize;

    /// The byte `dist` positions back from the end of this call's output
    /// (1 = the last byte written), or `None` if `dist` is 0 or reaches
    /// before the start of the output.
    fn get_back(&self, dist: usize) -> Option<u8>;

    /// This call's bytes from offset `start` on (empty if `start` is past
    /// the end).
    fn written_since(&self, start: usize) -> &[u8];
}

/// The caller-slice sink: writes into `buf` from position 0 and never
/// grows it.
pub(crate) struct SliceSink<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> SliceSink<'a> {
    /// A sink over `buf`, positioned at its start.
    #[inline]
    pub(crate) fn new(buf: &'a mut [u8]) -> Self {
        SliceSink { buf, pos: 0 }
    }

    /// A sink over `buf` that already holds `pos` bytes of output (the
    /// slice-API helpers of the tests start mid-buffer).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn at(buf: &'a mut [u8], pos: usize) -> Self {
        let pos = pos.min(buf.len());
        SliceSink { buf, pos }
    }
}

impl OutputSink for SliceSink<'_> {
    #[inline]
    fn written(&self) -> usize {
        self.pos
    }

    #[inline]
    fn room(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    #[inline]
    fn push(&mut self, byte: u8) -> bool {
        match self.buf.get_mut(self.pos) {
            Some(slot) => {
                *slot = byte;
                self.pos += 1;
                true
            }
            None => false,
        }
    }

    #[inline]
    fn extend_from_slice(&mut self, data: &[u8]) -> usize {
        let n = data.len().min(self.room());
        let end = self.pos + n;
        if let (Some(dst), Some(src)) = (self.buf.get_mut(self.pos..end), data.get(..n)) {
            dst.copy_from_slice(src);
            self.pos = end;
            n
        } else {
            0
        }
    }

    #[inline]
    fn copy_within_back(&mut self, dist: usize, n: usize) -> usize {
        if dist == 0 || dist > self.pos || n > dist {
            return 0;
        }
        let n = n.min(self.room());
        let src = self.pos - dist;
        if src + n > self.buf.len() || self.pos + n > self.buf.len() {
            return 0;
        }
        self.buf.copy_within(src..src + n, self.pos);
        self.pos += n;
        n
    }

    #[inline]
    fn get_back(&self, dist: usize) -> Option<u8> {
        if dist == 0 || dist > self.pos {
            return None;
        }
        self.buf.get(self.pos - dist).copied()
    }

    #[inline]
    fn written_since(&self, start: usize) -> &[u8] {
        self.buf.get(start..self.pos).unwrap_or(&[])
    }
}

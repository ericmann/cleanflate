//! The 32 KiB DEFLATE history (RFC 1951 S2: "distance is drawn from
//! (1..32,768)"), including overlapping-copy semantics.
//!
//! Within one `step` call the caller's output slice is the match history:
//! history is the window as it stood at call entry followed by
//! `output[..out_pos]`. The window itself only keeps what must survive
//! across calls, and is refreshed from the output at every exit (DESIGN.md
//! D-30).

use crate::decode_path::DecodePath;
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

    /// Copy a length/distance match into `output[cur..]`: up to `n` bytes,
    /// bounded by the room left in `output`, returning how many were
    /// written (0 if `dist` is 0 or reaches beyond the history
    /// available). The history is the window followed by `output[..cur]`,
    /// and an overlapping match (`dist` smaller than the length) repeats
    /// the pattern it has just written, as RFC 1951 S3.2.3 describes.
    ///
    /// `Fast` copies in slices: bytes that reach into the window come from
    /// [`Self::read_back`], the rest from an earlier part of `output` with
    /// `copy_within`, at most `dist` bytes at a time so that source and
    /// destination never overlap. `Reference` copies byte by byte.
    pub(crate) fn copy_match(
        &self,
        path: DecodePath,
        output: &mut [u8],
        cur: usize,
        dist: usize,
        n: usize,
    ) -> usize {
        let n = n.min(output.len().saturating_sub(cur));
        if n == 0 || dist == 0 || dist > self.history_available(cur) {
            return 0;
        }
        match path {
            DecodePath::Fast => self.copy_match_slices(output, cur, dist, n),
            DecodePath::Reference => self.copy_match_bytewise(output, cur, dist, n),
        }
    }

    fn copy_match_slices(&self, output: &mut [u8], cur: usize, dist: usize, n: usize) -> usize {
        let mut c = cur;
        let mut left = n;
        while left > 0 {
            if dist > c {
                // Reaches into the window: `dist - c` bytes before its end.
                let chunk = left.min(dist - c);
                let ok = output
                    .get_mut(c..c + chunk)
                    .is_some_and(|dst| self.read_back(dist - c, dst));
                if !ok {
                    break;
                }
                c += chunk;
                left -= chunk;
            } else {
                let chunk = left.min(dist);
                let src = c - dist;
                if src + chunk > output.len() || c + chunk > output.len() {
                    break;
                }
                output.copy_within(src..src + chunk, c);
                c += chunk;
                left -= chunk;
            }
        }
        c - cur
    }

    fn copy_match_bytewise(&self, output: &mut [u8], cur: usize, dist: usize, n: usize) -> usize {
        let mut written = 0;
        for i in 0..n {
            let c = cur + i;
            let byte = if dist <= c {
                output.get(c - dist).copied()
            } else {
                self.get_back(dist - c)
            };
            let Some(byte) = byte else { break };
            match output.get_mut(c) {
                Some(slot) => *slot = byte,
                None => break,
            }
            written += 1;
        }
        written
    }
}

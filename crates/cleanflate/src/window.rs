//! The 32 KiB DEFLATE history window (RFC 1951 S2: "distance is drawn
//! from (1..32,768)"), including overlapping-copy semantics.

use crate::tables::WINDOW_SIZE;

/// A ring buffer of the last `WINDOW_SIZE` bytes produced, used to
/// resolve length/distance back-references.
#[derive(Debug, Clone)]
pub(crate) struct Window {
    buf: [u8; WINDOW_SIZE],
    /// Total bytes ever pushed (including bytes now overwritten in the
    /// ring), so distance/availability arithmetic is correct even past
    /// the first `WINDOW_SIZE` bytes.
    total: u64,
}

impl Window {
    /// An empty window.
    pub(crate) fn new() -> Self {
        Window {
            buf: [0u8; WINDOW_SIZE],
            total: 0,
        }
    }

    /// Push one byte, overwriting the oldest byte once the ring is full.
    pub(crate) fn push(&mut self, byte: u8) {
        let idx = (self.total % WINDOW_SIZE as u64) as usize;
        if let Some(slot) = self.buf.get_mut(idx) {
            *slot = byte;
        }
        self.total += 1;
    }

    /// Bytes currently available to copy from: `min(total, WINDOW_SIZE)`.
    pub(crate) fn available(&self) -> usize {
        self.total.min(WINDOW_SIZE as u64) as usize
    }

    /// The byte `distance` positions back from the most recently pushed
    /// byte (distance 1 = the last byte pushed). `None` if `distance` is
    /// 0 or exceeds what is available.
    pub(crate) fn get_back(&self, distance: usize) -> Option<u8> {
        if distance == 0 || distance > self.available() {
            return None;
        }
        let idx = (self.total - distance as u64) % WINDOW_SIZE as u64;
        self.buf.get(idx as usize).copied()
    }

    /// Seed the window with `data`, keeping only the last `WINDOW_SIZE`
    /// bytes (used for preset dictionaries).
    pub(crate) fn prefill(&mut self, data: &[u8]) {
        for &byte in data {
            self.push(byte);
        }
    }

    /// Copy a length/distance match into `out`, pushing each copied byte
    /// into the window as it goes so an overlapping copy (`distance <
    /// len`) correctly repeats the pattern already written into `out`.
    /// Writes at most `out.len()` bytes (bounded also by `len`) and
    /// returns how many were written; returns 0 if `distance` is
    /// unavailable, which should not happen for a `distance` the caller
    /// already validated against `available()`.
    ///
    /// When `distance >= n` (no overlap possible: every source byte was
    /// already final history before this match started), the copy uses
    /// [`Self::copy_match_bulk`], up to two contiguous slice copies each
    /// way instead of `n` single-byte `get_back`/`push` calls (P4-08:
    /// SPEC S5 M4 performance pass; the guarded fast decode path in
    /// `inflate.rs` is the main beneficiary, but every caller gets this
    /// for free since it changes nothing observable).
    pub(crate) fn copy_match(&mut self, distance: usize, len: usize, out: &mut [u8]) -> usize {
        let n = len.min(out.len());
        if n == 0 {
            return 0;
        }
        if distance == 0 || distance > self.available() {
            return 0;
        }
        if distance >= n {
            return self.copy_match_bulk(distance, n, out);
        }
        let mut written = 0;
        for slot in out.iter_mut().take(n) {
            let Some(byte) = self.get_back(distance) else {
                break;
            };
            *slot = byte;
            self.push(byte);
            written += 1;
        }
        written
    }

    /// Bulk copy for the non-overlapping case (`distance >= n`). `n` is
    /// always <= 258 (RFC 1951 S3.2.5's longest match), so a small stack
    /// scratch buffer holds the whole match: it is read from the ring in
    /// at most two contiguous pieces (splitting only at the ring's own
    /// wraparound point), copied whole into `out`, then written back into
    /// the ring in at most two contiguous pieces. Using a scratch buffer
    /// (rather than copying ring-to-`out` and ring-to-ring directly)
    /// avoids holding both a `&self.buf` read and a `&mut self.buf` write
    /// at once, with no allocation.
    fn copy_match_bulk(&mut self, distance: usize, n: usize, out: &mut [u8]) -> usize {
        debug_assert!(n <= 258, "DEFLATE matches are never longer than 258 bytes");
        let mut scratch = [0u8; 258];
        let src_start = ((self.total - distance as u64) % WINDOW_SIZE as u64) as usize;

        let mut read = 0usize;
        while read < n {
            let src_pos = (src_start + read) % WINDOW_SIZE;
            let chunk = (WINDOW_SIZE - src_pos).min(n - read);
            let Some(src) = self.buf.get(src_pos..src_pos + chunk) else {
                break;
            };
            if let Some(dst) = scratch.get_mut(read..read + chunk) {
                dst.copy_from_slice(src);
            }
            read += chunk;
        }
        if read < n {
            // Unreachable in practice (distance already validated), but
            // keep the "stops early without error" contract rather than
            // writing a partial/garbage match.
            return 0;
        }

        if let (Some(dst), Some(src)) = (out.get_mut(..n), scratch.get(..n)) {
            dst.copy_from_slice(src);
        }

        let dst_start = (self.total % WINDOW_SIZE as u64) as usize;
        let mut written = 0usize;
        while written < n {
            let dst_pos = (dst_start + written) % WINDOW_SIZE;
            let chunk = (WINDOW_SIZE - dst_pos).min(n - written);
            if let (Some(dst), Some(src)) = (
                self.buf.get_mut(dst_pos..dst_pos + chunk),
                scratch.get(written..written + chunk),
            ) {
                dst.copy_from_slice(src);
            }
            written += chunk;
        }
        self.total += n as u64;
        n
    }
}

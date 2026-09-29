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
    #[allow(dead_code)] // used once zlib preset dictionaries arrive (Phase 3)
    pub(crate) fn prefill(&mut self, data: &[u8]) {
        for &byte in data {
            self.push(byte);
        }
    }

    /// Copy a length/distance match into `out`, byte by byte, pushing
    /// each copied byte into the window as it goes so an overlapping
    /// copy (`distance < len`) correctly repeats the pattern already
    /// written into `out`. Writes at most `out.len()` bytes (bounded also
    /// by `len`) and returns how many were written; stops early (without
    /// error) if `distance` becomes unavailable, which should not happen
    /// for a `distance` the caller already validated against
    /// `available()`.
    pub(crate) fn copy_match(&mut self, distance: usize, len: usize, out: &mut [u8]) -> usize {
        let n = len.min(out.len());
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
}

//! Opt-in decoder counters (feature `stats`). With the feature off, none
//! of the counter fields or increments exist: `stat!` expands to nothing
//! and this module holds only that macro.

/// Add `$n` to the counter `$place`, saturating. Compiled to nothing
/// without the `stats` feature.
#[cfg(feature = "stats")]
macro_rules! stat {
    ($place:expr, $n:expr) => {
        $place = $place.saturating_add($n as u64)
    };
}

/// Without the `stats` feature the counters do not exist.
#[cfg(not(feature = "stats"))]
macro_rules! stat {
    ($place:expr, $n:expr) => {};
}

/// Counters describing one decoder's work. Everything is plain data held
/// by the decoder itself: no clocks and no global state, so building a
/// table is counted in calls and entries, never in time.
#[cfg(feature = "stats")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    /// Calls to the inflater's `step`.
    pub step_calls: u64,
    /// Iterations of `step`'s state loop.
    pub state_dispatches: u64,
    /// Bytes written to the caller's output.
    pub output_bytes: u64,
    /// Stored blocks started.
    pub blocks_stored: u64,
    /// Fixed-Huffman blocks started.
    pub blocks_fixed: u64,
    /// Dynamic-Huffman blocks started.
    pub blocks_dynamic: u64,
    /// Bytes copied out of stored blocks.
    pub stored_bytes: u64,
    /// Literal symbols decoded.
    pub literals: u64,
    /// Length/distance matches decoded.
    pub matches: u64,
    /// Total bytes covered by matches.
    pub match_bytes: u64,
    /// Matches whose distance is smaller than their length.
    pub overlap_matches: u64,
    /// Match lengths bucketed 3-4, 5-8, 9-16, 17-32, 33-64, 65-128, 129-258.
    pub match_len_hist: [u64; 7],
    /// Match distances bucketed 1, 2-4, 5-16, 17-64, 65-256, 257-1024,
    /// 1025-4096, 4097-16384, 16385-32768.
    pub match_dist_hist: [u64; 9],
    /// Literal/length symbols resolved by the lookup table.
    pub litlen_table_decodes: u64,
    /// Literal/length symbols resolved by the bit-serial walk.
    pub litlen_serial_decodes: u64,
    /// Distance symbols resolved by the lookup table.
    pub dist_table_decodes: u64,
    /// Distance symbols resolved by the bit-serial walk.
    pub dist_serial_decodes: u64,
    /// Code-length symbols resolved by the lookup table.
    pub codelen_table_decodes: u64,
    /// Code-length symbols resolved by the bit-serial walk.
    pub codelen_serial_decodes: u64,
    /// Bit-buffer fills that loaded at least one byte.
    pub refills: u64,
    /// Bytes loaded by those fills.
    pub refill_bytes: u64,
    /// Bytes returned unread to the input by an exit path.
    pub given_back_bytes: u64,
    /// Huffman tables built while decoding block headers (the two fixed
    /// tables built when a decoder is created are not counted).
    pub table_builds: u64,
    /// Lookup-table entries allocated by those builds.
    pub table_entries_built: u64,
    /// Fixed tables cloned into place (two per fixed block).
    pub fixed_table_clones: u64,
    /// Bytes written into the history window (a preset dictionary is not
    /// counted).
    pub window_bytes_written: u64,
    /// Entries into a specialised fast decode loop.
    pub fast_loop_entries: u64,
    /// Symbols decoded inside that loop.
    pub fast_loop_symbols: u64,
    /// Output-buffer growths made by `decompress_with_stats`.
    pub oneshot_resizes: u64,
    /// Bytes added by those growths.
    pub oneshot_bytes_allocated: u64,
}

#[cfg(feature = "stats")]
impl Stats {
    /// Add every counter of `other` into `self`.
    pub fn merge(&mut self, other: &Stats) {
        fn add(a: &mut u64, b: u64) {
            *a = a.saturating_add(b);
        }
        add(&mut self.step_calls, other.step_calls);
        add(&mut self.state_dispatches, other.state_dispatches);
        add(&mut self.output_bytes, other.output_bytes);
        add(&mut self.blocks_stored, other.blocks_stored);
        add(&mut self.blocks_fixed, other.blocks_fixed);
        add(&mut self.blocks_dynamic, other.blocks_dynamic);
        add(&mut self.stored_bytes, other.stored_bytes);
        add(&mut self.literals, other.literals);
        add(&mut self.matches, other.matches);
        add(&mut self.match_bytes, other.match_bytes);
        add(&mut self.overlap_matches, other.overlap_matches);
        for (a, b) in self.match_len_hist.iter_mut().zip(&other.match_len_hist) {
            add(a, *b);
        }
        for (a, b) in self.match_dist_hist.iter_mut().zip(&other.match_dist_hist) {
            add(a, *b);
        }
        add(&mut self.litlen_table_decodes, other.litlen_table_decodes);
        add(&mut self.litlen_serial_decodes, other.litlen_serial_decodes);
        add(&mut self.dist_table_decodes, other.dist_table_decodes);
        add(&mut self.dist_serial_decodes, other.dist_serial_decodes);
        add(&mut self.codelen_table_decodes, other.codelen_table_decodes);
        add(
            &mut self.codelen_serial_decodes,
            other.codelen_serial_decodes,
        );
        add(&mut self.refills, other.refills);
        add(&mut self.refill_bytes, other.refill_bytes);
        add(&mut self.given_back_bytes, other.given_back_bytes);
        add(&mut self.table_builds, other.table_builds);
        add(&mut self.table_entries_built, other.table_entries_built);
        add(&mut self.fixed_table_clones, other.fixed_table_clones);
        add(&mut self.window_bytes_written, other.window_bytes_written);
        add(&mut self.fast_loop_entries, other.fast_loop_entries);
        add(&mut self.fast_loop_symbols, other.fast_loop_symbols);
        add(&mut self.oneshot_resizes, other.oneshot_resizes);
        add(
            &mut self.oneshot_bytes_allocated,
            other.oneshot_bytes_allocated,
        );
    }
}

/// Index into `match_len_hist` for a match length of 3..=258.
#[cfg(feature = "stats")]
pub(crate) fn len_bucket(len: u32) -> usize {
    match len {
        0..=4 => 0,
        5..=8 => 1,
        9..=16 => 2,
        17..=32 => 3,
        33..=64 => 4,
        65..=128 => 5,
        _ => 6,
    }
}

/// Index into `match_dist_hist` for a distance of 1..=32768.
#[cfg(feature = "stats")]
pub(crate) fn dist_bucket(dist: u32) -> usize {
    match dist {
        0..=1 => 0,
        2..=4 => 1,
        5..=16 => 2,
        17..=64 => 3,
        65..=256 => 4,
        257..=1024 => 5,
        1025..=4096 => 6,
        4097..=16384 => 7,
        _ => 8,
    }
}

/// Count one symbol resolved from a table of `kind`, by table lookup
/// (`via_table`) or by the bit-serial walk.
#[cfg(feature = "stats")]
pub(crate) fn count_decode(stats: &mut Stats, kind: crate::huffman::TableKind, via_table: bool) {
    use crate::huffman::TableKind;
    let slot = match (kind, via_table) {
        (TableKind::LitLen, true) => &mut stats.litlen_table_decodes,
        (TableKind::LitLen, false) => &mut stats.litlen_serial_decodes,
        (TableKind::Distance, true) => &mut stats.dist_table_decodes,
        (TableKind::Distance, false) => &mut stats.dist_serial_decodes,
        (TableKind::CodeLength, true) => &mut stats.codelen_table_decodes,
        (TableKind::CodeLength, false) => &mut stats.codelen_serial_decodes,
    };
    stat!(*slot, 1);
}

/// Record one decoded match of `len` bytes at `distance`.
#[cfg(feature = "stats")]
pub(crate) fn record_match(stats: &mut Stats, len: u32, distance: u32) {
    stat!(stats.matches, 1);
    stat!(stats.match_bytes, len);
    if len > distance {
        stat!(stats.overlap_matches, 1);
    }
    if let Some(b) = stats.match_len_hist.get_mut(len_bucket(len)) {
        stat!(*b, 1);
    }
    if let Some(b) = stats.match_dist_hist.get_mut(dist_bucket(distance)) {
        stat!(*b, 1);
    }
}

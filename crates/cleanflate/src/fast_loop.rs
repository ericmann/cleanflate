//! The Fast-path decode loop for the literal/length/distance state
//! (RFC 1951 S3.2.5, S3.2.6). It runs only while the input and output
//! margins of `config.rs` hold, so no step inside an iteration can run out
//! of bits or of output room, and it leaves to the state machine everything
//! near a buffer edge: at the end of a block, on any error, and when a
//! margin no longer holds. Its exits keep every rule of the bit-reader
//! invariant (DESIGN.md D-27): it only ever consumes whole symbols and
//! leaves buffered bytes for the caller's shared exit to give back.

use crate::bitreader::BitReader;
use crate::config::{FAST_LOOP_MIN_INPUT, FAST_LOOP_MIN_OUTPUT};
use crate::decode_path::DecodePath;
use crate::error::Error;
#[cfg(feature = "stats")]
use crate::huffman::TableKind;
use crate::huffman::{Decoded, Huffman};
use crate::sink::OutputSink;
#[cfg(feature = "stats")]
use crate::stats::Stats;
use crate::tables::{DIST_BASE, DIST_EXTRA, END_OF_BLOCK, LENGTH_BASE, LENGTH_EXTRA};
use crate::window::Window;

/// Bits one iteration needs at most: a 15-bit literal/length code, 5
/// extra length bits, a 15-bit distance code and 13 extra distance bits.
const ITERATION_BITS: u32 = 15 + 5 + 15 + 13;

/// Why the loop stopped.
pub(crate) enum FastExit {
    /// A margin no longer holds; the state machine continues in `LitLen`.
    Margin,
    /// The end-of-block symbol was consumed.
    EndOfBlock,
    /// A match could not be copied in full (defensive; the output margin
    /// makes it impossible); the state machine finishes it.
    Copy { remaining: u16, dist: u16 },
    /// The stream is invalid; the same error the state machine reports.
    Error(Error),
}

/// What the loop works on.
pub(crate) struct FastCtx<'a> {
    /// The literal/length table in use for the block.
    pub litlen: &'a Huffman,
    /// The distance table in use for the block.
    pub dist: &'a Huffman,
    /// The bit reader.
    pub br: &'a mut BitReader,
    /// The history window.
    pub window: &'a Window,
    /// Counters (feature `stats`).
    #[cfg(feature = "stats")]
    pub stats: &'a mut Stats,
}

/// Run the loop. The bit buffer and the input position live in locals and
/// are written back on every exit. `out_base` is the sink offset where the
/// caller's own output starts: a match reaches back only into the window
/// and the sink's bytes since `base`.
pub(crate) fn run<S: OutputSink>(
    ctx: &mut FastCtx,
    input: &[u8],
    pos: &mut usize,
    sink: &mut S,
    out_base: usize,
) -> FastExit {
    let mut br = ctx.br.clone();
    let mut p = *pos;
    let exit = run_loop(ctx, &mut br, input, &mut p, sink, out_base);
    *ctx.br = br;
    *pos = p;
    exit
}

fn run_loop<S: OutputSink>(
    ctx: &mut FastCtx,
    br: &mut BitReader,
    input: &[u8],
    pos: &mut usize,
    sink: &mut S,
    out_base: usize,
) -> FastExit {
    stat!(ctx.stats.fast_loop_entries, 1);
    loop {
        if input.len().saturating_sub(*pos) < FAST_LOOP_MIN_INPUT
            || sink.room() < FAST_LOOP_MIN_OUTPUT
        {
            return FastExit::Margin;
        }
        if br.available() < ITERATION_BITS {
            br.refill_bulk(input, pos);
        }

        #[cfg(feature = "stats")]
        let (decoded, via_table) = ctx.litlen.decode_table_method(br);
        #[cfg(not(feature = "stats"))]
        let decoded = ctx.litlen.decode_table(br);
        let (symbol, len) = match decoded {
            Decoded::Symbol { symbol, len } => (symbol, len),
            Decoded::Invalid => return FastExit::Error(Error::InvalidSymbol),
            // Cannot happen with a full iteration's bits buffered; the
            // state machine would re-decode and report whatever is true.
            Decoded::NeedMoreBits => return FastExit::Margin,
        };
        stat!(ctx.stats.fast_loop_symbols, 1);

        if symbol < 256 {
            br.consume(len);
            #[cfg(feature = "stats")]
            crate::stats::count_decode(ctx.stats, TableKind::LitLen, via_table);
            stat!(ctx.stats.literals, 1);
            sink.push(symbol as u8);
        } else if symbol == END_OF_BLOCK {
            br.consume(len);
            #[cfg(feature = "stats")]
            crate::stats::count_decode(ctx.stats, TableKind::LitLen, via_table);
            return FastExit::EndOfBlock;
        } else if (257..=285).contains(&symbol) {
            br.consume(len);
            #[cfg(feature = "stats")]
            crate::stats::count_decode(ctx.stats, TableKind::LitLen, via_table);
            let idx = (symbol - 257) as usize;
            let extra_bits = LENGTH_EXTRA.get(idx).copied().unwrap_or(0) as u32;
            let base = LENGTH_BASE.get(idx).copied().unwrap_or(0);
            let extra = br.peek(extra_bits);
            br.consume(extra_bits);
            let length = base.saturating_add(extra as u16);

            #[cfg(feature = "stats")]
            let (dec, dist_via_table) = ctx.dist.decode_table_method(br);
            #[cfg(not(feature = "stats"))]
            let dec = ctx.dist.decode_table(br);
            let (dsym, dlen) = match dec {
                Decoded::Symbol { symbol, len } => (symbol, len),
                Decoded::Invalid => return FastExit::Error(Error::InvalidSymbol),
                Decoded::NeedMoreBits => return FastExit::Error(Error::InvalidSymbol),
            };
            if dsym >= 30 {
                // Distance codes 30 and 31 never occur in real data
                // (RFC 1951 S3.2.6).
                return FastExit::Error(Error::InvalidSymbol);
            }
            br.consume(dlen);
            #[cfg(feature = "stats")]
            crate::stats::count_decode(ctx.stats, TableKind::Distance, dist_via_table);
            let didx = dsym as usize;
            let dextra_bits = DIST_EXTRA.get(didx).copied().unwrap_or(0) as u32;
            let dbase = DIST_BASE.get(didx).copied().unwrap_or(0);
            let dextra = br.peek(dextra_bits);
            br.consume(dextra_bits);
            let distance = (dbase as u32).saturating_add(dextra);
            let available = ctx
                .window
                .history_available(sink.written().saturating_sub(out_base))
                as u64;
            if distance as u64 > available {
                return FastExit::Error(Error::InvalidDistance {
                    distance,
                    available,
                });
            }
            #[cfg(feature = "stats")]
            crate::stats::record_match(ctx.stats, u32::from(length), distance);
            let written = ctx.window.copy_match_sink(
                DecodePath::Fast,
                sink,
                out_base,
                distance as usize,
                length as usize,
            );
            if written < length as usize {
                return FastExit::Copy {
                    remaining: length.saturating_sub(written as u16),
                    dist: distance as u16,
                };
            }
        } else {
            // 286 and 287 are valid codes in the fixed table that never
            // occur in real data (RFC 1951 S3.2.6).
            return FastExit::Error(Error::InvalidSymbol);
        }
    }
}

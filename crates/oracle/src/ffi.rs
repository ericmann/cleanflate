//! Safe Rust wrappers over the C zlib API exposed by `libz-sys`, used only
//! through its public function/type surface (SPEC S2.1, S4, CR5). `unsafe`
//! is confined to this module.

use std::ffi::c_int;
use std::os::raw::c_void;

use libz_sys as sys;

const Z_OK: c_int = 0;
const Z_STREAM_END: c_int = 1;
const Z_NEED_DICT: c_int = 2;
const Z_DATA_ERROR: c_int = -3;
const Z_BUF_ERROR: c_int = -5;
const Z_NO_FLUSH: c_int = 0;
const Z_FINISH: c_int = 4;
const Z_DEFLATED: c_int = 8;

const Z_DEFAULT_STRATEGY: c_int = 0;
const Z_FILTERED: c_int = 1;
const Z_HUFFMAN_ONLY: c_int = 2;
const Z_RLE: c_int = 3;
const Z_FIXED: c_int = 4;

/// The three DEFLATE-family framings the oracle can produce/consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// Raw RFC 1951 DEFLATE stream, no wrapper.
    Raw,
    /// RFC 1950 zlib-wrapped stream.
    Zlib,
    /// RFC 1952 gzip-wrapped stream.
    Gzip,
}

/// The deflate strategies zlib supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Default strategy.
    Default,
    /// Filtered strategy (small distances, more literals).
    Filtered,
    /// Huffman-only: no string matching.
    HuffmanOnly,
    /// Run-length encoding limited to distance 1.
    Rle,
    /// Fixed Huffman tables only.
    Fixed,
}

impl Strategy {
    fn as_c_int(self) -> c_int {
        match self {
            Strategy::Default => Z_DEFAULT_STRATEGY,
            Strategy::Filtered => Z_FILTERED,
            Strategy::HuffmanOnly => Z_HUFFMAN_ONLY,
            Strategy::Rle => Z_RLE,
            Strategy::Fixed => Z_FIXED,
        }
    }
}

/// Parameters for one `deflate_bytes`/`deflate_zeros` call.
#[derive(Debug, Clone, Copy)]
pub struct DeflateParams {
    /// Compression level, 0..=9.
    pub level: u8,
    /// Compression strategy.
    pub strategy: Strategy,
    /// zlib windowBits magnitude, 9..=15.
    pub window_bits: u8,
    /// zlib memLevel, 1..=9.
    pub mem_level: u8,
    /// Output framing.
    pub framing: Framing,
}

/// Errors from the oracle's deflate path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleError {
    /// `deflateInit2_`/`inflateInit2_` returned a non-OK status.
    InitFailed(c_int),
    /// `deflate`/`inflate` returned an unexpected non-OK, non-STREAM_END
    /// status.
    StepFailed(c_int),
    /// `deflateSetDictionary`/`inflateSetDictionary` failed.
    DictionaryFailed(c_int),
}

/// Outcome of `inflate_bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InflateOutcome {
    /// Decompressed bytes produced before `status` was reached.
    pub output: Vec<u8>,
    /// Number of input bytes consumed.
    pub consumed: usize,
    /// Final status.
    pub status: OracleStatus,
}

/// The terminal status of an oracle inflate call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleStatus {
    /// Reached the end of the stream (Z_STREAM_END).
    StreamEnd,
    /// Z_DATA_ERROR: malformed input.
    DataError,
    /// Z_NEED_DICT: a preset dictionary is required.
    NeedDict,
    /// Ran out of input before the stream ended.
    Truncated,
    /// The caller's max_output bound was reached before Z_STREAM_END.
    OutputLimit,
    /// Any other zlib return code.
    OtherError(c_int),
}

extern "C" fn oracle_zalloc(
    _opaque: *mut c_void,
    items: libz_sys::uInt,
    size: libz_sys::uInt,
) -> *mut c_void {
    let total = (items as usize).saturating_mul(size as usize);
    // SAFETY: calloc is called with a plain byte count and returns either
    // null or a pointer to `total` zeroed bytes, which zlib treats as an
    // opaque allocation it will only pass back to oracle_free.
    unsafe { libc::calloc(1, total) }
}

extern "C" fn oracle_free(_opaque: *mut c_void, address: *mut c_void) {
    // SAFETY: `address` was returned by oracle_zalloc (i.e. by calloc), or
    // is null, both of which are valid inputs to free.
    unsafe { libc::free(address) }
}

fn new_stream() -> sys::z_stream {
    sys::z_stream {
        next_in: std::ptr::null_mut(),
        avail_in: 0,
        total_in: 0,
        next_out: std::ptr::null_mut(),
        avail_out: 0,
        total_out: 0,
        msg: std::ptr::null_mut(),
        state: std::ptr::null_mut(),
        zalloc: oracle_zalloc,
        zfree: oracle_free,
        opaque: std::ptr::null_mut(),
        data_type: 0,
        adler: 0,
        reserved: 0,
    }
}

fn stream_size() -> c_int {
    std::mem::size_of::<sys::z_stream>() as c_int
}

fn version_ptr() -> *const std::os::raw::c_char {
    // SAFETY: zlibVersion() takes no arguments and returns a pointer to a
    // static, NUL-terminated string owned by the library.
    unsafe { sys::zlibVersion() }
}

fn deflate_window_bits(framing: Framing, magnitude: u8) -> c_int {
    let wb = magnitude as c_int;
    match framing {
        Framing::Raw => -wb,
        Framing::Zlib => wb,
        Framing::Gzip => wb + 16,
    }
}

fn inflate_window_bits(framing: Framing) -> c_int {
    match framing {
        Framing::Raw => -15,
        Framing::Zlib => 15,
        Framing::Gzip => 31,
    }
}

/// Compress `input` with C zlib according to `params`, optionally using a
/// preset dictionary (zlib framing only).
pub fn deflate_bytes(
    input: &[u8],
    params: &DeflateParams,
    dictionary: Option<&[u8]>,
) -> Result<Vec<u8>, OracleError> {
    let mut strm = new_stream();
    let window_bits = deflate_window_bits(params.framing, params.window_bits);

    // SAFETY: `strm` is a fully-initialized z_stream (every field written
    // explicitly above); `version_ptr()` and `stream_size()` match this
    // build's zlib; deflateInit2_ takes ownership of `strm.state` on
    // success, freed later by deflateEnd.
    let rc = unsafe {
        sys::deflateInit2_(
            &mut strm,
            params.level as c_int,
            Z_DEFLATED,
            window_bits,
            params.mem_level as c_int,
            params.strategy.as_c_int(),
            version_ptr(),
            stream_size(),
        )
    };
    if rc != Z_OK {
        return Err(OracleError::InitFailed(rc));
    }

    if let Some(dict) = dictionary {
        // SAFETY: `strm` was just successfully initialized; `dict` outlives
        // this call and its length fits in a uInt for oracle-sized fixtures.
        let rc = unsafe {
            sys::deflateSetDictionary(&mut strm, dict.as_ptr(), dict.len() as libz_sys::uInt)
        };
        if rc != Z_OK {
            // SAFETY: strm.state is valid (deflateInit2_ succeeded above).
            unsafe { sys::deflateEnd(&mut strm) };
            return Err(OracleError::DictionaryFailed(rc));
        }
    }

    let result = run_deflate(&mut strm, input);

    // SAFETY: strm.state is valid regardless of run_deflate's outcome.
    unsafe { sys::deflateEnd(&mut strm) };

    result
}

fn run_deflate(strm: &mut sys::z_stream, input: &[u8]) -> Result<Vec<u8>, OracleError> {
    let mut output: Vec<u8> = Vec::with_capacity(input.len() / 2 + 64);
    let mut chunk = vec![0u8; 64 * 1024];

    strm.next_in = input.as_ptr() as *mut u8;
    strm.avail_in = input.len() as libz_sys::uInt;

    loop {
        strm.next_out = chunk.as_mut_ptr();
        strm.avail_out = chunk.len() as libz_sys::uInt;

        // SAFETY: strm is a valid, initialized stream; next_in/avail_in and
        // next_out/avail_out point at live buffers sized as set above.
        let rc = unsafe { sys::deflate(strm, Z_FINISH) };

        let produced = chunk.len() - strm.avail_out as usize;
        output.extend_from_slice(&chunk[..produced]);

        if rc == Z_STREAM_END {
            return Ok(output);
        }
        if rc != Z_OK && rc != Z_BUF_ERROR {
            return Err(OracleError::StepFailed(rc));
        }
        if produced == 0 && rc == Z_BUF_ERROR {
            return Err(OracleError::StepFailed(rc));
        }
    }
}

/// Compress `total` zero bytes with C zlib, feeding them in 1 MiB chunks
/// with `Z_NO_FLUSH` and finishing with `Z_FINISH`.
pub fn deflate_zeros(total: u64, params: &DeflateParams) -> Result<Vec<u8>, OracleError> {
    let mut strm = new_stream();
    let window_bits = deflate_window_bits(params.framing, params.window_bits);

    // SAFETY: see deflate_bytes -- identical initialization contract.
    let rc = unsafe {
        sys::deflateInit2_(
            &mut strm,
            params.level as c_int,
            Z_DEFLATED,
            window_bits,
            params.mem_level as c_int,
            params.strategy.as_c_int(),
            version_ptr(),
            stream_size(),
        )
    };
    if rc != Z_OK {
        return Err(OracleError::InitFailed(rc));
    }

    let result = (|| {
        let mut output: Vec<u8> = Vec::new();
        let mut out_chunk = vec![0u8; 64 * 1024];
        let zero_chunk = vec![0u8; 1024 * 1024];
        let mut remaining = total;

        loop {
            let this_in = std::cmp::min(remaining, zero_chunk.len() as u64) as usize;
            let flush = if remaining <= zero_chunk.len() as u64 {
                Z_FINISH
            } else {
                Z_NO_FLUSH
            };

            strm.next_in = zero_chunk.as_ptr() as *mut u8;
            strm.avail_in = this_in as libz_sys::uInt;

            loop {
                strm.next_out = out_chunk.as_mut_ptr();
                strm.avail_out = out_chunk.len() as libz_sys::uInt;

                // SAFETY: strm valid; next_in points into zero_chunk (live
                // for this call); next_out points into out_chunk.
                let rc = unsafe { sys::deflate(&mut strm, flush) };

                let produced = out_chunk.len() - strm.avail_out as usize;
                output.extend_from_slice(&out_chunk[..produced]);

                if rc == Z_STREAM_END {
                    return Ok(output);
                }
                if rc != Z_OK && rc != Z_BUF_ERROR {
                    return Err(OracleError::StepFailed(rc));
                }
                if strm.avail_in == 0 && produced == 0 {
                    break;
                }
            }

            remaining -= this_in as u64;
            if remaining == 0 {
                // Requested exact multiple of the chunk size and already
                // flushed with Z_FINISH above if this_in consumed it all;
                // loop again only if more remains (it doesn't).
                return Ok(output);
            }
        }
    })();

    // SAFETY: strm.state is valid regardless of outcome.
    unsafe { sys::deflateEnd(&mut strm) };

    result
}

/// Decompress `input` with C zlib, stopping at `max_output` bytes produced.
pub fn inflate_bytes(
    input: &[u8],
    framing: Framing,
    dictionary: Option<&[u8]>,
    max_output: usize,
) -> InflateOutcome {
    let mut strm = new_stream();
    let window_bits = inflate_window_bits(framing);

    // SAFETY: strm fully initialized above; version/size match this build.
    let rc = unsafe { sys::inflateInit2_(&mut strm, window_bits, version_ptr(), stream_size()) };
    if rc != Z_OK {
        return InflateOutcome {
            output: Vec::new(),
            consumed: 0,
            status: OracleStatus::OtherError(rc),
        };
    }

    let outcome = run_inflate(&mut strm, input, dictionary, max_output);

    // SAFETY: strm.state is valid (inflateInit2_ succeeded above).
    unsafe { sys::inflateEnd(&mut strm) };

    outcome
}

fn run_inflate(
    strm: &mut sys::z_stream,
    input: &[u8],
    dictionary: Option<&[u8]>,
    max_output: usize,
) -> InflateOutcome {
    let mut output: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];

    strm.next_in = input.as_ptr() as *mut u8;
    strm.avail_in = input.len() as libz_sys::uInt;

    loop {
        let remaining_budget = max_output.saturating_sub(output.len());
        if remaining_budget == 0 {
            let consumed = input.len() - strm.avail_in as usize;
            return InflateOutcome {
                output,
                consumed,
                status: OracleStatus::OutputLimit,
            };
        }
        let this_chunk_len = std::cmp::min(chunk.len(), remaining_budget);

        strm.next_out = chunk.as_mut_ptr();
        strm.avail_out = this_chunk_len as libz_sys::uInt;

        // SAFETY: strm valid; next_in points into `input` (borrowed for
        // this call's lifetime); next_out points into `chunk`.
        let rc = unsafe { sys::inflate(strm, Z_NO_FLUSH) };

        let produced = this_chunk_len - strm.avail_out as usize;
        output.extend_from_slice(chunk.get(..produced).unwrap_or(&[]));

        let consumed = input.len() - strm.avail_in as usize;

        match rc {
            _ if rc == Z_STREAM_END => {
                return InflateOutcome {
                    output,
                    consumed,
                    status: OracleStatus::StreamEnd,
                };
            }
            _ if rc == Z_NEED_DICT => {
                if let Some(dict) = dictionary {
                    // SAFETY: strm valid and mid-stream; dict outlives this
                    // call.
                    let src = unsafe {
                        sys::inflateSetDictionary(strm, dict.as_ptr(), dict.len() as libz_sys::uInt)
                    };
                    if src != Z_OK {
                        return InflateOutcome {
                            output,
                            consumed,
                            status: OracleStatus::OtherError(src),
                        };
                    }
                    continue;
                }
                return InflateOutcome {
                    output,
                    consumed,
                    status: OracleStatus::NeedDict,
                };
            }
            _ if rc == Z_DATA_ERROR => {
                return InflateOutcome {
                    output,
                    consumed,
                    status: OracleStatus::DataError,
                };
            }
            _ if rc == Z_OK => {
                if produced == 0 && strm.avail_in == 0 {
                    return InflateOutcome {
                        output,
                        consumed,
                        status: OracleStatus::Truncated,
                    };
                }
                continue;
            }
            _ if rc == Z_BUF_ERROR => {
                return InflateOutcome {
                    output,
                    consumed,
                    status: OracleStatus::Truncated,
                };
            }
            other => {
                return InflateOutcome {
                    output,
                    consumed,
                    status: OracleStatus::OtherError(other),
                };
            }
        }
    }
}

/// CRC-32 (RFC 1952) of `data`, computed by C zlib.
pub fn crc32(data: &[u8]) -> u32 {
    // SAFETY: crc32() takes a plain buffer pointer and length; `data`
    // outlives the call.
    (unsafe { sys::crc32(0, data.as_ptr(), data.len() as libz_sys::uInt) }) as u32
}

/// Adler-32 (RFC 1950) of `data`, computed by C zlib.
pub fn adler32(data: &[u8]) -> u32 {
    // SAFETY: adler32() takes a plain buffer pointer and length; `data`
    // outlives the call. The initial value for an empty checksum is 1 per
    // RFC 1950.
    (unsafe { sys::adler32(1, data.as_ptr(), data.len() as libz_sys::uInt) }) as u32
}

/// Decompress a concatenated multi-member gzip stream with C zlib (RFC
/// 1952 S2.2): calls [`inflate_bytes`] repeatedly, once per member,
/// concatenating each member's output, for as long as the remaining
/// input starts with a gzip member's magic bytes (`1f 8b`) *and* the
/// previous member reached [`OracleStatus::StreamEnd`]. Stops (without
/// error) at the first remainder that doesn't start a new member --
/// C zlib's own single-member `inflate` (`windowBits = 31`) has no
/// concept of "trailing data", so this is what makes the oracle
/// RFC-1952-S2.2-aware for R1-07's differential fuzz target. If any
/// member's own decode fails, that member's status is returned as the
/// whole call's status, with `output`/`consumed` covering only the
/// members that succeeded before it.
pub fn inflate_gzip_members(input: &[u8], max_output: usize) -> InflateOutcome {
    let mut output: Vec<u8> = Vec::new();
    let mut consumed = 0usize;

    let status = loop {
        let remaining = input.get(consumed..).unwrap_or(&[]);
        let budget = max_output.saturating_sub(output.len());
        let outcome = inflate_bytes(remaining, Framing::Gzip, None, budget);

        if outcome.status != OracleStatus::StreamEnd {
            // A failing member's own partial output (e.g. C zlib had
            // already inflated the body before its trailer's CRC/ISIZE
            // check failed) is not folded in: `output`/`consumed` stay
            // exactly at the last successfully completed member.
            break outcome.status;
        }
        output.extend_from_slice(&outcome.output);
        consumed += outcome.consumed;

        if output.len() >= max_output {
            break OracleStatus::OutputLimit;
        }

        let next = input.get(consumed..).unwrap_or(&[]);
        let starts_new_member = next.first() == Some(&0x1f) && next.get(1) == Some(&0x8b);
        if !starts_new_member {
            break OracleStatus::StreamEnd;
        }
    };

    InflateOutcome {
        output,
        consumed,
        status,
    }
}

/// The C zlib version string linked into this build.
pub fn zlib_version() -> String {
    let ptr = version_ptr();
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: zlibVersion() returns a pointer to a static, NUL-terminated
    // C string.
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    cstr.to_string_lossy().into_owned()
}

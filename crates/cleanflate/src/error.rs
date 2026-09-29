//! The typed error model (SPEC S3.3: "a typed error enum with distinct
//! variants for bad header, bad block type, invalid Huffman table,
//! invalid distance (too far back), checksum mismatch, truncated input,
//! trailing garbage policy, and output limit exceeded. No stringly-typed
//! errors.").

use core::fmt;

/// Everything that can go wrong decoding a zlib (RFC 1950) header.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    /// RFC 1950 S2.2: "The FCHECK value must be such that CMF and FLG,
    /// when viewed as a 16-bit unsigned integer stored in MSB order
    /// (CMF*256 + FLG), is a multiple of 31."
    ZlibFcheck,
    /// RFC 1950 S2.2: CM must be 8 ("deflate" compression method).
    ZlibCompressionMethod(u8),
    /// RFC 1950 S2.2: CINFO (upper 4 bits of CMF) must be <= 7.
    ZlibWindowSize(u8),
    /// RFC 1952 S2.3.1: the two-byte magic number must be 0x1f 0x8b.
    GzipMagic,
    /// RFC 1952 S2.3.1: CM must be 8 ("deflate" compression method).
    GzipCompressionMethod(u8),
    /// RFC 1952 S2.3.1: reserved FLG bits must be zero.
    GzipReservedFlags(u8),
}

/// Everything that can go wrong constructing or decoding a Huffman table
/// (RFC 1951 S3.2.2, S3.2.7).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HuffmanError {
    /// More codes were assigned than the code-length counts allow.
    OverSubscribed,
    /// Fewer codes were assigned than the code-length counts allow (and
    /// the RFC 1951 S3.2.7 single-code/all-zero exceptions do not apply).
    Incomplete,
    /// More code-length symbols were transmitted than HLIT+HDIST+258
    /// allows.
    TooManyLengthCodes,
    /// A repeat code (16) appeared with no previous code length to
    /// repeat.
    RepeatWithoutPrevious,
    /// A repeat code's count would overrun the number of lengths being
    /// transmitted.
    RepeatOverflow,
    /// A literal/length table was built with no code for symbol 256
    /// (end-of-block).
    MissingEndOfBlock,
}

/// Which checksum a [`Error::ChecksumMismatch`] refers to.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumKind {
    /// RFC 1950 zlib trailer.
    Adler32,
    /// RFC 1952 gzip member trailer.
    Crc32,
    /// RFC 1952 gzip member trailer ISIZE field.
    Isize,
    /// RFC 1952 gzip header FHCRC field.
    GzipHeaderCrc,
}

/// Every way `cleanflate` can fail to decode a stream (SPEC S3.3).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A zlib or gzip header failed validation.
    BadHeader(HeaderError),
    /// RFC 1950 S2.2: FDICT was set, but no preset dictionary was
    /// supplied via the API.
    PresetDictionaryRequired {
        /// The Adler-32 id of the dictionary the stream expects.
        dict_id: u32,
    },
    /// A preset dictionary was supplied, but its Adler-32 does not match
    /// the id the stream expects.
    DictionaryMismatch {
        /// Adler-32 the stream expects.
        expected: u32,
        /// Adler-32 of the dictionary actually supplied.
        actual: u32,
    },
    /// RFC 1951 S3.2.3: BTYPE 11 is reserved and must not occur.
    BadBlockType,
    /// RFC 1951 S3.2.4: a stored block's LEN did not match the one's
    /// complement of NLEN.
    InvalidStoredLength {
        /// The stored LEN field.
        len: u16,
        /// The stored NLEN field.
        nlen: u16,
    },
    /// A dynamic or fixed Huffman table failed construction or
    /// validation.
    InvalidHuffmanTable(HuffmanError),
    /// A decoded symbol has no meaning in its alphabet (e.g. length code
    /// 286/287, or distance code 30/31).
    InvalidSymbol,
    /// RFC 1951 S3.2.5: a decoded distance reaches further back than the
    /// bytes produced so far (in this stream, plus any preset
    /// dictionary).
    InvalidDistance {
        /// The decoded distance.
        distance: u32,
        /// Bytes available to copy from (output so far plus dictionary).
        available: u64,
    },
    /// A trailer checksum did not match the data actually decoded.
    ChecksumMismatch {
        /// Which checksum mismatched.
        kind: ChecksumKind,
        /// Value recorded in the stream.
        expected: u32,
        /// Value computed while decoding.
        actual: u32,
    },
    /// The input ended before a complete stream was decoded.
    Truncated,
    /// SPEC S3.3: trailing-data policy is `TrailingData::Reject` and
    /// bytes remained after the logical end of the stream.
    TrailingData {
        /// Byte offset (from the start of input) where trailing data
        /// begins.
        offset: u64,
    },
    /// SPEC S3.3: decompression would exceed the configured maximum
    /// output size.
    OutputLimitExceeded {
        /// The configured limit, in bytes.
        limit: u64,
    },
}

/// The variant-name-only projection of [`Error`], used where a stable,
/// stringly-comparable tag is more convenient than matching on `Error`
/// itself (e.g. the malformed-corpus `EXPECTED.tsv`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// See [`Error::BadHeader`].
    BadHeader,
    /// See [`Error::PresetDictionaryRequired`].
    PresetDictionaryRequired,
    /// See [`Error::DictionaryMismatch`].
    DictionaryMismatch,
    /// See [`Error::BadBlockType`].
    BadBlockType,
    /// See [`Error::InvalidStoredLength`].
    InvalidStoredLength,
    /// See [`Error::InvalidHuffmanTable`].
    InvalidHuffmanTable,
    /// See [`Error::InvalidSymbol`].
    InvalidSymbol,
    /// See [`Error::InvalidDistance`].
    InvalidDistance,
    /// See [`Error::ChecksumMismatch`].
    ChecksumMismatch,
    /// See [`Error::Truncated`].
    Truncated,
    /// See [`Error::TrailingData`].
    TrailingData,
    /// See [`Error::OutputLimitExceeded`].
    OutputLimitExceeded,
}

impl ErrorKind {
    /// The variant name, e.g. `"InvalidHuffmanTable"`.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::BadHeader => "BadHeader",
            ErrorKind::PresetDictionaryRequired => "PresetDictionaryRequired",
            ErrorKind::DictionaryMismatch => "DictionaryMismatch",
            ErrorKind::BadBlockType => "BadBlockType",
            ErrorKind::InvalidStoredLength => "InvalidStoredLength",
            ErrorKind::InvalidHuffmanTable => "InvalidHuffmanTable",
            ErrorKind::InvalidSymbol => "InvalidSymbol",
            ErrorKind::InvalidDistance => "InvalidDistance",
            ErrorKind::ChecksumMismatch => "ChecksumMismatch",
            ErrorKind::Truncated => "Truncated",
            ErrorKind::TrailingData => "TrailingData",
            ErrorKind::OutputLimitExceeded => "OutputLimitExceeded",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Error {
    /// Project this error onto its [`ErrorKind`].
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::BadHeader(_) => ErrorKind::BadHeader,
            Error::PresetDictionaryRequired { .. } => ErrorKind::PresetDictionaryRequired,
            Error::DictionaryMismatch { .. } => ErrorKind::DictionaryMismatch,
            Error::BadBlockType => ErrorKind::BadBlockType,
            Error::InvalidStoredLength { .. } => ErrorKind::InvalidStoredLength,
            Error::InvalidHuffmanTable(_) => ErrorKind::InvalidHuffmanTable,
            Error::InvalidSymbol => ErrorKind::InvalidSymbol,
            Error::InvalidDistance { .. } => ErrorKind::InvalidDistance,
            Error::ChecksumMismatch { .. } => ErrorKind::ChecksumMismatch,
            Error::Truncated => ErrorKind::Truncated,
            Error::TrailingData { .. } => ErrorKind::TrailingData,
            Error::OutputLimitExceeded { .. } => ErrorKind::OutputLimitExceeded,
        }
    }
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::ZlibFcheck => write!(f, "zlib header failed the FCHECK mod-31 test"),
            HeaderError::ZlibCompressionMethod(cm) => {
                write!(f, "zlib CM {cm} is not 8 (deflate)")
            }
            HeaderError::ZlibWindowSize(cinfo) => {
                write!(f, "zlib CINFO {cinfo} exceeds the maximum of 7")
            }
            HeaderError::GzipMagic => write!(f, "gzip magic number is not 1f 8b"),
            HeaderError::GzipCompressionMethod(cm) => {
                write!(f, "gzip CM {cm} is not 8 (deflate)")
            }
            HeaderError::GzipReservedFlags(flg) => {
                write!(f, "gzip FLG {flg:#04x} sets a reserved bit")
            }
        }
    }
}

impl fmt::Display for HuffmanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            HuffmanError::OverSubscribed => "Huffman code table is over-subscribed",
            HuffmanError::Incomplete => "Huffman code table is incomplete",
            HuffmanError::TooManyLengthCodes => "too many code-length symbols transmitted",
            HuffmanError::RepeatWithoutPrevious => "repeat code with no previous code length",
            HuffmanError::RepeatOverflow => "repeat code overruns the code-length sequence",
            HuffmanError::MissingEndOfBlock => "literal/length table has no end-of-block code",
        };
        f.write_str(msg)
    }
}

impl fmt::Display for ChecksumKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ChecksumKind::Adler32 => "Adler-32",
            ChecksumKind::Crc32 => "CRC-32",
            ChecksumKind::Isize => "ISIZE",
            ChecksumKind::GzipHeaderCrc => "gzip header CRC",
        };
        f.write_str(msg)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadHeader(e) => write!(f, "bad header: {e}"),
            Error::PresetDictionaryRequired { dict_id } => {
                write!(f, "stream requires preset dictionary {dict_id:#010x}")
            }
            Error::DictionaryMismatch { expected, actual } => write!(
                f,
                "preset dictionary mismatch: expected {expected:#010x}, got {actual:#010x}"
            ),
            Error::BadBlockType => write!(f, "reserved block type (BTYPE 11)"),
            Error::InvalidStoredLength { len, nlen } => write!(
                f,
                "stored block LEN {len} is not the one's complement of NLEN {nlen}"
            ),
            Error::InvalidHuffmanTable(e) => write!(f, "invalid Huffman table: {e}"),
            Error::InvalidSymbol => write!(f, "decoded symbol has no meaning in its alphabet"),
            Error::InvalidDistance {
                distance,
                available,
            } => write!(
                f,
                "distance {distance} exceeds {available} bytes available to copy from"
            ),
            Error::ChecksumMismatch {
                kind,
                expected,
                actual,
            } => write!(
                f,
                "{kind} mismatch: expected {expected:#010x}, got {actual:#010x}"
            ),
            Error::Truncated => write!(f, "input ended before a complete stream was decoded"),
            Error::TrailingData { offset } => {
                write!(f, "trailing data at byte offset {offset}")
            }
            Error::OutputLimitExceeded { limit } => {
                write!(f, "output would exceed the {limit}-byte limit")
            }
        }
    }
}

impl std::error::Error for Error {}

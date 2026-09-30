//! Selection between the production decode techniques and the simple
//! reference ones. Only the test-hooks feature can request the reference
//! path; every other build always uses [`DecodePath::Fast`].

/// Which set of decode techniques a decoder uses.
///
/// `Fast` is the production path. `Reference` is the simple path used as
/// the differential oracle: exact byte-at-a-time fill, bit-serial Huffman
/// decode, byte-by-byte match copy and no fast loop. Both produce the
/// same output and the same errors for every input.
#[cfg_attr(not(any(test, feature = "test-hooks")), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecodePath {
    /// The production path.
    #[default]
    Fast,
    /// The simple reference path.
    Reference,
}

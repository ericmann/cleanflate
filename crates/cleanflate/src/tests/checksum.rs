use crate::checksum::{adler32, crc32, Adler32, Crc32};

#[test]
fn adler32_empty_is_one() {
    assert_eq!(adler32(b""), 1);
}

#[test]
fn adler32_wikipedia() {
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
}

#[test]
fn crc32_empty_is_zero() {
    assert_eq!(crc32(b""), 0);
}

#[test]
fn crc32_check_value_123456789() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn adler32_incremental_equals_oneshot() {
    let data = b"the quick brown fox jumps over the lazy dog";
    let oneshot = adler32(data);

    let mut incremental = Adler32::new();
    for chunk in data.chunks(7) {
        incremental.update(chunk);
    }
    assert_eq!(incremental.finish(), oneshot);
}

#[test]
fn crc32_incremental_equals_oneshot() {
    let data = b"the quick brown fox jumps over the lazy dog";
    let oneshot = crc32(data);

    let mut incremental = Crc32::new();
    for chunk in data.chunks(7) {
        incremental.update(chunk);
    }
    assert_eq!(incremental.finish(), oneshot);
}

#[test]
fn adler32_bound_worst_case() {
    // The exact worst case the deferred-modulo bound (ADLER_NMAX = 5552,
    // DESIGN.md D-23) is derived for: a run of 0xFF bytes -- the
    // maximum possible per-byte contribution -- long enough to cross
    // several ADLER_NMAX-sized chunk boundaries. If the bound were
    // wrong (too large), this would silently wrap a u32 mid-chunk and
    // produce a value that disagrees with the direct, per-byte
    // reference computation below.
    let lengths = [
        1,
        crate::checksum::ADLER_NMAX - 1,
        crate::checksum::ADLER_NMAX,
        crate::checksum::ADLER_NMAX + 1,
        3 * crate::checksum::ADLER_NMAX + 17,
    ];
    for &len in &lengths {
        let data = vec![0xFFu8; len];
        let result = adler32(&data);

        let mut s1: u64 = 1;
        let mut s2: u64 = 0;
        for _ in 0..data.len() {
            s1 = (s1 + 0xFF) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        let expected = ((s2 as u32) << 16) | (s1 as u32);
        assert_eq!(result, expected, "len={len}");
    }
}

#[test]
fn adler32_large_buffer_of_ff() {
    let data = vec![0xFFu8; 1024 * 1024];
    // Cross-checked against the RFC 1950 S2.2 definition computed by hand
    // for this buffer size/content; also cross-checked against the C
    // zlib oracle in crates/oracle/tests/checksums.rs.
    let result = adler32(&data);
    // s1 accumulates (1 + 255*n) mod 65521 repeatedly; verify via a
    // direct re-derivation rather than a second implementation of the
    // same algorithm, to catch a modulus/order bug.
    let mut s1: u64 = 1;
    let mut s2: u64 = 0;
    for _ in 0..data.len() {
        s1 = (s1 + 0xFF) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    let expected = ((s2 as u32) << 16) | (s1 as u32);
    assert_eq!(result, expected);
}

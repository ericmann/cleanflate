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
fn crc32_table_matches_bitwise_reference() {
    // Recovers `CRC32_TABLE[n]` from `crc32` of a single byte without
    // exposing the table: `crc32(&[b])` starts from `crc = 0xFFFF_FFFF`,
    // whose low byte is `0xFF`, so the table index it looks up is
    // `0xFF ^ b`. Choosing `b = 0xFF ^ n` therefore looks up index `n`.
    // Undoing the post-XOR (`^ (crc >> 8)`) and the final invert
    // (`finish = crc ^ 0xFFFF_FFFF`) algebraically isolates the raw
    // table entry: `finish = entry ^ 0x00FF_FFFF ^ 0xFFFF_FFFF = entry ^
    // 0xFF00_0000`.
    fn reference_entry(n: u32) -> u32 {
        let mut c = n;
        for _ in 0..8 {
            if c & 1 != 0 {
                c = 0xEDB8_8320 ^ (c >> 1);
            } else {
                c >>= 1;
            }
        }
        c
    }

    for n in 0u32..256 {
        let byte = (0xFFu32 ^ n) as u8;
        let table_entry = crc32(&[byte]) ^ 0xFF00_0000;
        assert_eq!(table_entry, reference_entry(n), "n={n}");
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

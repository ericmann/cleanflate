use crate::checksum::{
    adler32, adler32_update_bytewise, crc32, crc32_update_bytewise, crc32_update_sliced, Adler32,
    Crc32, CRC32_TABLES,
};
use crate::config::CRC32_SLICES;

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

/// Test-local xorshift64 generator.
fn seeded_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// The reference: the register updated one byte at a time with table 0.
fn bytewise(data: &[u8]) -> u32 {
    crc32_update_bytewise(0xFFFF_FFFF, data) ^ 0xFFFF_FFFF
}

#[test]
fn crc32_slices_match_bytewise() {
    let data = seeded_bytes(0xC0FFEE, 8192);
    // Every length 0..=64 at every start offset 0..=7.
    for offset in 0..=7usize {
        for len in 0..=64usize {
            let slice = &data[offset..offset + len];
            assert_eq!(crc32(slice), bytewise(slice), "offset {offset} len {len}");
        }
    }
    // Random lengths up to 4096.
    let mut x = 0x1234_5678_9ABC_DEF1u64;
    for _ in 0..200 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let len = (x % 4097) as usize;
        let offset = ((x >> 20) % 8) as usize;
        let slice = &data[offset..offset + len];
        assert_eq!(crc32(slice), bytewise(slice), "offset {offset} len {len}");
    }
    // Split updates at every boundary of a short buffer, and in random
    // pieces of a long one.
    let short = &data[..100];
    for cut in 0..=short.len() {
        let mut c = Crc32::new();
        c.update(&short[..cut]);
        c.update(&short[cut..]);
        assert_eq!(c.finish(), bytewise(short), "cut {cut}");
    }
    let mut c = Crc32::new();
    let mut pos = 0usize;
    let mut step = 1usize;
    while pos < data.len() {
        let end = (pos + step).min(data.len());
        c.update(&data[pos..end]);
        pos = end;
        step = step * 3 % 41 + 1;
    }
    assert_eq!(c.finish(), bytewise(&data));
}

#[test]
fn crc32_tables_follow_zero_byte_recurrence() {
    // Table k maps byte b to the register after b followed by k zero
    // bytes, so table k follows from table k-1 by one more zero-byte step:
    // T_k[b] = T_0[T_{k-1}[b] & 0xFF] ^ (T_{k-1}[b] >> 8).
    let t0 = &CRC32_TABLES[0];
    for k in 1..CRC32_SLICES {
        for b in 0..256usize {
            let prev = CRC32_TABLES[k - 1][b];
            let expected = t0[(prev & 0xFF) as usize] ^ (prev >> 8);
            assert_eq!(CRC32_TABLES[k][b], expected, "table {k} byte {b}");
        }
    }
    // And table 0 is the classic table: T_0[1] is the polynomial's
    // one-bit-set reflection (RFC 1952 S8), T_0[0] is 0.
    assert_eq!(t0[0], 0);
    assert_eq!(t0[1], 0x7707_3096);
    assert_eq!(t0[255], 0x2D02_EF8D);
}

#[test]
fn crc32_slices_config_valid() {
    assert!(std::hint::black_box(CRC32_SLICES) >= 4);
}

#[test]
fn crc32_sliced_variants_match_bytewise() {
    let data = seeded_bytes(0xBADC_0FFE, 8192);
    fn check<const N: usize>(data: &[u8], offset: usize, len: usize) {
        let slice = &data[offset..offset + len];
        let got = crc32_update_sliced::<N>(0xFFFF_FFFF, slice) ^ 0xFFFF_FFFF;
        assert_eq!(got, bytewise(slice), "N={N} offset {offset} len {len}");
        // A non-initial starting register too.
        let got = crc32_update_sliced::<N>(0x1234_5678, slice);
        assert_eq!(
            got,
            crc32_update_bytewise(0x1234_5678, slice),
            "N={N} seeded"
        );
    }
    for offset in 0..=15usize {
        for len in 0..=64usize {
            check::<4>(&data, offset, len);
            check::<8>(&data, offset, len);
            check::<16>(&data, offset, len);
        }
    }
    let mut x = 0x0DDB_1A5E_5BAD_5EEDu64;
    for _ in 0..200 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let len = (x % 4097) as usize;
        let offset = ((x >> 20) % 16) as usize;
        check::<4>(&data, offset, len);
        check::<8>(&data, offset, len);
        check::<16>(&data, offset, len);
    }
    // Split updates: folding in two pieces equals one piece, for each width.
    let short = &data[..200];
    for cut in 0..=short.len() {
        for n in [4usize, 8, 16] {
            let run = |d: &[u8], r: u32| match n {
                4 => crc32_update_sliced::<4>(r, d),
                8 => crc32_update_sliced::<8>(r, d),
                _ => crc32_update_sliced::<16>(r, d),
            };
            let split = run(&short[cut..], run(&short[..cut], 0xFFFF_FFFF));
            assert_eq!(
                split,
                crc32_update_bytewise(0xFFFF_FFFF, short),
                "N={n} cut {cut}"
            );
        }
    }
    // Widths with no tables or too narrow to fold a register fall back to
    // the bytewise result.
    let s = &data[..1000];
    assert_eq!(
        crc32_update_sliced::<2>(0xFFFF_FFFF, s),
        crc32_update_bytewise(0xFFFF_FFFF, s)
    );
    assert_eq!(
        crc32_update_sliced::<32>(0xFFFF_FFFF, s),
        crc32_update_bytewise(0xFFFF_FFFF, s)
    );
}

#[test]
fn adler32_bytewise_matches_production() {
    let data = seeded_bytes(0xA11CE, 20_000);
    for len in [0usize, 1, 2, 255, 5551, 5552, 5553, 11_104, 20_000] {
        let slice = &data[..len];
        assert_eq!(
            adler32_update_bytewise(1, slice),
            adler32(slice),
            "len {len}"
        );
    }
    let ff = vec![0xFFu8; 3 * crate::checksum::ADLER_NMAX + 5];
    assert_eq!(adler32_update_bytewise(1, &ff), adler32(&ff));
    // Split updates from a non-initial value.
    let mid = adler32_update_bytewise(1, &data[..777]);
    assert_eq!(adler32_update_bytewise(mid, &data[777..]), adler32(&data));
}

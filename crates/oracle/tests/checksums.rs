use cleanflate::checksum::{adler32, crc32};
use oracle::ffi;
use oracle::prng::SplitMix64;

const SEED: u64 = 0xC0FF_EE00_1234_5678;

#[test]
fn adler32_matches_oracle_random_buffers() {
    let mut rng = SplitMix64::new(SEED);
    for i in 0..200u64 {
        let len = (rng.next_u64() % 10_001) as usize;
        let buf = rng.bytes(len);
        assert_eq!(
            adler32(&buf),
            ffi::adler32(&buf),
            "mismatch at i={i} len={len}"
        );
    }
}

#[test]
fn crc32_matches_oracle_random_buffers() {
    let mut rng = SplitMix64::new(SEED ^ 0xABCD_EF01_2345_6789);
    for i in 0..200u64 {
        let len = ((rng.next_u64() % 10_001) as usize).min(10_000);
        let buf = rng.bytes(len);
        assert_eq!(crc32(&buf), ffi::crc32(&buf), "mismatch at i={i} len={len}");
    }
}

/// Long buffers (up to 1 MiB), updated incrementally in random pieces:
/// the multi-byte CRC-32 must agree with C zlib's, however the input is
/// split.
#[test]
fn crc32_matches_oracle_long() {
    let mut rng = SplitMix64::new(SEED ^ 0x5151_5151_A5A5_A5A5);
    for i in 0..24u64 {
        let len = (rng.next_u64() % (1 << 20)) as usize + if i == 0 { 1 << 20 } else { 0 };
        let len = len.min(1 << 20);
        let buf = rng.bytes(len);
        let mut c = cleanflate::checksum::Crc32::new();
        let mut pos = 0usize;
        while pos < buf.len() {
            let piece = 1 + (rng.next_u64() % 100_000) as usize;
            let end = (pos + piece).min(buf.len());
            c.update(&buf[pos..end]);
            pos = end;
        }
        assert_eq!(c.finish(), ffi::crc32(&buf), "mismatch at i={i} len={len}");
        assert_eq!(crc32(&buf), ffi::crc32(&buf), "one-shot mismatch at i={i}");
    }
}

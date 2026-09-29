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

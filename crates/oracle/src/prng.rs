//! SplitMix64 PRNG and FNV-1a 64-bit hash, used to seed and generate the
//! deterministic synthetic conformance corpus (SPEC S4).

/// A SplitMix64 generator, per the standard `state += 0x9E3779B97F4A7C15`
/// step followed by the usual xor-shift-multiply finalizer.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Create a generator seeded with `seed`.
    pub fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// Produce the next 64-bit output and advance the generator.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Fill `buf` with pseudo-random bytes.
    pub fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut chunks = buf.chunks_exact_mut(8);
        for chunk in &mut chunks {
            let v = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&v);
        }
        let rem = chunks.into_remainder();
        if !rem.is_empty() {
            let v = self.next_u64().to_le_bytes();
            for (dst, src) in rem.iter_mut().zip(v.iter()) {
                *dst = *src;
            }
        }
    }

    /// Generate a `Vec<u8>` of pseudo-random bytes of length `len`.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut buf = vec![0u8; len];
        self.fill_bytes(&mut buf);
        buf
    }
}

/// FNV-1a 64-bit hash of `data`, used to derive a per-input seed from its
/// id string (SPEC S4: `CORPUS_SEED ^ fnv1a64(id)`).
pub fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut hash = OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

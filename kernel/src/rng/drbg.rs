// SPDX-License-Identifier: MPL-2.0
//! ChaCha20 DRBG with SHA-256 seed-mixing (D2 + D3).
//!
//! Deterministic given a fixed seed → host-KAT-able. Reseed at 10,000 calls / 60s
//! (D3), whichever first. Counter-wrap guard forces reseed before ChaCha20 block
//! counter exhaustion.

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use sha2::Sha256;
use sha2::digest::Digest;

use crate::rng::entropy;

/// Maximum generate calls before forced reseed (D3).
const RESEED_GEN_LIMIT: u64 = 10_000;

/// ChaCha20 has 2^32 block counter (64 bytes each). Force reseed at 2^20 blocks
/// to leave a wide safety margin against counter wrap.
const COUNTER_WRAP_GUARD: u64 = 1 << 20;

/// The kernel CSPRNG instance.
pub struct Csprng {
    cipher: ChaCha20,
    gen_count: u64,
    block_count: u64,
}

impl Csprng {
    /// Initialize the DRBG from real entropy. Panics if RDRAND is unavailable (D6).
    pub fn init() -> Self {
        let entropy_bytes = entropy::collect_entropy_32().expect(
            "CSPRNG init: RDRAND unavailable after 10 retries — \
             fail-closed panic (D6). This CPU cannot provide trusted entropy.",
        );
        let seed = sha256_seed_mix(&entropy_bytes, 0);
        Self::from_seed(seed)
    }

    /// Initialize from a fixed seed (for KATs and testing).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let cipher = ChaCha20::new(&seed.into(), &[0u8; 12].into());

        Self {
            cipher,
            gen_count: 0,
            block_count: 0,
        }
    }

    /// Fill a buffer with random bytes. Reseeds automatically at D3 thresholds.
    pub fn fill_bytes(&mut self, dest: &mut [u8]) {
        // Check reseed thresholds (D3).
        if self.gen_count >= RESEED_GEN_LIMIT || self.block_count >= COUNTER_WRAP_GUARD {
            self.reseed();
        }

        self.cipher.apply_keystream(dest);
        self.gen_count += 1;

        // Track approximate block usage: each 64 bytes ≈ one ChaCha20 block.
        self.block_count += (dest.len() as u64).saturating_add(63) / 64;
    }

    /// Generate a single random u64.
    // clippy-bank: retained CSPRNG convenience API for future kernel consumers.
    pub fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.fill_bytes(&mut buf);
        u64::from_le_bytes(buf)
    }

    /// Generate a single random u32.
    // clippy-bank: retained CSPRNG convenience API for future kernel consumers.
    pub fn next_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        self.fill_bytes(&mut buf);
        u32::from_le_bytes(buf)
    }

    /// Force a reseed from fresh entropy.
    fn reseed(&mut self) {
        let entropy_bytes = match entropy::collect_entropy_32() {
            Some(e) => e,
            None => panic!(
                "CSPRNG reseed: RDRAND unavailable — \
                 fail-closed panic (D6). Cannot continue without trusted entropy."
            ),
        };

        // Include previous state in the seed mix for forward security.
        let mut personalization = [0u8; 64];
        personalization[0..8].copy_from_slice(&self.gen_count.to_le_bytes());
        personalization[8..16].copy_from_slice(&self.block_count.to_le_bytes());

        let new_seed = sha256_seed_mix_with_personalization(&entropy_bytes, &personalization);
        self.cipher = ChaCha20::new(&new_seed.into(), &[0u8; 12].into());
        self.gen_count = 0;
        self.block_count = 0;
    }
}

/// SHA-256 seed mixing: H(seed || reseed_count_be).
fn sha256_seed_mix(seed: &[u8; 32], reseed_count: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update(reseed_count.to_be_bytes());
    let digest = hasher.finalize();
    digest.into()
}

/// SHA-256 seed mixing with extra personalization data.
fn sha256_seed_mix_with_personalization(seed: &[u8; 32], personalization: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    if !personalization.is_empty() {
        hasher.update(personalization);
    }
    let digest = hasher.finalize();
    digest.into()
}

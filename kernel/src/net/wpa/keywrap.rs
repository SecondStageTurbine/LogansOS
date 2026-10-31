// SPDX-License-Identifier: MPL-2.0
//! AES Key Wrap (RFC 3394) — key UNWRAP, for the WPA2 4-way handshake message 3:
//! the GTK in the EAPOL-Key "Key Data" field is AES-key-wrapped with the PTK's
//! KEK (EAPOL-Key descriptor version 2 / CCMP). W0 derives the KEK (`kdf`) but
//! did not implement the unwrap; W2 (ISC-2) adds it here — new code, not a
//! reimplementation of any existing W0 routine. Host-KAT'd against the RFC 3394
//! §4.1 published vector (independent of this code). Alloc-free (caller buffer).

use aes::Aes128;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};

/// RFC 3394 §2.2.3.1 default integrity check value (the recovered A must equal
/// this 64-bit constant, else the unwrap is rejected).
const DEFAULT_IV: [u8; 8] = [0xa6; 8];

/// Largest key-data we unwrap: 8 × 64-bit blocks = 64 bytes (a CCMP GTK is 16,
/// a 256-bit group key 32 — well within this).
const MAX_BLOCKS: usize = 8;

/// AES-128 Key Unwrap (RFC 3394 §2.2.2). `wrapped` is the `(n+1)`×64-bit wrapped
/// key (≥ 16 bytes, a multiple of 8, n ≤ 8). Writes the `n`×64-bit unwrapped key
/// data into `out` and returns its length, or `None` if the length is invalid or
/// the integrity check fails (a wrong KEK, a corrupted/forged M3) — the check is
/// what makes the wrapped GTK tamper-evident.
pub fn aes_unwrap(wrapped: &[u8], kek: &[u8; 16], out: &mut [u8]) -> Option<usize> {
    if wrapped.len() < 16 || !wrapped.len().is_multiple_of(8) {
        return None;
    }
    let n = wrapped.len() / 8 - 1;
    if n == 0 || n > MAX_BLOCKS || out.len() < n * 8 {
        return None;
    }
    let cipher = Aes128::new(GenericArray::from_slice(kek));

    // A = C[0]; R[1..=n] = C[1..=n] (64-bit blocks).
    let mut a = [0u8; 8];
    a.copy_from_slice(&wrapped[..8]);
    let mut r = [[0u8; 8]; MAX_BLOCKS];
    for (i, blk) in r.iter_mut().enumerate().take(n) {
        blk.copy_from_slice(&wrapped[8 + i * 8..16 + i * 8]);
    }

    // for j = 5..0, for i = n..1: B = AES-Dec(KEK, (A ^ t) | R[i]); A = MSB(B);
    // R[i] = LSB(B); t = n*j + i.
    let mut block = [0u8; 16];
    for j in (0..=5usize).rev() {
        for i in (1..=n).rev() {
            let t = (n * j + i) as u64;
            block[..8].copy_from_slice(&a);
            let tb = t.to_be_bytes();
            for k in 0..8 {
                block[k] ^= tb[k];
            }
            block[8..].copy_from_slice(&r[i - 1]);
            let mut ga = GenericArray::clone_from_slice(&block);
            cipher.decrypt_block(&mut ga);
            block.copy_from_slice(&ga);
            a.copy_from_slice(&block[..8]);
            r[i - 1].copy_from_slice(&block[8..]);
        }
    }

    if a != DEFAULT_IV {
        return None; // integrity check failed
    }
    for (i, blk) in r.iter().enumerate().take(n) {
        out[i * 8..i * 8 + 8].copy_from_slice(blk);
    }
    Some(n * 8)
}

/// AES-128 Key Wrap (RFC 3394 §2.2.1) — the inverse of `aes_unwrap`. `plaintext`
/// is the `n`×64-bit key data (a non-zero multiple of 8, n ≤ 8); writes the
/// `(n+1)`×64-bit wrapped output into `out` and returns its length, or `None` on
/// an invalid length. New in CB-5a: the hermetic WPA3-SAE gate's in-process AP
/// (`test_harness`) must PRODUCE the AES-wrapped GTK/IGTK Key Data of the 4-way M3
/// and the Group-Key-Handshake M1 that the STA's `aes_unwrap` consumes — so the
/// gate exercises the real wrap→unwrap pair, not a bypass. KAT'd forward against
/// the published RFC 3394 §4.1 vector (a fixed ciphertext, not a round-trip).
pub fn aes_wrap(plaintext: &[u8], kek: &[u8; 16], out: &mut [u8]) -> Option<usize> {
    if plaintext.is_empty() || !plaintext.len().is_multiple_of(8) {
        return None;
    }
    let n = plaintext.len() / 8;
    if n == 0 || n > MAX_BLOCKS || out.len() < (n + 1) * 8 {
        return None;
    }
    let cipher = Aes128::new(GenericArray::from_slice(kek));

    // A = IV; R[1..=n] = P[1..=n].
    let mut a = DEFAULT_IV;
    let mut r = [[0u8; 8]; MAX_BLOCKS];
    for (i, blk) in r.iter_mut().enumerate().take(n) {
        blk.copy_from_slice(&plaintext[i * 8..i * 8 + 8]);
    }

    // for j = 0..=5, for i = 1..=n: B = AES-Enc(KEK, A | R[i]);
    // A = MSB(B) ^ (n*j + i); R[i] = LSB(B).
    let mut block = [0u8; 16];
    for j in 0..=5usize {
        for i in 1..=n {
            block[..8].copy_from_slice(&a);
            block[8..].copy_from_slice(&r[i - 1]);
            let mut ga = GenericArray::clone_from_slice(&block);
            cipher.encrypt_block(&mut ga);
            block.copy_from_slice(&ga);
            a.copy_from_slice(&block[..8]);
            let t = (n * j + i) as u64;
            let tb = t.to_be_bytes();
            for k in 0..8 {
                a[k] ^= tb[k];
            }
            r[i - 1].copy_from_slice(&block[8..]);
        }
    }

    out[..8].copy_from_slice(&a);
    for (i, blk) in r.iter().enumerate().take(n) {
        out[8 + i * 8..16 + i * 8].copy_from_slice(blk);
    }
    Some((n + 1) * 8)
}

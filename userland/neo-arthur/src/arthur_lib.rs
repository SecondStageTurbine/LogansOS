// SPDX-License-Identifier: MPL-2.0
//! Neo-arthur library target (placeholder).
//!
//! The M2 encryption KATs used to live here, but this `[lib]` target is built
//! with `test = false`, so they **never ran** under `cargo test` — and they
//! bound the raw `chacha20poly1305` crate (plus a wrong `[4..12]` nonce layout)
//! rather than M2's own `encrypt_plaintext` / `decrypt_file` / `nonce_for`.
//!
//! The real, *running* direct-function byte-oracle now lives in
//! `tools/arthur-logic-tests` (module `m2_crypto_kats`), which mounts the actual
//! `memory_crypto.rs` and is exercised by the standard `cargo test --workspace`
//! host gate. See ADR-049 (M2 encryption) for the format authority.

#![no_std]

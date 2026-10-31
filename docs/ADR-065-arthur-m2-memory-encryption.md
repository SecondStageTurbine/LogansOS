<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-065: Arthur M2 — memory encryption-at-rest (ChaCha20-Poly1305)

**Status:** Accepted
**Date:** 2026-06-22
**Dispatch:** `Plans/DISPATCH-2026-06-22-arthur-memory-m2-encryption-bounce-cerebro.md`
**Prerequisite:** M1 (plaintext `.mem` memory); ADR-058 (`SYS_GETRANDOM`)
**Scope:** Arthur's per-category `.mem` files gain encryption-at-rest. This ADR
records the data-path design **as corrected at the PM verify-before-merge bounce**
(the original M2 was code-complete but had five defects, B1–B5, that a non-falsifiable
QEMU scenario hid). The on-disk format is unchanged from the original M2; M3/M4
(cap-delegation, forgetting beyond crypto-erase) are out of scope.

## Decision

Wrap each M1 `.mem` file in ChaCha20-Poly1305 AEAD, keyed per category.

- **On-disk `.mem` format:** `version(1B) || nonce(12B) || ciphertext(var) || tag(16B)`.
  The byte-layout authority is the pure module `userland/neo-arthur/src/memory_crypto.rs`
  (no `libneo`, no `MemoryType` deps), so it is host-testable by a direct-function
  byte-oracle (see KAT discipline below).
- **Key file:** `<category>.key` = `key(32B) || counter(16B LE u128)` = 48 B (`KEY_FILE_LEN`).
- **Single-blob model (B1 fix).** Each `.mem` file is **one** AEAD blob under **one**
  nonce. A `remember` decrypts the existing blob to the whole-file plaintext, appends
  the new record line, re-encrypts the **entire** plaintext as a single blob, and
  overwrites the file from offset 0. The original M2 *appended* a second blob per
  write while the reader only ever decrypted the first — so the 2nd `remember`
  silently lost data and the `.key` grew 48 B/save (then the next reader rejected the
  96 B `.key`, generated a fresh key, and lost the whole category). The reader was
  always single-blob; only the writers were wrong.
- **Counter-nonce, write-ahead durability (B2 fix).** The per-category counter is the
  *next nonce to use*. Per save: (1) `reserve_nonce` advances the counter in RAM;
  (2) the `.key` file is persisted + `fsync`ed **first** (durable); (3) the plaintext
  is encrypted with the reserved nonce; (4) the `.mem` blob is written + `fsync`ed.
  A crash between (2) and (4) leaves the counter advanced on disk, so the next boot
  only ever **skips** a nonce value — never reuses one (nonce reuse under one key is
  catastrophic for ChaCha20-Poly1305). The original code's "write-ahead persist" was
  a no-op stub and persisted the **pre-advance** counter; reuse-safety rested entirely
  on `apply_rollback_safety` scanning the first blob's nonce, which is sound only
  under the single-blob rewrite this ADR mandates.
- **Overwrite-from-offset-0, not truncate.** libneo exposes **no** `truncate`/`unlink`/
  `O_TRUNC`/seek primitive. A fresh `sys_open` leaves the file offset at 0; we
  deliberately do **not** drain (draining seeks to EOF → the write would append, the
  original B1 bug). Correctness rests on a grow-only invariant: the `.mem` plaintext
  only ever gains records, so each blob is `>=` the prior one and fully overwrites it
  (no stale tail); the `.key` is always exactly 48 B. **FS gap (flagged):** a real
  `truncate` primitive is the proper general fix for any shrinking path (the M3
  forget-beyond-erase case), and would remove the crypto-erase padding dance below.
- **Crypto-erase (`forget`).** Destroys the key (NIST 800-88: CSPRNG-overwrite the
  `.key`), then re-keys with a fresh key and overwrites `.mem` with a fresh, valid,
  empty-records blob **padded** so its length `>=` the old blob (no stale tail, since
  no truncate). The `\n` padding parses to zero records, so a subsequent `recall` is
  empty until the next `remember`, which re-keys cleanly. After erase the prior
  ciphertext is irrecoverable.
- **Burn-wipe via `None` (B3 fix).** `burn_key` drops the in-RAM key by setting
  `keys[idx] = None` — no `unsafe`. The original copied the **real** key into a buffer
  misleadingly named `zeroed_key` (via an unflagged `unsafe ptr::copy`), preserving
  the key in RAM and never wiping it. M2 now introduces **no new `unsafe`**.
- **Kill-switch (ISC-6).** The `arthur-memory-encryption` feature is **default-OFF**.
  Disabled, neo-arthur is M1 plaintext, bit-for-bit (the M1 append path is untouched
  and lives in its own `#[cfg(not(...))]` branch). The crypto crate, `memory_crypto`,
  and `memory_encryption` modules are entirely feature-gated.

## KAT discipline + falsifiable scenario (B4, B5 fixes)

- **Direct-function byte-oracle (B4).** The original RFC 8439 KAT exercised the raw
  `chacha20poly1305` crate, not M2's `encrypt_plaintext`/`decrypt_file`, *and* it lived
  in a `[lib] test = false` target so it never ran, *and* it asserted a nonce layout
  (`[4..12]`) that the real `nonce_for` (`to_le_bytes()[..12]`) does not use. The real
  oracle now lives in `tools/arthur-logic-tests` (`m2_crypto_kats`), runs under the
  standard `cargo test --workspace`, mounts the real `memory_crypto.rs`, and asserts
  M2's own `encrypt_plaintext` against an independently hand-laid blob byte-for-byte,
  plus decrypt round-trip, tamper → `AuthFail` (output cleared), wrong-key → `AuthFail`,
  the `nonce_for` layout, and distinct-counter → distinct-blob.
- **Falsifiable QEMU scenario (B5).** The original `arthur_memory_encrypted` scenario
  printed a **hardcoded** `…encrypt_write=ok…crypto_erase=ok…` banner on exit 0 while
  neo-init (built with empty features) ran a no-op driver — it tested **no** crypto.
  The scenario now runs the neo-init `arthur_memory_encrypted_tests` driver, which
  returns a non-zero exit on any failure, so the banner is **earned**: (a) two records
  of one category, then recall must return the LATEST (recall re-reads the `.mem`, so
  this catches B1's second-blob corruption); (b) the on-disk `.key` is exactly 48 B
  after two saves — a **direct** falsifier for B1's growth (the bug appends 48 B/save);
  (c) flip one ciphertext byte → recall fails **closed** (no plaintext leak); (d)
  `forget` → recall empty → re-`remember` → recall it.
- **Single-spawn scenario + a kernel finding.** The driver uses ONE arthur process by
  design. `spawn → reap → spawn` with a heavy first process (the M2 arthur does crypto
  + multiple file writes) trips a kernel thread-slab UB panic (`ptr::write` null/
  misaligned in `alloc_from_slab`, `kernel/src/sched/thread.rs`) — a **separate,
  pre-existing scheduler bug** this falsifiable scenario surfaced (the lighter M1
  driver, which also re-spawns, does not hit it). M2 correctness needs no second
  process: recall re-reads the file each call, the `.key` growth is asserted directly
  on disk, and the tamper is applied while arthur idles between IPC turns. The kernel
  spawn-reap-spawn panic is filed as a follow-on, out of M2 scope.

## Consequences

- M2 stays a provable security boundary: nonce-reuse is forbidden by write-ahead
  ordering; fail-closed on any auth failure; crypto-erase is real; the kill-switch is
  a bit-for-bit M1 equality anchor; every crypto property is now exercised by a test
  that can fail.
- The no-truncate FS gap is a known limitation, carried explicitly: grow-only `.mem`
  plus crypto-erase padding are the workarounds until a `truncate` primitive lands.
- No new cross-cutting invariant; no new `unsafe`.

<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Vault Phase A re-spin — genuine KDF KAT + oracle/zeroize fixes → Janus

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Janus (branch `codex/password-vault-phase-a-janus` @ `c6b1a2d8`)
**Status:** NARROW BOUNCE — the crypto **usage is sound** (audited RustCrypto, no nonce-reuse, Argon2id 64 MiB/3/1, real AEAD verify, zeroize wired, genuine RFC KATs + genuine record byte-oracle). But three small fixes before merge, one of which is our own byte-oracle rule violated on the KDF. Push-CI only (no QEMU) — quick.

**PL policy ratified (2026-07-20): external RustCrypto crates are ACCEPTED for userland apps** (kernel stays in-tree/hand-rolled). Record this one-liner in the ADR so future userland coders know the precedent. Your dep choice stands — don't rip it out.

## Rename first — ADR number collision
`ADR-074` is already claimed by Jarvis's Slice-4 (`ADR-074-atrium-kernel-routed-input.md`, in flight). **Rename your ADR to `docs/ADR-075-cap-bounded-password-vault.md`** and fix every internal `ADR-074` reference to `ADR-075`.

## The three fixes (all small)
1. **F2 — genuine KAT on the PRODUCTION `derive_master_key` (the important one; byte-oracle rule).** Today the RFC-9106 KAT exercises Argon2id through a *different* API (`ParamsBuilder` + `new_with_secret` + AD) than `derive_master_key` (`Params::new` + `Argon2::new(Argon2id, V0x13)` + `hash_password_into`, no secret/AD). The only test that calls `derive_master_key` is a **round-trip** (seal-then-open) — self-consistent even if the KDF is wrong (a salt/password-arg swap, `Argon2i` instead of `Argon2id`, or the wrong `Version` would pass every test). That is exactly the closed-loop-green-while-wrong failure the byte-oracle rule forbids. **Add a fixed-input → fixed-expected-key literal assertion against `derive_master_key` itself** (known password + known salt + `INTERACTIVE_V1`-or-a-pinned-param → the exact 32 derived-key bytes, computed from raw Argon2id, not from the function under test).
2. **F3 — de-tautologize the envelope oracle.** In `protocol.rs` the `opcode` (`expected[6..8]`) and `payload_len` (`expected[20..24]`) fields are built with the same `.to_le_bytes()` the encoder uses, so `actual == expected` by construction — an endianness bug wouldn't be caught. Lay those two fields as **literal bytes** (as the record oracle already does for its fields).
3. **F1 — scrub the transient derived-key stack copy.** `derive_master_key` (`lib.rs:92-96`) copies the `[u8;32]` into `MasterKey::from_bytes` *by value* (`Copy`), leaving the local `key` array un-zeroized on the stack. Derive into a `Zeroizing<[u8;32]>` (or `key.zeroize()` after the move) so no transient plaintext key survives.

## Explicitly Phase-B (do NOT do here — the ADR already defers them; just keep them tracked)
- **F4 — header-as-AAD binding:** the ADR guarantees the 40-byte header is AEAD AAD, but no Phase-A code passes `header.encode()` as the `aad`. Phase B wires store/retrieve to bind it + a regression that a mutated header byte forces `Authentication`. (If a 3-line Phase-A regression sealing with `header.encode()` as AAD is easy, it's welcome — but not required now.)
- **F5 — KDF param floor:** `derive_master_key` accepts any params incl. the weak KAT preset; Phase B enforces the lower bound before trusting stored params at unlock. Keep the floor on the Phase-B list.

## Close
- `scripts/worker-close-gate.sh` (rebase-fresh on current `origin/main`; full host-check set; SPDX; Step-4 self-stat). **Push-CI only — no QEMU** (Phase A). RETURN the green push-CI URL + the new `derive_master_key` KAT vector's provenance (raw Argon2id computation, not WebFetch).
- **PM re-verify-delta:** confirm the `derive_master_key` KAT is a genuine fixed→fixed literal (would fail if the KDF wiring changed), the two envelope fields are literals, and F1's stack copy is scrubbed. Then cherry-pick to `main` as ADR-075.

## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-vault-phase-a-oracle-fixes-janus.md`.
- **One-line:** Vault Phase A crypto usage is sound (no nonce reuse, strong Argon2id, real AEAD verify, zeroize wired, genuine RFC KATs + record oracle) — three small fixes before merge: **(F2, important)** add a **genuine fixed→fixed KAT on the production `derive_master_key`** (it's currently only round-trip-validated — a wrong Version/algorithm/arg-swap would pass green, the byte-oracle rule's exact forbidden case); **(F3)** lay the envelope oracle's `opcode`+`payload_len` as **literal bytes** (they're built with the same `to_le_bytes` as the encoder = tautological); **(F1)** derive the master key into a `Zeroizing<[u8;32]>` so no transient key copy is left on the stack. Also **rename the ADR to ADR-075** (074 is taken by Slice 4) and record that **RustCrypto-for-userland is PL-ratified**. F4 (header-AAD) + F5 (param floor) stay Phase B. Push-CI only, no QEMU.
- **RAG:** `derive_master_key argon2 params`, `MasterKey Zeroizing`, `record Header encode aad`, `protocol Header encode opcode`. Health-check first; note-and-fall-back if down.

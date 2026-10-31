<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur memory M2 — encryption-at-rest (build) → Prometheus

**Date:** 2026-06-20
**From:** PM (Claude)
**To:** Prometheus (authored the ratified M2 design; built 10.1 ChaCha20 + 10.3 `SYS_GETRANDOM`). New branch off `origin/main` HEAD: `prometheus/arthur-memory-m2-encryption`.
**Status:** DISPATCHED. **The build of the ratified M2 design — Arthur's memories become encrypted-at-rest with crypto-erase.** All prerequisites are in: **M1 landed** (`2fe9a6e`, the `.mem` substrate), **D1 ratified** (ChaCha20-Poly1305 via the in-tree `chacha20poly1305 v0.10` crate), **`SYS_GETRANDOM` landed** (10.3, the key source). Implement the design THINK exactly, with the PM-review build requirements baked in.

## 0. The decision this build makes
Turn M1's plaintext `.arthur/memory/<category>.mem` into **AEAD-encrypted** records keyed per category, where **burning the `.key` crypto-erases the category** (the D5 forget). Kill-switch off ⇒ M1 plaintext, bit-for-bit.

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-20-arthur-memory-m2-encryption-design.md   # the RATIFIED design (D1-D7) + the PM-review section (the build requirements below come from it)
git show origin/main:kernel/Cargo.toml                                              # chacha20poly1305 = "0.10" (line 51) — the ratified AEAD, already a dep (net/tls_record.rs uses it)
# M1's memory read/write path (the .mem produce/consume boundary M2 wraps) — phase-8 arthur memory (2fe9a6e); find it via the arthur_memory scenario + the .mem record code
git show origin/main:docs/ADR-058-kernel-csprng-sys-getrandom.md                    # the SYS_GETRANDOM key source
git show origin/main:Plans/THINK-2026-06-18-arthur-disagreement-reversibility-design.md  # OT-19 D2: undo backs up .mem ciphertext, never .key
```

## 2. ISCs (build the THINK's D-block)
- **ISC-1 — AEAD via the in-tree crate (D1).** Use `chacha20poly1305 v0.10` (already a kernel dep) for encrypt/decrypt of `.mem` records. Do NOT hand-roll the primitive. Encrypt at the boundary where the `.mem` bytes are produced/consumed (M1's memory write/read path).
- **ISC-2 — Encrypted record format + an independent byte oracle (D2).** `version(1) || nonce(12) || ciphertext(var) || tag(16)`. **Ship a hand-laid byte fixture** (a fixed key+nonce+plaintext → the exact ciphertext+tag) asserted against the decryptor independently — NOT a writer↔reader round-trip alone. **Acquire the AEAD KAT vectors raw (RFC 8439 §2.8.2 / a crate KAT), NOT via WebFetch** (the byte-oracle corollary).
- **ISC-3 — Per-category key lifecycle (D3).** `.key` = key(32) || counter(16), key from `SYS_GETRANDOM(32)` at first category write; cap-protected (OT-17 D3 / the file cap). The counter is the nonce source.
- **ISC-4 — Nonce-counter durability (the PM-review build requirements — load-bearing, not optional).** Nonce = the persisted monotonic counter. **(a) Write-ahead:** the counter must be persisted *durably before* the data it nonces is written (per-record fsync is correct; **batch-reservation** — bump+flush the counter by K once, consume K from memory, a crash only *skips* values — is the preferred form, no fsync-per-record). **(b) Rollback:** the counter-vs-max-observed-nonce check closes the *asymmetric* `.key`-only rollback; **name the residual** — a full-consistent `.mem`+`.key` snapshot rollback is NOT closed (inherent to counter-nonces w/o trusted monotonic state) — and **where a HW monotonic counter (TPM/secure element) exists, source the counter from it.** Do NOT ship "self-healing / no reuse window" unqualified. **(c) NeoFS primitives:** verify NeoFS actually provides the durability barrier + atomic-rename this needs — no POSIX assumption; if absent, surface it as a NeoFS dependency. Nonce reuse under one key is catastrophic — this ISC is the security crux.
- **ISC-5 — Crypto-erase = key-burn (D4 × OT-19).** Forget = overwrite `.key` with `SYS_GETRANDOM` bytes + unlink ⇒ the `.mem` ciphertext (and any OT-19 undo backup, which is ciphertext under that key) is irrecoverable. **undo backs up ciphertext, never `.key`** (the structural exclusion). Prove it.
- **ISC-6 — Fail-closed (D6).** AEAD returns plaintext only inside `Ok`; **auth-tag mismatch → reject + zeroize, never surface unverified bytes**; missing/corrupt `.key` or `SYS_GETRANDOM` failure → degrade to the ephemeral OD-6 window (never crash/leak).
- **ISC-7 — Standards + kill-switch.** **Kill-switch `arthur-memory-encryption` default-OFF ⇒ M1 plaintext, bit-for-bit** (ship the equality regression). New `unsafe`: none expected (the crate is safe) — flag if you reach for it. **No new invariant** (composes M1 + the crate + getrandom — state it; STOP+flag if you think you need one). SPDX. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). ADR-058/060 amendment or a new M2 ADR (the format + crypto-erase + the named rollback residual).

## 3. Gate
- **Host:** the AEAD KAT (raw RFC 8439 vectors + **negative auth-fail** vectors → rejection) + the hand-laid byte oracle + the nonce-uniqueness/write-ahead logic + crypto-erase (key-burn → undecryptable) + the kill-switch equality (off ≡ M1) — green; builds on+off the feature.
- **QEMU scenario `arthur_memory_encrypted`:** write a memory (encrypted) → fresh-session decrypt read-back; **off ⇒ M1 plaintext**; corrupt/tampered record → auth-fail → ephemeral (no leak); **key-burn → category unrecoverable**. At **NEO_SMP=4**.
- **Kernel-touching ⇒ pre-merge `workflow_dispatch` QEMU gate (all 4 shards)** + clippy via Docker. **SERIALIZE behind any other in-flight kernel gate** (the M1/OT-23 collision lesson — do not run your QEMU gate while Genesis's E1b gate is running; rebase LAST onto the then-current main).

## 4. Scope guard
M2 encryption-at-rest **only**. **Do NOT** change M1's substrate/format beyond wrapping it in AEAD, change 10.1's DRBG or 10.3's `SYS_GETRANDOM` (consume them), build M3 (delegation) / M4 (forgetting-policy beyond crypto-erase), or hand-roll crypto (use the crate). No new invariant.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness — **rebase LAST**, base moves fast; the exact CI host-checks; SPDX; self-stat). Two hard rules: green-CI-URL (push-CI + the 4-shard QEMU URL — kernel-touching), and the PM re-inspects your `git show --stat`. **PM scrutinizes ISC-4 (nonce durability) + ISC-6 (fail-closed) hardest** — the security crux. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**Arthur memory is now encrypted-at-rest with crypto-erase** — a milestone). 

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-20-arthur-memory-m2-encryption-build-prometheus.md` (+ your ratified `THINK-2026-06-20-arthur-memory-m2-encryption-design.md` incl. the PM-review section). **Branch:** `prometheus/arthur-memory-m2-encryption` off `origin/main`.
- **One-line:** Build **M2 — Arthur memory encryption-at-rest** from your ratified design. AEAD = **`chacha20poly1305 v0.10` (the in-tree crate, don't hand-roll)** over M1's `.mem`; format `version(1)||nonce(12)||ct||tag(16)` with a **hand-laid byte oracle** + **raw-sourced RFC 8439 KAT + negative auth-fail vectors** (NOT WebFetch); per-category `.key`=key(32)||counter(16) from `SYS_GETRANDOM`. **Security crux (ISC-4):** nonce-counter **write-ahead persist** (batch-reservation preferred over per-record fsync), **name the full-snapshot rollback residual** (the `.key`-only case self-heals via counter-vs-max-nonce; the full `.mem`+`.key` rollback is the inherent residual — use a HW monotonic counter where available), and **verify NeoFS's durability + atomic-rename primitives** (no POSIX assumption). **Crypto-erase = key-burn** (overwrite+unlink ⇒ ciphertext + OT-19 undo-backup irrecoverable; undo backs up ciphertext, never `.key`). **Fail-closed:** auth-fail rejects+zeroizes, never surfaces unverified plaintext; degrade-to-ephemeral. **Kill-switch `arthur-memory-encryption` off ⇒ M1 plaintext bit-for-bit.** No hand-rolled crypto, no new invariant. Gate: host KATs + the `arthur_memory_encrypted` QEMU scenario (write→fresh-session decrypt; off≡M1; tamper→ephemeral; key-burn→unrecoverable) @ NEO_SMP=4 + the 4-shard gate — **serialize behind Genesis's E1b gate, rebase LAST.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + 4-shard QEMU URLs.**

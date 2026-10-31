<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Password vault — the split-GPG secret architecture (OT-60 shelf apps) → Janus

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Janus (main-tier; free lane)
**Status:** DISPATCHED — **two-phase**. **Phase A (design ADR + host-KAT crypto core) is push-CI only — NO QEMU gate**, so it starts immediately without contending for runners. **Phase B (the vault process + cap-gated IPC running in the OS) is kernel/QEMU-gated + AAG** and staggers behind the priority lanes (Jarvis Slice 4, Cerebro OT-63, Oracle Slice 3). Motivated by the Qubes pass (`docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` §3 / T7, C4) — the split-GPG pattern, our #1 concrete OT-60 shelf app.

## The architecture — split-GPG, mapped onto capabilities
Qubes keeps secrets in a `vault` qube with **no network**, and only *operations* (decrypt, sign) cross the boundary over qrexec — the key material **never leaves the vault**. We map that directly onto capabilities (cheaper than a VM):
- The vault is a **cap-bounded userland process** holding the secrets, with **NO network capability** (grep-checkable + asserted — a vault that can't reach the network can't exfiltrate).
- Secrets **never cross the process boundary**. Callers send *requests* (`unlock`, `store`, `retrieve`, later `sign`/`decrypt`) over a **cap-gated IPC channel** (the same SurfaceCap-style channel convention); the vault returns only the *result* of an operation, or places a retrieved secret into a **caller-provided grant** with least rights — never the master key, never the store.
- `qrexec` policy → our **capability derivation** (finer than Qubes: per-operation caps with rights attenuation + transitive revocation — the Qubes FINDINGS §2.3 "key insight" that caps are strictly more expressive than domain-identity policy).

## Phase A — design ADR + host-KAT crypto core (push-CI only, start now)
1. **ADR-0xx (author it):** the vault threat model (what a compromised caller can/can't get: only the *decrypted output of an authorized op*, never the master key or the ciphertext store), the crypto choices (below), the **on-disk encrypted-secret byte layout** with an **authoritative worked example** (I-12 — the byte-oracle rule: a format ADR ships a hand-laid byte example), and the cap-gated operation protocol (verbs + wire envelope, byte-oracle per verb).
2. **Crypto core (host-KAT'd, `#![no_std]`-compatible):**
   - **AEAD for secrets-at-rest** — choose **one** (recommend ChaCha20-Poly1305, RFC 8439; AES-GCM acceptable). Note: the kernel has AES-CCM in `net/wpa/ccmp.rs` but it's WPA-scoped/kernel-side — the vault needs its own userland-usable AEAD; do not entangle with the WPA path.
   - **Master-key KDF** from the user's master password — **memory-hard** (recommend **Argon2id**; scrypt acceptable). None exists yet — this is new.
   - **CSPRNG** — reuse the existing kernel DRBG/CSPRNG (already shipped) for salts + nonces; do **not** roll your own.
   - **Zeroize** — master key + plaintext secrets are wiped from memory after use (no lingering plaintext).
   - **KATs FROM RAW SOURCE (mandatory — the exact-bytes rule):** pull the AEAD + KDF known-answer vectors from the **raw authoritative source** (RFC 8439 §2.8.2 for ChaCha20-Poly1305; the Argon2 reference `KATs`/RFC 9106 for Argon2id) — `curl`/`git` the literal bytes, **NEVER via WebFetch/an LLM-summarizing read** (it mangles exact hex and poisons the KAT — this bit us in BUILD-W0). The KAT is only trustworthy if its acquisition is. Cite the raw source + the exact vector in the ADR.
3. **Byte-oracle (I-12):** the encrypted-secret record (header ‖ salt ‖ nonce ‖ ciphertext ‖ tag) is an on-disk layout ⇒ a **hand-laid byte fixture** asserted independently of the writer+reader, plus the cap-gated verb wire envelopes. This is the falsifiable property in place of a kill-switch (the vault is an additive app, not a substrate layer — but if Phase B adds any kernel primitive, that primitive ships a kill-switch).

## Phase B — the vault process + cap-gated operation IPC in the OS (QEMU-gated, staggers)
- The vault as a cap-bounded process (the `neo-worker`/`template-app` userland pattern); the cap-gated operation channel; store unlock/lock lifecycle; the retrieve-into-a-least-rights-grant path.
- **I-14 fault-containment (required — the vault is a secret-holding subsystem):** (a) the vault/IPC path takes no AI-governance lock; (b) a vault crash is contained — it does not leak secrets (zeroized), does not stall governance (I-14(b)), and a misbehaving caller can't wedge it (bounded per-channel handling); (c) no new DMA. **No-network-cap is the containment property** — assert the vault process holds no network capability.
- A **QEMU scenario** proving: unlock with the right master key succeeds + wrong key fails (no oracle leak on failure), store→retrieve round-trips through the cap-gated channel, the retrieved secret lands in the caller grant (never the master key), and the vault holds no network cap.

## Scope boundary — do NOT build here
The **GUI shelf front-end** (the visual password-manager app) — that needs Slice 4 input + the GUI slices; **deferred**. Slice B is the **headless vault + protocol** the GUI will later drive. Browser/autofill integration — deferred. Keep Phase B to the vault core + IPC.

## Gate + close + AAG
- **Phase A:** host KATs (crypto vectors from raw source) + the byte-oracle fixtures; push-CI green (no QEMU). RETURN the ADR + green push-CI URL + the raw-source KAT provenance.
- **Phase B:** `scripts/worker-close-gate.sh` (rebase-fresh; full host-check set; SPDX; Step-4 self-stat); 4-shard `workflow_dispatch` QEMU gate — **stagger, one wave at a time**, behind the priority lanes. RETURN green push-CI + 4-shard URLs.
- **Disk reclaim** after PM-confirmed merge.
- **AAG (PM, cross-model lenses):** ABI/byte-oracle (the at-rest record + verb envelopes byte-correct + independent; **KATs genuinely from raw source, not mangled**), memory-safety (zeroize actually clears; no plaintext/key lingering; no `unsafe` misuse), completeness (wrong-master-key leaks no oracle; retrieve never returns the master key/store; **the vault provably holds no network cap**), concurrency (cap-gated channel handling, no governance lock).

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-password-vault-splitgpg-janus.md`. Design input: `git show origin/main:docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` §3 (split-GPG) + §2.3 (caps > domain-policy). The userland app pattern: `userland/neo-worker` / `userland/template-app`. The existing CSPRNG/DRBG to reuse (don't roll your own). PM artifacts live on `origin/main`; `git show`, not a local open.
- **One-line:** Build the **password vault** = Qubes' split-GPG, on capabilities: a **cap-bounded process holding secrets with NO network cap**, secrets **never cross the boundary** — callers send *operations* (`unlock`/`store`/`retrieve`) over a **cap-gated IPC channel** and get back only the result / a least-rights grant, never the master key or the store. **Phase A (start now, push-CI only, no QEMU):** an ADR (threat model + the on-disk encrypted-secret byte layout w/ a hand-laid worked example + the operation protocol) + a host-KAT'd `#![no_std]` crypto core — **AEAD** (ChaCha20-Poly1305, RFC 8439) + **memory-hard KDF** (Argon2id, RFC 9106) + reuse the kernel CSPRNG + zeroize — with **KATs pulled from RAW SOURCE, never WebFetch** (the exact-bytes rule; WebFetch mangles hex and poisons the KAT). **Phase B (staggers behind the kernel lanes):** the vault process + cap-gated operation IPC in the OS, QEMU-gated, with I-14 containment (no network cap = the containment property; vault crash leaks no secret, stalls no governance). Do NOT build the GUI shelf (needs Slice 4 + GUI slices — deferred). Byte-oracle the at-rest record + verb envelopes.
- **RAG:** `CSPRNG DRBG random bytes`, `ccmp AES-CCM wpa`, `neo-worker userland process spawn`, `IPC channel cap grant rights`, `capability derivation attenuate revoke`. Health-check first; note-and-fall-back if down (PL owns RAG startup).

<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Password vault Phase B — the vault process + cap-gated operation IPC in the OS → Oracle

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Oracle (main-tier; free lane)
**Status:** DISPATCHED — kernel/userland-touching (the vault runs as a cap-bounded process, IPC over existing primitives) ⇒ 4-shard `workflow_dispatch` QEMU gate + AAG. **Staggers behind Janus's OT-63 GPU Slice 3** on the runners (one 4-shard wave at a time). Builds on Janus's merged **Phase A** (`userland/vault-core`, ADR-075 — the `#![no_std]` ChaCha20-Poly1305 + Argon2id crypto core, host-KAT'd, with the genuine `derive_master_key` reference KAT). Phase B is the OS integration.

## What Phase B is
Phase A shipped the crypto *core* (a library: seal/open/derive, the encrypted-record byte layout, the operation-protocol envelopes, all host-tested). **Phase B makes it a live secret-holding subsystem in the OS** — the Qubes split-GPG pattern realized on capabilities:
- The vault as a **cap-bounded userland process** (the `neo-worker`/`template-app` pattern) holding the encrypted store, with **NO network capability**.
- Callers send **operations** — `unlock`(master password) → `store`(name, secret) / `retrieve`(name) / `lock` — over a **cap-gated IPC channel** (the SurfaceCap-style convention). The vault returns only the *result*: a retrieved secret is placed into a **caller-provided grant with least rights**, never the master key, never the ciphertext store.
- Secrets and the master key **never cross the process boundary** except as an authorized operation's output.

## The three Phase-A obligations to discharge (from ADR-075 §F4/F5 + §3)
1. **Fresh nonce from the kernel CSPRNG (F4-adjacent).** Phase A deferred nonce generation. Every `seal` in the running vault must draw a **fresh 96-bit nonce from `SYS_GETRANDOM`** (the shipped kernel CSPRNG/DRBG — reuse it, do NOT roll your own); a nonce is never reused with the same derived key; generation failure fails **closed**. Prove no-nonce-reuse in the running path.
2. **Header-as-AAD binding (F4).** The store/retrieve path must pass the encrypted-record's 40-byte header (salt + Argon2 params + version) as the AEAD **Associated Data**, so an attacker can't swap the salt/params on a stored record (downgrade/confusion). Ship the regression: a **mutated header byte forces `Authentication` failure** on open.
3. **KDF parameter floor (F5).** Before trusting the stored Argon2 params at unlock, enforce the lower bound (≥ the `INTERACTIVE_V1` floor) — a record claiming weak params is rejected, not honored. Guard + test.

## I-14 fault-containment (required — the vault is a secret-holding subsystem)
- **(a) No-network-cap is THE containment property.** The vault process's spawn manifest / handle set contains **only** its operation channel + store cap(s) + required runtime handles — **no socket, interface, DNS, or network-service channel/meta-cap.** A vault that cannot reach the network cannot exfiltrate. **The test inspects the actual child handle set** (per ADR-075: "grep is supporting evidence, not the proof") — assert the network cap is absent from the live process's table.
- **(b) Liveness / crash containment:** a vault crash (bad operation, malformed record, flood) leaks no secret (Phase-A zeroize holds through process teardown) and stalls no governance (bounded per-channel handling; no AI-governance lock on the vault/IPC path).
- **(c) No new DMA.**

## Kill-switch + byte-oracle
- **Kill-switch:** the vault is an additive userland app — vault-off ≡ no vault (feature-gated out; no regression-to-prior-layer needed since it adds no substrate). If Phase B adds any **kernel primitive** (it should not — it composes existing IPC/cap/spawn syscalls), that primitive ships a kill-switch.
- **Byte-oracle (I-12):** the Phase-A record + verb byte-oracles are exercised **end-to-end** in the running vault (the on-disk record written by the live vault matches the hand-laid fixture; the operation envelopes on the wire match). No new layout expected — if one is added, hand-lay its fixture.

## Scope boundary — do NOT build here
The **GUI shelf front-end** (the visual password-manager app you unlock/type into) — that's a later G1 app-UI build (it needs the interactive-surface + app-toolkit work); **deferred**. Browser/autofill integration — deferred. Phase B is the **headless vault process + the cap-gated operation protocol**, which the GUI later drives.

## Gate + close + AAG
- Host KATs (the F4/F5 guards, the nonce-freshness logic) + a **QEMU scenario**: spawn the vault process; unlock with the right master password succeeds and a **wrong password fails with no oracle leak**; `store` → `retrieve` round-trips through the cap-gated channel; the retrieved secret lands in the caller's **least-rights grant** (never the master key); a **mutated-header record fails `Authentication`**; and the vault process **holds no network cap** (inspect the handle set). `scripts/worker-close-gate.sh` (rebase-fresh; full host-check set incl. kernel clippy; SPDX; Step-4 self-stat). 4-shard `workflow_dispatch` gate — **stagger behind Janus**. Work in `Workers\oracle`; disk-reclaim after PM-confirmed merge.
- **AAG (PM, cross-model lenses):** completeness (wrong-master-key leaks no oracle; retrieve never returns the master key/store; **the no-network-cap is proven by handle-set inspection, not grep**; header-AAD binding actually enforced), memory-safety (zeroize holds through teardown; no plaintext/key lingering), concurrency (cap-gated channel handling, no governance lock; peer-death/vault-crash clean), ABI/byte-oracle (the live record/envelopes match the Phase-A fixtures).

## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-vault-phase-b-process-ipc-oracle.md`. The Phase-A core you integrate: `git show origin/main:userland/vault-core/src/lib.rs` (+ `record.rs`/`protocol.rs`) and `git show origin/main:docs/ADR-075-cap-bounded-password-vault.md` (threat model + the §F4/F5 Phase-B obligations + the byte layout). The userland process pattern: `userland/neo-worker`. Reuse the kernel CSPRNG (`SYS_GETRANDOM`) — do NOT roll your own. Work in `Workers\oracle`.
- **One-line:** Vault **Phase B** — make Janus's Phase-A crypto core a live OS subsystem: a **cap-bounded vault process with NO network cap** holding the encrypted store; callers send **operations** (`unlock`/`store`/`retrieve`/`lock`) over a **cap-gated IPC channel** and get back only the result / a least-rights grant — the master key + store never cross the boundary. Discharge the three Phase-A obligations: **fresh `SYS_GETRANDOM` nonce per seal** (no reuse, fail-closed), **header-as-AAD binding** (mutated-header → `Authentication` fail), **KDF param floor** (reject weak stored params). **I-14: no-network-cap is the containment property — prove it by inspecting the live process's handle set** (not grep); vault crash leaks no secret, stalls no governance. QEMU scenario: unlock ✓ / wrong-password no-oracle-leak, store→retrieve round-trip into a least-rights grant, mutated-header fails auth, network-cap absent. GUI shelf deferred. Close-gate + 4-shard (stagger behind Janus) + AAG.
- **RAG:** `vault-core seal open derive_master_key`, `SYS_GETRANDOM csprng`, `neo-worker spawn process handle`, `IPC channel cap grant rights`, `capability least rights attenuate`. Health-check first; note-and-fall-back if down (PL owns RAG startup).

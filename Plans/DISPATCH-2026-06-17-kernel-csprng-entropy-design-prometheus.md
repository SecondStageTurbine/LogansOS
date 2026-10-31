<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: kernel CSPRNG + entropy source — design prep → Prometheus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Prometheus (deep-research/design lane). **Fresh** branch off `origin/main` HEAD: `prometheus/kernel-csprng-entropy-design`.
**Status:** DISPATCHED. **Design + raw-source research only — NO production code, NO `ci.yml`.** One design THINK under `Plans/`. Non-colliding (touches only `Plans/`).

## 0. The decision this sets up
LogansOS has **no kernel CSPRNG / entropy source today** — and it has become a load-bearing gap:
- **W2's `gen_snonce` is a counter, "NOT a CSPRNG; bring-up only"** (`mt7601u/mod.rs:160,1510`, ADR-057). A predictable SNonce means W2 can associate but **cannot be a security boundary**. The fix is a real CSPRNG; this design is its prerequisite.
- **Any crypto that needs unpredictability** (WPA SNonce now; Arthur's cap-escalation nonces, future kTLS/network randomness, key generation) needs it.
- It is foundational to the sovereignty thesis: a sovereign OS that does its own crypto must own its own randomness — not borrow trust.

This note designs the kernel CSPRNG + entropy substrate so the W2 close (and Arthur's later crypto) is *consuming a decided design*, not inventing entropy under pressure.

## 1. What to work out (raw-source the references; cite file:line/SHA)
- **Entropy sources on x86_64 bare metal.** `RDRAND`/`RDSEED` (Intel/AMD on-chip DRBG — availability via CPUID, the retry/failure protocol, the well-documented trust controversy); jitter/timing entropy; interrupt-timing entropy; the **early-boot entropy problem** (little/no entropy at boot — the classic Linux `getrandom` blocking issue) and how `RDRAND` mitigates it. **Do we trust `RDRAND` alone, or mix it with an independent source?** (The conservative answer + why.)
- **The CSPRNG construction.** A vetted DRBG — ChaCha20-based (the Linux post-2016 design) vs HMAC-DRBG/CTR-DRBG (NIST SP 800-90A) — built on **RustCrypto** primitives (the W0/W0b `#[cfg(feature)]` precedent — `chacha20`, `sha2`/`hmac` already in tree; **no hand-rolled crypto**, Article 15.7). The reseed policy (when, from what, the reseed interval) and the fork/VM-reseed hazards.
- **The in-kernel API + the consumer.** The kernel-internal `fill_random(&mut [u8])` the WiFi supplicant's `gen_snonce` (and future consumers) call; where it lives; its lock-order/IRQ posture (must be callable from the supplicant path without violating the lock order — is it a leaf?).
- **The userland surface.** A `getrandom`-equivalent syscall for Arthur/userland crypto. Is randomness a **capability** or an ambient primitive like time/`SYS_WRITE(fd=1)`? (Recommend — note that `getrandom` is conventionally ambient, but state the capability-model stance per Article 12.1.)
- **I-12 oracle + kill-switch.** KAT the DRBG against **published** test vectors (NIST CAVP DRBG, or the ChaCha20 stream KATs — **raw-source, never WebFetch**; cite SHA). The entropy *source* can't be KAT'd (it's random) — so the design must say how correctness is gated (the DRBG-given-a-fixed-seed is deterministic and KAT-able; the entropy mixing is property-tested). Kill-switch shape (a build without the RNG ≡ today; but note: consumers like W2-as-security-boundary then simply can't be enabled).
- **Threat model fit (I-14 / the governance core).** Confirm the RNG path takes no AI-governance lock and is fault-contained; note it does **not** become a new cross-cutting invariant unless genuinely required (Rule 2).

## 2. Deliverable
`Plans/THINK-2026-06-17-kernel-csprng-entropy-design.md`:
- the entropy-source survey + the RDRAND-trust decision (mix or not);
- the DRBG choice + reseed policy (RustCrypto-based, no hand-rolled crypto) with a recommendation;
- the in-kernel `fill_random` API + lock-order posture + the `gen_snonce` consumer wiring;
- the userland `getrandom` surface + the capability-vs-ambient stance;
- the I-12 KAT plan (published DRBG vectors, raw-source) + the kill-switch;
- a build slicing (e.g. entropy source → DRBG+KAT → in-kernel API + W2 `gen_snonce` swap → userland syscall), each kill-switchable, each gated;
- **open decisions for PM/PL ratification** before any of it is dispatched.

## 3. Scope guard
Design + `Plans/` THINK **only**. **Do NOT** write production code, touch `ci.yml`/kernel/`ai-ops-tests`, or pre-empt the Arthur/benchmark lanes. No hand-rolled crypto in the design — RustCrypto primitives only. Raw-source any exact vector/CPUID detail; flag second-hand numbers. **⚠ PUSH the branch before reporting done.**

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-kernel-csprng-entropy-design-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/kernel-csprng-entropy-design` off `origin/main` (fresh).
- **One-line:** Design the **kernel CSPRNG + entropy source** — the prerequisite to making W2 a real security boundary (its `gen_snonce` is a counter, "NOT a CSPRNG; bring-up only", ADR-057) and to all future OS crypto. Work out: x86_64 entropy sources (`RDRAND`/`RDSEED` + the trust question — mix or not; the early-boot entropy problem); the DRBG construction (ChaCha20 vs NIST CTR/HMAC-DRBG) on **RustCrypto** primitives (no hand-rolled crypto, Article 15.7) + reseed policy; the in-kernel `fill_random` API (lock-order/IRQ posture; the `gen_snonce` consumer) + a userland `getrandom` syscall (capability vs ambient — recommend); the **I-12 KAT** against published DRBG vectors (raw-source, **never WebFetch**, cite SHA) + the kill-switch; and confirm it takes no AI-governance lock / needs no new invariant (Rule 2). Deliver `Plans/THINK-2026-06-17-kernel-csprng-entropy-design.md` with a build slicing + open decisions to ratify. **Design doc only — no code, no `ci.yml`.** Push before reporting done.

<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: kernel CSPRNG BUILD 10.1 — entropy source + ChaCha20 DRBG + KATs → Prometheus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Prometheus (designed the CSPRNG; host-tier crypto + light coding — the W0/W0b precedent). New branch off `origin/main` HEAD: `prometheus/kernel-csprng-10.1-entropy-drbg`.
**Status:** DISPATCHED. **Host-tier — published-vector KATs (I-12).** The first slice of the ratified kernel-CSPRNG arc. *(Note: "10.x" is the CSPRNG THINK's internal slice numbering — NOT bible Phase 10. This is security infrastructure that unblocks the W2 security boundary, OT-2.)*

## 0. The decision this build makes
Build the **host-proven RNG core** — the entropy sources + the ChaCha20 DRBG + the KATs — behind the `kernel-rng` feature (default-off ≡ today bit-for-bit). This is the dongle-independent, dependency-light slice: no kernel API wiring, no W2 swap, no syscall (those are 10.2/10.3). It proves the crypto is correct before anything consumes it — exactly the W0/W0b pattern.

## 1. Authoritative design — RATIFIED (read first, build to it)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-17-kernel-csprng-entropy-design.md   # §10 = the ratified D1-D7; §2 entropy, §3 DRBG, §6 KAT plan, §8 slicing
```
The design is **decided** (D1–D7, ratified 2026-06-17) — build it, don't re-decide it. RustCrypto primitives only (no hand-rolled crypto, Article 15.7); the `chacha20`/`sha2` crates are already in-tree (kTLS/wifi precedent).

## 2. ISCs (the ratified design is law)
- **ISC-1 — Entropy layer (D1 mix + D6 fail-closed).** `RDRAND` (CPUID bit-30 detection; the 10-retry protocol) **mixed** with timer-jitter (D1 — eliminates single-source trust). **D6 fail-closed:** if `RDRAND` is unavailable, **panic** (block-5s-then-panic) — **jitter alone is NOT trusted as a sole seed** (it is a *mixing* input only; do not silently degrade to weak entropy). The `RDRAND` intrinsic is the one permitted new `unsafe` — carry a `// SAFETY:` note. The mixing/decision logic factors into a host-testable `*_logic` split (RDRAND stubbed); the live `RDRAND` path is exercised under QEMU/hardware.
- **ISC-2 — ChaCha20 DRBG (D2 + D3).** ChaCha20 (in-tree) keyed from the mixed seed, with SHA-256 seed-mixing (personalization + reseed count). **Reseed at 10,000 calls / 60 s** (D3), whichever first; counter-wrap guard (force reseed before ChaCha20 block-counter exhaustion). The DRBG given a **fixed seed is deterministic** → host-KAT-able.
- **ISC-3 — The KATs (D7, I-12 — raw-source, never WebFetch).** (a) **ChaCha20** against **RFC 8439** published vectors; (b) **SHA-256** against **NIST** published vectors; both **raw-sourced** (`curl`/`git` the authoritative text, cite SHA — never a summarizing fetch). (c) A **golden composition self-test**: a fixed seed → a recorded golden DRBG output stream (the wiring regression *on top of* the component anchors — not the sole oracle). The entropy *source* is not KAT-able (it's random); state that explicitly.
- **ISC-4 — Standards + the dep-isolation requirement.** **Kill-switch:** `kernel-rng` default-off ⇒ the crypto deps aren't pulled and the kernel is **bit-for-bit today** (the Rule-1 regression). **No AI-governance lock; no new invariant (Rule 2)** — confirm explicitly. No hand-rolled crypto. **⚠ DEP-ISOLATION (the #243 trap):** `chacha20`/`sha2` are exactly the RustCrypto-family crates whose `crypto_common` leaked `std` into the no_std userland binaries and broke benchmark CI #243 (`E0152 duplicate lang item panic_impl`). The `kernel-rng` deps **must NOT unify into the no_std userland binaries** — keep them `#[cfg(feature = "kernel-rng")]`-gated to the kernel, default-off, and **verify with the exact CI command** (§3). (Jarvis's consolidation is adding a workspace-hygiene guard for this class — coordinate; do not re-trigger the leak.)

## 3. Gate
- **Host:** the three KATs green (ChaCha20 RFC 8439 + SHA-256 NIST + the golden composition); the entropy-mixing/`*_logic` unit-tested (incl. the no-RDRAND fail-closed decision via the stub); builds **on AND off** `kernel-rng` (kill-switch ≡ today); `cargo fmt --all --check`.
- **`cargo clippy --workspace --all-targets -- -D warnings` GREEN** — the **exact CI command** (not `-p`), specifically to catch any `chacha20`/`sha2` leak into the no_std userland binaries (the #243 trap). If the no_std binaries fail to build in the full-workspace context, the dep isolation is wrong — fix it before pushing.
- **Kernel-touching (the rng module + the `RDRAND` path) ⇒ run the pre-merge `workflow_dispatch` QEMU gate green.** Add a `kernel_rng_selftest` QEMU scenario: boot → `RDRAND` available → the DRBG boot self-test (golden composition) passes (QEMU needs `rdrand` passthrough — `-cpu host,+rdrand` or equivalent; document it; the no-RDRAND path must hit the fail-closed panic, not hang).

## 4. Scope guard
The entropy layer + the ChaCha20 DRBG + the KATs **only**, behind `kernel-rng` default-off. **Do NOT** build the in-kernel `fill_random` consumer API or the **W2 `gen_snonce` swap** (10.2), the userland `SYS_GETRANDOM` (10.3), RDSEED (D5 — deferred), or any AI-governance/lock touch. No hand-rolled crypto; the only new `unsafe` is the documented `RDRAND` intrinsic; no new invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; the three KATs + the mixing tests green; builds on+off `kernel-rng`; **`cargo clippy --workspace --all-targets` green (dep-isolation verified)**; **pre-merge QEMU gate green** (`kernel_rng_selftest`); push (CI). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (a short ADR for the CSPRNG, or fold into the THINK; `OPEN-THREADS.md` OT-2 → "10.1 landed, 10.2 next"). Next: 10.2 (the `fill_random` kernel API + the W2 `gen_snonce` swap — the OT-2 close).

**Disk reclaim (after PM confirms merge):** once on `origin/main` and PM-confirmed, delete this build's tree (`git worktree remove` / folder delete; never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-kernel-csprng-10.1-entropy-drbg-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/kernel-csprng-10.1-entropy-drbg` off `origin/main` (fresh).
- **One-line:** Build **CSPRNG 10.1 — the host-proven RNG core** (your ratified design, `THINK-2026-06-17-kernel-csprng-entropy-design.md` §10 = D1–D7). The entropy layer: **`RDRAND` mixed with timer-jitter** (D1), **fail-closed panic if no RDRAND** — jitter is NOT a trusted sole seed (D6); the `RDRAND` intrinsic is the one documented `unsafe`. The **ChaCha20 DRBG** (in-tree `chacha20`+`sha2`, no hand-rolled crypto): SHA-256 seed-mix, **reseed 10K calls / 60s** (D3), counter-wrap guard; deterministic given a seed. The **KATs (D7, raw-source never WebFetch):** ChaCha20 **RFC 8439** + SHA-256 **NIST** published vectors + a **golden composition** self-test. **All behind `kernel-rng` default-off ≡ today bit-for-bit; no AI-governance lock; no new invariant.** **⚠ The #243 trap is YOUR risk here:** `chacha20`/`sha2` are the crate family whose `crypto_common` leaked `std` into the no_std userland binaries — keep the deps `kernel-rng`-gated and **run `cargo clippy --workspace --all-targets -- -D warnings` (the exact CI command, not `-p`)** to prove the no_std binaries still build; coordinate with Jarvis's consolidation hygiene-guard. Kernel-touching ⇒ add the `kernel_rng_selftest` QEMU scenario (boot DRBG self-test; needs rdrand passthrough) + **run the pre-merge QEMU gate green**. **Scope: 10.1 ONLY — NO `fill_random`/W2-swap (10.2), NO `SYS_GETRANDOM` (10.3), NO RDSEED.** fmt/clippy clean.

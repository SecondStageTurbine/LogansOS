<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CSPRNG 10.3 — `SYS_GETRANDOM` userland randomness + the kernel fill-helper → Prometheus

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Prometheus (owns the CSPRNG arc — built 10.1). New branch off `origin/main` HEAD: `prometheus/csprng-10.3-sys-getrandom`.
**Status:** DISPATCHED. **The userland randomness surface — unblocks Arthur's memory encryption (M2).** 10.1 built the ChaCha20 DRBG; this exposes it to *userland* via a native `SYS_GETRANDOM` syscall + a libneo wrapper, so userland (Arthur) can get key material. **Independent of W2** (per the canonical slicing) — this does **not** touch the W2 SNonce swap (that's the W2-gated remainder of OT-2, Cerebro's).

## 0. The decision this build makes
Userland can obtain CSPRNG bytes. A native `SYS_GETRANDOM` syscall fills a userland buffer from 10.1's DRBG; a libneo wrapper makes it ergonomic. This is the **userland randomness slice** Arthur's memory encryption (OT-17 M2) depends on. **Not POSIX `getrandom`** — our own ABI (Article 5.1), getrandom-*shaped*. **No W2 swap** (still gated on W2 silicon), **no Arthur encryption** (that's M2, the consumer).

## 1. References
```
git fetch origin
git show origin/main:kernel/src/rng/                                  # 10.1's DRBG core (the source)
git show origin/main:docs/ADR-057-kernel-csprng.md                    # the CSPRNG ADR
git show origin/main:docs/OPEN-THREADS.md                             # OT-2 (canonical slicing: 10.3 = SYS_GETRANDOM, independent of W2), OT-17 (M2 is the consumer)
```
Reuse 10.1's `kernel/src/rng/` DRBG. The syscall dispatch + userland-buffer-validation idioms exist (the Phase-4/5 syscall ABI; `SYS_READ`/`SYS_GETRANDOM` shape).

## 2. ISCs
- **ISC-1 — The kernel random-fill helper.** A kernel function that fills a `&mut [u8]` of arbitrary length from the 10.1 ChaCha20 DRBG (reseed-aware per 10.1's D3). If 10.1 exposed no public consumer fn, add the minimal one here (it's the non-W2-gated half of the former "10.2"). The DRBG is the only source — no new entropy path.
- **ISC-2 — `SYS_GETRANDOM` syscall (the security-critical one).** A native syscall `getrandom(buf_ptr, len) -> isize` filling the caller's buffer with `len` DRBG bytes. **Validate the userland buffer rigorously** — the classic syscall hazard: confirm `buf_ptr..buf_ptr+len` is **userland-mapped + writable**, cap `len` to a sane max, reject on bad pointer with the error ABI (no kernel-memory disclosure, no write through an unvalidated pointer). Fail-closed: if the DRBG is somehow unseeded (shouldn't happen — 10.1 seeds at boot, D6), return an error, never zero/weak bytes.
- **ISC-3 — libneo wrapper.** A userland `libneo` fn (e.g. `libneo::rng::fill(&mut [u8])` / `neo_getrandom`) over the syscall — the ergonomic surface Arthur's M2 will call. Thin; handles the syscall return/error.
- **ISC-4 — KAT (I-12) via the neo-test fixed seed.** Randomness isn't directly KAT-able, so use 10.1's pattern: **neo-test mode uses a fixed DRBG seed** ⇒ `SYS_GETRANDOM` returns a *deterministic, known sequence* — assert it (a published-from-our-own-DRBG known sequence, like 10.1's `kernel_rng_selftest`). Plus: distinct successive calls return distinct bytes (non-triviality); the buffer-validation truth table (bad ptr / oversize len / unmapped → correct error); length-exactness. Real entropy on hardware.
- **ISC-5 — Standards + kill-switch.** Feature-gated (extend `kernel-rng` or a `getrandom` feature); **default-off ⇒ the syscall returns `ENOSYS` (or isn't registered), kernel bit-for-bit today.** Buffer-validation + fill logic in a host-testable `*_logic` split. **New `unsafe`:** the validated userland-buffer write (documented `/// SAFETY` — the validation is the precondition). No new invariant; no AI-governance touch. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). ANSPEC + ADR-057 amendment (the userland surface).

## 3. Gate
- **Host:** the fixed-seed KAT (deterministic sequence) + the buffer-validation truth table + length-exactness/non-triviality, green; builds on+off the feature.
- **QEMU scenario `sys_getrandom`:** a userland process calls the syscall → gets `len` bytes → a second call returns distinct bytes → a bad buffer (unmapped/oversize) is rejected with the error ABI. At **NEO_SMP=4**.
- **Kernel-touching (new syscall) ⇒ pre-merge `workflow_dispatch` QEMU gate green (all 4 shards)** + clippy via Docker.

## 4. Scope guard
The kernel fill-helper + `SYS_GETRANDOM` + the libneo wrapper **only**. **Do NOT** do the **W2 SNonce swap** (W2-gated — Cerebro's; the remaining OT-2 piece), the **Arthur M2 encryption** (the consumer, a separate memory slice), add new entropy sources (use 10.1's DRBG), or add POSIX-`getrandom` flag semantics (our ABI, minimal — `buf,len` only unless a flag earns its place). No new invariant.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-check set on the rebased tree, SPDX, self-stat). The two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL + the `workflow_dispatch` QEMU URL — kernel-touching), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (ADR-057 amendment; **OT-17 M2 unblocked** — Arthur encryption can now generate keys; **OT-2 still open** until the W2 swap lands). Next: **M2** (Arthur memory encryption — now has its randomness source) and, on W2's silicon, the SNonce swap that closes OT-2.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-csprng-10.3-sys-getrandom-prometheus.md`. **Branch:** `prometheus/csprng-10.3-sys-getrandom` off `origin/main`.
- **One-line:** Build **CSPRNG 10.3 — the `SYS_GETRANDOM` userland randomness surface** over 10.1's DRBG: (ISC-1) a kernel random-fill helper from the ChaCha20 DRBG (the non-W2-gated half of the old "10.2"); (ISC-2) a native `getrandom(buf,len)` syscall — **rigorously validate the userland buffer** (userland-mapped + writable + len-capped; no kernel-memory disclosure; fail-closed on unseeded DRBG — the security-critical ISC); (ISC-3) a thin `libneo` wrapper (what Arthur M2 will call). **KAT via neo-test fixed seed** (deterministic known sequence, like 10.1's selftest) + the buffer-validation truth table + non-triviality. Kill-switch default-off ⇒ `ENOSYS`, kernel bit-for-bit; new `unsafe` = the validated userland-buffer write (SAFETY-doc'd); no new invariant. **Independent of W2.** **Scope: the helper + syscall + wrapper ONLY — NOT the W2 SNonce swap (Cerebro/W2-gated, the remaining OT-2 piece), NOT the Arthur M2 encryption (the consumer), NO POSIX flag semantics (our ABI, `buf,len` minimal).** Gate: host KAT + the `sys_getrandom` QEMU scenario (call → bytes → distinct second call → bad-buffer rejected) at NEO_SMP=4 + **pre-merge QEMU gate** + clippy via Docker. This **unblocks Arthur memory M2**. **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + QEMU URLs.**

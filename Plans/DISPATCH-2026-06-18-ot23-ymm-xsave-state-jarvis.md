<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-23 — kernel YMM/AVX state preservation (FXSAVE → XSAVE) → Jarvis

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Jarvis (found this prerequisite in 9.3's ISC-3; owns the AVX2 lane). New branch off `origin/main` HEAD: `codex/ot23-ymm-xsave-state-jarvis`.
**Status:** DISPATCHED. **The prerequisite that unblocks in-kernel AVX2** (and any future in-kernel SIMD). Your 9.3 finding: the kernel preserves only x87/SSE (XMM) via `FXSAVE`/`FXRSTOR`, **not** the YMM high halves — so YMM used in a kernel code path would corrupt across a preemption/IRQ. This upgrades the FPU-state save/restore to **XSAVE/XRSTOR** so YMM survives. **Infrastructure only — does NOT wire the AVX2 op into the forward pass** (that's the next slice, after this lands).

## 0. The decision this build makes
Upgrade the kernel's FPU-state save/restore from **FXSAVE/FXRSTOR** (legacy: x87 + SSE/XMM, fixed 512 B) to **XSAVE/XRSTOR** (x87 + SSE + AVX/YMM, CPU-sized area), at the same boundaries fxsave/fxrstor are used today (context switch / IRQ / syscall — wherever Jarvis's 9.3 finding located them: `context.rs`, `syscall.rs`, `timer.rs`). After this, YMM is preserved — so a later slice can safely run 9.3's AVX2 op in the forward pass. **No AVX2 wiring here.**

## 1. Reference
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md     # OT-23 (this) + OT-7 (the 9.3 latency measurement it pairs with)
# the 9.3 finding's exact sites: kernel/src/arch/x86_64/{context.rs, syscall.rs, timer.rs}
```

## 2. ISCs
- **ISC-1 — XSAVE bring-up.** Enable `CR4.OSXSAVE`; program **XCR0** with the supported, intended state components (**x87 | SSE | AVX**) via `XSETBV`; size the per-context save area from **CPUID.(EAX=0x0D, ECX=0).EBX/ECX** (variable, CPU-dependent — do **not** hardcode 512 B); replace the `FXSAVE`/`FXRSTOR` calls with `XSAVE`/`XRSTOR` (use `XSAVEOPT` only if CPUID advertises it). All at the same save/restore boundaries the current code uses.
- **ISC-2 — CPUID-gated, fail-safe to legacy.** Use XSAVE/AVX **only** if CPUID supports it (`XSAVE` = CPUID.1.ECX[26], `OSXSAVE` enableable, `AVX` = CPUID.1.ECX[28]); **fall back to the current FXSAVE path** on a CPU without it. Never execute XSAVE-with-AVX unguarded. (QEMU TCG supports AVX/XSAVE, so the gate runs in CI.)
- **ISC-3 — The YMM-preservation proof (the whole point — a decisive test).** A test that **writes a known YMM pattern, forces a context switch / IRQ, and asserts the YMM register survived** byte-for-byte. Without this the build is unfalsifiable. Pair it with the regression: existing x87/SSE/XMM state is *also* preserved correctly across the switch (XSAVE saving x87+SSE is a superset of fxsave — prove no regression).
- **ISC-4 — Kill-switch + standards.** Behind a feature (e.g. `xsave-fpu`), **default-off ⇒ the current FXSAVE/FXRSTOR behavior, bit-for-bit** (the I-2 regression — the full family unchanged). Note the honest equivalence: **no in-kernel code uses YMM yet** (9.3's op is host-only), so feature-on is *also* observably equivalent to today *until* a later slice uses YMM — the proof is (a) the family passes with the feature on, and (b) the ISC-3 YMM-survives test passes. **New `unsafe`:** `XSAVE`/`XRSTOR`/`XGETBV`/`XSETBV` + the CR4/XCR0 writes (privileged) — each `/// SAFETY`-documented. No new invariant. **I-14:** FPU-state plumbing touches no AI-governance lock — state it. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking).

## 3. Gate
- **The YMM-preservation test (ISC-3) green** + the x87/SSE no-regression test; builds on+off the feature; the CPUID-fallback path exercised.
- **Kernel-touching (arch FPU paths) ⇒ pre-merge `workflow_dispatch` QEMU gate green (all 4 shards)** — the YMM-survives-a-context-switch scenario runs under QEMU (TCG emulates AVX/XSAVE) + the full family bit-identical with the feature off. `clippy --workspace --all-targets` via Docker.

## 4. Scope guard
The FPU-state FXSAVE→XSAVE upgrade + the YMM-preservation proof **only**. **Do NOT** wire the 9.3 AVX2 op into `forward_logic` (the next slice, after this lands — that's when YMM is actually *used* in-kernel), add new AVX/SIMD kernel code, change the AI ops, or touch governance. The point is to make YMM-in-kernel *safe to use*; *using* it is later. New `unsafe` limited to the documented XSAVE/XCR0/CR4 primitives.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-check set, SPDX, self-stat). The two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL + the `workflow_dispatch` QEMU URL — kernel-touching), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (the YMM-state prerequisite; OT-23 → RESOLVED-pending-the-in-kernel-AVX2-slice). Next: the **in-kernel AVX2 forward swap** (9.3's op into `forward_logic`, now YMM-safe) + the OT-7 latency measurement — both bundle with the hardware window where the realized speedup is finally measured.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-ot23-ymm-xsave-state-jarvis.md`. **Branch:** `codex/ot23-ymm-xsave-state-jarvis` off `origin/main`.
- **One-line:** Build **OT-23 — the kernel YMM/AVX state preservation you found blocked in 9.3.** Upgrade the FPU-state save/restore from **FXSAVE/FXRSTOR → XSAVE/XRSTOR** (x87|SSE|AVX) at the same boundaries (`context.rs`/`syscall.rs`/`timer.rs`): enable `CR4.OSXSAVE`, program `XCR0` via `XSETBV`, size the save area from CPUID.0xD (don't hardcode 512 B), use XSAVE/XRSTOR (XSAVEOPT if advertised). **CPUID-gated** with **fallback to the current FXSAVE path** on non-AVX CPUs. **The decisive test (ISC-3):** write a YMM pattern → force a context switch/IRQ → assert YMM survived byte-for-byte (+ the x87/SSE no-regression). Kill-switch `xsave-fpu` default-off ≡ current FXSAVE bit-for-bit; honest note: nothing in-kernel uses YMM yet, so on/off are both observably equivalent today — the proof is the family-on-green + the YMM-survives test. New `unsafe` = the XSAVE/XCR0/CR4 primitives, `/// SAFETY`-doc'd; no new invariant; I-14 untouched (state it). **Scope: the state-preservation infra + the YMM proof ONLY — do NOT wire the AVX2 op into the forward pass (that's the next slice, now unblocked).** Gate: the YMM test + no-regression + family-off bit-identical + **pre-merge QEMU gate** (TCG emulates AVX/XSAVE) + clippy via Docker. **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + QEMU URLs.**

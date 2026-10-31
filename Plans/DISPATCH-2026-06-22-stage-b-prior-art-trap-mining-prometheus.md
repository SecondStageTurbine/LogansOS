<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Stage-B prior-art trap-mining + bare-metal readiness (research THINK, NO code) → Prometheus

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Prometheus (research/design + prior-art-mining — its scoped lane; this is design only, NO code, no gate). New branch off `origin/main` HEAD: `prometheus/stage-b-prior-art-trap-mining`.
**Status:** DISPATCHED. **Pre-seed Stage-B's failure-mode bank from documented prior art, before the blind bare-metal run.** Stage-B (`DISPATCH-2026-06-22-stage-b-bundled-brain-bringup-jarvis.md`, gated on F3) boots the OptiPlex with the bundled real Qwen2.5 brain — a **blind hardware run** (serial/MMIO breadcrumbs only). The standing rule (driver/blind-hardware dispatches require a deep prior-art trap-mining pass) means Stage-B must enter its bring-up with the **known field gotchas already in the discriminator bank** — not discovered on a failing boot. This THINK does that mining now, while F3/F4 finish, so Stage-B is ready to fire conclusively the moment F3 lands. **Mirror your strong forward-path THINK (`cf50ffb`).**

## 0. The decision this build (eventually) makes
Make Stage-B's bare-metal bring-up **diagnostic on failure, not just green on success** — by enumerating, from real prior art, the documented traps for **(a) booting a from-scratch x86-64 kernel on the real OptiPlex via UEFI/Limine, (b) reading/holding a ~485 MB file off NeoFS over the real block device, (c) running the CPU inference forward on the OptiPlex's actual CPU** — each with the observation→cause discriminator Stage-B should pre-arm. This THINK's trap bank is consumed by Stage-B's §3 failure-mode section.

## 1. References (read before mining)
```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-06-22-stage-b-bundled-brain-bringup-jarvis.md  # the Stage-B build this feeds — §3 failure-modes + §3.5 (this pass)
git show origin/main:Plans/THINK-2026-06-21-forward-path-real-model-integration.md       # D4 residency/memory at real scale; D5 latency
git show origin/main:docs/ADR-063-arthur-boot-model-provisioning.md                       # the boot/provisioning decision
git show origin/main:boot/limine.conf                                                     # the boot config; how the kernel is loaded on real UEFI
git show origin/main:kernel/src/arch/x86_64/  (boot/UEFI/memory-map entry)                # the real-UEFI memory-map + arena placement path
git show origin/main:kernel/src/fs/neofs/  (the read path) + sys_model_load               # the ~485 MB sequential read over the real block device (L2 indirect walk)
```
**Confirm the exact OptiPlex CPU + RAM with the PL** (model/CPU drives AVX2 support — relevant to F4 — core count — relevant to OT-34 threading — and the RAM budget for the ~700 MB 0.5B / ~1.7 GB 1.5B resident). Don't guess the model; state the assumption + flag it for PL confirmation.

## 2. What to mine (raw-source the prior art — OSDev, Limine docs, real-hardware boot reports, kernel mailing lists; never a summarizing fetch for exact gotchas)
- **Boot / firmware (real UEFI vs OVMF).** The memory-map differences that bite a from-scratch kernel on real hardware vs QEMU/OVMF: usable-region fragmentation, runtime-services regions, ACPI reclaim, the framebuffer GOP mode, where a ~700 MB resident arena can actually be placed. Limine-specific real-hardware gotchas (HHDM, the memory map it hands off, KASLR interaction). Each ⇒ which serial breadcrumb localizes it.
- **Storage at scale.** The real block device (SATA/NVMe on the OptiPlex) vs QEMU virtio-blk: sector/alignment/DMA constraints for a **485 MB sequential read**, the L2 indirect-block walk over real media, timeout/retry behavior, and whether the read path assumes virtio semantics. ⇒ the `model_load_resident` breadcrumb's expected progress shape.
- **ACPI / SMP bring-up on the specific OptiPlex.** Real-hardware ACPI table quirks, AP startup, the IO-APIC/timer differences from QEMU that could stall boot before Arthur even starts. (OT-29's slab fix made heavy spawn→reap→spawn sound — note that the first *bare-metal* heavy-Arthur spawn is still unproven.)
- **CPU inference on real silicon.** AVX2 availability on the actual CPU (the F4 hot path) + the honest per-token latency expectation on this CPU vs the dev host's 1563 ms/token (Stage-A) — roofline/memory-bandwidth reasoning for a batch-1 decode; and the core count for the banked OT-34 threading lever. ⇒ the `forward token=0`/`per_token_ms` breadcrumb's expected magnitude (so a 10× surprise is flagged, not accepted).
- **The easier-path question** (the standing pairing): is there any boot/provisioning choice that *deletes* a Stage-B failure mode rather than just de-risking it? (e.g., a smaller first-boot model, a staged-vs-bundled tradeoff.)

## 3. Deliverable
A design THINK (`Plans/THINK-2026-06-…-stage-b-bare-metal-trap-mining.md`): a **pre-seeded trap bank** — each documented trap with (the field source, the failure signature, the serial-breadcrumb discriminator that localizes it, and the pre-emptive mitigation if any), organized to drop straight into Stage-B's §3 failure-mode section. Plus the **OptiPlex CPU/RAM assumptions** (flagged for PL confirmation) feeding D4 (residency budget) / D5 (real-CPU latency) / F4 (AVX2) / OT-34 (cores). An honest-uncertainties table. **No code, no `ci.yml`, no kernel/boot change** — research + the written bank only.

## 4. Scope guard
Prior-art trap-mining + the readiness THINK **only**. **Do NOT** write any Stage-B code, change boot/kernel/NeoFS, or build the ISO (that's the Jarvis Stage-B build, post-F3). Design/research only.

## 5. Close (PM)
**Design THINK — no CI gate** (host/research, your lane). On your branch + a RETURN summarizing the trap bank. **PM reviews** (esp. that the discriminators are concrete + the latency/AVX2 assumptions are honest, not over-claimed) → folds the bank into the Stage-B dispatch's §3 + lands the THINK on `main`. No QEMU, no merge-gate.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-stage-b-prior-art-trap-mining-prometheus.md` (+ the Stage-B dispatch, the forward-path THINK D4/D5, ADR-063, `boot/limine.conf`, the NeoFS read path). **Branch:** `prometheus/stage-b-prior-art-trap-mining`.
- **One-line:** **Research THINK (NO code, your lane) — pre-seed Stage-B's blind-bare-metal failure-mode bank from prior art**, so the OptiPlex bring-up is diagnostic-on-failure before it's run. Mine the documented field gotchas (raw-source OSDev/Limine/real-hardware reports — never a summarizing fetch) for: **real-UEFI vs OVMF memory-map** differences (where a ~700 MB arena can be placed; Limine HHDM/handoff quirks); the **real block device** reading a **485 MB** file (sector/DMA/alignment vs virtio; the L2 indirect walk on real media); **ACPI/SMP** bring-up quirks on the OptiPlex (+ the first *bare-metal* heavy-Arthur spawn, post-OT-29); and **CPU inference on real silicon** (AVX2 for F4, core count for OT-34, the honest per-token latency vs the dev host's 1563 ms/token). Each trap ⇒ its serial-breadcrumb discriminator (drop-in for Stage-B §3). **Confirm the exact OptiPlex CPU/RAM with the PL** (drives AVX2/cores/residency) — don't guess, flag it. Also ask the easier-path question (any boot choice that *deletes* a failure mode). **Deliverable:** `Plans/THINK-…-stage-b-bare-metal-trap-mining.md` with the trap bank + assumptions + uncertainties. **No code/ci/kernel/boot change.** PM reviews + folds into the Stage-B dispatch + lands the THINK; no gate.

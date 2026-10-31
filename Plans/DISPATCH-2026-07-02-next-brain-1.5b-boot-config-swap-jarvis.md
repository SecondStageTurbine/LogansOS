<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Next-brain 1.5B boot config-swap — the userland selector + QEMU load-smoke → Jarvis

**Date:** 2026-07-02
**From:** PM (Claude)
**To:** Jarvis (main-tier). New branch off `origin/main` HEAD (`db61c7f`): `codex/next-brain-1.5b-boot-jarvis`.
**Status:** DISPATCHED — the **named follow-on** from Oracle's 1.5B build (`5316abd`). That build proved the 1.5B Qwen loads (heap budget → 1.75 GiB), parities (teacher-forced strict=23/near_tie=1/gross=0), and fits (1.035 GiB .neom < NeoFS 2 GiB) — but left the **bootable-ISO wiring** as the follow-on: "needs the userland `arthur-real-brain-boot-15b` selector + an xtask scenario." This build lands that, making the 1.5B a true config-swap. **Non-conflicting** with Cerebro's WiFi surface and Oracle's CB-5a (this is the Arthur boot path + a QEMU scenario, not mt7601u/wpa; not the forward math, which is frozen/data-driven). **Kernel/userland-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. What this build is
Oracle made the forward + memory data-driven and host-proved the 1.5B; the ONE thing missing to boot a 1.5B ISO is the **selector wiring**: the boot path must load the 1.5B `.neom` (via `NEO_STAGE_B_NEOM`) under its feature, and a QEMU scenario must prove the boot+load path end-to-end. **The full 1.5B forward cannot run inside the 3600s QEMU-TCG gate** (no SIMD JIT/KVM — the same reason the 485 MB 0.5B forward is a local/bare-metal proof, not an in-gate one). So the QEMU deliverable is a **boot/load SMOKE** (model opens from NeoFS, config parses from the v2 header, load-resident fits the 1.75 GiB heap, breadcrumbs to the pre-forward line), with the **coherent-reply forward deferred to the bare-metal 1.5B run** (a later follow-on, mirroring the 0.5B gold-standard split).

## 1. References
```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-06-29-next-brain-1.5b-scaleup-oracle.md   # the parent build — ISCs, kill-switch, the -15b feature
git show origin/main:docs/next-brain-1.5b-provisioning.md                          # Oracle's import recipe + memory budget + NeoFS confirm
git show origin/main:kernel/src/mm/heap.rs                                         # HEAP_SIZE via heap_policy(cfg!(real-brain-boot), cfg!(real-brain-boot-15b))
git show origin/main:Plans/DISPATCH-2026-06-22-stage-b-bundled-brain-bringup-jarvis.md   # the 0.5B Stage-B boot pattern this mirrors (selector + breadcrumb ladder + blind-hw discriminators)
git show origin/main:userland/neo-arthur/src/main.rs                               # the boot: model load path the selector feeds
git show origin/main:tools/xtask/src/main.rs                                       # the neo-test scenario harness + the real-brain ISO staging (NEO_STAGE_B_NEOM)
```

## 2. ISCs
- **ISC-1 — The `arthur-real-brain-boot-15b` selector.** Wire Arthur's boot path so, under the `-15b` feature, it loads the 1.5B `.neom` (config-driven from the v2 header — F2, no forward-math change) instead of the 0.5B. Reuse the existing single presized `byte_len` load (no double-buffer) into the 1.75 GiB heap Oracle sized. The selector is the *only* new behavior; the forward/`.neom`/vocab are frozen.
- **ISC-2 — The `arthur_real_brain_boot_15b` xtask QEMU scenario (load-smoke).** A scenario that stages the 1.5B `.neom` (`NEO_STAGE_B_NEOM`, no blob in git), boots, and asserts the **pre-forward** breadcrumbs: `neofs_mount` → `model_open` (1.035 GiB served from NeoFS) → `config` (28 layers / head_dim 128 / rope 1e6 parsed from the header) → `model_load_resident` (bytes-resident fits the 1.75 GiB heap). PASS = the load ladder completes + `init_continues=true`; the forward itself is **out of scope in-gate** (TCG can't finish it — assert the boot reaches the pre-forward line, don't wait on a token). Gated on `NEO_REAL_NEOM` (CI has no model → skips), exactly like the 0.5B `arthur_real_brain_boot`.
- **ISC-3 — Blind-hardware discriminator seed for the (later) bare-metal 1.5B run.** Mirror the Stage-B/gold-standard ladder so the eventual bare-metal 1.5B boot is self-diagnosing: capture bytes-resident-vs-heap-cap at load, the indirect-block levels on the NeoFS read of a >1 GiB file, and a fault *address* (not just type) on any #GP/#PF. Capture-before-you-need-it — you don't get to discover a missing capture on the bare-metal run. (No bare-metal run in THIS build — just seed the captures.)
- **ISC-4 — Kill-switch + docs.** `-15b` OFF ≡ the 0.5B boot bit-for-bit (Oracle's heap const already folds identically; the selector is behind the feature — assert the default Arthur boot path is unchanged). Update `docs/next-brain-1.5b-provisioning.md` with the boot recipe (how to build + run a 1.5B ISO).

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** `-15b` OFF ≡ 0.5B boot, bit-for-bit (the equality is the artifact; ISC-4).
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant** (boot wiring + a scenario, not a cross-cutting property). I-1 result-invariance is untouched (forward frozen).
- **Byte-oracle:** the `.neom` v2 header + per-quant paths already carry oracles (F1/F2/Oracle's 1.5B build); no new byte format here — the scenario asserts the *load ladder*, not new bytes.
- **I-14:** N/A by construction (Arthur boot + NeoFS read + a QEMU scenario; no driver/ISR/DMA, no AI-governance lock).
- **Blind-hardware note:** the bare-metal 1.5B boot is a *later* follow-on; this build only seeds its discriminators (ISC-3). The gate here is QEMU load-smoke + host.

## 4. Gate + close
- **Kernel/userland-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch + AAG** (the new scenario + the boot-selector path). Push-CI green (host + the kill-switch equality). **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main` — mind Oracle's CB-5a may land in parallel; re-run the gate after rebase; host set; SPDX; self-stat).
- **RETURN** the QEMU shard URLs + SHA, the load-smoke breadcrumb transcript (or its CI-skip note if `NEO_REAL_NEOM` unset on the runner), the kill-switch equality proof, and confirmation the 0.5B boot path is unchanged. **Disk reclaim** after merge.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-02-next-brain-1.5b-boot-config-swap-jarvis.md` (+ Oracle's 1.5B dispatch + provisioning doc, the 0.5B Stage-B pattern, `mm/heap.rs`). **Branch:** `codex/next-brain-1.5b-boot-jarvis` off `origin/main` HEAD (`db61c7f`).
- **One-line:** **Make the 1.5B a bootable config-swap** — Oracle proved it loads/parities/fits; wire the `arthur-real-brain-boot-15b` boot **selector** (load the 1.5B `.neom` under the feature, reuse the presized `byte_len` load, forward frozen) + an `arthur_real_brain_boot_15b` xtask **load-smoke** QEMU scenario asserting the pre-forward ladder (`neofs_mount → model_open → config → model_load_resident → init_continues`). **The full 1.5B forward can't run in TCG** (like the 485 MB) — assert the boot reaches the pre-forward line; the coherent-reply forward is the later **bare-metal** proof (seed its discriminators, ISC-3, don't run it here). **Kill-switch** `-15b` OFF ≡ 0.5B boot bit-for-bit. **No forward-math/`.neom`/vocab change, no connectivity.** **Gate:** 4-shard QEMU + AAG; rebase before close (Oracle's CB-5a may land in parallel). RETURN shard URLs + SHA + the load ladder + kill-switch equality.

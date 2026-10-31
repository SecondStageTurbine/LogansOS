<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (RESEARCH/DESIGN THINK): Phase-7.5 closeout — the SMP AP-bringup hang + the 7.5-D identity/interface-wiring design → Oracle

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Oracle (free main-tier; fresh off OT-32's per-CPU slab/SMP context — the right brain for the concurrency half). **Research/design only — produce a THINK, NO code, NO QEMU.** New branch off `origin/main` HEAD: `codex/phase-7.5-closeout-smp-and-identity-oracle`.
**Status:** DISPATCHED. One of **four parallel closeout-research lanes**; the PM synthesizes all four THINKs into the Phase-7.5-closeout PLAN. **Host-only design THINK — must not open a QEMU gate** (Stage-B owns the runners).

## Why this lane
Two of the closeout's open items are *not* MT7601U-chip-internal (those are Cerebro/Prometheus): one is a kernel-concurrency knot, one is the lane's capstone design. Both fit a main-tier seat with SMP + capability fluency.

### Charter A — the intermittent SMP AP-bringup hang (OT-27(c))
The W1/W2 lane intermittently hangs during **SMP AP (application-processor) bring-up**. Characterize it as a design THINK:
- **Where in AP bring-up** the hang sits (the IPI/INIT-SIPI-SIPI sequence, the per-CPU init, the first scheduler entry on the AP) and **what the WiFi driver shares with that path** that could deadlock/livelock it (a lock taken on both the AP-init path and the driver's IRQ/MCU-poll path; the I-14(a) lock-isolation invariant says the driver IRQ path takes **no** governance lock — verify it takes none on the AP-bringup path either).
- A **ranked hypothesis + discriminator bank** (the blind-hardware standing rule): for each candidate (lock-order inversion, an IPI lost under the bulk-poll spin, a per-CPU structure raced), name the serial breadcrumb / lock-state capture that confirms-or-refutes it. Note the OT-31 `exit()`/reap lock-release window as a *separate* unconfirmed SMP item — say whether it's related or orthogonal.
- This is **characterization + a repro/discriminator plan**, not a fix. If a capture must be added before a confirming run, name it.

### Charter B — the 7.5-D identity / interface-wiring capstone design
The lane order ends `…→C2(WPA2/3 control plane)→D(identity wiring)`. **7.5-D is the phase capstone and is undesigned.** Produce the design THINK:
- **What "identity wiring" is** at this layer: the interface/identity surface that unifies the connectivity backends (USB-Ethernet `eth0`-class + WiFi `wlan0`) under one neutral interface-identity model the kernel connection-management surface (bible Art. 13) and userland can name — *not* a second control plane per device.
- **The cap/identity shape**: how an interface identity is named, who holds the cap to it, how it composes with the existing `NetDevice`/socket layer and the I-15 file-cap model — reuse the frozen Phase-2 primitives, do not invent a new one.
- The **kill-switch + invariant budget** check: 7.5-D must ship disabled ≡ today bit-for-bit, and should need **no new cross-cutting invariant** (if it genuinely does, name it + justify it per the invariant-budget rule). Apply the I-14 fault-containment analysis (a connectivity/identity layer dispatch requires it): which of the three vectors (lock isolation / liveness / memory) it touches and the guard.

## References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                                # OT-27(c) the SMP hang; OT-31 the exit/reap window
git show origin/main:docs/complexity-governance.md                       # I-14 (a/b/c) fault-containment; the lock census + lock-order ceiling (OT-13)
git show origin/main:docs/bible.md                                       # Article 13 lane order (…→D identity wiring) + the kernel connection-management surface intent
git show origin/main:Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md   # the lane plan that defined 0→A→B→V→C1→C2→D
git show origin/main:Plans/THINK-2026-06-20-w2-mt7601u-discriminator-bank-and-fault-containment.md   # the I-14 analysis pattern + the SMP AP-bringup note
# kernel/src/sched/ (AP bring-up / per-CPU init / smp), kernel/src/drivers/net/ (NetDevice, wlan0/eth0), kernel/src/cap/ (the Handle model 7.5-D rides)
```

## Deliverable
A THINK on your branch: `Plans/THINK-2026-06-23-phase-7.5-closeout-smp-and-identity-oracle.md` (SPDX `CC-BY-4.0`) — Charter A (the SMP-hang characterization + ranked hypothesis/discriminator bank + repro plan) and Charter B (the 7.5-D identity-wiring design: the identity/cap shape, composition with NetDevice/I-15, kill-switch + invariant-budget + I-14 analysis). RETURN the path.

## Scope guard
**Research/design only.** No code, no QEMU, no gate, no kernel change. Compose the frozen Phase-2 cap primitives + the existing NetDevice/I-15 layers — do not modify them in this THINK. If Charter B needs a new invariant, **name and justify it** (don't smuggle it). If Charter A needs a pre-run capture, name it as a build requirement.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-closeout-smp-and-identity-oracle.md` (+ references). **Branch:** `codex/phase-7.5-closeout-smp-and-identity-oracle` off `origin/main`.
- **One-line:** **Research/design THINK, no code/QEMU** — one of four Phase-7.5-closeout brains. **(A)** Characterize the **intermittent SMP AP-bringup hang (OT-27(c))**: where in INIT-SIPI-SIPI/per-CPU-init/first-AP-schedule it sits, what the WiFi driver shares with that path (verify the IRQ/MCU-poll path takes no lock that AP-bringup holds — I-14(a)), a ranked hypothesis+discriminator bank, and whether OT-31's `exit()`/reap window is related or orthogonal — characterization + repro plan, not a fix. **(B)** Design the **7.5-D identity/interface-wiring capstone**: the neutral interface-identity model unifying `eth0`/`wlan0` under the kernel connection-management surface, the cap/identity shape composing with NetDevice + I-15 (reuse frozen Phase-2 prims), with the kill-switch (off ≡ today bit-for-bit), invariant-budget, and I-14 fault-containment analysis. **Deliver `Plans/THINK-2026-06-23-phase-7.5-closeout-smp-and-identity-oracle.md`; RETURN the path.** Host-only — no QEMU gate.

<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-61 browser C1 — the host-runtime primitives (the depth bottleneck) → Cerebro

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Cerebro (the C0/C0.5 author + browser-C-ladder owner)
**Status:** DISPATCHED — **kernel-touching program, QEMU-gated (NO hardware).** ADR-078 §3 names **C1 the depth bottleneck**: the real host-runtime primitives a hosted Servo/SpiderMonkey needs. C0 built the PAL skeleton (loud stubs), C0.5 ran a real-`std` ELF; C1 fills in the primitives those stubs point at. This is the single biggest real lane on the 1.0 long pole and needs no OptiPlex. **Design-first, then slice — do NOT try to land it all in one build.**

## Scope — the C1 primitive set (ADR-078 §3)
User **threads** (create/join/detach, named, stack sizing + **bounds/guard page**), **TLS** (per-thread + destructors), **sync** (mutex + condvar **wait** — kernel-backed, lost-wakeup-free), a **monotonic clock**, and **VM map/unmap/PROTECT with W^X enforcement**. Each is a new cap-confined user-facing primitive (a syscall + libneo shim + the PAL surface it satisfies), replacing the C0 loud stub.

## The two hard constraints (these shape the ABI — get them right before freezing)
- **A3 — co-design for BOTH consumers.** Every primitive's ABI must serve **`std::sys`** (Rust's platform layer) AND **SpiderMonkey's native pthread/TLS port** — enumerate both consumers' exact expectations (from the real upstream source, by commit) for each primitive BEFORE freezing its shape. A thread/TLS ABI that fits `std` but not pthread forces a C1 redo. This enumeration is the first deliverable.
- **A1 — W^X is enforced here, JIT-relaxation is the FUTURE budgeted rung.** VM-PROTECT must **enforce W^X**: a page is R+W or R+X, **never W+X** (the W^X negative battery proves a W+X map is refused). JIT (which needs W^X relaxation) is the separate, later, **kill-switchable invariant-budget rung** (ADR-078 §4) — do NOT introduce any W^X-violating path in C1.
- **I-14 / kill-switch / no new cross-cutting invariant:** the primitives are cap-confined; a fault in a browser thread degrades within its process, never stalls governance; default build bit-for-bit unchanged (all behind the browser target/feature). If a primitive needs a NEW invariant, name + justify + guard it (the invariant-budget rule).

## Deliverable — design doc + the FIRST slice
1. **The C1 primitive-ABI design** (a THINK/FINDINGS): per-primitive, the `std::sys` need + the SpiderMonkey pthread need + the unified cap-confined ABI (A3), the W^X enforcement model (A1), and the **slice ladder** (propose C1.0 / C1.1 / … — e.g. threads+TLS first, or VM-protect+W^X first — your call, justified).
2. **Build C1.0 — the first slice** you scoped, with its close-gate battery in QEMU (per ADR-078: contention / timeout / TLS-destructor / guard-page / **W^X negative** batteries, as applicable to the slice). A real-`std` (or minimal pthread-shape) smoke exercises it on-target.

## Close-gate + merge
- `scripts/worker-close-gate.sh` (rebase-fresh on current `main`), green 4-shard `workflow_dispatch` QEMU URL, PM AAG (concurrency/lost-wakeup on the sync-wait, mem-safety on the thread/TLS/stack bootstrap, W^X-enforced [no W+X map], kill-switch default-build-unchanged, no-new-invariant-or-justified). Branch `codex/ot61-c1-host-runtime-cerebro`; PM merges after gate + AAG. `Workers/cerebro` persistent.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-ot61-c1-host-runtime-primitives-cerebro.md` + `…:docs/ADR-078-ot61-browser-c-ladder.md` (§3 C1 + §4 reserved rungs) + your C0/C0.5 FINDINGS + the PAL manifest `…:tools/ot61-pal-manifest.toml`. Work in `Workers\cerebro`; branch `codex/ot61-c1-host-runtime-cerebro`. **Start after C0.5 merges (imminent).**
- **One-line:** **Open C1 — the browser depth bottleneck (host-runtime primitives), QEMU-gated, no hardware.** Threads (create/join/detach + stack bounds/guard), TLS (+destructors), sync (mutex/condvar wait, lost-wakeup-free), monotonic clock, VM map/unmap/**PROTECT with W^X enforced** — each replacing a C0 loud stub. **Design-FIRST (A3): enumerate BOTH `std::sys` and SpiderMonkey pthread's exact needs per primitive from real upstream source before freezing the ABI** (a std-only fit forces a redo). **W^X is enforced here (no W+X map); JIT-relaxation is the future kill-switchable budgeted rung (A1) — don't introduce it.** Deliver the per-primitive ABI design + the slice ladder, then build C1.0 (the first slice you scope) with its QEMU battery (contention/timeout/TLS-destructor/guard-page/W^X-negative). Kill-switch: default build bit-for-bit unchanged. Close-gate + green 4-shard QEMU URL + PM AAG.
- **RAG** (PowerShell): `libneo thread TLS sync syscall`, `mm vm map protect W^X NX guard page`, `sched thread create join`, `neo-std-pal stub thread_local`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read.

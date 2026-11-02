<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-078: The OT-61 browser C-ladder — a bounded native OS port for confined Servo + SpiderMonkey

**Status:** ACCEPTED (architecture ratified RATIFY-AFTER-AMENDMENT; C0 BUILT). Authoritative for the browser bring-up ladder.
**Date:** 2026-07-22
**Owner:** PM (Claude) — records the ratified direction; built by the OT-61 lane.
**Supersedes/relates:** OT-61 (the ship-gating browser program), OT-63 (the GPU/accel path — the hardware-GL provider is a separate program, see §4), OT-44 (cap-bundle/WASM sandbox — the bundle boundary), OT-52 (the compositor + `scanout_authorized()`), ADR-028 (byte-oracle discipline), ADR-076 (scanout DMA fence), `docs/complexity-governance.md` (the invariant budget).
**Grounded in (raw-source-verified, pinned commits):** `docs/RESEARCH-2026-07-12-web-browser-FINDINGS.md` (Janus), `docs/RESEARCH-2026-07-20-ot61-browser-{architecture,crosscheck}-FINDINGS.md` (Prometheus + Chad), `docs/RESEARCH-2026-07-22-ot61-{servo-host-closure,swgl-browser-floor}-spike-FINDINGS.md` (Janus), `docs/RESEARCH-2026-07-22-ot61-browser-closure-crosscheck-FINDINGS.md` (Cerebro, cross-family), `docs/RESEARCH-2026-07-22-ot61-c0-libstd-pal-FINDINGS.md` (Cerebro, C0).

## 1. Context

The web browser is a **ship-gating** requirement (OT-61, PL-ratified): first-ship/1.0 is held until a capability-confined browser passes the modern-site battery. The engine is **confined Servo-embed + bounded SpiderMonkey** (a bounded component behind a narrow authority boundary — no C++/POSIX personality at the kernel/user ABI). Two prerequisite spikes + a **cross-family** review (a Claude lens over a Codex-authored, Codex-corroborated direction) established *how* to host it and *how* it renders. This ADR records the resulting ladder.

## 2. The two load-bearing decisions

### 2.1 Hosting — a **bounded native OS port**, not a `no_std` strip, not a POSIX ABI
The useful Servo layout/paint/DOM closure is intrinsically **hosted and concurrent** (Servo spawns constellation/script/paint/font/net threads; SpiderMonkey needs a native pthread/TLS port). Therefore:
- **REFUTED:** a four-crate `no_std` extraction (rewriting DOM/layout/style/GC to `no_std` is a second engine).
- **REFUTED:** a general POSIX personality at the ABI.
- **ACCEPTED:** a supported **`libstd` platform layer (PAL)** + a **private static C++/libc++ subset**, both **behind the browser bundle boundary** (OT-44). The kernel/user ABI stays libneo capabilities; neither web content nor general LogansOS programs receive a POSIX personality. **Fork only the narrow capability-sensitive provider seams** (net/fs/entropy/process); NEVER html5ever/Stylo/layout/DOM/WebRender/GC semantics — prefer upstreamable target-cfg patches, re-audited on every Servo/SpiderMonkey ESR bump.
- **The anti-compat-island guard (A4):** the PAL's syscall surface is a **build-breaking manifest** (`tools/ot61-pal-manifest.toml`). Adding a host-call outside it FAILS the build; a new syscall-shaped compat entry requires its capability provenance + a negative gate in the same change. This structurally bounds the "growing Linux-compat island" risk.

### 2.2 Rendering — SWGL into a confined `SharedPixelBuffer`, no scanout authority
`Servo/WebRender → SWGL software-GL → confined Atrium `SharedPixelBuffer` → compositor`. SWGL satisfies WebRender's real `gleam::gl::Gl` contract in software over a **caller-owned framebuffer** and writes **only virtual pixels** — it never touches the DMA arena, `GpuSubmitCap`, MMIO, physical addresses, or scanout. A browser client direct-present still hits `scanout_authorized()`→EPERM (OT-52 preserved by construction). **Hardware GL is a later drop-in behind the same `AtriumSwglRenderingContext` provider seam** — so "browser renders correctly" is **decoupled from "GPU driver done."**

## 3. The C-ladder (falsifiable, per-rung go/no-go)

| Rung | Builds | Close gate | Status |
|---|---|---|---|
| **C0** | First-class Rust target + `libstd` PAL **skeleton** (primitive tier: print/heap/exit → raw libneo syscalls; every C1+ surface a **deterministic loud stub**); the A5 host de-risks (PAL surface manifest + SWGL↔Atrium byte oracle); the A4 build-breaking manifest gate. | `std-pal-smoke` builds on-target (static ELF, raw-syscall-verified); unsupported APIs fail deterministically; A4 gate falsifiable. | ✅ **BUILT** (`617a36b`) |
| **C0.5** | The bounded `rust-src` `sys/*` **overlay** so a **real `std`-linked** program compiles/links/runs: route most `sys/*`→the PAL unsupported stubs + custom stdio/alloc/exit modules + an io-errno table + a panic (abort) arm. | A real-`std`-linked smoke runs on-target **in QEMU** (prints/allocates/exits); A4 manifest still holds; adds no syscall. | **NEXT** |
| **C1** | User threads/TLS, mutex+condvar wait, monotonic clock, VM map/unmap/**protect**, stack bounds — **co-designed to serve BOTH `std` AND SpiderMonkey's pthread port** (A3). | Contention + timeout + TLS-destructor + guard-page + **W^X** negative batteries pass in QEMU. **C1 is the depth bottleneck.** | future |
| **C1.3** | Wire `std` to the C1 primitives: std's sync (`Once`/`Mutex`/`Condvar`/`RwLock`) to the C1.1 futex layer, `std::thread` to `SYS_THREAD_CREATE` and the TLS-free bootstrap, and std's `thread_local` key route to the C1.2 per-thread key block (`libneo::ntls`) instead of C1.2's process-global table. Split out of C1.2 by the PM (2026-10-27). | A real-`std`-linked program spawns, joins and synchronizes threads in QEMU, with per-thread `thread_local!` values and their destructors at thread exit; the key-only TLS gate still holds. | **built on `claude/ot61-c1.3-std-threads`, gate pending.** `std_threads` (QEMU, `NEO_SMP=4`) passes all 20 checks with the capability and refuses spawn (`PermissionDenied`) without it. `Instant` is on the monotonic clock; `SystemTime` stays unsupported, because `CLOCK_REALTIME` is wall time only on `rtc-epoch` kernels. One key layer is shared by std and libneo. |
| **C2** | Sealed file/resource handles, entropy, process identity/**crash containment**; the browser gets **no dial capability**; the **per-content-process memory quota** (A2). | `servo-host-smoke` passes incl. all ambient-authority refusals; a content OOM/crash kills **only** that process. | future |
| **C3** | Static C++ runtime + **SpiderMonkey** (interpreter/no-JIT first). | One realm evaluates JS, GC compacts, helper threads join, OOM/crash kills only the content process. | future |

Downstream of C0–C2: the **SWGL browser floor** (§2.2) is a bounded rung; the modern-site correctness battery begins on it (software render), before hardware GL exists.

## 4. Reserved invariant-budget rungs (NAMED now, per the invariant budget)

Two future rungs introduce **new cross-cutting invariants**; they are reserved here so they are neither forgotten nor introduced by surprise, and so C0/C0.5 do not violate them:

- **A1 — W^X ⊗ JIT.** LogansOS user memory is R+W+NX; there is **no runtime-executable-code primitive**. A usable browser eventually needs SpiderMonkey's JIT, which is the **first-ever W^X-violating mechanism** — a security-invariant change that MUST be named, justified, guarded, and **kill-switchable** (interpreter-only is the fail-safe) in the build that adds it. **C0/C0.5 introduce no executable memory** (target + heap are NX). The no-JIT floor (C3) is honest for correctness; JIT is a distinct, later, budgeted rung.
- **A2 — per-content-process memory quota.** Resource containment ≠ crash containment: there is no per-process frame ceiling today (`sys_map_region` caps only per-call), so an adversarial page can **DoS the whole system before its process dies**. "OOM kills only that process" is **not deliverable** without a per-content-process memory quota — a C1/C2 kernel-VM rung.

## 5. Consequences

- The browser long pole is now a **de-risked, gated ladder**, not a hand-wave. The two biggest unknowns are retired: the PAL surface is **bounded** (C0's manifest, grounded in a real `build-std` probe — not larger than modeled), and the render seam **holds within confinement** (C0's byte oracle + SWGL's caller-owned framebuffer).
- The hardware-GL provider (behind the `AtriumSwglRenderingContext` seam) is a **separate OT-63/Vello-scale program OFF the browser critical path** — the browser ships correct on the software floor first.
- Every rung is falsifiable with its own gate; failure at C1 means the Servo track is not ready (it does **not** justify a general POSIX layer); failure at C3 means reassess the SpiderMonkey port or a bounded WASI-derived path, not the DOM/layout closure.
- **No-human-time:** rung effort is dependency-depth (the DAG: C0→C0.5→C1→C2→C3, C1 the bottleneck) + iteration cycles + per-rung gates — never calendar.

## 6. Kill-switch / ledger

The whole ladder is **additive and opt-in** (a new target + browser-bundle crates); the default kernel/toolchain build stays bit-for-bit unchanged, and no web-facing surface reaches the kernel/user ABI (libneo capabilities only). C0 added **no** new cross-cutting invariant (I-1…I-16 unchanged); A1/A2 are the two future ones, budgeted above. The A4 manifest gate is the standing structural guard that keeps this true as the ladder climbs.

## Amendment, 2026-11-02 (J.c.): a type-only layout seam for the first static page

`servo-layout` 0.7.0 hard-depends on `servo-script` (and SpiderMonkey) for its DOM wrapper types. To draw a static
page before C3, J.c. chose option A of `docs/THINK-2026-11-01-ot61-first-page-layout.md`: `servo-layout`'s
`script::layout_dom` imports go through a crate-local alias (or a generic over the DOM bundle). §2.1's "never fork
layout" still holds for layout *semantics*; this seam is allowed only under the three conditions in
`Plans/DECISION-2026-11-02-ot61-first-page-option-a.md` (types only, byte-identical elsewhere; converges back after
C3 and is offered upstream; cross-family review of the patch line by line).


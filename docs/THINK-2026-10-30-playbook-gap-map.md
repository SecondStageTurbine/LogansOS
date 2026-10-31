<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the Combined Playbook against LogansOS: what's there, what isn't, how to get there

**Date:** 2026-10-30 · **Author:** PM · **Basis:** an inventory of `origin/main` @ `46fa59fd` (repo survey; the
key claims were PM-spot-checked). "Image A" is the product image (`PRODUCT_IMAGE_KERNEL_FEATURES`,
`tools/xtask/src/main.rs`). Scope is costed in dependencies, cycles and gates, never calendar time (CLAUDE.md).

## The headline

LogansOS already has an unusual amount of the **architecture** these seven principles ask for: a capability
kernel, on-device inference, a local-first filesystem, a kill-switch on every feature, an agent with grants,
confirmations and undo, and memory with verifiable forgetting. Most of the gap is that **image A ships little of it.**
The agent's actions, grants and memory, the capability-rooted open, the network capabilities and the quotas are all
built and scenario-tested, but switched off in the product. So the cheapest, highest-leverage first step is
**integration, not invention**: turn on what's built, behind the UI that makes it legible. After that, three genuinely
new subsystems decide whether this is a platform: **updates, apps, and a compatibility room.**

## Principle by principle

### 1. Integrate the core, open the edges

| Have | Partial | Missing |
|---|---|---|
| Kernel → WM (neo-init is PID 1 and the WM) → Arthur → login/SystemStore, all in image A | SDK v0 (`docs/SDK-ABI.md`, 49 of 111 syscalls; last touched 07-18); app model OT-88 designed, with A1a (#220) unmerged; the installer is VM-only (R5 unbuilt) | **OS update and rollback** (OT-51 is a THINK only); **launching an app from disk** (spawn is Limine modules only); a package format; a third-party driver model; ambient `sys_spawn` (K2 SpawnCap absent) |

### 2. Make intent the interface

| Have | Partial | Missing |
|---|---|---|
| Agentic arc A1–A3 (13 action kinds, kernel-constrained decode), grants (`grants.json`), stakes and typed confirmation, copy-before-write undo: **all scenario-only** | Image A's Arthur is chat-only, plus "list /" | **Showing its work:** an action/audit log and a "what I did and why" view; acting across apps (needs the app platform) |

### 3. Privacy as architecture

| Have | Partial | Missing |
|---|---|---|
| On-device model in image A; local NeoFS; no telemetry by charter; encrypted memory with verifiable forgetting (scenario builds) | `net-dial-cap`/`interface-caps` exist but **egress is ambient in image A**; `cap-rooted-open` is off, so the root open is ambient; model content isn't verified (OT-73) | A **permission inspector** (see and revoke grants and caps); **memory export**; full-disk encryption and Secure Boot (deferred); explicit, user-initiated sync |

### 4. Beauty with an escape hatch

| Have | Partial | Missing |
|---|---|---|
| Atrium Glass, Inter/Plex type, motion, app icons; Reduce Transparency/Motion | Settings store #222 (gating now); floating windows with snap (designed); `neo-shell` is a console REPL, not a desktop app | **Terminal app** (PTY + VT100, OT-57); a **developer mode** that opens the hood; a text editor (core crate only); a file manager beyond the Lens |

### 5. Compatibility without surrender

| Have | Partial | Missing |
|---|---|---|
| Real Rust `std` apps on the overlay (threads, TLS, entropy, sealed fs); VM-map primitives with W^X; per-process memory and object budgets; a faulting process dies alone | All of it behind `browser-runtime`/test features; Servo C2.3b is a dependency spike | **Any foreign-binary room:** no Linux personality (ADR-023 accepted, no code), no hypervisor, no browser engine |

### 6. Enterprise-grade boring underneath

| Have | Partial | Missing |
|---|---|---|
| Login with Argon2id, recovery code, failure count across reboots; the write guard; a crash-safe installer state machine; the freeze watchdog; Rule-1 kill-switches; CI builds exactly what ships | Observability (`SYS_SYSSTAT` in; `SYS_OBSERVE` off); the mount authority (dispatched 10/30) | **Signed, A/B, rollback-able updates**; an **audit log**; reproducible image builds; driver restart (OT-56); multi-user and policy; fleet management |

### 7. Ruthless subtraction

| Have | Partial | Missing |
|---|---|---|
| A curated image-A feature list; HWP performance (Zenbook "2+2" from 57 s to about 18 s); tick-sleep naps; quotas (feature-only) | OT-90 power (S5 logic merged, `power-control` off); boot time only from CI QEMU | **A boot-time number on real hardware**; a **`.text`/image-size budget in CI**; shutdown/restart in the UI; sleep/wake; NeoFS cache eviction (OT-92) |

## How to get there: seven lanes, in dependency order

Each lane is costed by dependencies, iteration cycles and gates. Each carries the usual rules: a kill-switch, the AAG
plus a cross-family review, a byte oracle for any format, and **a prior-art survey (CLAUDE.md) for every unfamiliar
subsystem**: updates, packages, terminal, hypervisor.

**L1. Turn on what's built (integration; low uncertainty; mostly gate cycles).** It covers principles 2, 3 and 6.
1. **`cap-rooted-open` in image A** after K1b (per-client RootDirCap scoping, so Arthur no longer spans `/`). That
   ends the ambient root open, and it's the general fix the `system-store` and mount-authority rules are special cases
   of.
2. **`net-dial-cap` and `interface-caps` in image A:** egress becomes something a process is granted, not something
   it has.
3. **Arthur's grants, stakes/confirm and reversibility in image A**, with a first permission prompt in the WM
   ("Arthur wants to write ~/notes.txt: Allow once / Always / Deny").
4. **`power-control` in image A:** shutdown and restart from the UI.

The dependency chain is K1b → (1); (3) needs only the WM prompt. Each is a product-integration scenario plus a
Zenbook run.

**L2. Show its work (new but small; one design cycle and a few gate cycles).** It covers principles 2, 3 and 6.
- An **append-only, hash-chained action log** on NeoFS: every grant, confirm, action and undo, with the intent that
  caused it.
- A **"What Arthur did" view** in the Lens: replay, and undo where reversible.
- A **permission inspector:** list grants and caps by app, and revoke (the cap graph already supports revoke).
- **Memory export:** a documented format, with a byte oracle.

The same log is the enterprise audit trail, so one artifact serves three principles. It depends on L1 (3).

**L3. Updates and rollback, OT-51 (genuinely new; high stakes; several cycles; bare-metal proof required).** It
covers principles 1 and 6.
- A/B system slots, or NeoFS-generation snapshots; a boot counter with automatic rollback; signed update bundles.
- It depends on the installer's R5 (Try or Install), and on a **signing-root decision (J.c.)**.
- Survey RAUC, SWUpdate, Mender, ChromeOS/Android A/B and TUF (for the signing and metadata model). Read GPL code for
  the principle only.
- **The rule this lane establishes:** nothing ships to a user's disk that can't be rolled back. Every later lane
  depends on it.

**L4. The app platform (new; medium-high uncertainty; the longest DAG).** It covers principles 1 and 2.
1. A1a (#220) merges, then A1b/A1c (multiple windows, move/resize/snap).
2. **Launch an ELF from NeoFS** under a **SpawnCap** (K2): no ambient spawn.
3. **A package format:** a signed bundle whose manifest **declares the caps it wants**. The manifest *is* the
   permission model the inspector (L2) shows. It needs a byte oracle. Survey Flatpak/OSTree, Fuchsia packages and
   Android APK permissions.
4. **SDK-ABI refresh** (49 of 111 syscalls are documented today), driven by the template app.

Intent acting "across apps" needs (1) and (2); apps exposing actions to Arthur is A2's work.

**L5. The escape hatch (contained; a few cycles).** It covers principle 4.
- A **terminal app**: PTY plus a VT100 subset (OT-57; survey `vte`/`alacritty_terminal`, both permissively
  licensed), hosting `neo-shell`.
- **Developer mode:** an explicit, persisted setting that unlocks the terminal, `SYS_OBSERVE` and the Lens's raw view.
  It stays off by default ("taste sets the defaults").

It depends on L4 (1) for a window.

**L6. Compatibility rooms (the deepest; a strategy decision first).** It covers principle 5. Two shapes:
- **(a) A Linux-binary personality** (ADR-023). Syscall translation in a sealed process, on our quotas and caps.
  Deep and many-cycle, with an endless surface.
- **(b) A hypervisor room.** VT-x/EPT running an unmodified guest, with virtio devices we already speak, and a
  removable disk image. New and deep, but bounded by hardware.

**Decision for J.c.** Either way, the room is a sandboxed app (L4) with a declared cap set, a budget (quotas) and a
"remove" button. The browser (Servo C2.3b) is its own track, on the std overlay.

**L7. Subtraction as a gate (cheap; continuous).** It covers principle 7.
- A **boot-time number on the Zenbook** (fb-console timestamps; the runbook already exists) and an **image-A size
  and `.text` budget** enforced in CI, so a feature pays its way.
- Then sleep/wake (S0ix on the Zenbook) and NeoFS eviction (OT-92).

## Decisions for J.c.

1. **Lane order.** The PM recommends L1 → L2 → L7 now (all three reuse built pieces), then L3 and L4 in parallel.
2. **The update signing root and slot model** (L3).
3. **The compatibility strategy** (L6): a Linux personality or a hypervisor room.
4. **Arthur's agentic actions in image A** (L1 (3)): ship with a prompt on every consequential action, or start with
   read-only actions only.

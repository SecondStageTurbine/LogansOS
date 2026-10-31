<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-61 browser C0 — the libstd PAL skeleton + host de-risk (the ladder's first rung) → Cerebro

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Cerebro (main-tier; Claude family — you authored the cross-family check, so you hold the deepest current context on this architecture)
**Status:** DISPATCHED — **the browser program's first build rung.** The host-closure + SWGL spikes (Janus) + the cross-family check (you) ratified the direction **RATIFY-AFTER-AMENDMENT**. This opens **C0** on the *amended* ladder: a first-class Rust target + a `libstd` **PAL skeleton**, gated by `std-pal-smoke`. **This is the gating precursor for the whole browser program** — nothing downstream (SWGL floor, Servo, SpiderMonkey) can start until the target surface exists. Build to the two FINDINGS + your cross-check as authoritative.

## The amendments are folded — read them first (they change what C0 does)
Your cross-check surfaced six; all six are in this dispatch:
- **A5 (leads — the cheapest de-risk FIRST, zero kernel):** do the two host-buildable steps BEFORE any target/kernel work, so a surprise is cheap.
- **A4 (build-breaking PAL manifest):** the PAL surface is a *checked* manifest — a host-call outside it fails the build (the anti-compat-island guard).
- **A3 (co-design against BOTH consumers):** shape the PAL so the C1 primitives will serve Rust `std` AND SpiderMonkey's native pthread port — don't freeze a `std`-only ABI.
- **A1 + A2 (NAME them as reserved rungs, do NOT build them in C0):** the W^X⊗JIT invariant-budget rung and the per-content-process memory quota are required *later* rungs; C0 reserves them explicitly and must not violate them.
- **A6 (drive the GL gate from the real Servo corpus):** carried to the SWGL rung, but the A5(b) oracle below seeds it.

## Scope — C0 deliverables, in this order

### Step 1 — the host de-risk (A5; zero kernel work; do this FIRST)
1. **std-PAL symbol-surface enumeration.** Link/compile the browser closure's `libstd`-touching crates (or the smallest representative subset) against the proposed target and **enumerate the exact `libstd`/PAL symbol surface** they reference (undefined-symbol collection). Produce the **authoritative PAL surface manifest** — the definitive list of what the PAL must implement, grouped by the gap-table surfaces (threads/TLS/sync/heap/VM/fs/time/entropy/process/panic/C++-runtime). This converts "the PAL is big" into an exact, falsifiable inventory. **If the surface is materially larger than the FINDINGS modeled, that is a cheap early finding — report it, don't paper over it.**
2. **SWGL↔Atrium byte-contract oracle (a byte-oracle, ADR-028 discipline).** A **host-only** oracle asserting the byte contract between SWGL's output framebuffer and our `SharedPixelBuffer`: XRGB8888_LE memory order (`BB GG RR FF`), stride, the mapped-pointer/`init_default_framebuffer` handoff, `get_color_buffer(…, flush=true)` delayed-clear resolution → the exact bytes the compositor reads. **Hand-laid expected bytes, independent of any SWGL/Atrium code** (a writer+reader from one spec agree even when both are wrong). This proves the render seam contract before a single line of kernel work and seeds A6's real-corpus GL gate.

### Step 2 — the C0 target + PAL skeleton
3. **A first-class Rust target spec + static CRT entry.** A custom target (the kernel-target discipline) for the confined browser userland module, with a static CRT entry.
4. **The `libstd` PAL skeleton.** Map `libstd` onto the **existing** libneo syscall surface where it already suffices (print/serial, the existing heap, process exit), and **stub every not-yet-supported PAL symbol to fail DETERMINISTICALLY** (an explicit "unsupported PAL entry" error/abort with the symbol name) — never a silent wrong answer. C0 does NOT build the C1 primitives (threads/TLS/waits/VM-protect); it makes their absence *loud and enumerable*.
5. **A4 — the build-breaking PAL manifest mechanism.** Wire the manifest from Step 1 as a **checked gate**: a PAL symbol referenced but not in the manifest (or a new host-call added later without its manifest entry + capability provenance) **fails the build**. Tightest discipline reserved for the eventual C++/libc++ surface (C3). This is the structural guard against the Linux-compat-island growth your check flagged.

### The C0 gate — `std-pal-smoke` (Janus's falsifiable go/no-go)
6. A target-native **`std-pal-smoke`**: it **prints, allocates, and exits cleanly**, and **every unsupported PAL API fails deterministically** (the stub fires with its symbol, not a silent hang/UB). Green `std-pal-smoke` = C0 passes; a deterministic-failure on an unsupported API is a PASS of the "loud absence" property, not a defect.

## The reserved rungs — NAME, do not build (A1, A2)
State these explicitly in the deliverable's ladder section so they're not forgotten and C0 doesn't violate them:
- **A1 — W^X ⊗ JIT invariant-budget rung (reserved).** LogansOS user memory is R+W+NX; there is **no runtime-executable-code primitive**. C0 is no-JIT and **MUST NOT introduce any W^X-violating mapping** (stays R+W+NX). The eventual JIT is a **new cross-cutting invariant** (per `docs/complexity-governance.md` the invariant budget) — it will need to be named, justified, guarded, and **kill-switchable** in the build that adds it. Reserve it in the ladder now; do not build it.
- **A2 — per-content-process memory quota (reserved, required rung).** Crash containment needs it: there is no per-process frame ceiling today (`sys_map_region` caps only per-call, not per-process total), so an adversarial page can DoS the whole system before its process dies. This is a **kernel VM change landing around C1/C2** (with the tracked-region/`unmap`/`protect` work), not C0. Name it as a required rung so "OOM kills only that process" becomes deliverable.

## Standing rules
- **A3 co-design constraint:** the PAL skeleton's shape (esp. the thread/TLS/wait/VM-facing stubs) must anticipate **both** `libstd`'s platform layer AND SpiderMonkey's native pthread/TLS port — enumerate both consumers' expectations in Step 1 so C1 freezes an ABI serving both.
- **Kill-switch / no-regression:** C0 is purely additive (a new opt-in target + a PAL crate + a smoke scenario). The default kernel/toolchain build is **bit-for-bit unchanged**; nothing web-facing reaches the kernel/user ABI (it stays libneo capabilities — no POSIX personality leaks).
- **Invariant budget:** C0 introduces **no** new cross-cutting invariant (it names A1/A2 as future ones). Confirm the ledger is unchanged.
- **No-human-time rule:** scope in dependency-depth / cycles / gates (C1 is the depth bottleneck downstream). Never calendar/person-months — restate any figure from the FINDINGS.
- **SPDX** on every new file; the A5(b) oracle follows the byte-oracle discipline (raw XRGB8888 layout, hand-laid).

## Gate coverage
- **Host (no QEMU needed):** Step 1 (symbol enumeration + the byte-contract oracle) + the A4 manifest check — host-buildable now, the cheap de-risk.
- **QEMU (`std-pal-smoke`):** the on-target smoke runs as a userland process; if C0 touches only the existing syscall/process surface it is userland+toolchain, but treat the on-target smoke as gate-worthy — a green scenario URL. Kernel-touching ONLY if you add a syscall (avoid it in C0; the new primitives are C1).

## Close-gate + merge
- `scripts/worker-close-gate.sh` (rebase-fresh on current `main`, host-check set, SPDX, self-stat). Green push-CI; a `std-pal-smoke` scenario URL (4-shard `workflow_dispatch` if it's kernel-touching). PM AAG scaled to C0 (the A4 manifest genuinely fails the build on an off-manifest symbol; the unsupported-PAL stubs fail deterministically not silently; the byte-contract oracle is hand-laid-independent; no W^X mapping introduced [A1]; kill-switch = default build unchanged). Disk reclaim: `Workers/cerebro` persistent.
- Branch `codex/ot61-c0-libstd-pal-skeleton-cerebro`; PM merges after the gate + AAG, and folds the PAL surface manifest + the C0=BUILT status into OT-61 + a new C-ladder ADR.

## Relay — what to tell Cerebro
- **Read (all on `main`; your tree is on a feature branch, so `git show`, don't file-open):** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-ot61-c0-libstd-pal-skeleton-cerebro.md`. Authoritative: your own cross-check `…:docs/RESEARCH-2026-07-22-ot61-browser-closure-crosscheck-FINDINGS.md` + the two Janus spikes (`…-servo-host-closure-spike-FINDINGS.md`, `…-swgl-browser-floor-spike-FINDINGS.md`) + the OT-61 row in `…:docs/OPEN-THREADS.md`. Our code: `userland/libneo/src/lib.rs` (the syscall surface + heap the PAL maps onto), `kernel/src/atrium/buffer.rs` (the `SharedPixelBuffer` layout for the byte-oracle), our kernel target-spec JSON (the target-spec discipline to mirror). Work in `Workers\cerebro`; branch `codex/ot61-c0-libstd-pal-skeleton-cerebro`.
- **One-line:** **Build C0 — the browser ladder's first rung (a `libstd` PAL skeleton + first-class target), amendments folded.** Do the **host de-risk FIRST (A5, zero kernel):** (1) enumerate the exact `libstd`/PAL symbol surface the browser closure needs → the authoritative **PAL surface manifest** (grouped by gap-table surface; a bigger-than-modeled surface is a cheap early finding), and (2) a **host-only SWGL↔Atrium byte-contract oracle** (hand-laid XRGB8888_LE bytes, ADR-028) proving the render seam before any kernel work. **Then** the C0 target + PAL skeleton: map `libstd` onto existing libneo syscalls where they suffice, **stub every unsupported PAL symbol to fail DETERMINISTICALLY** (loud, named — never silent), and wire **A4's build-breaking PAL manifest** (an off-manifest host-call fails the build — the anti-compat-island guard). **A3:** shape the stubs to anticipate BOTH `libstd` and SpiderMonkey's pthread port. **NAME but do NOT build (A1/A2):** the W^X⊗JIT invariant-budget rung (C0 stays R+W+NX, introduces NO W^X mapping — JIT is a future kill-switchable invariant) and the per-content-process memory quota (crash containment needs it; a C1/C2 kernel-VM rung). Gate: `std-pal-smoke` (prints/allocates/exits + unsupported APIs fail deterministically). Kill-switch: the default build is bit-for-bit unchanged (new opt-in target; no POSIX personality at the ABI). No-human-time framing. Close-gate + green CI URL + PM AAG.
- **RAG** (PowerShell): `libneo syscall surface heap sys_map_region`, `kernel target spec json`, `SharedPixelBuffer XRGB8888 layout`, `process exit spawn handle`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker).

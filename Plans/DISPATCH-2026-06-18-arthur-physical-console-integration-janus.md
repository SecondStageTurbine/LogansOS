<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: "Arthur at the physical console" — the end-to-end integration scenario → Janus

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Janus (built Slice 1 fd=0/fd=1 routing, Slice 4 the action surface, and the 8b input path — owns every piece this ties together). New branch off `origin/main` HEAD: `codex/phase-8-arthur-physical-console-janus`.
**Status:** DISPATCHED. **The "put the traveler on the road" build — in QEMU.** Each piece of the "sit down and talk to Arthur at the machine" path is built and gated *separately*; nothing exercises them as **one flow**. This is the integration scenario that proves the whole single-principal path end-to-end on already-landed code, and de-risks the bare-metal Stage-B silicon run.

## 0. The decision this build makes
Prove, in one QEMU scenario, the full physical path: **real(-emulated) keyboard → HID → fd=0 → neo-shell → `arthur` builtin → neo-arthur → reply → fd=1 → framebuffer console → screen.** No new subsystem, no new capability, no new model — an **integration scenario + the minimal glue** to compose pieces that are each already proven. Single-trusted-principal throughout: this is composition, **not** an OT-22 frontier pillar (no multi-tenant, no untrusted code, no personality).

## 1. References
```
git fetch origin
git show origin/main:docs/specs/phase-8b-input-path-console.md                       # the 8b input path (fb console + HID kbd)
git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md  # OD-1/OD-6, the fd routing + Arthur process
```
Reuse, unchanged: the 8b framebuffer console + USB-HID keyboard (fd routing); Slice 1 `neo-shell` (fd=0 read, `arthur` builtin spawn+relay); Slice 3 `neo-arthur` (converse) + Slice 4 `libneo` action surface; the 8.5 generate path.

## 2. ISCs
- **ISC-1 — The `arthur_console` QEMU scenario.** Boot with QEMU `usb-kbd` + the UEFI framebuffer → the shell prompt renders **on the framebuffer** → scripted HID keystrokes arrive via the 8b boot-protocol path → `neo-shell` reads the line off fd=0 → the `arthur` builtin spawns `neo-arthur` with its scoped cap bundle → a scripted user line → Arthur replies → the reply renders **on the framebuffer** via fd=1. Exercised at **NEO_SMP=4**. Checkable via the existing framebuffer glyph-region capture (the 8b test hook) + the serial trace.
- **ISC-2 — Use the deterministic Arthur path for the scenario, not the heavy model.** Slice 3's kill-switch gives a **dummy-echo / deterministic** Arthur (feature-off ⇒ dummy-echo). The scenario uses *that* — the integration point is the **I/O plumbing** (keyboard→console→shell→arthur→screen), which must be deterministic and QEMU-cheap; the *real-model inference* is already proven by the 8.5 top-1 oracle and is **not** re-run here. (Optionally include one real-model line behind a separate, non-default selector if it stays within the shard budget — but the default scenario is the deterministic path.)
- **ISC-3 — Wire fd=1 routing for a spawned child to the framebuffer (the one likely glue gap).** Slice 1 routed the shell's fd=1; confirm a **spawned** `neo-arthur`'s fd=1 also reaches the framebuffer console (inherited handle / the OD-1 routing). If it doesn't compose for the child, that small routing fix is the build's real content; if it already composes, the scenario is the deliverable. Either way: **no change to neo-shell/neo-arthur/Arthur semantics** — only transport composition.
- **ISC-4 — I-14 (state explicitly — input is a device path).** No new vector: the HID keyboard is a USB device behind xHCI, **arena-fenced already** (8b/7.5-0/A), no new DMA master, the ISR takes **no AI-governance lock** — inherited containment, **no new guard**. (If you find a genuine new vector, BLOCKER-to-PM.)
- **ISC-5 — Standards + kill-switch.** Behind the **existing** features (`framebuffer-console`/`usb-hid`/`ai-generate`/`arthur`) — all default-off ⇒ serial-only, kernel **bit-for-bit today**. No new invariant; no new `unsafe` (reuse the 8b MMIO/framebuffer idiom). `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy **blocking**). ANSPEC: extend the 8b §10.x note or a short integration note.

## 3. Gate
- **QEMU scenario `arthur_console`** green at NEO_SMP=4 (prompt-on-framebuffer → HID keystrokes → shell → `arthur` → reply-on-framebuffer), the deterministic Arthur path; host logic tests for any new routing glue.
- **Kernel-touching (new scenario in `test_harness`/`xtask` + possible fd-routing glue) ⇒ run the pre-merge `workflow_dispatch` QEMU gate green** (all 4 shards) + the now-blocking clippy via Docker.

## 4. Scope guard
The `arthur_console` integration scenario + the minimal fd-routing glue **only**. **Do NOT** add a new Arthur capability or action, change intent parsing, swap/retrain the model, build a TTY/line-discipline, add multi-principal or any untrusted/personality surface (that's the OT-22 frontier — **out of scope, needs a design THINK first**), or change neo-shell/neo-arthur/Arthur semantics. Reuse the 8b path + Slices 1/3/4 + 8.5. No new invariant/`unsafe`.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main` — the check that bounced the input path twice; the exact CI host-check set on the rebased tree; SPDX; self-stat). The two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL **and** the `workflow_dispatch` QEMU run URL — kernel-touching), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13/28 — **the full physical-console Arthur path, end-to-end in QEMU**). This is the QEMU proof that makes the **bare-metal Stage-B** (real keyboard + screen + Arthur at the OptiPlex) a focused hardware run rather than a first integration.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-arthur-physical-console-integration-janus.md`. **Branch:** `codex/phase-8-arthur-physical-console-janus` off `origin/main`.
- **One-line:** Build **the `arthur_console` end-to-end QEMU scenario** — prove the whole physical path in one flow on already-landed code: QEMU `usb-kbd` + framebuffer → prompt **on screen** → scripted HID keystrokes → `neo-shell` reads fd=0 → `arthur` builtin spawns `neo-arthur` → reply renders **on the framebuffer** (fd=1). Use the **deterministic/dummy-echo Arthur path** (the integration point is the I/O plumbing; real-model inference is already proven by the 8.5 oracle — don't re-run it). The likely real content is **ISC-3**: confirm a *spawned child*'s fd=1 reaches the framebuffer (inherited routing) — small fix if it doesn't compose, nothing if it does. **No new capability/model/intent/TTY/multi-principal** (the untrusted-userland frontier is OT-22 — out of scope, design-THINK-first). I-14 inherited (HID behind xHCI, arena-fenced, no new lock — state it). Kill-switch: the existing `framebuffer-console`/`usb-hid`/`ai-generate`/`arthur` features default-off ≡ serial-only today; no new invariant/`unsafe`. Gate: `arthur_console` at NEO_SMP=4 + **pre-merge `workflow_dispatch` QEMU gate green** + clippy via Docker. **Close-gate: run `scripts/worker-close-gate.sh` before declaring done; RETURN pastes the green push-CI + QEMU URLs.** This is the QEMU proof behind the bare-metal "sit down and talk to Arthur" Stage-B.

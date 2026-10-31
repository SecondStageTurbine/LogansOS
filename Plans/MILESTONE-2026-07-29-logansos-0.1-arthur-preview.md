<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# MILESTONE: LogansOS **0.1 "Arthur Preview"** — PL-ratified 2026-07-29

**Status:** **RATIFIED** (PL, 2026-07-29: *"Yes, do the 0.1 Arthur Preview — we need to track everything to make sure nothing is lost and we have actual goals"*).
**Relationship to 1.0:** 0.1 is a **milestone on the way, not a redefinition.** **1.0 remains the ratified five-journey bar and remains browser-gated** (PL, 2026-07-12: *"an OS that can't browse the internet isn't shippable"*). Nothing here relaxes that.

## What 0.1 is
The demo no other OS can give, built from assets that are **already merged and silicon-proven**:

> **Boot on real hardware → a real desktop → talk to Arthur, our own LLM on our own kernel → it's online → your work persists.**

**Explicitly NOT in 0.1:** the browser (that's 1.0's gate), an installer (0.1 ships as a bootable image, the way it boots today), login/session, a document editor, printing. Those are tracked as OT-65…OT-68 and are 1.0 scope.

## The acceptance chain (falsifiable — one bare-metal boot, no test scenarios)
Each rung must be observable from the serial log + the PL's live on-screen read, per `docs/runbook-baremetal-arthur-optiplex.md`.

| # | Rung | Acceptance evidence | Status today |
|---|---|---|---|
| 1 | **Boots on real hardware** | Limine → kernel → framebuffer up, no panic | ✅ **PROVEN** (Arthur bare-metal, 2026-07-26) |
| 2 | **A real desktop** | ≥2 `ROLE_TEXT` surfaces composited; keyboard focus switch A↔B; only the focused client observes input; both clients' direct-present → `EPERM` | ✅ **MERGED 2026-08-01** (`0727050e`, Jarvis). 4-shard gate green on the exact SHA; PM stat clean (13 files, +1076/−11, zero deletions); **AAG bounced then cleared** — Cerebro's cross-family lens found INV-WM-1 was enforced on only 1 of 4 verb handlers (a client could `Destroy`/`Commit` a **peer** by naming it, since surface ids and provenance constants are guessable) plus a focus-atomicity assert that was a printed string with no counter. Both fixed via a shared `require_channel_identity()` helper (4 call sites in DW-1 **and** 4 in `arthur_compositor.rs`, closing the same latent hole on `main`) + a real `EAGAIN` drain + a KAT naming a peer over the wrong channel. Rebase verified by code-subtree tree-identity — no re-gate burned. |
| 3 | **Talk to Arthur** | Typed question → coherent LLM reply on the framebuffer, `provenance=real_silicon` | ✅ **PROVEN** (`REPLY: Hello! How can I assist you today?`, 2026-07-26) |
| 4 | **It's online** | `eth0` up + **nonzero DHCP lease at boot**, no test scenario; no dongle ⇒ boots clean, never hangs | ✅ **PROVEN ON HARDWARE 2026-08-01** (`aa6c5b27`, Chad + PL, run 3). Serial: **`eth0: link up ip=<optiplex-ip> … fence_refusals=0`** — a real lease on a **product** boot with no test scenario, I-14(c) containment intact, `/dev/sda` (Arthur model volume) untouched. **Corroboration:** the OS took the *same* address Ubuntu holds on that dongle, proving our DHCP client presented the correct MAC and the server honoured its existing lease — end-to-end, not a coincidence. Took 3 boots: run 1 = link-notification vetoed a live PHY; run 2 = link fixed, `frames=0` (RX never armed); run 3 = PASS — each failure self-diagnosing from captured evidence, which is what the blind-hardware discriminator rule exists to produce. **✅ MERGED 2026-08-02 (`f898569b`, Chad).** Gate `30753257424` PM-verified: 12/12 jobs, 0 failed, attempt 1, exact SHA `2912cfbe`; stat 15 files +880/−41, zero deletions; tree-identity confirmed at merge. **AAG returned clean** (Oracle — 3 findings + 3 notes, none blocking). Two merge conditions were met: the RETURN's I-14(b) figure corrected from `10_000` to the shipped `750_000` pump passes, and **`boot-eth0` added to the OT-69 coverage matrix** so 523 newly gated lines did not land in the never-compiled hole. **On the rebase Chad caught a genuine integration defect** — a duplicate `product` feature where `boot-neofs` and `boot-eth0` each defined it — and composed them; that collision was invisible to either branch's own gate. *(Prior state:)* **gate cleared:** 4-shard `30678862941` **PM-verified per-shard on the exact SHA** (all 4 QEMU shards + host + SPDX + monolithic-identity green, every shard's test step `success` not skipped, no attempt-2). Last gate is the **I-14 AAG → Oracle** (`Plans/DISPATCH-2026-08-01-aag-eth0-boot-product-flow-oracle.md`); PM rebases at merge. |
| 5 | **Work persists** | Something written survives a power cycle and is readable after reboot (NeoFS) | ✅ **MERGED 2026-08-02** (in the integration set `6dda37e2`, Chad). Gate PM-verified on the exact SHA; **AAG clear** (Cerebro re-check — mount-after-`init_bsp` traced clean, no lock held across a park, and F1 proven a *strict improvement* since the streaming blake3 read-time check survives). F2 ratified as the **unified NeoFS product volume**. *(Prior state:)* 🔧 **AUDITED — dispatched** (Chad, 2026-07-30). Verdict: *mostly wired, one small build.* NeoFS is a mature **read-write** FS (6,648 L, zero `todo!`, CoW + rotating superblock + writeback + GC + crash recovery) and **durability is already PROVEN** by a genuine two-boot test (`phase3_neofs_reboot_persistence_small_file`). Gaps are wiring: NeoFS is **never mounted on a product boot**, the Arthur bare-metal showcase mounts it **READ-ONLY** (writing structurally impossible on the demo boot), no user-facing write, and no userland-syscall write→reboot→read test. Build = `boot-neofs` default-off mount (the eth0 pattern) + flip the showcase to RW + **"Arthur remembers you across a reboot"** as the demo + the missing two-boot userland scenario. |

**Non-functional bars (all five rungs):** no hang or panic on absent hardware; every kill-switch intact (**OFF ≡ prior layer, bit-for-bit**); `fence_refusals=0` (I-14(c) DMA containment); no credential or secret in any shipped artifact.

## Open lanes for 0.1
| Lane | Owner | State |
|---|---|---|
| **DW-1 desktop WM** | Jarvis | dispatched `Plans/DISPATCH-2026-07-29-dw1-desktop-window-manager-jarvis.md` |
| **eth0 at boot** | Chad | dispatched `Plans/DISPATCH-2026-07-29-eth0-boot-product-flow-chad.md` |
| **Rung-5 persistence** | Chad | ✅ audit DONE (2026-07-30 — verdict: mostly wired, one small build, exactly as suspected) → dispatched `Plans/DISPATCH-2026-07-30-rung5-persistence-product-flow-chad.md` |
| **Follow-on: NeoFS `mkdir`/`unlink`** | unassigned | explicitly **out of scope** of the rung-5 build. `create_regular` needs a pre-existing directory (tree baked by `tools/neofs-mkfs`). **Gates a real editor — a prerequisite of OT-67.** |
| **0.1 integration + bare-metal showcase** | unassigned | the closing build: one ISO, one boot, all five rungs in a single run. **Queued behind the break gate** (`Plans/BREAK-GATE-2026-08-01-open-lane-closeout.md`) — not dispatched until the PL lifts it. |

## ⭐ ALL FIVE RUNGS ARE ON `main` — 2026-08-02

Every rung of the 0.1 acceptance chain now has its code merged, gate-verified on its exact SHA, and adversarially reviewed:

| rung | how it landed |
|---|---|
| 1 · boots on real hardware | PROVEN on Arthur bare metal, 2026-07-26 |
| 2 · a real desktop | merged `0727050e` (DW-1, Jarvis) — AAG bounced once on INV-WM-1, then cleared |
| 3 · talk to Arthur | PROVEN — `REPLY: Hello! How can I assist you today?`, 2026-07-26 |
| 4 · it's online | merged `f898569b` (Chad) — **and silicon-proven**: a real DHCP lease, `ip=<optiplex-ip>`, `fence_refusals=0` |
| 5 · work persists | merged in `6dda37e2` (Chad) — two-boot durability proven by a genuine reboot scenario |

**What this does and does not mean.** Per the **merge bar vs acceptance bar** rule: the *code* for all five rungs is on `main`, each cleared by gate + AAG. It does **not** mean 0.1 is demonstrable — **no single boot has yet shown all five rungs together.** That is the **0.1 integration showcase**: one ISO, one boot, five rungs in one run, with the PL's live on-screen read. It is the closing build and it sits in the post-break queue.

**Carried debt that touches the showcase, all recorded and none blocking the merges:** OT-73 (model content unverified under `arthur-real-brain-boot` — **settle before the OptiPlex boot**, and verify the physical volume offline first so the switch never flips on the acceptance run), OT-78 (`product` is built by nothing — all three degrade arms and the device-selection path are exercised by no test), OT-74 (`arthur_reversibility` is a genuine race at ~37.5% attempt-1 failure on good hardware).

**BREAK GATE (PL, 2026-08-01):** *"after all these open lanes start closing I want to come to a break, so no more dispatches after everyone is green."* The closing condition for every in-flight lane, and the recorded post-break queue, live in `Plans/BREAK-GATE-2026-08-01-open-lane-closeout.md`. **Rungs 4 and 5 close inside the break gate; the 0.1 integration showcase is queued behind it.**

## Tracking rule (PL directive, 2026-07-29)
*"We need to track everything to make sure nothing is lost and we have actual goals."* Accordingly:
- The four previously-invisible 1.0 journeys are now **OT-65 (install), OT-66 (login/session), OT-67 (document/editor), OT-68 (print)** in `docs/OPEN-THREADS.md`. Each is **1.0 scope, not 0.1**, and each needs a **THINK before any build dispatch** (printing especially — zero prior art in-tree).
- The audit that produced this is `Plans/LEDGER-2026-07-29-logansos-release-ledger.md`; it is the standing release ledger and is updated as rungs close.
- **1.0 critical path is unchanged:** the browser C-ladder (C1.1 → C1.2 → C2 → C3 → SWGL → modern-site battery). 0.1 does not sit on it and must not consume its runner budget when they contend.

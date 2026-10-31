<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: AAG lens on rung-5 `boot-neofs` (`dc72d8b3`) → Cerebro

**Date:** 2026-08-01
**From:** PM (Claude)
**To:** Cerebro (Claude-family — the cross-family lens on Chad/Codex work)
**Status:** DISPATCHED — **doc-only adversarial review, no build, no gate.** Gate is GREEN (PM-verified, exact SHA, all 4 QEMU shards, clean 23-min run, no retries); PM stat is clean (10 files, +269/−14, **zero file deletions**). This is the last gate before **0.1 rung 5 — "your work persists"** lands on `main`.

**Candidate:** `codex/rung5-boot-neofs-product-flow-chad` @ `dc72d8b3`. **Spec:** `git show origin/main:Plans/DISPATCH-2026-07-30-rung5-persistence-product-flow-chad.md`.
**What it does:** adds default-off `boot-neofs` (in the `product` feature set) mounting NeoFS device 0 **read-write** after `fs::init()`; flips the Arthur compositor mount to RW; enables `arthur-memory` in the compositor closure; adds a `boot_neofs_userland_persistence` two-boot scenario.

## The lenses — REFUTE-default
**1. DATA SAFETY (highest priority — this is the one that can destroy a user's disk).** Chad claims *"No repair path, no formatting."* **Verify it, don't accept it.** Can **any** code path reachable from boot **write to a disk that failed to mount**? Specifically: a bad/absent superblock, a partially-written superblock, a wrong-magic disk, or a disk from a different filesystem entirely — does any of those trigger a write, a "fix", a superblock rotation, a GC pass, or a bump-allocator publish? An RW mount that decides to *heal* a disk it misread is how a user loses data that had nothing to do with us.

**2. ⚠ WHICH DISK IS "DEVICE 0" ON BARE METAL? (I want this answered explicitly.)** On the OptiPlex, **`/dev/sda` is the Arthur NeoFS model volume** and the boot ISO lives on `nvme0n1`. If `boot-neofs` mounts "device 0" **read-write** and device 0 resolves to the **AHCI model volume**, then every product boot now has an RW mount of Arthur's model store. Trace the enumeration order concretely: what does device 0 resolve to under the bare-metal `product` config, and under the QEMU scenario config? If they differ, say so — the gate cannot see the bare-metal answer.

**3. Degrade completeness.** No device / bad superblock / mount failure must each let boot **continue** — never hang, never panic. Are all waits **bounded**? Is there any path where a wedged or slow disk blocks boot indefinitely?

**4. Non-vacuous persistence proof (the reason this build exists).** The pre-existing `phase3_neofs_reboot_persistence_small_file` drives the VFS from **inside the kernel test harness** — that was Gap E. Chad's new `boot_neofs_userland_persistence` is supposed to write **through userland syscalls** on boot 1 and read back **through userland syscalls** on boot 2. **Verify both halves actually go through `SYS_OPEN`/`SYS_WRITE`/`SYS_FSYNC` from a userland process** — if either side quietly reaches into the kernel VFS, the scenario is vacuous and re-creates the exact hole it was written to close. Also: does boot 2 genuinely re-read from disk, or could it be observing in-memory state carried across?

**5. Kill-switch.** `boot-neofs` OFF ≡ the current boot path, **bit-for-bit**. Check whether the cfg edits are additive arms (structurally equivalent when off, like DW-1's) or runtime branches (weaker).

**6. I-14 + completeness.** (a) no AI-governance lock on the block/FS path; (b) a wedged disk degrades liveness only; (c) DMA stays in the fenced arena, `fence_refusals=0`. Then: **what did the green gate NOT cover?** It is a QEMU virtio-blk run — name what that structurally cannot exercise about a real AHCI/SATA disk on the OptiPlex.

## Rules
A finding **bounces only if it survives your own re-verification**; uncertain ⇒ a note naming the missing fact, never a speculative blocker; **"no findings" requires stating what each lens checked** (no rubber stamp). Your DW-1 lens was exemplary — same standard.

## Relay — Cerebro
- **WORKTREE:** `E:\AI Automation\workers\cerebro` (yours alone) · **BRANCH:** none — doc-only.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-01-aag-rung5-boot-neofs-cerebro.md`; candidate via `git show origin/codex/rung5-boot-neofs-product-flow-chad:<path>` @ `dc72d8b3`.
- **One-line:** **AAG on rung-5 `boot-neofs` — the last gate before "your work persists" lands.** Gate green, stat clean, small diff (+269/−14). Six lenses, REFUTE-default: **(1) data safety** — Chad claims "no repair path, no formatting"; prove no reachable boot path *writes* to a disk that failed to mount (bad/partial/foreign superblock must not trigger a heal, rotation, GC or publish); **(2) which disk is "device 0" on bare metal?** — on the OptiPlex `/dev/sda` is the **Arthur model volume**; if device 0 resolves there, every product boot now RW-mounts Arthur's model store — trace enumeration for both the bare-metal `product` config and the QEMU config and say whether they differ; **(3) degrade** — no device / bad superblock / mount failure each continue boot with **bounded** waits, never hang or panic; **(4) non-vacuous proof** — `boot_neofs_userland_persistence` must write AND read through **userland syscalls** both boots (the old scenario drove the kernel VFS — that was the gap this build closes; if either half cheats, it's vacuous), and boot 2 must genuinely re-read from disk; **(5) kill-switch** OFF ≡ current boot bit-for-bit (additive cfg arms or runtime branches?); **(6) I-14 (a)(b)(c) + completeness** — what can a QEMU virtio-blk run structurally not tell us about a real AHCI disk? Doc-only, no build, no gate.
- **RAG** health-check first; e.g. `mount_neofs_from_device0_readwrite superblock validate`, `neofs publish_superblock rotation gc allocate`, `block device enumerate device0 ahci virtio order`, `boot_neofs_userland_persistence scenario stage write readback`.

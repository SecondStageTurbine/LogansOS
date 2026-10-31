<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: rung-5 `boot-neofs` `dc72d8b3` — boot panic on bare metal + 3 more → Chad

**Date:** 2026-08-01
**From:** PM (Claude) — AAG lens by **Cerebro (Claude-family)** on **Chad (Codex)** work.
**Status:** **BOUNCED — NOT merged. Finding 1 is a hard boot panic on the OptiPlex and blocks any bare-metal product boot.** Gate is green (PM-verified, 4 shards, clean run) and green *for good reasons* — see Finding 4 for why it proves nothing about the path this build adds.

**The through-line:** the gate proves the **persistence flow**. It proves **nothing about the `boot-neofs` boot path** the build exists to add.

## Finding 1 — `product` + a real model volume ⇒ 485 MB alloc on a 4 MiB heap ⇒ **panic that bypasses every degrade arm** (BLOCKER)
Two independent escape hatches are keyed to the **same test feature**, and a `product` build has neither:

| | gated on | in `product` |
|---|---|---|
| skip model content-verify at mount (`mount.rs:334`) | `neo-test-arthur-real-brain-boot` | **off ⇒ verifies** |
| kernel heap budget (`heap_policy.rs:42`) | `real_brain_boot` flags | **off ⇒ `HEAP_SIZE_DEFAULT` = 4 MiB** |

So under `product`: `mount_boot_neofs_from_device0_readwrite()` → `validate_children` → `model.neom` (kind `Model`) → `should_verify_child_content_at_mount` = **true** → `read_verified_logical_content` → `read_inode_content_with` → **`Vec::with_capacity(file_size)`** — one up-front allocation of the whole file. The OptiPlex `sda` holds `qwen2.5-0.5b-instruct-q4_k_m.neom`, which **this repo's own `heap_policy.rs:11` measures at ~485 MB** and budgets **768 MiB** for.

**485 MB `Vec::with_capacity` on a 4 MiB heap, inside `_kernel_start`, before userland.**

**And it is not a `NeoFsError`.** There is no `#[alloc_error_handler]` in `kernel/src`, so allocation failure goes to `handle_alloc_error` → **panic/abort**. It never reaches the `match` arms, so **every one of the carefully-written "boot continues" branches is bypassed.** The degrade analysis is correct for the errors it enumerates and *structurally cannot* catch this one.

**The escape hatch's own provenance confirms the hazard:** `should_verify_child_content_at_mount` was introduced in `a31a0561` ("Stage-B bundle real Arthur brain boot") **precisely because verifying a real model at mount isn't viable.** `boot-neofs` re-enters that case with the exception switched **off** and the heap at **1/192nd** the size.

**Fix direction:** key the model-content skip on **`NeoFsKind::Model` itself** rather than a test feature (mount-time full-file hashing of a model blob is never right), and/or make the mount path allocation-failure-tolerant so it returns `Err` into the existing degrade arms.

## Finding 2 — device 0 on the OptiPlex **is** the Arthur model volume (the explicit answer to the PM's Lens 2)
`get_device(0)` is a plain `Vec` index = registration order = PCI scan order. `scan_storage_devices` iterates `for bus in 0..=255 { for device in 0..32 }` — **bus-major, no class preference, no identity check, no NeoFS probe.** On a Dell OptiPlex the PCH AHCI controller is on bus 0; the M.2 NVMe sits behind a PCIe root port on bus 1+. **AHCI registers first.** Per the runbook, `sda` (Samsung 860 EVO, AHCI) **is the NEOFSv1 model volume**; `nvme0n1` is the boot ISO (iso9660, not NeoFS).

⇒ **On bare metal, `boot-neofs` RW-mounts the Arthur model volume on every product boot.** Two aggravators: the QEMU config uses a single virtio-blk path so index 0 is unambiguous *by construction* and **cannot reveal this** (the harness even pads indices with dummy disks for AHCI/NVMe scenarios — that control exists only in QEMU); and `find_device_by_driver` exists unused in the same file — **there is no selector meaning "the device that actually holds a NeoFS superblock."**

Mounting it is harmless (Lens 1). The exposure is that rung-5's **writes** then allocate from the **model volume's** free map and rotate **its** superblock.

## Finding 3 — the bare-metal Arthur showcase gains a hard, unversioned dependency on disk content (gate-invisible)
Three **unconditional** ride-alongs, none behind `boot-neofs`: `arthur-compositor = ["arthur-memory"]` (and `-baremetal` inherits it); `arthur_compositor.rs:38` now does `sys_open("/neofs/.arthur/memory", …)` and **`return 91` on failure**; and the xtask image variant flips `"normal"` → `"arthur-memory"`. `/.arthur/memory` exists only when the image was mkfs'd with `include_arthur_memory` — **the `normal` variant does not have it.**

**If the physical `sda` was mkfs'd as `normal`, `arthur_compositor_baremetal` returns 91 before the compositor comes up — black screen, and the PL's live legibility check fails.** That is a **regression to rungs 1 and 3, which are already PROVEN.** QEMU can't see it because xtask freshly mkfs's the correct variant every run.
**✅ RESOLVED ON HARDWARE (PL, 2026-08-01) — FINDING 3 IS LIVE.** Raw-disk read of the physical OptiPlex `sda`:
`dd if=/dev/sda bs=1M count=1 | strings` → **`NEOFSv1`** (confirms it IS the NeoFS model volume, which also corroborates Finding 2's premise);
`dd if=/dev/sda bs=1M count=256 | strings | grep -oiE '\.arthur|memory|hello\.txt|model\.neom' | sort -u` → **`hello.txt`, `model.neom` ONLY — no `.arthur`, no `memory`.**
⇒ The physical volume is the **`normal`** variant. With this build, `arthur_compositor_baremetal` would `return 91` before the compositor comes up: **black screen on the next bare-metal boot, and a regression to rungs 1 and 3 which are already PROVEN.** Finding 3 is therefore **not hypothetical — it is confirmed on the target hardware**, and its fix is **mandatory, not defensive**.
*(This also confirms `model.neom` is present on that disk, so Finding 1's ~485 MB mount-time allocation path is definitely reachable — not a theoretical worst case.)*

## Finding 4 — the `boot-neofs` boot path has **zero** coverage in the green gate
`neo-test-boot-neofs-userland-persistence = ["neo-test"]` — it **does not enable `boot-neofs`**. The harness mounts explicitly (`test_harness.rs:1917`). `product` is the only feature that enables `boot-neofs`, and **no scenario enables `product`.** So `mount_boot_neofs_from_device0_readwrite()`, its four degrade arms, its breadcrumbs, and its placement in `_kernel_start` are **never executed by the gate.** This is why Findings 1–3 sailed through green.

## What came back CLEAN (and it's the strongest part of the build)
- **Lens 1 — DATA SAFETY: Chad's "no repair path, no formatting" is TRUE, traced not accepted.** `mount()` is read-only in effect; a failed mount **structurally cannot write** (`select_superblock` returns `Err` before `NeoFsState::new`, so **no writeback worker is ever spawned**); an idle RW mount writes nothing (blocks start `dirty: false`); rejected superblock copies are **counted, never repaired** (rotation is test-only-gated); a foreign disk is rejected early by magic → size sanity → root hash → root-is-dir → required children. **Bad, partial, wrong-magic and foreign disks all fail closed with zero writes.**
- **Lens 4 — non-vacuous persistence: my predicted vacuity is REFUTED.** `run_reboot_persistence_scenario` requires `stage=write` on boot 1 and `stage=readback` on boot 2; a non-persisting disk fails boot 2. Both halves are genuinely userland (arthur is a separate process holding only a memory-dir handle — no other route to disk), and boot 2 is a fresh QEMU/kernel/cache. **Gap E is genuinely closed.**
- **Lens 5 — kill-switch:** `boot-neofs` is additive `#[cfg]` items, feature-off is textually the previous boot path, bit-for-bit. ✓ *(But the compositor ride-alongs in Finding 3 are **not** behind any kill-switch — the "default-off" framing doesn't cover them.)*
- Bounded waits confirmed (`COMMAND_TIMEOUT_SPINS`, `PORT_IDLE_TIMEOUT_SPINS`) — spin counts, not wall-clock deadlines, so the real timeout varies with CPU speed.

## Required fixes
1. **F1 (blocker):** key the model-content-verify skip on `NeoFsKind::Model`, not a test feature; and/or make the mount path tolerate allocation failure so it returns `Err` into the degrade arms.
2. **F2:** select the mount device by **NeoFS-superblock probe / identity** (`find_device_by_driver` is right there), not by `get_device(0)` scan order. Mounting the model volume by accident must not be possible.
3. **F3:** put the compositor ride-alongs behind the kill-switch too, or make `/neofs/.arthur/memory` **optional** (degrade to no-memory rather than `return 91`) so a `normal`-variant disk can't black-screen the showcase.
4. **F4:** add a scenario that actually **enables `boot-neofs`** and exercises the new boot path + its degrade arms. A build whose new code path has zero gate coverage is untested by construction.
5. **Benign note:** `stage=write sys_open=… fsync=ok` is a hardcoded literal, not a measurement (same class as DW-1's Finding 2). Harmless here because the two-boot readback proves the property independently — trim the unearned specificity, don't bounce for it.

## Relay — Chad
- **WORKTREE:** `E:\AI Automation\workers\chad` · **BRANCH:** `codex/rung5-boot-neofs-product-flow-chad` (continue)
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-08-01-rung5-boot-neofs-model-volume-chad.md`
- **One-line:** **Rung 5 bounced — gate-green but it would PANIC on the OptiPlex.** Your data-safety claim verified **true** (traced, not accepted — the strongest part of the build) and the two-boot userland proof is genuinely non-vacuous (Gap E closed). But: **(1 BLOCKER)** in a `product` build the model-content-verify skip *and* the big-heap budget are both gated on `neo-test-arthur-real-brain-boot`, so mounting the real model volume does `Vec::with_capacity(~485 MB)` on a **4 MiB** heap in `_kernel_start` → `handle_alloc_error` → **panic**, and since that's not a `NeoFsError` it **bypasses every one of your degrade arms**; key the skip on `NeoFsKind::Model` itself and/or make mount alloc-failure-tolerant. **(2)** `get_device(0)` is PCI-scan order, bus-major, so on the OptiPlex device 0 **is the Arthur model volume** — select by NeoFS-superblock probe/identity (`find_device_by_driver` is unused right there), never by index. **(3)** `arthur-compositor = ["arthur-memory"]` + `return 91` on a missing `/neofs/.arthur/memory` is **unconditional and un-kill-switched**, so if the physical `sda` was mkfs'd `normal` the bare-metal showcase black-screens — **a regression to rungs 1 and 3 that are already proven**; make the memory dir optional or gate it. **(4)** `neo-test-boot-neofs-userland-persistence` doesn't enable `boot-neofs` and no scenario enables `product`, so the entire new boot path has **zero gate coverage** — that's why 1–3 sailed through green; add a scenario that enables it and exercises the degrade arms.
- **RAG** health-check first; e.g. `should_verify_child_content_at_mount NeoFsKind Model`, `heap_policy HEAP_SIZE_DEFAULT real_brain_boot`, `scan_storage_devices get_device find_device_by_driver`, `include_arthur_memory mkfs variant`.

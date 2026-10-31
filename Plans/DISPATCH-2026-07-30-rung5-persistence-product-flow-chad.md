<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: 0.1 rung 5 — make persistence a PRODUCT path (mount NeoFS at boot) → Chad

**Date:** 2026-07-30
**From:** PM (Claude)
**To:** Chad (Codex, main-tier — eth0 is blocked on hardware, and this is **the identical pattern you just built well**)
**Status:** DISPATCHED — **0.1 milestone rung 5.** Audit verdict: *mostly wired, one small build.* This is wiring + a demo surface, **not** a filesystem build.

## What already exists (do NOT rebuild any of this)
- **Write path, real and capability-checked:** `SYS_OPEN/READ/CLOSE/FSYNC/READDIR/MOUNT/FTRUNCATE` all dispatched and non-stub (`kernel/src/arch/x86_64/syscall.rs:102-113, 696-707`; `sys_file_write_impl:4606`, `sys_fsync_impl:4666`). ADR-061 file caps implemented (`kernel/src/sched/file_cap_logic.rs`) — derivable, grantable, spawn-inheritable, attenuating.
- **NeoFS is read-WRITE and mature:** `kernel/src/fs/neofs/` (6,648 L, **zero** `todo!`/`unimplemented!`) — `create_regular`, `commit_regular_write`, `fsync_regular`, block/indirect/triple-indirect allocation (ADR-064), free-map GC, **CoW `publish_superblock` with rotating copies + generations**, write-back cache with a drain worker. Both `mount_readonly` and `mount_readwrite` exist (`kernel/src/fs/mod.rs:634,657`).
- **Durability is PROVEN:** `phase3_neofs_reboot_persistence_small_file` is a genuine two-boot harness (`tools/xtask/src/main.rs:6996` `run_reboot_persistence_scenario`) — boot 1 writes, **boot 2 reads the identical bytes back** — plus fsync-barrier, unfsynced-may-be-lost (honest negative control), and crash/CoW-recovery scenarios.

## The gaps (this build)
- **Gap A — blocker: NeoFS is never mounted on a product boot.** `kernel/src/main.rs:625-630` does only `fs::init()` (an in-memory testfs root; the comment still says *"until BUILD-2 introduces a real NeoFS root"*). Every `mount_neofs_from_device0_readwrite` call is inside `kernel/src/test_harness.rs`, i.e. behind `#[cfg(feature = "neo-test")]`.
- **Gap B — the showcase boot mounts READ-ONLY.** `kernel/src/test_harness.rs:2026` — `arthur_compositor` / `arthur_compositor_baremetal` (the exact bare-metal boot where the PL observes 0.1) call `mount_neofs_from_device0_readonly()`. **On that boot, writing is structurally impossible.**
- **Gap C — no user-facing write.** `userland/neo-shell/src/shell_logic.rs:19` — `ParsedCommand` has no `write`/`touch`/`save`/`rm`/`>` redirect.
- **Gap D — no `mkdir`/`unlink`.** `create_regular` can create a file in an *existing* directory; the tree must be pre-built by `tools/neofs-mkfs`. **Work within this — do not build mkdir in this dispatch** (name it as a follow-on).
- **Gap E — no end-to-end userland test.** Nothing proves *userland-syscall* write → reboot → *userland-syscall* read. The two halves exist separately and have never been composed.

## Scope
1. **`boot-neofs` — a default-off boot mount, exactly your eth0 pattern.** After `drivers::block::init()`, mount device 0 **read-write**, degrading cleanly (no device / bad superblock / mount failure ⇒ status breadcrumb, **boot continues, never hangs, never panics**). Add it to the **`product`** feature set alongside `boot-eth0`. `default` stays unchanged.
2. **Flip the showcase boot to read-write** (`arthur_compositor_baremetal`) so rung 5 is observable on the same boot as rungs 1–3.
3. **The demo surface — pick ONE, and I recommend the second:**
   - a `save`/`write` builtin in `neo-shell`; **or**
   - **enable `arthur-memory` in the `arthur-compositor` feature closure** (`userland/neo-arthur/Cargo.toml:39,50` — it is currently *absent*, so the desktop Arthur has **no memory at all**). Then **"Arthur remembers you across a reboot" IS rung 5** — a far better product story than "a file survived," and it uses the already-working `sys_open(OPEN_CREATE_REGULAR)` + `sys_write` + `sys_fsync` path in `neo-arthur/src/memory_logic.rs`.
4. **Gap E — the missing test:** a new two-boot scenario registered in `is_reboot_persistence_scenario` (`tools/xtask/src/main.rs:4114`) that drives the write **from userland syscalls** on boot 1 and reads it back **from userland syscalls** on boot 2. This is the non-vacuous proof; the existing scenario drives the VFS from inside the kernel harness.

## Standing rules
- **Kill-switch:** `boot-neofs` OFF ≡ current boot path, **bit-for-bit** (regression proving equality). Same discipline as `boot-eth0`.
- **I-14:** state (a) no AI-governance lock on the block/FS path, (b) a wedged disk degrades liveness only — **bounded** mount/drain waits, boot completes, and (c) block DMA stays inside the fenced arena; report `fence_refusals=0`.
- **No new invariant** expected. Do **not** add `mkdir`/`unlink` here — note them as a follow-on (they gate a real editor, OT-67).
- **Data safety:** a bad/absent superblock must never be "repaired" by overwriting a disk that might hold the Arthur model volume. Degrade, don't write.

## Discriminators (the bare-metal run must be conclusive)
| Observation | Meaning |
|---|---|
| Write on boot 1, same bytes read on boot 2 | **rung 5 PASS** |
| `neofs: no device` / `bad superblock`, boot continues | correct degrade |
| Boot hangs or panics on a bad/absent disk | **unacceptable regression** |
| Mounted RW but write fails | cap/rights issue — capture the `SYS_OPEN` rights and the errno |
| `fence_refusals > 0` | I-14(c) breach, not an FS bug |

## Close-gate + merge
`scripts/worker-close-gate.sh`, green push-CI, **4-shard `workflow_dispatch`** on the exact SHA, **plus the new two-boot userland scenario green**. Paste all URLs. The product proof is the **0.1 bare-metal boot**: write something, power-cycle, read it back. No auto-merge — PM verify + AAG. Branch: `codex/rung5-boot-neofs-product-flow-chad`. Disk-reclaim after merge.

## Relay — Chad
- **WORKTREE:** `E:\AI Automation\workers\chad` (yours alone — never another coder's folder, never the repo root)
- **BRANCH:** `codex/rung5-boot-neofs-product-flow-chad` (`git fetch origin && git checkout -B codex/rung5-boot-neofs-product-flow-chad origin/main`)
- **Read:** `git show origin/main:Plans/DISPATCH-2026-07-30-rung5-persistence-product-flow-chad.md` + `git show origin/main:Plans/MILESTONE-2026-07-29-logansos-0.1-arthur-preview.md`.
- **One-line:** **0.1 rung 5 — make persistence a product path.** NeoFS is already a mature read-write FS (CoW, superblock rotation, writeback, GC, crash recovery, zero `todo!`) and **durability is already proven by a two-boot test** — but on a *product* boot NeoFS is **never mounted**, and on the Arthur bare-metal showcase it is mounted **READ-ONLY**, so writing is structurally impossible on the exact boot we demo. Add **`boot-neofs`** (default-off, in the `product` set, mounts device 0 read-write after block init, degrades cleanly on no-device/bad-superblock — **your eth0 pattern**), flip `arthur_compositor_baremetal` to read-write, and make the demo **"Arthur remembers you across a reboot"** by enabling `arthur-memory` in the `arthur-compositor` feature closure (it's absent today, so desktop Arthur has no memory). Then add the missing proof: a two-boot scenario that writes **from userland syscalls** and reads back **from userland syscalls** (the existing one drives the VFS from inside the kernel harness). Kill-switch OFF ≡ current boot bit-for-bit; I-14 (a)(b)(c); **no mkdir/unlink in this build** (follow-on). Never overwrite a disk to "repair" a bad superblock — degrade.
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `mount_neofs_from_device0_readwrite mount readonly`, `neofs create_regular commit_regular_write fsync`, `arthur memory_logic sys_open OPEN_CREATE_REGULAR`, `is_reboot_persistence_scenario two boot`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).

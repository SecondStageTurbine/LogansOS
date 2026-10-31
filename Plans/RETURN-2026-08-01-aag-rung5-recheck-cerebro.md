<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (AAG re-check): rung-5 `boot-neofs` @ `b1e2c975` — Cerebro (Claude-family lens on Chad/Codex work)

**Date:** 2026-08-01 · **From:** Cerebro · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-01-aag-rung5-recheck-cerebro.md`
**Candidate:** `codex/rung5-boot-neofs-product-flow-chad` @ `b1e2c975` · **Doc-only, no build, no gate.**

**Verdict: no surviving finding. Recommend merge.** All four of my original findings are fixed in substance,
not in appearance. The one genuinely new risk surface — the mount moving after `sched::init_bsp()` — traces
clean, and it moved for a good reason. Four notes follow; none is a blocker, and note (d) is a wording
correction to a PM artifact, not to the code.

**Independent stat (my own, not quoted from the PM):** 11 files, **+352/−35, zero file deletions** — matches
the PM's figure exactly. No phantom deletions.

---

## 1. The mount moved after `sched::init_bsp()` — the trace

`kernel/src/main.rs:673-681`: `sched::init_bsp()` → `sched::smoke_test()` → `#[cfg(feature = "boot-neofs")]
fs::mount_boot_neofs_from_device0_readwrite()`. **This is not merely safe, it is the only correct placement**,
and the previous placement was a latent panic that the new scenario found:

**(i) A registered BSP thread is a hard precondition of the mount path, and only `init_bsp()` creates one.**
`init_bsp` (`sched/scheduler.rs:312`) calls `init_cpu_scheduler(0, "main", "BSP")` (`:406`), whose first act is
`Thread::adopt_current("main")` (`:413`) — the boot context *becomes* a scheduled thread, and the per-CPU
scheduler pointer is published (`:438`). Before that call, `current_scheduler()` (`:456-460`) **panics**:
`"current_scheduler: cpu_id={} not initialized"`. The mount reaches that function on two independent paths:
- `virtio_blk::submit_pending_and_wait` acquires the device I/O gate in a `sched::yield_now()` loop
  (`drivers/block/virtio_blk.rs:660-666`) — `yield_now` → `rotate` (`sched/scheduler.rs:757-758`) →
  `current_scheduler()`;
- an RW mount **spawns the writeback worker kthread** at construction (`fs/neofs/inode.rs:114-115`,
  `spawn_writeback_worker`), which needs a live scheduler to enqueue onto.

So a pre-`init_bsp` RW mount panics as soon as the first submit misses the inline-completion poll. That is
exactly the boot panic the F4 scenario caught, and the fix is placement, not a workaround.

**(ii) No lock is held across a park.** The park is real —
`block_current_with_token_and_guard` (`virtio_blk.rs:758-775`), with the `io_gate` deliberately held across it
(documented invariant at `virtio_blk.rs:556-575`: the completion path takes only `inner`, never the gate, so
the waker can always run). What matters for CP-2/CP-3 is what the *callers above it* hold, and the answer is
nothing:
- `NeoFsBlockCache::read_block` (`fs/neofs/cache.rs:136-161`) **drops** the cache guard before
  `read_fs_block(&self.device, …)` and re-takes it after — the drop-before-blocking-I/O discipline, done
  correctly; `read_block_prefix` (`:164-197`) is the same shape.
- `write_block` (`:215-231`) collects the worker waiter under the lock, `drop(inner)`, **then** `wake_worker` —
  the wake-outside-the-foreign-lock discipline (CP-17's rule), already right.
- `MOUNTS` (`fs/mod.rs:588`) is a `SwitchMutex`, and in both mount functions all device I/O completes
  (`NeoFs::mount_readwrite`) **before** `MOUNTS.lock()` is taken (`fs/mod.rs:693`); the critical section is
  `install_root_child` + a cfg'd field store, no I/O, `drop(mounts)` before the log line. The `map_err(…)?`
  early return inside it drops the guard *and* the `NeoFs`, whose `Drop` stops the writeback worker
  (`inode.rs:1043`) — no orphaned kthread on the failure path.
- **CP-2 specifically:** the gate acquire loop runs inside `with_interrupts_enabled` (`virtio_blk.rs:660`),
  forcing IF=1 for the wait regardless of caller state. `_kernel_start` already has IF=1 here, so this is
  belt-and-braces, not a rescue.

**(iii) Ordering-before-userland holds on both paths.** Line 680 precedes both exits from `_kernel_start`:
the `neo-test` path at `:707` (`run_selected_scenario(spawn_neo_init)`) and the product path at `:713`
(`spawn_neo_init()`). There is no earlier userland entry — `prepare_bare_metal_mode` (`test_harness.rs:1346`)
runs before the mount but touches the **block layer directly** (the NVMe log sink), never the VFS, so nothing
that used to see `/neofs` lost it by the move.

**(iv) Nothing between the old and new mount points depends on `/neofs`.** The code the mount was moved
*past* is `fs::mount_net_interface_fs()`, `ai::init()` (which is just `drivers::accel::init()`,
`ai/mod.rs:108-110`), `bring_up_aps()`, `init_bsp()`, `smoke_test()`. None reads the VFS.

**The one honest residual:** the mount is now concurrent with a live PIT preemption and with the writeback
worker it just spawned. That is not a *new* class — every test-harness mount already runs in exactly that
state, later in boot, and is gate-exercised heavily. **No finding.**

## 2. F1 changed a shared function on all builds — what actually lost coverage

`should_verify_child_content_at_mount(kind) -> bool { kind != NeoFsKind::Model }`
(`fs/neofs/mount.rs:343-345`) now applies to **every** build, not just `product`. The dispatch's question is
the right one — *did we just delete the only integrity check on the model blob?* — and the answer is **no,
this is a strict improvement**:

NeoFS has **two independent** integrity checks on a `Model` inode, and F1 removed the wrong-shaped one:
- **Mount-side (removed):** `read_verified_logical_content` → `read_inode_content_with` → a single
  `Vec::with_capacity(file_size)`. On a real model that is the ~485 MB allocation on a 4 MiB heap that made
  my original Finding 1 a boot panic.
- **Read-side (survives, and is the correct one):** `NeoInode::read_at` → `should_verify_before_range_read`
  (`fs/neofs/inode.rs:1150-1153`, `:1521-1528`) → `ensure_verified_content` →
  `verify_inode_content_hash_cached` (`:1388-1413`) — **streaming** blake3, block-by-block through one
  4 KiB `block_buf`, `remaining` tracked exactly, compared against `inode.content_hash`, **memoised per inode**
  via `self.verified_content`. Same cryptographic strength, bounded memory, and it fires before the first
  range read rather than at mount.

So F1 deleted a redundant-and-catastrophic check and kept the correct one. **Nothing lost coverage — except
in one build, and it was already lost there before this branch:**

**`neo-test-arthur-real-brain-boot` now verifies model content NOWHERE.** `should_verify_before_range_read`
(`inode.rs:1521-1528`) carries the *same* `#[cfg(feature = "neo-test-arthur-real-brain-boot")]` Model
exemption F1 just deleted from the mount-side predicate. Under that feature both checks are now off — mount-side
unconditionally, read-side by the feature. **This is pre-existing** (the mount-side skip was gated on that same
feature before, so that build already skipped both), so it is **not a regression and not a bounce** — but F1
makes the mount-side skip permanent, which means the read-side exemption is now the *only* remaining lever, and
`arthur-real-brain-boot` is precisely the build that loads a real multi-hundred-MB model on the OptiPlex. **Filed
as OT-73, PM-owned, to settle before the bare-metal boot — not a rung-5 blocker.**

## 3. F3's degrade — Arthur copes, and `89`/`90` leak nothing

**The degraded spawn is byte-identical to the pre-branch spawn.** `arthur_compositor.rs:75-79`: on
`sys_open("/neofs/.arthur/memory")` failure it logs `SHOWCASE memory mode=ephemeral
reason=missing-neofs-arthur-memory` and spawns with `[session_client]` — **exactly** the one-handle array the
compositor used before this branch. So "does Arthur cope with one handle instead of four" has an unusually
strong answer: that configuration is what every prior green compositor run already exercised.

Tracing it forward anyway, because the dispatch is right that a degrade which moves the failure downstream is
worse than the failure it replaced:
- `MemoryRuntime::new()` (`neo-arthur/src/main.rs:1076-1078`) sets `available: memory_dir_cap_is_valid()`,
  which is a `sys_readdir(MEMORY_DIR)` probe (`:1827-1831`) on the hardcoded `FileHandle(3)`. With one
  inherited handle, slot 3 does not exist ⇒ `EBADF` ⇒ `available = false`. **A probe, not a fault.**
- `available == false` lands in first-class, pre-existing reply paths: `remember` → `arthur: memory: ephemeral`
  (`:1137-1139`), `recall` → `arthur: memory: empty` (`:1153-1155`), and every I/O error also self-disables
  (`:1141`, `:1157`) rather than propagating. Arthur answers; only the memory verbs degrade.
- The model handle is unaffected: `INHERITED_MODEL = ModelHandle(2)` (`:153`) is probed via `sys_model_query`
  (`:2080-2085`), which fails ⇒ fallback to `sys_model_load`. Same as before.
- **This degrade is gate-covered**, not merely argued: `arthur_compositor` / `arthur_compositor_baremetal` use
  the **`normal`** image variant (`xtask/src/main.rs:7109`, restored by this branch) which has no
  `/.arthur/memory`, while `neo-init`'s `arthur-compositor = ["arthur-memory"]` compiles the new arm in. So
  every compositor scenario in the green gate takes the `mode=ephemeral` path.

**`89`/`90` (`arthur_compositor.rs:53`, `:60`) — reachable, and they do not leak.** Both are
`sys_chan_create` failures, and **both `sys_close(memory)` before returning** — the opened directory handle is
released on the way out. Precisely: the `90` path does not close the slot-1 channel pair it created, but both
are terminal returns from `arthur_compositor::run()`, so the process exits and the kernel reclaims every
handle; nothing outlives the return. The success arm's `sys_close(memory)` correctly happens **after**
`sys_spawn`, not before.

## 4. Kill-switch — the claim is true of the new code and **false of the layer as a whole**

**`boot-neofs` OFF ≡ prior layer bit-for-bit is literally false**, and the RETURN should say so plainly rather
than let "default-off" carry more weight than it earns. The *added* path is properly switched:
`mount_boot_neofs_from_device0_readwrite` is `#[cfg(feature = "boot-neofs")]` (`fs/mod.rs:713-714`) and its
call site is too (`main.rs:679-680`) — feature-off is textually the previous boot. But **three changes ride
unconditionally, outside any cfg**, and they alter behaviour for builds that never enable `boot-neofs`:

1. **F1** — `should_verify_child_content_at_mount` (`mount.rs:343`) is now `kind != Model` on **all** builds.
2. **F2** — `find_neofs_block_device()` (`fs/mod.rs:634`) replaces `get_device(0)` in **both**
   `mount_neofs_from_device0_readonly` and `_readwrite` (call sites `:660`, `:687`) — every existing NeoFS-mounting
   scenario now selects by superblock probe.
3. **F3's ride-alongs** — `arthur-compositor = ["arthur-memory"]` in **both** `neo-arthur/Cargo.toml:50` and
   `neo-init/Cargo.toml:39`, plus the `arthur_compositor` harness mount flipping RO→RW
   (`test_harness.rs:2052-2055`).

**This is not a bounce, and it cannot be fixed by more cfg:** items 1 and 2 are the fixes the AAG itself
demanded, and a fix hidden behind the feature it fixes would be no fix at all. The correct action is
truth-in-labelling: the blast radius of this branch is **every scenario that mounts NeoFS**, not just the ones
enabling `boot-neofs` — which is precisely why the 4-shard green on `b1e2c975` is load-bearing evidence here
rather than a formality. I hunted the shape I caught last time (something outside the `cfg` that assumes the
feature exists) and found no *new* instance: `product = ["boot-neofs"]` (`kernel/Cargo.toml:78`) and
`boot-neofs-userland-persistence = ["arthur-memory"]` (`neo-init/Cargo.toml:83`) are both plain additive
aliases with no external assumption.

## 5. Completeness — how much of the `boot-neofs` path the gate now covers

My Finding 4 was *zero*. It is no longer zero, and the honest accounting is **the happy path only**:

**Now covered** — `neo-test-boot-neofs-userland-persistence = ["neo-test", "boot-neofs"]`
(`kernel/Cargo.toml:423`) genuinely enables the feature, so the gate executes `mount_boot_neofs_…()` inside
`_kernel_start`, at its new post-`init_bsp` position, over virtio-blk, on a valid superblock:
the **`Ok` arm**, the `find_neofs_block_device` probe loop, the identity logging, and the placement-before-userland.
Plus the property the build exists for: a two-boot userland write/readback
(`xtask/src/main.rs:1540-1546`, `is_reboot_persistence_scenario` `:4145-4148`, `arthur-memory` image variant `:7097`).

**Still zero coverage:**
- **All three degrade arms** (`fs/mod.rs:726-737`): `NoBlockDevice`, `NoValidSuperblock`/`InvalidSuperblock`,
  and the catch-all. Every one of them is a "boot continues" claim that nothing executes. (Cheap to close:
  a scenario booted with `boot-neofs` and a disk with no valid superblock.)
- **Device *selection*.** The scenario presents a single NeoFS disk, so the probe loop hits at index 0 and
  the interesting cases — a non-NeoFS disk ahead of the NeoFS one, several NeoFS volumes, AHCI-before-NVMe
  ordering — are all unexercised. This is the QEMU-can't-see-it half of the ratified F2 decision.
- **The `product` feature itself is compiled by no gate job.** `product = ["boot-neofs"]` appears in
  `kernel/Cargo.toml:78` and **nowhere else in the build system** — no xtask scenario, no CI job. The
  configuration the feature is named for is therefore never built by the gate, only by a hand-made bare-metal
  image, so the specific combination that produced my original Finding 1 (`product`'s 4 MiB
  `HEAP_SIZE_DEFAULT` + a real model volume) still has no automated build, let alone a run. F1 makes that
  combination safe by construction rather than by test.
- **Real AHCI/SATA on the OptiPlex** — unchanged from my first pass: a virtio-blk gate cannot speak to it.

## Notes (not findings, no action required to merge)

**(a) Naming debt is now actively misleading.** `mount_neofs_from_device0_readonly`,
`mount_neofs_from_device0_readwrite` and `mount_boot_neofs_from_device0_readwrite` no longer use device 0 —
they probe. The names now assert the exact property the F2 fix removed. `DECISION-2026-08-01-unified-neofs-
product-volume.md` already books this; recording it here so the two agree.

**(b) The `product` feature is a one-feature alias that nothing builds.** See §5. Worth either wiring a CI
build of `product` (compile-only would catch most of this class) or renaming it so no reader takes "product"
to mean "the configuration we ship and test."

**(c) The inherited-handle ABI is positional and hardcoded, and the padding exists only to make the indices
line up.** `INHERITED_MODEL = ModelHandle(2)`, `MEMORY_DIR = FileHandle(3)` (`neo-arthur/src/main.rs:153-155`);
both the compositor (`arthur_compositor.rs:63`) and the scenario driver (`neo-init/src/main.rs:2957`) create
**two throwaway channel pairs** purely so the memory dir lands at slot 3. Slot 2 is *named* the model handle
but receives a channel endpoint. That is safe today only because `model_lookup`
(`arch/x86_64/syscall.rs:1741-1754`) type-checks the handle and returns `EBADF` on a non-`Handle::Model`,
which sends Arthur down the `sys_model_load` fallback. **Pre-existing** (the same padding is already on `main` in
`arthur_memory_tests` — `neo-init/src/main.rs:2760-2768` on the base, `:2777` on the branch — not introduced
here), so nothing to bounce — but it is a fragile positional ABI
where correctness rests on a syscall's type check rather than on the layout being right, and it will bite the
first time a slot is added or reordered.

**(d) The unified-volume DECISION's safety-boundary wording needs one amendment — the boundaries themselves
hold.** The dispatch asked me to say if the decision's stated boundaries are false; they are not. But the
sentence "*the model inode's content is neither mount-verified by full allocation nor rewritten by this
path*" is accurate and still misleadable: a future reader can take it to mean model integrity is unverified.
It is not — the streaming `verify_inode_content_hash_cached` survives and runs at first range-read (§2).
Recommend the wording state explicitly that F1 removed the redundant-and-catastrophic check and **kept** the
correct one — a strict improvement, not a deletion — with the `arthur-real-brain-boot` exemption named as the
one exception (OT-73). *(PM has since folded this into the DECISION doc.)*

## Close

- **Doc-only.** No code touched, no build, no gate required. Branch `codex/aag-rung5-return-cerebro` off
  `origin/main`; base is `origin/main` tip at authoring time (ff-clean).
- **Recommendation: merge `b1e2c975`.** Gate green on the exact SHA, my independent stat is clean, all four
  original findings fixed in substance, no surviving finding from this pass.
- **Follow-ons already booked:** OT-73 (`arthur-real-brain-boot` verifies model content nowhere — settle
  before the OptiPlex boot); the degrade-arm and device-selection coverage gaps in §5; notes (a)–(c).
- **Disk reclaim:** nothing to reclaim — doc-only branch, no `target/` produced by this dispatch.

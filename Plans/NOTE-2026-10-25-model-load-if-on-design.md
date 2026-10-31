<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: design — model-load file reads at IF=0 (item (a) from #121/#122)

**Seat:** S4. **Status:** design only, no code. The PM asked for it while item (a) waits on the A6 timing
photos, because model load is on Arthur's hot path.

## Correction first

The #121 NOTE and the CP-56 entry say `sys_model_load` **and `sys_tensor_write`** read files at IF=0.
**`sys_tensor_write` reads no file:** it copies user bytes into a tensor (`tensor_write_checked` →
`arena::mint` → `write_bytes`). My function-attribution script assigned `neom_total_size_hint` and
`read_model_file` to it by mistake. Only `sys_model_load` reads a file. The fix to those two documents
goes in with this note.

## Where the IF=0 work is

`sys_model_load_impl` runs at the syscall's IF=0 throughout, on the caller's 32 KiB kernel stack:

1. **A `/boot/models/...` path** (feature `model-boot-module`) with **`boot-model-preload`** returns the
   preloaded `Arc<Model>` (`boot_modules::preloaded`). No file read and no build happen in the syscall:
   CP-74's guard moved both to boot, on the inference stack, at IF=1.
2. **A `/boot/models/...` path without preload**: `boot_modules::load_model` in the syscall. This is CP-74's
   exposed case (the hash and build at IF=0 on 32 KiB). It is not a file read.
3. **Any other path (NeoFS-resident models):**
   - `fs::open_root_temporary`: inode locks at IF=0. #121 wrapped only `sys_open`'s call.
   - `read_model_file`: whole-file `read_at_offset` loops, which take the NeoFS inode locks at IF=0. This is
     the same CP-37 exposure #121 closed for the file syscalls: `sys_write` holds those locks IF-on and
     preemptibly.
   - `Model::from_neom_bytes`: the build (hashing, dedup, copy into tensor storage), taking `ARENA`, `DEDUP`
     and `REGISTRY`. That is CP-74's long-compute shape, at IF=0, for whatever size the file is.

**On the product hot path, item (a) is reached only if Arthur's model is not preloaded.** In an image with
`boot-model-preload`, the A6 load is case 1, and neither the reads nor the build run in the syscall. What
the A6 photos and serial need to show is **which case A6 takes**:
- `BOOTMODEL preload path=… ok=1 … ticks=…` at boot, then `model_open path=/boot/models/… source=boot-module`,
  means case 1: (a) is off the hot path, and its cost to Arthur is zero.
- Anything else (a NeoFS `model_open path=/neofs/…` line, or a load timed inside the ask) means case 2 or 3,
  and the options below apply to the hot path.

## What IF-on would cost

**Stack.**
- The LAPIC timer runs on IST 2 (`TIMER_IST_INDEX`), so ticks add no depth to the 32 KiB thread stack.
- Device MSIs and IPIs (virtio-blk 244, NVMe 241, AHCI 240, TLB shootdown 252, reschedule 253) have no IST.
  With IF on, one handler can nest on the current stack (handlers run with IF off: interrupt gates).
- During the reads of case 3, that **already happens today**. Every uncached NeoFS read reaches
  `submit_pending_and_wait`, whose I/O gate turns IF on (CP-2). So wrapping the reads IF-on adds no new
  stack class. It only makes the windows longer.
- The build is different. Its depth is BLAKE3's, which is already over 32 KiB at 485 MB (CP-74), so it must
  not run on the 32 KiB stack at all, IF-on or not.

**Locks.**
- Reads IF-on: the inode locks get one more IF=1 holder kind. That changes nothing, because `sys_read`
  already holds them IF-on. It removes the IF=0 taker, which is the hazard.
- Build IF-on: `ARENA`, `DEDUP` and `REGISTRY` become preemptible IF=1 holders while userland runs. The tensor
  syscalls take `ARENA` at IF=0 (`arena::mint`/`lookup` from `tensor_write_checked` and its siblings). So a
  build preempted while holding `ARENA`, with a same-CPU tensor syscall spinning on it, is CP-37.
  - CP-74's lock note covers only the boot preload ("no thread takes those locks before userland exists").
  - Precondition for any IF-on build at runtime: audit `ARENA`/`DEDUP`/`REGISTRY` for IF=0 takers.
    Separately, check whether inference on the inference stack (IF=1) already takes `ARENA` while the tensor
    syscalls take it at IF=0. If it does, that exposure exists today, independent of model load.

**Time.**
- Reads IF-on: CPU 0 keeps taking ticks and device interrupts during the read, so the desktop and pointer
  don't freeze for the read's duration.
- Wall time can grow only by preemption: the loader shares the CPU instead of monopolizing it, plus one
  `sti`/`cli` pair per read call.
- Build on the inference stack: the inference stack is one shared, locked stack (`run_checked` →
  `Held::acquire`). A runtime load would serialize against every generate/infer for its whole duration.

## Options

| | Change | Fixes | Costs |
|---|---|---|---|
| **A** | Wrap case 3's open + `read_model_file` in `vfs_body_if_on` | the inode-lock CP-37 in the reads, with the same discipline as #121 | none new on the stack; build stays IF=0 |
| **B** | Run case 3's whole load (reads + build) on the inference stack via `run_checked`, as preload does | A, plus CP-74 for NeoFS-resident models (no 32 KiB overrun, no freeze) | serializes loads with inference; needs the `ARENA`/`DEDUP`/`REGISTRY` IF audit first |
| **C** | Leave it; document NeoFS model load as small-model, IF=0 | — | the inode-lock CP-37 stays reachable through `sys_model_load` |

**Recommendation: A, independent of the photos.** It is the #121 pattern, costs nothing on the stack, and
doesn't touch case 1 (the preload path), so it cannot move A6's timing if A6 preloads.

**B only if a NeoFS-resident large model becomes a product path,** and only after the `ARENA` IF audit
(which is worth doing anyway, given the inference-vs-tensor-syscall question above).

## Discriminator (A is built: lane A)

- **Correction to this note's first draft.** It said to hold the inode's `record` lock. `record` is
  per-`NeoInode`, and every lookup builds a new `NeoInode`, so a holder of another handle's `record` never
  contends with `sys_model_load`'s own open. The shared lock on this path is `entries`: open's
  `resolve_path`, and on every read `read_at_offset` → `validate_current` → `current_child_generation` →
  `resolve_path`.
- **The leg.** `phase3_neofs_syscall_if_on` gains a `model_load` leg. A small non-`.neom` file is written to
  NeoFS, and the existing same-CPU holder takes `entries` while yielding IF-on.
- `debug_sys_model_load_kernel_path_for_test` (the syscall's body after the user copy) is called at IF=0.
  It must open and read the whole file, return `EINVAL` (the build refuses a non-`.neom` file), and finish
  no earlier than the release.
- **Control.** The existing `neo-test-vfs-syscall-if-off-control` covers the call, because the wrap is the
  same `vfs_body_if_on`, and it must hang.
- **Falsification:** if the control passes, the path doesn't take the held lock.
- **Ordering.** The `model_load` leg runs **first**, so the control hangs on it rather than on `fsync` (the
  first cut ran it last, and its control hung at `fsync`, which proves nothing about `model_load`). Its
  file is created before the directory handle is opened, because creating it afterwards changes the
  directory's generation and the `readdir` leg would see `ESTALE`.

**Results (local, `NEO_SMP=4`):**

| run | result |
|---|---|
| fix | PASS, first attempt. Ticks (start / released / done): model_load 94/114/115 (`EINVAL`, after reading the whole file), fsync 115/135/164, open 164/184/184, readdir 184/204/204, ftruncate 204/224/231 |
| control | hangs on all 4 attempts before any leg line: the first leg, `model_load`, never completes |
| regressions | `model_load_cascade`, `infer_mlp_forward`, `boot_model_load_ticks`, `boot_model_load_stack`, `arthur_memory_forgetting`, `phase3_umount_drop_outside_mounts`, `phase3_neofs_fsync_issues_flush`: all PASS, first attempt |

## Finding: `ARENA` is a mixed-IF spin lock on `main` today (CP-37, separate lane)

The PM asked for this audit with option A. From static reading, not yet a reproduced hang:

- **IF=1, preemptible holders.**
  - Every generate/infer runs on the inference stack through `inference_stack::try_run` → `interruptible`
    (`with_interrupts_enabled`, CP-64), and the forward takes `ARENA`: `exec.rs:110`, `arena::mint` →
    `lookup` → `ARENA.lock()`, once per weight per pass.
  - The KV cache takes it too (`kv_cache.rs`: `alloc`, `mint`, `release`).
- **IF=0 takers.**
  - The tensor syscalls: `sys_tensor_query` (`syscall.rs:1894`) and the read and write paths (`1914`,
    `1988`, `2076`) via `arena::mint`; tensor create via `arena::alloc` (`1961`, `2257`, `3233`); tensor drop
    via `arena::release`.
  - The handle table's `retain_tensor`/`release_tensor` (`process.rs:1280/1287`), which run inside the
    table's IF-off sections.
- **The deadlock.** `ARENA` is a plain `SwitchMutex::new_ordered`. Suppose a tick lands inside an inference
  thread's `ARENA` section (a table lookup and an `Arc` clone, a few hundred cycles), and the scheduler then
  runs a thread on that CPU that makes a tensor syscall. That thread spins at IF=0 forever: CP-37. The
  window is short, so the hang is rare, but nothing rules it out. Arthur generating while a client does
  tensor I/O on the same CPU is enough.
- **Not covered by CP-74.** Its lock note is about the boot preload, before userland exists.

**Proposed fix (separate lane, as the PM asked):**
- Make `ARENA` an `IrqSafeMutex::new_ordered` (same level, `lock_order::ARENA`), so every acquisition is IF=0
  by construction (CP-56's structural guard).
- Its doc already states that no arena path parks while holding it, and `free` already moves the occupant
  out and drops it after the lock.
- The lane must still check that every `ARENA` section is short and park-free: no `Arc<Tensor>` dropped
  under it (`Tensor::drop` can enter `sharing::on_tensor_drop`, which takes `REGISTRY`/`DEDUP`), and no
  allocation that can block.
- **Discriminator:** a scenario where a same-CPU kthread holds `ARENA` while yielding IF-on (the state of a
  preempted forward) and a tensor syscall arrives at IF=0. The fix must leave no IF=1 holder at all, so the
  better witness is an IF=1-acquisition counter that must read 0 (the CP-56 `hid_contention` pattern), with
  a control that takes `ARENA` at the caller's IF.

## Lane C result: `ARENA` is IRQ-safe

- **Change.** `ARENA` is `IrqSafeMutex::new_ordered` at the same level (12), and every acquisition goes
  through `arena_lock()`.
  - Audit: every section is short and park-free. `free`/`release` take the occupant out and drain it after
    the lock, `lookup` only clones an `Arc`, and the only allocations are `Vec` pushes (the heap masks
    interrupts itself, CP-74).
  - No section takes another lock. `mint` drops `ARENA` before the slot lock, so the documented "a slot
    acquire MAY nest under it" never happens.
  - I-14(a) is unchanged: no ISR takes `ARENA`, and masking affects only the holder.
  - `docs/complexity-governance.md` needs no edit. It names `ARENA` as an ordered lock at level 12, which
    stays true, and the `new_ordered` census is unchanged.
- **Discriminator.** `arena_if_off` does an alloc at IF=1, a `mint` from the inference stack's IF-on
  closure, a `mint` at IF=0 and a release.
  - The fix: PASS, witness `held_if_on=0`.
  - The control `neo-test-arena-mixed-if-control` (plain `SwitchMutex`): FAIL
    `arena_held_with_interrupts_on`, `held_if_on=3`. The alloc, the inference-stack mint and the release
    counted; the IF=0 mint did not.
- **Regressions.** 11 pass: `tensor_create_query_drop`, `accel_tensor_sharing`, `accel_sim_migrate_roundtrip`,
  `accel_residency_daemon`/`_demote`/`_retain`, `accel_admission`, `ai_latency_oracle`, `i2c_hid_absent`
  (which holds `ARENA` around the ISR body), `inference_stack_kill`, `sys_generate_kv_persist_equivalence`.
- **Cost, measured under QEMU TCG (not silicon).**

  | measure | `main` (9564aa25) | lane C |
  |---|---|---|
  | `ai_latency_oracle` `mint` (TSC cycles, n=64) | 80,760 / 81,294 | 178,953 / 106,266 |
  | `ai_latency_oracle` `guarded_clone` (no `ARENA`) | 38,586 / 38,704 | 39,008 / 39,865 |
  | `ai_latency_oracle` `sys_tensor_query` | 150,346 / 153,292 | 177,254 / 177,943 |
  | `accel_residency_daemon` wall | 192.7 / 189.1 / 185.9 s | 239.0 / 232.4 / 240.9 s |
  | `sys_generate_kv_persist_equivalence` wall | 139.3 s | 151.8 s |

  - **The cost is confined to `ARENA` acquisitions:** `guarded_clone`, which doesn't take it, is unchanged.
  - **Hypothesis:** the IRQ-safe acquire reads RFLAGS and executes `cli`, and the release `sti`. Under TCG,
    an IF write ends the translation block, so each lock round trip is expensive in emulation. On silicon,
    `cli`/`sti`/`pushf` cost tens of cycles, against a `mint` of about 1.3k cycles per op here.
  - **Mechanism.** The acquire reads RFLAGS (`pushfq` into a register, in `interrupts::are_enabled`) and
    executes `cli`. The release executes `sti`, and only if IF was set before. There is no `popf`.
  - **Silicon budget (PM).** The forward does on the order of 290 mints per token. Even 100 extra cycles
    per mint is about 30k cycles per token, roughly 10 µs at 3.3 GHz, against 208 ms per token on A6-hwp.
  - **Falsification (PM, decided: ship).** A7 carries lane C. Its overlay's `perf pre ms/t` and
    `dec ms/t`, against A6-hwp's 208/304, are the test. If A7's per-token time rises by more than about 1%
    with everything else equal, the cost is real, this hypothesis is wrong, and the change is revisited.
  - Any CP-37 fix has to hold `ARENA` with interrupts off, so some version of this cost is inherent.
    The alternative to paying it is a lock-free table, which is out of scope.

## Lane D: a model's provenance check runs IF-on (review #58, item 4)

- **The defect (Jarvis, review of #121, item 4).** `Model::validate_current` → `FileProvenance` → the
  inode's `current_generation` → `current_child_generation` re-resolves a NeoFS-backed model's file name,
  taking `entries` at the syscall's IF=0. It is called from `sys_model_query` and from every infer/generate
  entry path (`sys_infer_run`, `sys_generate`, constrained, masked), always after `model_lookup` has
  released the handle-table lock, so no lock is held there.
- **Change.** The six call sites go through `model_still_current`, which runs `validate_current` in
  `vfs_body_if_on`. The provenance's short `MOUNTS` check (`contains_id`) runs under `without_interrupts`,
  so the IF-on check never holds `MOUNTS` preemptibly.
- **Arthur's hot path is unaffected.** A preloaded boot model's provenance is `FileProvenance::boot_module()`
  (`mount_id: None`, `generation: None`). Its `validate_current` returns `Ok` at once and touches neither
  `MOUNTS` nor NeoFS.
- **The leg.** `phase3_neofs_syscall_if_on` gains `model_query`, against the xtask fixture
  `/neofs/model.neom` (`valid_mlp`), loaded in setup.
- **A second control.** Each control run proves only the first leg it reaches, so
  `neo-test-vfs-syscall-if-on-skip-model-load` skips the `model_load` leg and makes `model_query` the
  first. Every leg now prints `leg_start=<name>` before it contends, so a hung control names its leg.

**Results (local, `NEO_SMP=4`):**

| run | result |
|---|---|
| fix | PASS, first attempt. model_query 129/149/149 (start / released / done, rc 0); the other legs as before |
| control (off) | hangs on all 4 attempts at the first leg, `model_load` |
| control (off + skip model_load) | hangs on all 4 attempts; each log ends at `leg_start=model_query` with no leg completed |
| regressions | `infer_mlp_forward`, `model_load_cascade`, `sys_generate_kv_persist_equivalence`, `accel_residency_demote` PASS; the `real-lm` and `product` kernel builds are clippy-clean |

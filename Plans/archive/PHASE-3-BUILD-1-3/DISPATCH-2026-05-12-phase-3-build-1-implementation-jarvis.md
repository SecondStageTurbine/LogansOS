# DISPATCH - Phase 3 BUILD-1 implementation: block layer + virtio-blk + minimal VFS (Jarvis)

**Date filed:** 2026-05-12
**From:** PM (NeoPlutarch Phase 3 cadence)
**To:** Jarvis (Codex-backed kernel implementation lane)
**Ferry:** PL ferries to Jarvis session.
**Status:** Implementation dispatch. Phase 3 frame is established by THINK v2 and ADR-021; Cerebro #2/#3 question-level returns have been consumed and ratified. This is not a research dispatch.

---

## 1. Mandate summary

Implement Phase 3 BUILD-1:

- block layer trait and request model;
- identity-DMA buffer abstraction;
- PCI/PCIe enumeration sufficient for modern virtio-blk;
- virtio-blk write-through read/write driver;
- storage completion through the SC-3C cross-CPU wake substrate;
- minimal VFS and test filesystem;
- `sys_open`, `sys_read`, `sys_write`, `sys_close`;
- thin `libneo` wrappers;
- QEMU smoke tests and acceptance gates.

The BUILD-1 correctness bar is not "driver appears to work." The hard gates are:

1. virtio-blk probe asserts `VIRTIO_BLK_F_FLUSH` is offered and refuses attach otherwise.
2. BUILD-1 does not negotiate `VIRTIO_BLK_F_FLUSH` or `VIRTIO_BLK_F_CONFIG_WCE`.
3. `IoRendezvous.waiting_thread` is weak, not owning: `Option<Weak<Thread>>`.
4. DMA buffers are Arc-managed and stay alive until completion.
5. The blocking path is atomic park-and-release; no lost-wakeup window.
6. Descriptor-ring barriers distinguish the pre-avail-index store barrier from the post-avail-index notification-suppression barrier.
7. aarch64 coverage exists before BUILD-1 is claimed closed, because x86 TSO hides the descriptor-ring barrier class.

If any hard gate cannot be satisfied, STOP and file the failure/blocker artifact. Do not paper over it.

---

## 2. Required inputs

Read in this order before coding:

1. `docs/ADR-021-phase-3-design-substrate.md` - canonical Phase 3 substrate. BUILD-1 boundary and acceptance gates are normative.
2. `Plans/PHASE-3-THINK-V2-2026-05-11.md` - PM synthesis that established the five-BUILD frame.
3. `Plans/PM-NOTE-2026-05-12-phase-3-cerebro-2-3-consumption-and-adr-021-extensions.md` - PM ratification of Cerebro #2/#3 extensions.
4. `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` - storage prior-art details; especially sections on virtio-blk durability cases, IoRendezvous, descriptor barriers, DMA abstraction, and PCI staging.
5. `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md` - VFS/FileCap prior-art details; BUILD-1 subset only.
6. `docs/ADR-010-capability-system-foundation.md` - capability vocabulary.
7. `docs/ADR-013-phase-2-smp-retrofit.md` and `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` - SMP and IRQ-safe scheduler/heap substrate.
8. `docs/ADR-019-switchmutex-irq-off-by-construction.md` and `docs/ADR-020-owner-metadata-production-diagnostic.md` - lock discipline and diagnostics.

Substrate files to inspect before planning edits:

- `kernel/src/main.rs` - boot/init order and Limine module root.
- `kernel/src/drivers/mod.rs` - driver module root; currently console/fb only.
- `kernel/src/mm/mod.rs` - HHDM, MMIO mapping, frame allocation.
- `kernel/src/mm/pmap.rs` - user copy and TLB-shootdown discipline.
- `kernel/src/mm/tlb_shootdown.rs` - SC-3C shootdown substrate.
- `kernel/src/sync/switch_mutex.rs`, `kernel/src/sync/irqsafe_mutex.rs` - lock APIs.
- `kernel/src/sched/thread.rs`, `kernel/src/sched/scheduler.rs`, `kernel/src/sched/process.rs` - Thread lifetime, wake/block paths, capability table.
- `kernel/src/ipc/mod.rs` - current synchronous blocking/rendezvous pattern.
- `kernel/src/arch/x86_64/syscall.rs` - syscall ABI and dispatch table.
- `kernel/src/test_harness.rs` - QEMU scenario pattern and NEO_TEST frame ABI.
- `kernel/Cargo.toml`, `tools/xtask/src/main.rs`, `.github/workflows/ci.yml` - feature/test/CI plumbing.
- `userland/libneo/src/lib.rs` - syscall wrapper ABI.

---

## 3. Coordination constraints

Use an agent-named branch off `main`, for example:

```text
codex/phase-3-build-1-jarvis
```

Jarvis owns code under:

- `kernel/src/drivers/`
- `kernel/src/fs/` if created
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/main.rs`
- `kernel/src/test_harness.rs`
- `kernel/Cargo.toml`
- `tools/xtask/src/main.rs`
- `.github/workflows/ci.yml`
- `userland/libneo/src/lib.rs`

Plans are allowed only for verification/failure artifacts. Do not edit canonical docs in this dispatch; ADR-021 is already amended.

Out of scope:

- NeoFS native on-disk format.
- Content-addressing/CAS.
- Buffer-cache write-back.
- Negotiated FLUSH support.
- AHCI/NVMe drivers.
- FAT32/ext4.
- Mount namespaces.
- `mmap`, `execve`, typed-inode enforcement beyond BUILD-1 structural hooks.
- FileCap GRANT/DERIVE/REVOKE for files.
- Retype.
- Disk-root pivot. Limine modules remain the boot root.

---

## 4. BUILD-1 decomposition

Land as sequential sub-commits. Each sub-commit gets a verification note under `Plans/` before advancing.

### 4.1 Sub-commit 1a - block substrate, DMA buffers, PCI discovery, QEMU disk plumbing

Scope:

- Create the block-driver module structure.
- Add identity-DMA buffer abstraction.
- Add generic block request/response types.
- Add PCI discovery sufficient to find modern virtio-blk.
- Add QEMU/xtask support for attaching a raw test disk to storage scenarios.
- Do not submit virtqueue requests yet.

Suggested files:

- `kernel/src/drivers/block/mod.rs` - block trait, device registry, request model.
- `kernel/src/drivers/block/dma.rs` - `DmaBuffer` abstraction.
- `kernel/src/drivers/block/virtio_blk.rs` - skeleton only in 1a; attach path can remain partial.
- `kernel/src/drivers/pci.rs` or `kernel/src/drivers/pci/mod.rs` - PCI walker.
- `kernel/src/drivers/mod.rs` - exports.
- `kernel/src/main.rs` - call storage init after `mm::init`, AP bring-up, and scheduler prerequisites are understood; choose the earliest safe point and document why.
- `tools/xtask/src/main.rs` - storage scenarios attach a QEMU raw disk.
- `kernel/Cargo.toml` and `kernel/src/test_harness.rs` - scenario feature names.

Design requirements:

- `DmaBuffer` must be Arc-managed by callers that submit I/O. The block API should naturally accept `&[Arc<DmaBuffer>]`, even if BUILD-1 uses one data buffer plus request/status buffers.
- Identity-DMA is acceptable in BUILD-1. Keep the abstraction name honest so later IOMMU or bounce-buffer work does not rewrite all callers.
- PCI walker should be forward-shaped:
  - recognize modern virtio-blk by vendor/device, not by class;
  - leave clean dispatch points for AHCI class `0x010601` and NVMe class `0x010802` in BUILD-5;
  - map BAR MMIO through the existing kernel MMIO mapping path.
- QEMU test disk should be raw, deterministic, recreated by xtask for scenarios, and not checked into git.

Verification gates:

- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- `cargo neo-test all` PASS.
- New scenario `phase3_pci_virtio_blk_probe_smoke` PASS:
  - QEMU attaches a virtio-blk test disk.
  - Kernel enumerates the PCI function.
  - Driver logs offered feature bits.
  - Probe does not yet claim read/write success.

Suggested commit title:

```text
kernel/storage: scaffold block substrate, DMA buffers, and virtio PCI probe
```

### 4.2 Sub-commit 1b - virtio-blk write-through read/write driver

Scope:

- Implement modern virtio-blk queue setup.
- Implement synchronous `read_block`/`write_block` through an async hardware completion path.
- Wire IRQ completion to unblock the waiting thread using SC-3C wake semantics.
- Add negative probe behavior when `VIRTIO_BLK_F_FLUSH` is not offered.

Required virtio feature policy:

| Feature | Bit | BUILD-1 disposition |
|---|---:|---|
| `VIRTIO_BLK_F_FLUSH` | 9 | MUST be offered; MUST NOT be negotiated |
| `VIRTIO_BLK_F_CONFIG_WCE` | 11 | MUST NOT be negotiated |
| legacy/unsupported feature bits | varies | ignore or reject per modern-device path; document |

If `VIRTIO_BLK_F_FLUSH` is not offered, probe must fail loudly and the driver must not attach. This is the write-through correctness gate from Cerebro #2.

IoRendezvous requirements:

```rust
struct IoRendezvous {
    done: AtomicBool,
    waiting_thread: Option<Weak<Thread>>,
    dma_buffers: Vec<Arc<DmaBuffer>>,
    result: IoResult,
}
```

- The issuing side must install the weak waiter and park while releasing the rendezvous/SwitchMutex as one scheduler operation.
- If the currently available scheduler API does not support atomic park-and-release for this shape, STOP and file a blocker. Do not split the check/park/release sequence.
- IRQ completion upgrades the weak reference and skips wake if the issuer is gone.
- DMA buffers are released only after completion path no longer needs them.

Descriptor-ring barrier requirements:

- Step before publishing `avail.idx`: ensure descriptor/table writes are visible before device observes the index.
- Step before checking notification suppression / issuing notify: use the stronger barrier required for store-before-load ordering.
- Do not encode both as a vague "one fence somewhere." Comments should name the distinct virtio steps.
- x86 may use a cheaper compiler fence for the first step if justified; the notification-suppression step needs a real ordering barrier. aarch64 must get explicit device-visible barriers in its lane.

Verification gates:

- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- `cargo neo-test all` PASS.
- New scenario `phase3_virtio_blk_raw_rw_smoke` PASS:
  - create deterministic raw disk;
  - write a known pattern to at least two LBAs;
  - read back and compare;
  - repeat across at least one CPU yield/block point.
- New scenario `phase3_virtio_blk_flush_offered_gate` PASS:
  - positive: feature log records `F_FLUSH` offered and not negotiated;
  - negative: QEMU/device config without F_FLUSH, or an injectable test shim, proves the driver refuses attach.
- New scenario `phase3_virtio_blk_abandoned_io_no_leak` PASS:
  - issue I/O from a thread/process that exits or is killed before completion;
  - completion must not keep the thread alive;
  - Arc-managed DMA buffers must not leak.
- `NEO_SMP=4 cargo neo-test phase3_virtio_blk_raw_rw_smoke` PASS.

Suggested commit title:

```text
kernel/storage: add write-through virtio-blk read/write path
```

### 4.3 Sub-commit 1c - minimal VFS, test filesystem, and file syscalls

Scope:

- Add a minimal VFS layer and an in-memory or test-backed filesystem sufficient for smoke tests.
- Add file object/open-handle abstraction.
- Add file handles as capabilities.
- Add `sys_open`, `sys_read`, `sys_write`, `sys_close`.
- Add `libneo` wrappers.

BUILD-1 VFS is intentionally small. It is allowed to be backed by a test filesystem or a synthetic mount; it does not mount NeoFS yet.

Required VFS shape:

- mount table;
- path lookup sufficient for smoke tests;
- inode object abstraction;
- file object/open-handle abstraction;
- file handles represented in the process handle/capability table;
- error mapping clear enough that BUILD-2 can distinguish type vs rights errors.

Carry these forward markers now, but do not implement the full future surface:

- `FileRights` must be a `u32`, not `u16`.
- Type-first, rights-second dispatch is the future policy.
- Reserve room for `TypeForbidden` and `InsufficientRights` as distinguishable errors.
- Absolute path behavior must not become the long-term API shape. The target model is `sys_open(dir_cap, path, flags, requested_rights)`. If BUILD-1 uses a temporary root-open shortcut for smoke tests, label it temporary and keep the kernel-side ABI easy to migrate.
- `..` should clamp at capability root when capability-rooted traversal lands. Do not design in reject-on-escape as a dependency.

Syscall ABI work:

- Extend `kernel/src/arch/x86_64/syscall.rs` with stable syscall numbers after the existing table.
- Mirror numbers and wrappers in `userland/libneo/src/lib.rs`.
- Preserve the current raw syscall calling convention.
- Validate user pointers using the existing user-copy discipline.
- Do not break existing `sys_write(fd=1)` serial console behavior unless deliberately replaced and covered.

Verification gates:

- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- `cargo neo-test all` PASS.
- New scenario `phase3_vfs_testfs_open_read_close` PASS:
  - open a known testfs file;
  - read exact bytes;
  - close;
  - second close returns the expected error.
- New scenario `phase3_vfs_testfs_write_readback` PASS:
  - open/create a writable testfs file if BUILD-1 supports create, or open a predeclared mutable test node;
  - write bytes;
  - seek or reopen if seek exists; otherwise read through a second handle;
  - compare exact bytes.
- Existing cap scenarios continue PASS.

Suggested commit title:

```text
kernel/vfs: add minimal VFS handles and file syscalls
```

### 4.4 Sub-commit 1d - BUILD-1 integration, CI, and closure gates

Scope:

- Add final integration tests that exercise block + VFS together.
- Add the aarch64 lane or file a hard blocker if the repo lacks enough cross-arch substrate to do it honestly.
- Produce BUILD-1 closure report.

Integration scenarios:

- `phase3_block_vfs_smoke`:
  - QEMU boots with Limine modules as root;
  - virtio-blk attaches;
  - block read/write smoke passes;
  - VFS testfs syscalls pass;
  - serial includes storage feature-negotiation log.
- `phase3_storage_completion_cross_cpu_wake`:
  - issue I/O from one CPU and complete/wake through another CPU if QEMU/IRQ routing permits;
  - if interrupt routing cannot force the cross-CPU path deterministically, add instrumentation that proves SC-3C wake composition is exercised or explain the limitation in the verification note.
- `phase3_build1_reboot_persistence_smoke`:
  - write a raw-block pattern;
  - reboot or restart QEMU with the same raw image;
  - read back the pattern.
  - This is a sanity smoke, not a substitute for BUILD-2 CoW crash consistency.

aarch64 lane:

- Required before BUILD-1 is closed.
- Preferred: QEMU aarch64 scenario that compiles and runs the virtqueue descriptor-publish/notify path against an emulated virtio-blk device.
- Acceptable only as an interim blocker-report path: if NeoPlutarch has no aarch64 boot substrate and adding it would dominate BUILD-1, file `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md` with:
  - exact missing substrate;
  - smallest viable lane proposal;
  - why x86-only tests cannot expose the barrier class;
  - PM decision needed.
- Do not claim BUILD-1 closed if this blocker path is taken.

Closure verification:

- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- `cargo neo-test all` PASS.
- `NEO_SMP=4 cargo neo-test smp_cross_cpu_rendezvous_stress` PASS.
- `NEO_SMP=4 cargo neo-test phase3_block_vfs_smoke` PASS.
- All new storage/VFS scenarios PASS.
- aarch64 lane PASS or blocker filed.
- `cargo fmt --check` if rustfmt is installed; otherwise record BLOCKED exactly as prior dispatches do.

Suggested commit title:

```text
kernel/storage: close Phase 3 BUILD-1 block/VFS smoke gates
```

---

## 5. Required verification artifacts

Create one verification note per sub-commit:

- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md`

If a sub-commit fails, create:

- `Plans/FAILURE-2026-05-12-phase-3-build-1-jarvis.md`

If the aarch64 lane blocks closure, create:

- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`

Each verification note must include:

- commit hash or working-tree note if not committed yet;
- files changed;
- test commands and pass/fail status;
- serial log paths for new QEMU scenarios;
- feature-negotiation log excerpt showing F_FLUSH offered and not negotiated;
- abandoned-I/O leak result;
- aarch64 lane result or blocker status;
- any deviation from this dispatch.

---

## 6. Stop conditions

Stop immediately and surface to PM if:

- `VIRTIO_BLK_F_FLUSH` cannot be made offered in the BUILD-1 QEMU/device configuration.
- The driver negotiates `VIRTIO_BLK_F_FLUSH` or `VIRTIO_BLK_F_CONFIG_WCE` in BUILD-1.
- The driver attaches when F_FLUSH is absent.
- The scheduler lacks an atomic park-and-release primitive for the storage rendezvous and no existing API composes safely.
- `IoRendezvous` requires `Arc<Thread>` to make tests pass.
- DMA buffers can be freed before device completion.
- Descriptor-ring barriers are unclear or fail under aarch64 coverage.
- Existing `cargo neo-test all` regresses.
- The new VFS handle model conflicts with ADR-010 capability derivation discipline.
- Any new syscall ABI breaks existing userland wrappers or cap tests.

---

## 7. Closure report back to PM

Jarvis closure report should include:

1. branch name and final commit list;
2. per-sub-commit summary;
3. verification artifact paths;
4. all new test scenario names;
5. exact `F_FLUSH` feature-negotiation log line;
6. exact IoRendezvous lifetime strategy (`Weak<Thread>` + `Arc<DmaBuffer>`);
7. descriptor-ring barrier implementation summary for x86 and aarch64;
8. aarch64 lane result or blocker;
9. any BUILD-2 forward markers discovered while implementing BUILD-1.

PM will not accept BUILD-1 closure without the F_FLUSH gate, abandoned-I/O lifetime gate, and aarch64 barrier gate accounted for.

---

**End of dispatch.**

*Phase 3 BUILD-1 is the first storage substrate. Keep it narrow, durable, and honest: virtio-blk write-through, synchronous blocking API over async completion, minimal VFS, Limine root preserved, NeoFS deferred to BUILD-2.*

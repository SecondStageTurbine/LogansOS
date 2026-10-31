# PM Decision - Phase 3 BUILD-3 Opening Gate

**Date:** 2026-05-14
**Status:** OPEN OBSERVE GATE; BUILD-3 implementation not authorized
**Consumes:**
- `docs/phases/PHASE-3.md`
- `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md`
- `Plans/PHASE-3-THINK-V2-2026-05-11.md`
- `Plans/PHASE-3-BUILD-2-POST-CLOSE-DOCS-VERIFICATION-2026-05-14.md`

---

## 1. Decision

Phase 3 BUILD-2 is locally integrated on `main`. BUILD-3 remains not started.

PM opens a BUILD-3 **observe/planning gate only**. No CAS, deduplication, buffer-cache write-back, virtio FLUSH negotiation, mark-sweep GC, kthread substrate change, rootfs pivot, mmap/exec, or broader POSIX work is authorized by this decision.

The next operational step is to answer the BUILD-3 entry question:

> Should BUILD-3 be admitted now as a Phase 3 implementation build, split into a smaller prerequisite build, or deferred to Phase 5 model-storage substrate?

## 2. Why This Gate Exists

`Plans/PHASE-3-THINK-V2-2026-05-11.md` makes BUILD-3 measurement-gated. CAS/dedup is not the first NeoFS correctness substrate; it is admitted only if measurement or Phase 5 model-storage planning proves it is now critical path.

THINK v2 also names a prerequisite before BUILD-3 PLAN dispatch: audit whether the scheduler exposes a kernel-internal kthread primitive suitable for background filesystem work.

## 3. Immediate Codebase Observation

Direct source search found an existing public kernel-thread spawn primitive:

```text
kernel/src/sched/scheduler.rs:231
pub fn spawn(name: &'static str, entry: extern "C" fn() -> !)
```

It is exported by `kernel/src/sched/mod.rs` and is already used by storage:

```text
kernel/src/drivers/block/virtio_blk.rs:770
sched::spawn("virtio-blk-completion", virtio_blk_completion_worker);
```

That answers the narrow question "does a kernel-thread spawn hook exist?" as **yes**.

It does not yet answer the BUILD-3 ownership question. The visible primitive does not return a worker handle, expose cancellation/stop state, support join/reap ownership by a filesystem instance, or define unmount-time worker teardown. The virtio completion worker is a global infinite loop and is not sufficient evidence for per-mount write-back or GC workers.

## 4. PM Lean

PM lean before observe results:

1. Do not start CAS first.
2. Treat the likely BUILD-3 first slice as a kthread ownership/lifecycle prerequisite if Jarvis confirms the gap.
3. Separate "write-back cache + FLUSH durability" from "CAS dedup" unless Cerebro proves they must land together.
4. Preserve BUILD-2 `sys_fsync` no-op semantics until a specific BUILD-3 dispatch changes it to a durable write-back/FLUSH contract.

## 5. Authorized Dispatches

Three planning artifacts are authorized:

1. Jarvis observe audit:
   `Plans/DISPATCH-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`
2. Cerebro structural challenge and prior-art sidecar:
   `Plans/DISPATCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`
3. Prometheus evidence-matrix prep:
   `Plans/DISPATCH-2026-05-14-phase-3-build-3-evidence-matrix-prometheus.md`

None of these dispatches authorize code, normative docs, or BUILD-3 implementation.

## 6. Stop Conditions

Stop and return to PM if any observe result suggests:

- kthread ownership cannot be added without revisiting scheduler/process lifetime invariants;
- virtio FLUSH negotiation requires changing BUILD-1 write-through assumptions in a way that invalidates closed evidence;
- buffer-cache write-back would make `sys_fsync` nontrivial before the fsync contract is specified;
- CAS/dedup has no Phase 3 workload evidence and no Phase 5 critical-path dependency;
- mark-sweep GC requires broad NeoFS format migration beyond BUILD-2 reserved fields;
- any proposed BUILD-3 plan starts AHCI, NVMe, foreign FS, rootfs pivot, retype, mmap/exec, or broad POSIX work.

## 7. Closure

BUILD-3 is open only as a planning/observe gate. Implementation remains blocked pending PM consumption of the Jarvis, Cerebro, and Prometheus returns.

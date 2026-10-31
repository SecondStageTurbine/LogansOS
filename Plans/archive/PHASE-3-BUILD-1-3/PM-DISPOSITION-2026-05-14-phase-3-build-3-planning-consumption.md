# PM Disposition - Phase 3 BUILD-3 Planning Consumption

**Date:** 2026-05-14
**Status:** BUILD-3 NARROWED; 3A authorized by separate dispatch
**Consumes:**
- `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`
- `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`
- `Plans/PREP-2026-05-14-phase-3-build-3-evidence-matrix-prometheus.md`
- `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md`

---

## 1. Decision

PM accepts the combined Jarvis/Cerebro/Prometheus returns.

Phase 3 BUILD-3 is admitted, but narrowed:

- CAS/dedup is deferred to Phase 5 unless the existing measurement gate is later satisfied.
- Mark-sweep GC is a Phase 3 correctness requirement because BUILD-2's bump allocator permanently fills the disk with orphaned CoW blocks.
- FLUSH negotiation, write-back cache, and durable `sys_fsync` are separable from CAS and remain BUILD-3 scope.
- The first implementation slice is kthread ownership/lifecycle only.

Implementation is authorized only through the explicit 3A dispatch:

```text
Plans/DISPATCH-2026-05-14-phase-3-build-3-3a-kthread-lifecycle-jarvis.md
```

## 2. Artifact Consumption

### 2.1 Cerebro

Artifact:

```text
Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md
```

PM ratifies the **NARROW** verdict:

- Admit kthread lifecycle, mark-sweep GC, FLUSH, write-back cache, and corrected `sys_fsync`.
- Defer CAS hash-as-address lookup, DDT-style dedup, and bloom-filter front-end to Phase 5.
- Apply the CAS admit rule: Phase 5 model-storage planning must prove CAS is critical path, or measured workload dedup ratio must exceed 5x with acceptable memory cost.

PM also ratifies Cerebro's top ordering constraints:

- GC must run at quiescence only until a later design proves concurrent GC safe.
- `sys_fsync` must commit the full path from file data through inode/root/superblock, not just file data.
- FLUSH must occur after all three superblock copies are written, not after the first copy.

### 2.2 Jarvis

Artifact:

```text
Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md
```

PM ratifies Jarvis's first-slice recommendation:

- Current `sched::spawn` is enough for immortal global workers.
- It is not enough for filesystem-owned write-back or GC workers because it lacks handle identity, stop, join/detach, completion observation, and unmount teardown.
- BUILD-3A must add scheduler-owned kthread lifecycle without exposing `Arc<Thread>` or raw `Thread` ownership to subsystems.

Jarvis's proposed later sequence is accepted as the working order, with one PM adjustment: mark-sweep GC remains mandatory before Phase 3 closure even though CAS is deferred.

### 2.3 Prometheus

Artifact:

```text
Plans/PREP-2026-05-14-phase-3-build-3-evidence-matrix-prometheus.md
```

PM accepts the evidence shell. Its pending Jarvis/Cerebro slots are now satisfied by the artifacts consumed in this disposition; the artifact itself does not need to be rewritten before 3A starts.

The stop conditions remain binding:

- no kthread closure without ownership/teardown evidence;
- no `sys_fsync` closure while it remains a BUILD-2 no-op;
- no FLUSH closure without runtime negotiation and command-completion evidence;
- no GC closure without reachable-block walk and orphan-reclamation evidence;
- no CAS closure without explicit PM admission or deferral language.

## 3. BUILD-3 Working Split

PM sets this working split:

| Slice | Scope | Status |
|---|---|---|
| 3A | kthread ownership/lifecycle substrate | Authorized by Jarvis dispatch |
| 3B | block-layer flush API and virtio-blk FLUSH negotiation/command path | Pending 3A |
| 3C | NeoFS free-space format addition plus quiescent mark-sweep GC | Pending 3B |
| 3D | write-back buffer cache using kthread handles | Pending 3C |
| 3E | durable `sys_fsync` over write-back plus crash/recovery gates | Pending 3D |
| CAS | content-addressed lookup and dedup | Deferred to Phase 5 unless explicitly re-admitted |

PM may revise boundaries after 3A returns, but 3A must not start later slices.

## 4. Corrections

Prometheus's returned summary named branch state as `main` at `d49f9ab`. Local PM state after BUILD-2 post-close docs is:

```text
main at 096414c4da9f2294c35bcfc9e0c459a99c50078b
```

`d49f9ab` is the BUILD-2 close evidence addendum commit, not the current post-close docs/index HEAD.

## 5. Anti-Scope

3A does not authorize:

- NeoFS format changes;
- free-space bitmap or GC implementation;
- buffer-cache write-back;
- virtio FLUSH negotiation;
- `sys_fsync` behavior change;
- CAS/dedup;
- AHCI/NVMe;
- foreign filesystems;
- rootfs pivot;
- file-backed mmap/exec;
- broader POSIX syscalls;
- multi-thread-per-process.

## 6. Closure

The BUILD-3 observe gate is consumed. BUILD-3 is narrowed and may begin with 3A only. All other BUILD-3 slices remain blocked on PM dispatch after 3A evidence returns.

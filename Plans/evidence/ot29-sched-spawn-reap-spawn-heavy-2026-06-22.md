<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-29 evidence — thread-slab magazine stride bug: captured before, green after

**Date:** 2026-06-22 · **Scenario:** `sched_spawn_reap_spawn_heavy` · **NEO_SMP=4** ·
local QEMU (container, `slab-debug-poison` armed) · 4 attempts each.

## Before the fix (capture-before-you-fix) — FAIL 4/4

The neo-init driver loops heavy-Arthur spawn → exit → reap → spawn. After two Arthur
processes are spawned + reaped, the third spawn's `Thread` allocation pulls a corrupted
magazine slot. The `slab-debug-poison` discriminator caught it one hop upstream of the raw
`thread.rs:135` panic:

```
NEO_TEST/1 event=BEGIN name=sched_spawn_reap_spawn_heavy phase=dispatch
[ kernel ] sched: reaped 'user-process' (id=10)
[ kernel ] sched: reaped 'user-process' (id=11)
!!  KERNEL PANIC  !!
panicked at kernel/src/mm/slab.rs:143:13:
slab[thread]: FREE-LIST HEAD CORRUPT node=0x5 (null/misaligned — a live field read as the free-list link)
NEO_TEST/1 event=FAIL name=sched_spawn_reap_spawn_heavy reason=kernel_panic
```

`node=0x5` is out-of-bounds garbage from the magazine: `Magazine::push`/`pop` indexed a
`*mut T`-typed buffer with `.add(index)`, striding by `size_of::<T>()` instead of the
pointer size, so the 2nd+ cached large `Thread` wrote/read off the end of the
`capacity * size_of::<*mut T>()` allocation.

## Misdiagnosis caught by the instrument

The dispatch hypothesis (and an independent Forge analysis) was an SMP exit-vs-reap
use-after-free. A trial fix re-ordering `exit()`'s lock release (release-on-new-stack via
`context_switch_with_release2`) was applied and re-run: **identical failure** (`node=0x5`,
unchanged). Fix-independence of `exit()` refuted the UAF hypothesis and pinned the defect
to the slab. The `exit()` change was reverted.

## After the fix (magazine stride: cast to `*mut *mut T` before `.add`) — PASS

```
[xtask] neo-test: sched_spawn_reap_spawn_heavy PASS
```

Two-line fix in `Magazine::push`/`pop`; `exit()` and the rest of the scheduler are
unchanged. The regression scenario stays armed permanently with the poison instrument.

## CI

- push-CI + 4-shard QEMU `workflow_dispatch` URLs: see the RETURN (head_sha-verified).
